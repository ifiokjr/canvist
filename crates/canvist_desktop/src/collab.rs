//! Collaboration over Yjs (feature `collab`): the editor's document as a
//! y-sync peer of a y-websocket or Hocuspocus server.
//!
//! A client that joins a room takes the room's content: [`DesktopEditor::connect`]
//! asks the server for everything, and the editor switches to the shared
//! document when the answer arrives. Sharing the local document instead
//! ([`DesktopEditor::share`]) suits the first peer of a new room.

use canvist_headless::HeadlessError;
use canvist_headless::canvist_collab::CollabError;
use canvist_headless::canvist_collab::yrs::StateVector;
use canvist_headless::canvist_collab::yrs::encoding::read::Cursor;
use canvist_headless::canvist_collab::yrs::sync::Message;
use canvist_headless::canvist_collab::yrs::sync::MessageReader;
use canvist_headless::canvist_collab::yrs::sync::SyncMessage;
use canvist_headless::canvist_collab::yrs::updates::decoder::DecoderV1;
use canvist_headless::canvist_collab::yrs::updates::encoder::Encode;

use crate::DesktopEditor;
use crate::DesktopResult;

/// A join waiting for the server's state.
#[derive(Clone, Debug, Default)]
pub(crate) struct Joining {
	client_id: Option<u64>,
	/// Messages that arrived before the state, handled once joined.
	held: Vec<Vec<u8>>,
}

impl<Handles: Copy> DesktopEditor<Handles> {
	/// Start joining a shared room as yrs client `client_id`, which must be
	/// unique among peers. Send the returned message to the server; the
	/// editor switches to the room's document when
	/// [`DesktopEditor::receive_sync`] gets the server's state.
	#[must_use]
	pub fn connect(&mut self, client_id: u64) -> Vec<u8> {
		self.joining = Joining {
			client_id: Some(client_id),
			held: Vec::new(),
		};
		Message::Sync(SyncMessage::SyncStep1(StateVector::default())).encode_v1()
	}

	/// Join a shared document from Yjs `state` (a v1 update) as yrs client
	/// `client_id`. The editor's document is replaced.
	pub fn join(&mut self, state: &[u8], client_id: u64) -> DesktopResult<()> {
		let doc = self.engine.from_yjs(state, client_id)?;
		self.joining = Joining::default();
		self.set_doc(doc)
	}

	/// Start sharing the current document as yrs client `client_id`: its
	/// content becomes the shared state that other peers join.
	pub fn share(&mut self, client_id: u64) -> DesktopResult<()> {
		Ok(self.doc.share(client_id)?)
	}

	/// The y-sync message that asks a server for whatever a shared document
	/// is missing, as on reconnecting.
	pub fn sync_start(&self) -> DesktopResult<Vec<u8>> {
		Ok(self.doc.sync_start()?)
	}

	/// Handle one y-sync message from the server, returning the messages to
	/// send back. Collaborators' changes lay out incrementally, and the
	/// selection moves with the text around it.
	pub fn receive_sync(&mut self, message: &[u8]) -> DesktopResult<Vec<Vec<u8>>> {
		if let Some(client_id) = self.joining.client_id {
			return self.receive_while_joining(message, client_id);
		}

		let (sync, commits) = self.doc.receive_sync_changes(message)?;

		if sync.changed {
			self.track(&commits);
		}

		Ok(sync.reply)
	}

	/// The local changes since the last call, as y-sync update messages for
	/// the server. Call it after handling input.
	pub fn flush_sync(&mut self) -> DesktopResult<Vec<Vec<u8>>> {
		if self.joining.client_id.is_some() {
			return Ok(Vec::new());
		}

		Ok(self.doc.flush_sync()?)
	}

	/// Whether the document is shared.
	#[must_use]
	pub fn is_shared(&self) -> bool {
		self.doc.is_shared()
	}

	/// Look for the server's state, join with it, then handle what was held
	/// back.
	fn receive_while_joining(
		&mut self,
		message: &[u8],
		client_id: u64,
	) -> DesktopResult<Vec<Vec<u8>>> {
		let mut messages = decode(message)?;
		let state = messages
			.iter()
			.position(|message| matches!(message, Message::Sync(SyncMessage::SyncStep2(_))))
			.map(|index| messages.remove(index));
		self.joining
			.held
			.extend(messages.iter().map(Encode::encode_v1));

		let Some(Message::Sync(SyncMessage::SyncStep2(state))) = state else {
			return Ok(Vec::new());
		};
		let held = std::mem::take(&mut self.joining.held);
		self.join(&state, client_id)?;

		// The room may have been empty, in which case joining wrote a first
		// paragraph that the server needs too.
		let mut replies = self.flush_sync()?;

		for message in held {
			replies.extend(self.receive_sync(&message)?);
		}

		Ok(replies)
	}
}

/// Every y-protocols message in `data`.
fn decode(data: &[u8]) -> DesktopResult<Vec<Message>> {
	let mut decoder = DecoderV1::new(Cursor::new(data));
	MessageReader::new(&mut decoder)
		.collect::<Result<Vec<_>, _>>()
		.map_err(|error| HeadlessError::from(CollabError::from(error)).into())
}
