//! Holding back updates that arrive before the updates they build on.
//!
//! Yjs keeps such updates pending until the gap is filled. yrs 0.27 and 0.28
//! instead integrate an update that overtakes an earlier one from the same
//! client, and the earlier one may then never apply, leaving peers diverged
//! ([y-crdt#670](https://github.com/y-crdt/y-crdt/issues/670),
//! [y-crdt#677](https://github.com/y-crdt/y-crdt/issues/677)). The binding
//! therefore hands yrs an update only once every client's earlier changes it
//! builds on, including the items it deletes, are already in the document.

use rustc_hash::FxHashMap;
use yrs::StateVector;
use yrs::Update;
use yrs::block::ClientID;

/// Updates waiting for the updates they build on.
#[derive(Default)]
pub(crate) struct Reorder {
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
	pub(crate) fn hold(&mut self, update: Update) {
		self.held.push(update);
	}

	/// Take every queued update that can apply to a document with `state`,
	/// in an order where each one's dependencies come first.
	pub(crate) fn take_ready(&mut self, state: &StateVector) -> Vec<Update> {
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
	pub(crate) fn is_waiting(&self) -> bool {
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
