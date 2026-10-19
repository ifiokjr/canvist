//! Presence for one room: the y-protocols awareness states of its peers.
//!
//! The relay keeps awareness itself rather than in a yrs `Awareness`, so it
//! can tie each awareness client id to the connection that introduced it.
//! A connection may only change states it owns, so one peer cannot move
//! another's caret, and a connection's states are withdrawn when it leaves.

use std::collections::HashMap;
use std::sync::Arc;

use yrs::block::ClientID;
use yrs::sync::AwarenessUpdate;
use yrs::sync::awareness::AwarenessUpdateEntry;

use super::room::PeerId;

/// The largest awareness state accepted, in bytes of JSON.
pub const MAX_STATE_BYTES: usize = 16 * 1024;
/// Awareness client ids one connection may introduce per room.
pub const MAX_STATES_PER_PEER: usize = 8;

const NULL: &str = "null";

#[derive(Debug)]
struct Entry {
	clock: u32,
	json: Arc<str>,
	owner: PeerId,
}

/// Every peer's awareness state in a room.
#[derive(Debug, Default)]
pub struct AwarenessTable {
	states: HashMap<ClientID, Entry>,
}

impl AwarenessTable {
	/// Merge `update` from `owner`, returning the entries that changed, to
	/// broadcast. Entries for client ids another peer owns, stale clocks,
	/// and oversized states are dropped.
	pub fn apply(&mut self, owner: PeerId, update: AwarenessUpdate) -> Option<AwarenessUpdate> {
		let mut accepted = HashMap::new();

		for (client, entry) in update.clients {
			if self.accepts(owner, client, &entry) {
				self.states.insert(
					client,
					Entry {
						clock: entry.clock,
						json: entry.json.clone(),
						owner,
					},
				);
				accepted.insert(client, entry);
			}
		}

		(!accepted.is_empty()).then_some(AwarenessUpdate { clients: accepted })
	}

	/// Whether `owner` may set `client`'s state to `entry`: states are
	/// bounded in size and count, owned by the connection that introduced
	/// them, and ordered by y-protocols' clock rule, under which a newer
	/// clock wins and an equal clock may only withdraw a state.
	fn accepts(&self, owner: PeerId, client: ClientID, entry: &AwarenessUpdateEntry) -> bool {
		if entry.json.len() > MAX_STATE_BYTES {
			return false;
		}

		match self.states.get(&client) {
			Some(current) if current.owner != owner => false,
			Some(current) => {
				current.clock < entry.clock
					|| (current.clock == entry.clock
						&& &*entry.json == NULL
						&& &*current.json != NULL)
			}
			None => self.owned_by(owner) < MAX_STATES_PER_PEER,
		}
	}

	fn owned_by(&self, owner: PeerId) -> usize {
		self.states
			.values()
			.filter(|entry| entry.owner == owner)
			.count()
	}

	/// Withdraw every state `owner` introduced, returning the removals to
	/// broadcast.
	pub fn remove_peer(&mut self, owner: PeerId) -> Option<AwarenessUpdate> {
		let clients: Vec<ClientID> = self
			.states
			.iter()
			.filter(|(_, entry)| entry.owner == owner)
			.map(|(client, _)| *client)
			.collect();
		let removed: HashMap<ClientID, AwarenessUpdateEntry> = clients
			.into_iter()
			.filter_map(|client| self.states.remove(&client).map(|entry| (client, entry)))
			.filter(|(_, entry)| &*entry.json != NULL)
			.map(|(client, entry)| {
				(
					client,
					AwarenessUpdateEntry {
						clock: entry.clock + 1,
						json: NULL.into(),
					},
				)
			})
			.collect();

		(!removed.is_empty()).then_some(AwarenessUpdate { clients: removed })
	}

	/// Every present state, for a peer that just joined.
	pub fn snapshot(&self) -> Option<AwarenessUpdate> {
		let clients: HashMap<ClientID, AwarenessUpdateEntry> = self
			.states
			.iter()
			.filter(|(_, entry)| &*entry.json != NULL)
			.map(|(client, entry)| {
				(
					*client,
					AwarenessUpdateEntry {
						clock: entry.clock,
						json: entry.json.clone(),
					},
				)
			})
			.collect();

		(!clients.is_empty()).then_some(AwarenessUpdate { clients })
	}

	/// Whether no states are present.
	#[cfg(test)]
	pub fn is_empty(&self) -> bool {
		self.states.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn update(client: u64, clock: u32, json: &str) -> AwarenessUpdate {
		AwarenessUpdate {
			clients: HashMap::from([(
				ClientID::new(client),
				AwarenessUpdateEntry {
					clock,
					json: json.into(),
				},
			)]),
		}
	}

	#[test]
	fn newer_clocks_win() {
		let mut table = AwarenessTable::default();
		assert!(table.apply(1, update(10, 1, "{}")).is_some());
		assert!(table.apply(1, update(10, 1, r#"{"a":1}"#)).is_none());
		assert!(table.apply(1, update(10, 2, r#"{"a":1}"#)).is_some());
	}

	#[test]
	fn peers_cannot_overwrite_each_other() {
		let mut table = AwarenessTable::default();
		table.apply(1, update(10, 1, "{}"));
		assert!(table.apply(2, update(10, 5, r#"{"evil":true}"#)).is_none());
	}

	#[test]
	fn leaving_withdraws_states() {
		let mut table = AwarenessTable::default();
		table.apply(1, update(10, 3, "{}"));
		let removal = table.remove_peer(1).expect("one state");
		assert_eq!(removal.clients[&ClientID::new(10)].clock, 4);
		assert_eq!(&*removal.clients[&ClientID::new(10)].json, "null");
		assert!(table.is_empty());
	}

	#[test]
	fn oversized_states_are_dropped() {
		let mut table = AwarenessTable::default();
		let big = format!("\"{}\"", "x".repeat(MAX_STATE_BYTES));
		assert!(table.apply(1, update(10, 1, &big)).is_none());
	}
}
