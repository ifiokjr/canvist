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
//! An update can also build on another client's items, through the items it
//! is inserted next to or inside. yrs keeps such an update pending until
//! those arrive, so it does not count as integrated: its client's later
//! updates are released only once the document's state vector, which leaves
//! pending items out, shows it.
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
/// [`take_next`](Reorder::take_next) returns for the document's current
/// state vector until it returns `None`:
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
/// 	while let Some(ready) = reorder.take_next(&txn.state_vector()) {
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

	/// Take one queued update that can apply to a document with `state`.
	///
	/// Apply it and call again with the document's new state vector, which
	/// may make more updates ready. The state must be read after each
	/// update: an update that yrs keeps pending, because it builds on another
	/// client's items that have not arrived, leaves the state unchanged, so
	/// its client's later updates keep waiting instead of overtaking it.
	#[must_use]
	pub fn take_next(&mut self, state: &StateVector) -> Option<Update> {
		let reach = clocks(state);
		let index = self
			.held
			.iter()
			.position(|update| is_ready(update, &reach))?;
		Some(self.held.swap_remove(index))
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

#[cfg(test)]
mod tests {
	use yrs::Doc;
	use yrs::GetString;
	use yrs::ReadTxn;
	use yrs::Text;
	use yrs::Transact;
	use yrs::updates::decoder::Decode;

	use super::*;

	fn insert(doc: &Doc, index: u32, chunk: &str) -> Vec<u8> {
		let text = doc.get_or_insert_text("text");
		let mut txn = doc.transact_mut();
		text.insert(&mut txn, index, chunk);
		txn.encode_update_v1()
	}

	fn decode(update: &[u8]) -> Update {
		Update::decode_v1(update).expect("decodes")
	}

	/// Apply every ready update to `doc` the way the binding does.
	fn apply_ready(reorder: &mut Reorder, doc: &Doc) {
		let mut txn = doc.transact_mut();

		while let Some(update) = reorder.take_next(&txn.state_vector()) {
			txn.apply_update(update).expect("applies");
		}
	}

	#[test]
	fn an_update_yrs_keeps_pending_holds_back_its_clients_later_updates() {
		let alice = Doc::with_client_id(1);
		let typed = insert(&alice, 0, "x");
		let bob = Doc::with_client_id(2);
		bob.transact_mut()
			.apply_update(decode(&typed))
			.expect("applies");
		// Bob's first update is inserted after alice's text, so it builds on
		// alice even though it starts bob's history.
		let first = insert(&bob, 1, "y");
		let second = insert(&bob, 2, "z");

		let carol = Doc::with_client_id(3);
		let mut reorder = Reorder::default();
		reorder.hold(decode(&second));
		reorder.hold(decode(&first));
		apply_ready(&mut reorder, &carol);

		assert!(
			reorder.is_waiting(),
			"bob's second update waits while his first is pending in yrs"
		);

		reorder.hold(decode(&typed));
		apply_ready(&mut reorder, &carol);

		assert!(!reorder.is_waiting());
		let text = carol.get_or_insert_text("text");
		let txn = carol.transact();
		assert!(txn.store().pending_update().is_none());
		assert_eq!(text.get_string(&txn), "xyz");
	}
}
