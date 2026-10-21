//! Collaboration over Yjs: an editor is a peer of every other canvist
//! binding and of Yjs itself.
//!
//! The transport is the host's: send what [`Editor::take_updates`] returns
//! to the other peers after every change an observer reports with
//! `updates`, and pass what arrives to [`Editor::apply_update`]. For a
//! y-websocket server such as canvist Sync, speak y-sync with
//! [`Editor::sync_start`], [`Editor::receive_sync`], and
//! [`Editor::flush_sync`] instead.

use crate::Editor;
use crate::SyncReply;
use crate::error::Result;

#[uniffi::export]
#[allow(
	clippy::needless_pass_by_value,
	reason = "UniFFI hands byte arrays over by value; Kotlin would see `ByteBuffer` for a slice"
)]
impl Editor {
	/// Start sharing the document as yrs client `client_id`, unique among
	/// peers: its content becomes the shared state others join from.
	pub fn share(&self, client_id: u64) -> Result<()> {
		self.change(|session| session.with_doc(|doc| doc.share(client_id)))
	}

	/// Replace the document with a shared one, joined from Yjs `state` (v1,
	/// as `encode_state` or `Y.encodeStateAsUpdate` returns) as yrs client
	/// `client_id`.
	pub fn join(&self, state: Vec<u8>, client_id: u64) -> Result<()> {
		self.change(|session| {
			let doc = session.engine().from_yjs(&state, client_id)?;
			session.set_doc(doc);
			Ok(())
		})
	}

	/// Whether the document is shared.
	pub fn is_shared(&self) -> bool {
		self.lock().doc.is_shared()
	}

	/// Integrate a v1 Yjs update from another peer. Updates may arrive in
	/// any order and more than once. Returns whether the document changed.
	pub fn apply_update(&self, update: Vec<u8>) -> Result<bool> {
		self.change(|session| session.with_doc(|doc| doc.apply_update(&update)))
	}

	/// The whole shared state as one v1 update, to persist or to send to a
	/// joining peer.
	pub fn encode_state(&self) -> Result<Vec<u8>> {
		Ok(self.lock().doc.encode_state()?)
	}

	/// The v1 state vector, which a peer answers with what this one lacks.
	pub fn state_vector(&self) -> Result<Vec<u8>> {
		Ok(self.lock().doc.state_vector()?)
	}

	/// Everything a peer at v1 `state_vector` is missing, as one v1 update.
	pub fn encode_diff(&self, state_vector: Vec<u8>) -> Result<Vec<u8>> {
		Ok(self.lock().doc.encode_diff(&state_vector)?)
	}

	/// Drain the v1 updates this editor's own changes produced, to send to
	/// every other peer.
	pub fn take_updates(&self) -> Result<Vec<Vec<u8>>> {
		Ok(self.lock().doc.take_updates()?)
	}

	/// The y-sync message that starts a connection: sync step 1, asking
	/// the other side for what this editor lacks.
	pub fn sync_start(&self) -> Result<Vec<u8>> {
		Ok(self.lock().doc.sync_start()?)
	}

	/// Handle one binary y-sync message, as y-websocket frames them. Send
	/// `reply` back on the connection it came from; a relaying peer also
	/// sends `broadcast` to its other connections.
	pub fn receive_sync(&self, message: Vec<u8>) -> Result<SyncReply> {
		self.change(|session| {
			let sync = session.with_doc(|doc| doc.receive_sync(&message))?;
			Ok(SyncReply {
				reply: sync.reply,
				broadcast: sync.broadcast,
				changed: sync.changed,
			})
		})
	}

	/// This editor's changes since the last call, as y-sync update messages
	/// for every connection.
	pub fn flush_sync(&self) -> Result<Vec<Vec<u8>>> {
		Ok(self.lock().doc.flush_sync()?)
	}
}
