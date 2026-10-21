//! Collaboration, behind the `collab` feature: the editor as a y-sync client
//! (the protocol y-websocket and Hocuspocus servers speak), which
//! `@canvist/web/collab` connects to a server over a WebSocket.
//!
//! The editor joins with no state of its own, so the room's document
//! replaces whatever it showed. Every remote change comes back as a commit,
//! so layout redoes only the blocks it touched.

use canvist_headless::join_message;
use js_sys::Array;
use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;

use crate::WebEditor;
use crate::error::JsResult;
use crate::error::headless;

#[wasm_bindgen]
impl WebEditor {
	/// Start joining a shared document as yrs client `client_id`, which must
	/// be unique among the room's peers. Returns the message to send the
	/// server first. The current document stays on screen until the
	/// server's state arrives in [`WebEditor::receive_sync`].
	pub fn join(&mut self, client_id: u32) -> Vec<u8> {
		self.joining = Some(client_id);
		join_message()
	}

	/// Whether the editor holds a shared document, as it does once the
	/// server's state has arrived.
	#[wasm_bindgen(js_name = isShared)]
	pub fn is_shared(&self) -> bool {
		self.doc.is_shared()
	}

	/// The message to send a server on reconnecting: sync step 1, which asks
	/// for what this editor missed.
	#[wasm_bindgen(js_name = syncStart)]
	pub fn sync_start(&self) -> JsResult<Vec<u8>> {
		self.doc.sync_start().map_err(headless)
	}

	/// Handle one binary y-sync message from the server, as
	/// `[replies, changed]`: the messages to send back, and whether the
	/// document changed, in which case the next frame shows it.
	#[wasm_bindgen(
		js_name = receiveSync,
		unchecked_return_type = "[replies: Uint8Array[], changed: boolean]"
	)]
	pub fn receive_sync(&mut self, message: &[u8]) -> JsResult<Array> {
		if let Some(client_id) = self.joining {
			return self.receive_join(message, client_id);
		}

		let (sync, commits) = self.doc.receive_sync_changes(message).map_err(headless)?;

		for commit in &commits {
			self.lay_out(commit);
		}

		Ok(reply(&sync.reply, sync.changed))
	}

	/// This editor's changes since the last call, as y-sync messages for the
	/// server.
	#[wasm_bindgen(js_name = flushSync, unchecked_return_type = "Uint8Array[]")]
	pub fn flush_sync(&mut self) -> JsResult<Array> {
		if !self.doc.is_shared() {
			return Ok(Array::new());
		}

		let messages = self.doc.flush_sync().map_err(headless)?;
		Ok(bytes(&messages))
	}
}

impl WebEditor {
	/// A message from the server while joining: swap in the shared document
	/// once it carries the server's state.
	fn receive_join(&mut self, message: &[u8], client_id: u32) -> JsResult<Array> {
		let joined = self
			.engine
			.join_sync(message, u64::from(client_id))
			.map_err(headless)?;

		let Some((doc, sync)) = joined else {
			return Ok(reply(&[], false));
		};

		self.joining = None;
		self.replace(doc);
		Ok(reply(&sync.reply, true))
	}
}

/// `[replies, changed]` for JavaScript.
fn reply(messages: &[Vec<u8>], changed: bool) -> Array {
	[JsValue::from(bytes(messages)), JsValue::from(changed)]
		.iter()
		.collect()
}

fn bytes(messages: &[Vec<u8>]) -> Array {
	messages
		.iter()
		.map(|message| Uint8Array::from(message.as_slice()))
		.collect()
}
