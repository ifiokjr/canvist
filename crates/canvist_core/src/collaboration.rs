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
use yrs::Text;
use yrs::TextRef;
use yrs::Transact;
use yrs::updates::decoder::Decode;

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
		let txn = self.doc.transact();
		txn.encode_state_as_update_v1(&yrs::StateVector::default())
	}

	/// Apply a binary update from a remote peer.
	pub fn apply_update(&self, update: &[u8]) {
		let mut txn = self.doc.transact_mut();
		let update = yrs::Update::decode_v1(update)
			.unwrap_or_else(|e| panic!("failed to decode Yrs update: {e}"));
		let _ = txn.apply_update(update);
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
}
