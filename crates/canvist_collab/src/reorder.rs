//! Holding back updates that arrive before the updates they build on.
//!
//! Yjs keeps such updates pending until the gap is filled. yrs 0.27 and 0.28
//! instead integrate an update that overtakes an earlier one from the same
//! client, and the earlier one may then never apply, leaving peers diverged
//! ([y-crdt#670](https://github.com/y-crdt/y-crdt/issues/670),
//! [y-crdt#677](https://github.com/y-crdt/y-crdt/issues/677)). The binding
//! therefore hands yrs an update only once every client's earlier changes it
//! builds on, including the items it deletes, are already in the document.
//!
//! [`Reorder`] is public so hosts that keep a bare yrs document, such as a
//! relay server, apply updates in the same safe order.

use rustc_hash::FxHashMap;
use yrs::StateVector;
use yrs::Update;
use yrs::block::ClientID;

/// Updates waiting for the updates they build on.
///
/// Hold every incoming update, then apply what
/// [`take_ready`](Reorder::take_ready) returns for the document's current
/// state vector:
///
/// ```
/// use canvist_collab::Reorder;
/// use canvist_collab::yrs::Doc;
/// use canvist_collab::yrs::GetString;
/// use canvist_collab::yrs::ReadTxn;
/// use canvist_collab::yrs::Text;
/// use canvist_collab::yrs::Transact;
/// use canvist_collab::yrs::Update;
/// use canvist_collab::yrs::updates::decoder::Decode;
///
/// let source = Doc::with_client_id(1);
/// let text = source.get_or_insert_text("text");
/// let edit = |index: u32, chunk: &str| {
/// 	let mut txn = source.transact_mut();
/// 	text.insert(&mut txn, index, chunk);
/// 	txn.encode_update_v1()
/// };
/// let first = edit(0, "a");
/// let second = edit(1, "b");
///
/// let target = Doc::with_client_id(2);
/// let mut reorder = Reorder::default();
/// // The second update overtakes the first and waits for it.
/// for update in [second, first] {
/// 	reorder.hold(Update::decode_v1(&update)?);
/// 	let mut txn = target.transact_mut();
/// 	for ready in reorder.take_ready(&txn.state_vector()) {
/// 		txn.apply_update(ready)?;
/// 	}
/// }
///
/// assert!(!reorder.is_waiting());
/// let merged = target.get_or_insert_text("text");
/// assert_eq!(merged.get_string(&target.transact()), "ab");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Default)]
pub struct Reorder {
	held: Vec<Update>,
}

impl std::fmt::Debug for Reorder {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Reorder")
			.field("held", &self.held.len())
			.finish()
	}
}

impl Reorder {
	/// Queue `update` until it can apply.
	pub fn hold(&mut self, update: Update) {
		self.held.push(update);
	}

	/// Take every queued update that can apply to a document with `state`,
	/// in an order where each one's dependencies come first.
	#[must_use]
	pub fn take_ready(&mut self, state: &StateVector) -> Vec<Update> {
		let mut reach = clocks(state);
		let mut ready = Vec::new();

		while let Some(index) = self.held.iter().position(|update| is_ready(update, &reach)) {
			let update = self.held.swap_remove(index);
			extend(&mut reach, &update);
			ready.push(update);
		}

		ready
	}

	/// Whether updates are waiting for earlier ones.
	#[must_use]
	pub fn is_waiting(&self) -> bool {
		!self.held.is_empty()
	}
}

fn clocks(state: &StateVector) -> FxHashMap<ClientID, u32> {
	state
		.iter()
		.map(|(&client, &clock)| (client, clock))
		.collect()
}

/// Whether every item `update` adds continues its client's history, and
/// every item it deletes exists or arrives with it.
fn is_ready(update: &Update, reach: &FxHashMap<ClientID, u32>) -> bool {
	let known = |client: &ClientID| reach.get(client).copied().unwrap_or(0);
	let continues = update
		.state_vector_lower()
		.iter()
		.all(|(client, &clock)| clock <= known(client));

	if !continues {
		return false;
	}

	let mut after = reach.clone();
	extend(&mut after, update);
	update.delete_set().iter().all(|(client, ranges)| {
		let known = after.get(client).copied().unwrap_or(0);
		ranges.iter().all(|range| range.end <= known)
	})
}

/// Record the items `update` adds as known.
fn extend(reach: &mut FxHashMap<ClientID, u32>, update: &Update) {
	for (client, ranges) in update.insertions(true).iter() {
		if let Some(end) = ranges.iter().map(|range| range.end).max() {
			let clock = reach.entry(*client).or_default();
			*clock = (*clock).max(end);
		}
	}
}
