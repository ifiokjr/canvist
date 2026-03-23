//! Real-time collaboration via Yjs CRDTs.
//!
//! This module provides a [`CollaborationSession`] that wraps a
//! [`yrs::Doc`] and keeps it synchronised with a canvist [`Document`].
//!
//! # Architecture
//!
//! ```text
//!  Local edits ──► CollaborationSession ──► Yrs Doc ──► Network
//!                         │                              │
//!  canvist Document ◄─────┘      Remote updates ◄────────┘
//! ```
//!
//! The Yrs document is the source of truth for collaboration — local edits are
//! written into the Yrs shared types, and remote updates are decoded and
//! applied back to the canvist document model.

use yrs::Doc;
use yrs::GetString;
use yrs::ReadTxn;
use yrs::StateVector;
use yrs::Text;
use yrs::TextRef;
use yrs::Transact;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;

use crate::Document;
use crate::operation::Operation;

/// A collaboration session backed by a Yrs CRDT document.
///
/// Create one per document and use it to produce/consume sync messages.
///
/// # Examples
///
/// ```
/// use canvist_core::collaboration::CollaborationSession;
///
/// let session = CollaborationSession::new();
///
/// // Insert text via the CRDT.
/// session.insert(0, "Hello!");
/// assert_eq!(session.text(), "Hello!");
/// ```
pub struct CollaborationSession {
	/// The underlying Yrs document.
	doc: Doc,
	/// Name of the shared text type within the Yrs doc.
	text_key: String,
}

impl CollaborationSession {
	/// Create a new collaboration session with a fresh Yrs document.
	#[must_use]
	pub fn new() -> Self {
		Self::with_key("content")
	}

	/// Create a new collaboration session using a custom shared-text key.
	#[must_use]
	pub fn with_key(key: impl Into<String>) -> Self {
		Self {
			doc: Doc::new(),
			text_key: key.into(),
		}
	}

	/// Return a reference to the underlying Yrs document.
	#[must_use]
	pub fn doc(&self) -> &Doc {
		&self.doc
	}

	/// Insert text at the given character offset in the shared text.
	pub fn insert(&self, offset: u32, text: &str) {
		let text_ref = self.shared_text();
		let mut txn = self.doc.transact_mut();
		text_ref.insert(&mut txn, offset, text);
	}

	/// Delete `length` characters starting at `offset` in the shared text.
	pub fn delete(&self, offset: u32, length: u32) {
		let text_ref = self.shared_text();
		let mut txn = self.doc.transact_mut();
		text_ref.remove_range(&mut txn, offset, length);
	}

	/// Return the current plain-text content of the shared text.
	#[must_use]
	pub fn text(&self) -> String {
		let text_ref = self.shared_text();
		let txn = self.doc.transact();
		text_ref.get_string(&txn)
	}

	/// Return the character count of the shared text.
	#[must_use]
	pub fn len(&self) -> u32 {
		let text_ref = self.shared_text();
		let txn = self.doc.transact();
		text_ref.len(&txn)
	}

	/// Whether the shared text is empty.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Encode the full document state as a binary update.
	///
	/// Send this to a new peer so they can bootstrap their local copy.
	#[must_use]
	pub fn encode_state(&self) -> Vec<u8> {
		self.encode_diff_from_state_vector(&StateVector::default())
	}

	/// Encode the local state vector for incremental sync handshakes.
	///
	/// Peers can exchange state vectors and request only missing updates.
	#[must_use]
	pub fn encode_state_vector(&self) -> Vec<u8> {
		let txn = self.doc.transact();
		txn.state_vector().encode_v1()
	}

	/// Decode a binary state vector payload.
	pub fn decode_state_vector(payload: &[u8]) -> Result<StateVector, yrs::encoding::read::Error> {
		StateVector::decode_v1(payload)
	}

	/// Encode an incremental update containing changes missing from `state_vector`.
	#[must_use]
	pub fn encode_diff_from_state_vector(&self, state_vector: &StateVector) -> Vec<u8> {
		let txn = self.doc.transact();
		txn.encode_state_as_update_v1(state_vector)
	}

	/// Encode an incremental update from a previously encoded state-vector payload.
	pub fn encode_diff_from_state_vector_payload(
		&self,
		payload: &[u8],
	) -> Result<Vec<u8>, yrs::encoding::read::Error> {
		let state_vector = Self::decode_state_vector(payload)?;
		Ok(self.encode_diff_from_state_vector(&state_vector))
	}

	/// Apply a semantic operation into the CRDT shared text.
	///
	/// Returns `true` if the operation was mapped to text changes.
	pub fn apply_operation(&self, operation: &Operation) -> bool {
		if let Some((offset, delete_len, insert_text)) = operation.as_text_delta() {
			let text_ref = self.shared_text();
			let mut txn = self.doc.transact_mut();
			if delete_len > 0 {
				text_ref.remove_range(&mut txn, offset, delete_len);
			}
			if !insert_text.is_empty() {
				text_ref.insert(&mut txn, offset, &insert_text);
			}
			true
		} else {
			false
		}
	}

	/// Sync the current plain text from a canvist [`Document`] into CRDT state.
	pub fn sync_from_document(&self, document: &Document) {
		let text_ref = self.shared_text();
		let mut txn = self.doc.transact_mut();
		let current = text_ref.get_string(&txn);
		if !current.is_empty() {
			let len = text_ref.len(&txn);
			text_ref.remove_range(&mut txn, 0, len);
		}
		let next = document.plain_text();
		if !next.is_empty() {
			text_ref.insert(&mut txn, 0, &next);
		}
	}

	/// Sync CRDT text content into a canvist [`Document`].
	pub fn sync_to_document(&self, document: &mut Document) {
		document.set_plain_text(&self.text());
	}

	/// Apply a binary update from a remote peer.
	pub fn apply_update(&self, update: &[u8]) {
		let mut txn = self.doc.transact_mut();
		let update = yrs::Update::decode_v1(update)
			.unwrap_or_else(|e| panic!("failed to decode Yrs update: {e}"));
		let _ = txn.apply_update(update);
	}

	/// Apply a binary update and then project the resulting CRDT text into
	/// the provided canvist [`Document`].
	pub fn apply_remote_update_to_document(&self, update: &[u8], document: &mut Document) {
		self.apply_update(update);
		self.sync_to_document(document);
	}

	// -- internal helpers --

	fn shared_text(&self) -> TextRef {
		self.doc.get_or_insert_text(&*self.text_key)
	}
}

impl Default for CollaborationSession {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn insert_and_read() {
		let session = CollaborationSession::new();
		session.insert(0, "Hello, world!");
		assert_eq!(session.text(), "Hello, world!");
		assert_eq!(session.len(), 13);
	}

	#[test]
	fn delete_range() {
		let session = CollaborationSession::new();
		session.insert(0, "Hello, world!");
		session.delete(5, 2); // remove ", "
		assert_eq!(session.text(), "Helloworld!");
	}

	#[test]
	fn sync_between_peers() {
		let peer_a = CollaborationSession::new();
		let peer_b = CollaborationSession::new();

		peer_a.insert(0, "Hello");

		// Sync A → B.
		let update = peer_a.encode_state();
		peer_b.apply_update(&update);

		assert_eq!(peer_b.text(), "Hello");

		// B makes an edit.
		peer_b.insert(5, " world");
		let update_b = peer_b.encode_state();
		peer_a.apply_update(&update_b);

		assert_eq!(peer_a.text(), "Hello world");
	}

	#[test]
	fn is_empty() {
		let session = CollaborationSession::new();
		assert!(session.is_empty());

		session.insert(0, "x");
		assert!(!session.is_empty());
	}

	#[test]
	fn sync_from_and_to_document() {
		let session = CollaborationSession::new();
		let mut doc = Document::new();
		doc.insert_text(crate::Position::zero(), "Hello bridge");

		session.sync_from_document(&doc);
		assert_eq!(session.text(), "Hello bridge");

		let mut projected = Document::new();
		session.sync_to_document(&mut projected);
		assert_eq!(projected.plain_text(), "Hello bridge");
	}

	#[test]
	fn apply_operation_maps_into_crdt() {
		let session = CollaborationSession::new();
		let inserted = Operation::insert(crate::Position::zero(), "Hello");
		assert!(session.apply_operation(&inserted));
		assert_eq!(session.text(), "Hello");

		let deleted = Operation::delete(crate::Selection::range(
			crate::Position::new(1),
			crate::Position::new(4),
		));
		assert!(session.apply_operation(&deleted));
		assert_eq!(session.text(), "Ho");
	}

	#[test]
	fn remote_update_projects_back_into_document() {
		let peer_a = CollaborationSession::new();
		let peer_b = CollaborationSession::new();
		let mut doc_b = Document::new();

		peer_a.insert(0, "Remote text");
		let update = peer_a.encode_state();

		peer_b.apply_remote_update_to_document(&update, &mut doc_b);
		assert_eq!(peer_b.text(), "Remote text");
		assert_eq!(doc_b.plain_text(), "Remote text");
	}

	#[test]
	fn state_vector_round_trip_and_incremental_diff() {
		let peer_a = CollaborationSession::new();
		let peer_b = CollaborationSession::new();

		peer_a.insert(0, "Hello");
		let bootstrap = peer_a.encode_state();
		peer_b.apply_update(&bootstrap);
		assert_eq!(peer_b.text(), "Hello");

		let b_sv_payload = peer_b.encode_state_vector();
		peer_a.insert(5, " world");
		let incremental = peer_a
			.encode_diff_from_state_vector_payload(&b_sv_payload)
			.expect("valid state vector payload should encode diff");
		peer_b.apply_update(&incremental);

		assert_eq!(peer_b.text(), "Hello world");
	}

	#[test]
	fn encode_diff_from_state_vector_payload_rejects_invalid_payload() {
		let session = CollaborationSession::new();
		let result = session.encode_diff_from_state_vector_payload(&[0xff, 0x00, 0x01]);
		assert!(result.is_err());
	}

	#[test]
	fn concurrent_edits_converge() {
		let peer_a = CollaborationSession::new();
		let peer_b = CollaborationSession::new();

		// Bootstrap both peers with the same initial state.
		peer_a.insert(0, "Hello");
		let bootstrap = peer_a.encode_state();
		peer_b.apply_update(&bootstrap);
		assert_eq!(peer_a.text(), "Hello");
		assert_eq!(peer_b.text(), "Hello");

		// Concurrent edits: A appends, B prepends.
		peer_a.insert(5, " World");
		peer_b.insert(0, "Say: ");

		// Exchange updates.
		let update_a = peer_a.encode_state();
		let update_b = peer_b.encode_state();
		peer_a.apply_update(&update_b);
		peer_b.apply_update(&update_a);

		// Both should converge to the same state (CRDT guarantee).
		assert_eq!(
			peer_a.text(),
			peer_b.text(),
			"peers should converge: A={:?} B={:?}",
			peer_a.text(),
			peer_b.text()
		);
		// Both texts should contain all edits.
		let final_text = peer_a.text();
		assert!(final_text.contains("Hello"), "should contain Hello");
		assert!(final_text.contains("World"), "should contain World");
		assert!(final_text.contains("Say:"), "should contain Say:");
	}

	#[test]
	fn three_peer_convergence() {
		let peer_a = CollaborationSession::new();
		let peer_b = CollaborationSession::new();
		let peer_c = CollaborationSession::new();

		peer_a.insert(0, "Base");
		let init = peer_a.encode_state();
		peer_b.apply_update(&init);
		peer_c.apply_update(&init);

		// Each peer makes a concurrent edit.
		peer_a.insert(4, " A");
		peer_b.insert(0, "B ");
		peer_c.insert(4, " C");

		// Full mesh sync.
		let ua = peer_a.encode_state();
		let ub = peer_b.encode_state();
		let uc = peer_c.encode_state();

		peer_a.apply_update(&ub);
		peer_a.apply_update(&uc);
		peer_b.apply_update(&ua);
		peer_b.apply_update(&uc);
		peer_c.apply_update(&ua);
		peer_c.apply_update(&ub);

		// All three should converge.
		assert_eq!(peer_a.text(), peer_b.text());
		assert_eq!(peer_b.text(), peer_c.text());

		let final_text = peer_a.text();
		assert!(final_text.contains("Base"), "should contain Base");
		assert!(final_text.contains('A'), "should contain A");
		assert!(final_text.contains('B'), "should contain B");
		assert!(final_text.contains('C'), "should contain C");
	}
}
