//! One peer's session with one room, independent of the transport.
//!
//! The session reads y-protocols messages and decides what to apply,
//! broadcast, and answer. The y-websocket and Hocuspocus transports only
//! frame the bytes.

use std::sync::Arc;

use yrs::encoding::read::Cursor;
use yrs::sync::Message;
use yrs::sync::MessageReader;
use yrs::sync::SyncMessage;
use yrs::updates::decoder::Decode;
use yrs::updates::decoder::DecoderV1;

use super::room::Limits;
use super::room::PeerId;
use super::room::Room;
use super::validate::Rejected;
use crate::auth::Scope;

/// What to send back to the peer that sent a message.
#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
	/// A y-protocols message.
	Message(Vec<u8>),
	/// The room's sync step 1, answering the peer's step 1. Hocuspocus peers
	/// expect it; y-websocket peers already got it on connect.
	Step1(Vec<u8>),
	/// Whether an update was accepted. Hocuspocus peers use it to track
	/// unsaved changes; y-websocket has no such message.
	SyncStatus(bool),
}

/// A message the session could not read.
#[derive(Debug, thiserror::Error)]
#[error("malformed y-protocols message")]
pub struct Malformed;

/// A peer's session with a room.
#[derive(Debug)]
pub struct Session {
	room: Arc<Room>,
	peer: PeerId,
	scope: Scope,
	editor: Arc<str>,
	limits: Limits,
}

impl Session {
	/// A session for `peer` in `room`.
	pub fn new(
		room: Arc<Room>,
		peer: PeerId,
		scope: Scope,
		editor: Arc<str>,
		limits: Limits,
	) -> Self {
		Self {
			room,
			peer,
			scope,
			editor,
			limits,
		}
	}

	/// The room.
	pub fn room(&self) -> &Arc<Room> {
		&self.room
	}

	/// The peer's id in the room.
	pub fn peer(&self) -> PeerId {
		self.peer
	}

	/// The scope the peer was granted.
	pub fn scope(&self) -> Scope {
		self.scope
	}

	/// What the server sends when a y-websocket peer connects: its state
	/// vector and everyone's presence.
	pub fn greeting(&self) -> Vec<Vec<u8>> {
		std::iter::once(self.room.step1())
			.chain(self.room.awareness())
			.collect()
	}

	/// Handle one frame of y-protocols messages.
	pub fn handle(&self, frame: &[u8]) -> Result<Vec<Reply>, Malformed> {
		let mut decoder = DecoderV1::new(Cursor::new(frame));
		let mut replies = Vec::new();

		for message in MessageReader::new(&mut decoder) {
			self.handle_message(message.map_err(|_| Malformed)?, &mut replies);
		}

		Ok(replies)
	}

	fn handle_message(&self, message: Message, replies: &mut Vec<Reply>) {
		match message {
			Message::Sync(SyncMessage::SyncStep1(state)) => {
				replies.push(Reply::Message(self.room.step2(&state)));
				replies.push(Reply::Step1(self.room.step1()));
			}
			Message::Sync(SyncMessage::SyncStep2(update) | SyncMessage::Update(update)) => {
				replies.push(Reply::SyncStatus(self.apply(&update)));
			}
			Message::Awareness(update) => self.room.apply_awareness(self.peer, update),
			Message::AwarenessQuery => replies.extend(self.room.awareness().map(Reply::Message)),
			// Clients only send auth messages to servers that ask; custom
			// messages have no meaning here.
			Message::Auth(_) | Message::Custom(..) => {}
		}
	}

	/// Apply `update`, returning whether it was accepted. Refused updates
	/// are dropped, not fatal: the peer keeps its local changes and stays
	/// connected.
	fn apply(&self, update: &[u8]) -> bool {
		// Viewers answer sync step 1 like everyone else, so an answer with
		// nothing new is accepted rather than refused as an edit.
		let result = match self.scope {
			Scope::Read if is_empty_update(update) => return true,
			Scope::Read => Err(Rejected::ReadOnly),
			Scope::Write => {
				self.room
					.apply(Some(self.peer), &self.editor, update, self.limits)
			}
		};

		match result {
			Ok(_) => true,
			Err(rejected) => {
				tracing::info!(document = %self.room.key(), editor = %self.editor, %rejected, "update refused");
				false
			}
		}
	}

	/// Leave the room.
	pub fn leave(&self) {
		self.room.leave(self.peer);
	}
}

fn is_empty_update(update: &[u8]) -> bool {
	yrs::Update::decode_v1(update).is_ok_and(|update| update.is_empty())
}
