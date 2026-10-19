//! Every loaded room, and the loop that persists, reports, and evicts them.
//!
//! Each [flush interval](crate::config::SyncConfig::flush_interval) the loop
//! writes rooms' queued changes to the store, fires due webhooks, and
//! unloads rooms that have had no peers for the
//! [idle eviction](crate::config::SyncConfig::idle_eviction) period, after
//! compacting them into a snapshot. Rooms are counted in and out under the
//! registry lock, so a room is never evicted while a peer is joining it.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Instant;

use futures_util::StreamExt;
use futures_util::stream;

use super::room::Membership;
use super::room::Room;
use super::webhooks::Webhooks;
use crate::config::SyncConfig;
use crate::names::DocumentKey;
use crate::store::Store;
use crate::store::StoreError;

/// A room claimed by a batched flush, with what it is writing.
struct Claim<'a> {
	room: &'a Arc<Room>,
	_guard: tokio::sync::MutexGuard<'a, ()>,
	pending: Vec<Vec<u8>>,
	rows: Vec<Vec<u8>>,
}

/// Rooms persisted at once by the flush loop.
const FLUSH_CONCURRENCY: usize = 16;

/// The registry of loaded rooms.
#[derive(Debug)]
pub struct Rooms {
	loaded: Mutex<HashMap<DocumentKey, Arc<Room>>>,
	store: Store,
	config: SyncConfig,
	webhooks: Webhooks,
}

/// Loaded rooms and connected peers, for metrics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct RoomStats {
	/// Documents in memory.
	pub rooms: usize,
	/// Connected peers across them.
	pub peers: usize,
}

impl Rooms {
	/// An empty registry.
	pub fn new(store: Store, config: SyncConfig, webhooks: Webhooks) -> Self {
		Self {
			loaded: Mutex::default(),
			store,
			config,
			webhooks,
		}
	}

	/// The sync settings.
	pub fn config(&self) -> &SyncConfig {
		&self.config
	}

	/// The store rooms persist to.
	pub fn store(&self) -> &Store {
		&self.store
	}

	fn lock(&self) -> MutexGuard<'_, HashMap<DocumentKey, Arc<Room>>> {
		self.loaded
			.lock()
			.unwrap_or_else(std::sync::PoisonError::into_inner)
	}

	/// Join the room for `key`, loading it from the store if needed.
	pub async fn join(&self, key: &DocumentKey) -> Result<(Arc<Room>, Membership), StoreError> {
		let (room, membership) = {
			let mut rooms = self.lock();
			let room = rooms
				.entry(key.clone())
				.or_insert_with(|| Arc::new(Room::new(key.clone())))
				.clone();
			let membership = room.join();

			(room, membership)
		};

		if let Err(error) = room.load(&self.store).await {
			room.leave(membership.peer);
			return Err(error);
		}

		Ok((room, membership))
	}

	/// The room for `key`, if it is loaded.
	pub fn get(&self, key: &DocumentKey) -> Option<Arc<Room>> {
		self.lock().get(key).cloned()
	}

	/// Unload `key`'s room without persisting it, disconnecting its peers.
	/// Used when the document is deleted.
	pub fn discard(&self, key: &DocumentKey) {
		if let Some(room) = self.lock().remove(key) {
			room.close();
		}
	}

	/// Loaded rooms and their peers.
	pub fn stats(&self) -> RoomStats {
		let rooms = self.lock();
		RoomStats {
			rooms: rooms.len(),
			peers: rooms.values().map(|room| room.peers()).sum(),
		}
	}

	fn snapshot(&self) -> Vec<Arc<Room>> {
		self.lock().values().cloned().collect()
	}

	/// One pass of the flush loop at `now`.
	pub async fn tick(&self, now: Instant) {
		let (idle, active): (Vec<_>, Vec<_>) = self
			.snapshot()
			.into_iter()
			.partition(|room| room.is_idle(now, self.config.idle_eviction));

		stream::iter(idle)
			.for_each_concurrent(
				FLUSH_CONCURRENCY,
				|room| async move { self.evict(&room).await },
			)
			.await;
		self.flush(&active).await;

		for room in &active {
			let due = room.take_due_changes(
				now,
				self.config.webhook_debounce,
				self.config.webhook_max_wait,
			);

			if let Some(changes) = due {
				self.webhooks.notify(room.key(), &changes);
			}
		}
	}

	/// Write every dirty room's queued updates in one store transaction, so
	/// the store sees a constant four writes a second however many documents
	/// are being edited, then compact the rooms that crossed the threshold.
	async fn flush(&self, rooms: &[Arc<Room>]) {
		let claims: Vec<Claim<'_>> = rooms
			.iter()
			.filter(|room| room.is_dirty())
			.filter_map(|room| {
				let guard = room.try_claim()?;
				let (pending, rows) = room.take_rows();
				Some(Claim {
					room,
					_guard: guard,
					pending,
					rows,
				})
			})
			.collect();

		if claims.is_empty() {
			return;
		}

		let batch: Vec<(&DocumentKey, &[u8])> = claims
			.iter()
			.flat_map(|claim| {
				claim
					.rows
					.iter()
					.map(|row| (claim.room.key(), row.as_slice()))
			})
			.collect();

		match self.store.append_batch(&batch).await {
			Ok(appended) => {
				for claim in &claims {
					if let Some(rows) = appended.get(claim.room.key()) {
						claim.room.record_rows(*rows);
					}
				}
			}
			Err(error) => {
				tracing::warn!(rooms = claims.len(), %error, "persisting failed; retrying next tick");
				for claim in claims {
					claim.room.requeue(claim.pending);
				}
				return;
			}
		}

		stream::iter(claims)
			.for_each_concurrent(FLUSH_CONCURRENCY, |claim| self.compact_if_due(claim))
			.await;
	}

	async fn compact_if_due(&self, claim: Claim<'_>) {
		if claim.room.stored_rows() < self.config.compact_after {
			return;
		}

		if let Err(error) = claim.room.compact(&self.store).await {
			tracing::warn!(document = %claim.room.key(), %error, "compaction failed; retrying later");
		}
	}

	/// Compact `room`, report its last changes, and unload it unless a peer
	/// joined meanwhile.
	async fn evict(&self, room: &Arc<Room>) {
		if let Err(error) = room
			.persist(&self.store, self.config.compact_after, true)
			.await
		{
			tracing::warn!(document = %room.key(), %error, "persisting before eviction failed; keeping the room");
			return;
		}

		if let Some(changes) = room.take_changes() {
			self.webhooks.notify(room.key(), &changes);
		}

		let mut rooms = self.lock();
		let unchanged = room.peers() == 0 && !room.is_dirty();

		if unchanged
			&& rooms
				.get(room.key())
				.is_some_and(|loaded| Arc::ptr_eq(loaded, room))
		{
			rooms.remove(room.key());
			tracing::debug!(document = %room.key(), "evicted idle document");
		}
	}

	/// Persist and compact every room, and report every pending change, for
	/// a graceful shutdown. Returns how many rooms failed to persist.
	pub async fn flush_all(&self) -> usize {
		let rooms = self.snapshot();
		let failures = AtomicUsize::new(0);

		stream::iter(rooms)
			.for_each_concurrent(FLUSH_CONCURRENCY, |room| self.finish(room, &failures))
			.await;

		failures.into_inner()
	}

	/// Persist and report one room at shutdown, counting a failure.
	async fn finish(&self, room: Arc<Room>, failures: &AtomicUsize) {
		let persisted = room
			.persist(&self.store, self.config.compact_after, true)
			.await;

		if let Err(error) = persisted {
			tracing::error!(document = %room.key(), %error, "could not persist at shutdown");
			failures.fetch_add(1, Ordering::Relaxed);
		}

		if let Some(changes) = room.take_changes() {
			self.webhooks.notify(room.key(), &changes);
		}
	}
}
