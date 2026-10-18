//! The y-sync protocol, independent of transport.
//!
//! A [`SyncSession`] turns incoming binary messages into model commits and
//! replies, and local changes into outgoing messages. The host moves the
//! bytes over whatever it likes: a WebSocket to a y-websocket or Hocuspocus
//! server, WebRTC, or a test harness.
//!
//! The handshake is the standard one: each side sends sync step 1 (its state
//! vector), answers the other's step 1 with step 2 (everything the other is
//! missing), and from then on sends each change as an update. Awareness
//! messages carry [`Presence`] alongside.
//!
//! Hocuspocus wraps every y-sync message with the document name and adds an
//! authentication step; [`Framing::Hocuspocus`] handles both.

use std::sync::Arc;

use canvist_model::Commit;
use canvist_model::Document;
use canvist_model::Selection;
use yrs::StateVector;
use yrs::encoding::read::Cursor;
use yrs::encoding::read::Read;
use yrs::encoding::write::Write;
use yrs::sync::AwarenessUpdate;
use yrs::sync::Message;
use yrs::sync::MessageReader;
use yrs::sync::SyncMessage;
use yrs::updates::decoder::Decode;
use yrs::updates::decoder::DecoderV1;
use yrs::updates::encoder::Encode;
use yrs::updates::encoder::Encoder;
use yrs::updates::encoder::EncoderV1;

use crate::Binding;
use crate::CollabError;
use crate::Collaborator;
use crate::Encoding;
use crate::Presence;
use crate::User;

/// How messages are wrapped on the wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Framing {
	/// Plain y-protocols messages, as y-websocket servers expect.
	YWebsocket,
	/// Hocuspocus: each message starts with the document name, and the
	/// session authenticates before syncing.
	Hocuspocus {
		/// The document name the server stores this document under.
		document: Arc<str>,
		/// The token the server's `onAuthenticate` hook checks; empty when the
		/// server needs none.
		token: Arc<str>,
	},
}

/// Hocuspocus message types beyond the y-protocols ones.
mod hocuspocus {
	pub(super) const SYNC: u8 = 0;
	pub(super) const AWARENESS: u8 = 1;
	pub(super) const AUTH: u8 = 2;
	pub(super) const QUERY_AWARENESS: u8 = 3;
	/// A sync message that must not trigger another step 1.
	pub(super) const SYNC_REPLY: u8 = 4;
	/// The server closed the document for this connection.
	pub(super) const CLOSE: u8 = 7;
	/// Auth sub-message carrying a token.
	pub(super) const TOKEN: u8 = 0;
	/// Auth sub-message denying access, with a reason.
	pub(super) const PERMISSION_DENIED: u8 = 1;
	/// Auth sub-message granting access, with a scope.
	pub(super) const AUTHENTICATED: u8 = 2;
}

/// The result of handling one incoming message.
#[derive(Debug)]
pub struct Received {
	/// The model commit mirroring remote changes, often empty.
	pub commit: Commit,
	/// Messages to send back to the peer, in order.
	pub replies: Vec<Vec<u8>>,
}

/// One connection's worth of y-sync state around a [`Binding`] and its
/// [`Presence`].
#[derive(Debug)]
pub struct SyncSession {
	binding: Binding,
	presence: Presence,
	framing: Framing,
	synced: bool,
}

/// What one incoming message asks of the session.
enum Incoming {
	/// A y-protocols message.
	Message(Message),
	/// Hocuspocus granted access.
	Authenticated,
	/// Anything this session has no use for, such as stateless messages.
	Ignored,
}

impl SyncSession {
	/// A session for `binding` and `presence`, framed as `framing`.
	pub fn new(binding: Binding, presence: Presence, framing: Framing) -> Self {
		Self {
			binding,
			presence,
			framing,
			synced: false,
		}
	}

	/// The binding.
	pub fn binding(&self) -> &Binding {
		&self.binding
	}

	/// The binding, for local commits, undo, and redo.
	pub fn binding_mut(&mut self) -> &mut Binding {
		&mut self.binding
	}

	/// The presence.
	pub fn presence(&self) -> &Presence {
		&self.presence
	}

	/// The presence, to publish this peer's selection.
	pub fn presence_mut(&mut self) -> &mut Presence {
		&mut self.presence
	}

	/// Publish who this peer is and where its selection is in `doc`; the
	/// next [`SyncSession::flush`] sends it.
	pub fn set_presence(&mut self, doc: &Document, user: &User, selection: Option<Selection>) {
		self.presence
			.set_local((&self.binding, doc), user, selection);
	}

	/// Every other connected peer, with selections resolved against `doc`.
	pub fn collaborators(&self, doc: &Document) -> Vec<Collaborator> {
		self.presence.collaborators(&self.binding, doc)
	}

	/// Whether the peer has sent everything this session was missing.
	pub fn is_synced(&self) -> bool {
		self.synced
	}

	/// The messages to send once the connection opens.
	pub fn open(&mut self) -> Result<Vec<Vec<u8>>, CollabError> {
		self.synced = false;

		match &self.framing {
			Framing::YWebsocket => self.greeting(),
			Framing::Hocuspocus { document, token } => Ok(vec![auth_message(document, token)]),
		}
	}

	/// Sync step 1 and this peer's presence.
	fn greeting(&self) -> Result<Vec<Vec<u8>>, CollabError> {
		let state = StateVector::decode_v1(&self.binding.state_vector())?;
		let messages = [
			Message::Sync(SyncMessage::SyncStep1(state)),
			Message::Awareness(self.presence.update()?),
		];

		Ok(messages.iter().map(|message| self.frame(message)).collect())
	}

	/// Handle one incoming binary message.
	pub fn receive(&mut self, doc: &mut Document, data: &[u8]) -> Result<Received, CollabError> {
		let mut received = Received {
			commit: doc.transaction().commit(),
			replies: Vec::new(),
		};

		for incoming in self.decode(data)? {
			self.handle(doc, incoming, &mut received)?;
		}

		Ok(received)
	}

	fn handle(
		&mut self,
		doc: &mut Document,
		incoming: Incoming,
		received: &mut Received,
	) -> Result<(), CollabError> {
		match incoming {
			Incoming::Message(message) => self.handle_message(doc, message, received),
			Incoming::Authenticated => {
				received.replies.extend(self.greeting()?);
				Ok(())
			}
			Incoming::Ignored => Ok(()),
		}
	}

	fn handle_message(
		&mut self,
		doc: &mut Document,
		message: Message,
		received: &mut Received,
	) -> Result<(), CollabError> {
		match message {
			Message::Sync(SyncMessage::SyncStep1(state)) => {
				let update = self.binding.encode_diff_from(&state, Encoding::V1);
				let reply = Message::Sync(SyncMessage::SyncStep2(update));
				received.replies.push(self.frame(&reply));
			}
			Message::Sync(SyncMessage::SyncStep2(update)) => {
				self.apply(doc, &update, received)?;
				self.synced = true;
			}
			Message::Sync(SyncMessage::Update(update)) => self.apply(doc, &update, received)?,
			Message::Awareness(update) => self.presence.apply_update(update)?,
			Message::AwarenessQuery => {
				let reply = Message::Awareness(self.presence.update()?);
				received.replies.push(self.frame(&reply));
			}
			Message::Auth(Some(reason)) => return Err(CollabError::PermissionDenied(reason)),
			Message::Auth(None) => {}
			Message::Custom(tag, _) => return Err(CollabError::UnsupportedMessage(tag)),
		}

		Ok(())
	}

	fn apply(
		&mut self,
		doc: &mut Document,
		update: &[u8],
		received: &mut Received,
	) -> Result<(), CollabError> {
		let commit = self.binding.apply_update(doc, update, Encoding::V1)?;
		received.commit = crate::binding::compose(
			std::mem::replace(&mut received.commit, doc.transaction().commit()),
			commit,
			doc,
		);
		Ok(())
	}

	/// Messages for everything that changed locally since the last call:
	/// document updates (local commits, undo, redo, and repairs) and this
	/// peer's presence.
	pub fn flush(&mut self) -> Result<Vec<Vec<u8>>, CollabError> {
		let mut messages: Vec<Vec<u8>> = self
			.binding
			.take_updates()
			.into_iter()
			.map(|update| self.frame(&Message::Sync(SyncMessage::Update(update))))
			.collect();

		if let Some(update) = self.presence.take_local_change()? {
			messages.push(self.frame(&Message::Awareness(update)));
		}

		Ok(messages)
	}

	/// Encode `message` with this session's framing.
	fn frame(&self, message: &Message) -> Vec<u8> {
		let mut encoder = EncoderV1::new();

		if let Framing::Hocuspocus { document, .. } = &self.framing {
			encoder.write_string(document);
		}

		message.encode(&mut encoder);
		encoder.to_vec()
	}

	fn decode(&self, data: &[u8]) -> Result<Vec<Incoming>, CollabError> {
		match &self.framing {
			Framing::YWebsocket => decode_messages(data),
			Framing::Hocuspocus { document, .. } => decode_hocuspocus(data, document),
		}
	}
}

/// Every y-protocols message in `data`. y-websocket sends one per frame, but
/// the protocol allows several back to back.
fn decode_messages(data: &[u8]) -> Result<Vec<Incoming>, CollabError> {
	let mut decoder = DecoderV1::new(Cursor::new(data));

	MessageReader::new(&mut decoder)
		.map(|message| Ok(Incoming::Message(message?)))
		.collect()
}

/// One Hocuspocus message, or nothing if it is for another document on a
/// shared connection.
fn decode_hocuspocus(data: &[u8], document: &str) -> Result<Vec<Incoming>, CollabError> {
	let mut decoder = DecoderV1::new(Cursor::new(data));

	if decoder.read_string()? != document {
		return Ok(Vec::new());
	}

	let kind: u8 = decoder.read_var()?;
	let incoming = match kind {
		hocuspocus::SYNC | hocuspocus::SYNC_REPLY => {
			Incoming::Message(Message::Sync(SyncMessage::decode(&mut decoder)?))
		}
		hocuspocus::AWARENESS => {
			let update = AwarenessUpdate::decode_v1(decoder.read_buf()?)?;
			Incoming::Message(Message::Awareness(update))
		}
		hocuspocus::QUERY_AWARENESS => Incoming::Message(Message::AwarenessQuery),
		hocuspocus::AUTH => decode_auth(&mut decoder)?,
		hocuspocus::CLOSE => {
			return Err(CollabError::Closed(decoder.read_string()?.to_owned()));
		}
		_ => Incoming::Ignored,
	};

	Ok(vec![incoming])
}

fn decode_auth(decoder: &mut DecoderV1<'_>) -> Result<Incoming, CollabError> {
	let kind: u8 = decoder.read_var()?;

	match kind {
		hocuspocus::AUTHENTICATED => Ok(Incoming::Authenticated),
		hocuspocus::PERMISSION_DENIED => {
			Err(CollabError::PermissionDenied(
				decoder.read_string()?.to_owned(),
			))
		}
		_ => Ok(Incoming::Ignored),
	}
}

/// The Hocuspocus authentication message carrying `token`.
fn auth_message(document: &str, token: &str) -> Vec<u8> {
	let mut encoder = EncoderV1::new();
	encoder.write_string(document);
	encoder.write_var(hocuspocus::AUTH);
	encoder.write_var(hocuspocus::TOKEN);
	encoder.write_string(token);
	encoder.to_vec()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn hocuspocus_messages_start_with_the_document_name() {
		let message = auth_message("notes", "secret");
		let mut decoder = DecoderV1::new(Cursor::new(&message));

		assert_eq!(decoder.read_string().ok(), Some("notes"));
		assert_eq!(decoder.read_var::<u8>().ok(), Some(hocuspocus::AUTH));
		assert_eq!(decoder.read_var::<u8>().ok(), Some(hocuspocus::TOKEN));
		assert_eq!(decoder.read_string().ok(), Some("secret"));
	}

	#[test]
	fn messages_for_other_documents_are_skipped() {
		let message = auth_message("other", "");

		assert!(
			decode_hocuspocus(&message, "notes")
				.expect("decodes")
				.is_empty()
		);
	}
}
