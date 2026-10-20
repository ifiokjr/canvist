//! Collaboration: a document as a Yjs peer, and a y-sync endpoint for
//! servers.
//!
//! A shared [`Doc`] mirrors every change into a yrs document through a
//! [`Binding`], so local commands become Yjs updates and Yjs updates from
//! other peers change the document. [`Doc::receive_sync`] speaks the y-sync
//! protocol that y-websocket clients use, from the server's side: it answers
//! the sender and says what to relay to every other connection.

use canvist_collab::Binding;
use canvist_collab::CollabError;
use canvist_collab::Encoding;
use canvist_collab::Track;
use canvist_collab::yrs::StateVector;
use canvist_collab::yrs::encoding::read::Cursor;
use canvist_collab::yrs::sync::Message;
use canvist_collab::yrs::sync::MessageReader;
use canvist_collab::yrs::sync::SyncMessage;
use canvist_collab::yrs::updates::decoder::Decode;
use canvist_collab::yrs::updates::decoder::DecoderV1;
use canvist_collab::yrs::updates::encoder::Encode;
use canvist_editor::Update;
use canvist_model::BlockTree;
use canvist_model::Commit;
use canvist_model::Document;
use canvist_model::InlineText;

use crate::Doc;
use crate::Engine;
use crate::HeadlessError;

/// A Yjs update that changes nothing: no structs and no deletions.
const EMPTY_UPDATE: [u8; 2] = [0, 0];

/// What to send after [`Doc::receive_sync`] handled a message.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncReply {
	/// Messages for the connection the message came from.
	pub reply: Vec<Vec<u8>>,
	/// Messages for every other connection to this document.
	pub broadcast: Vec<Vec<u8>>,
	/// Whether the document changed.
	pub changed: bool,
}

/// How well the shared state fits the schema, from [`Doc::validate`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Validation {
	/// Items in the shared document the schema rejects, such as blocks of
	/// types no extension registered. They are hidden from the document but
	/// kept in the shared state.
	pub rejected_blocks: u32,
	/// Whether updates are waiting for earlier updates that have not
	/// arrived.
	pub waiting: bool,
}

impl Validation {
	/// Whether everything in the shared state is shown in the document.
	#[must_use]
	pub fn is_valid(self) -> bool {
		self.rejected_blocks == 0 && !self.waiting
	}
}

impl<H: Copy> Engine<H> {
	/// Join a shared document from Yjs `state` (v1 encoded, such as
	/// `Y.encodeStateAsUpdate` returns) as yrs client `client_id`, which
	/// must be unique among peers.
	///
	/// A canvist document always has a block, so joining an empty shared
	/// document writes one empty paragraph into it.
	pub fn from_yjs(&self, state: &[u8], client_id: u64) -> Result<Doc, HeadlessError> {
		let mut binding = Binding::with_client_id(self.schema().clone(), client_id)?;
		let mut doc = Document::empty(self.schema().clone(), self.client());
		binding.apply_update(&mut doc, state, Encoding::V1)?;
		ensure_block(&mut doc, &mut binding)?;

		let mut shared = self.wrap(doc)?;
		shared.shared = Some(binding);
		Ok(shared)
	}
}

impl Doc {
	/// Start sharing this document as yrs client `client_id`: its content
	/// becomes the shared state, which other peers join from
	/// [`Doc::encode_state`] or through [`Doc::receive_sync`]. This peer is
	/// the first, so nothing waits in [`Doc::take_updates`] afterwards.
	pub fn share(&mut self, client_id: u64) -> Result<(), HeadlessError> {
		if self.shared.is_some() {
			return Err(HeadlessError::AlreadyShared);
		}

		let mut binding = Binding::with_client_id(self.document().schema().clone(), client_id)?;
		binding.seed(self.document())?;
		// Peers receive the seeded content as the shared state when they join,
		// so it is not an update to broadcast.
		binding.take_updates();
		self.shared = Some(binding);
		Ok(())
	}

	/// Whether the document is shared.
	#[must_use]
	pub fn is_shared(&self) -> bool {
		self.shared.is_some()
	}

	/// Integrate a v1 Yjs update from another peer. Updates may arrive in
	/// any order and more than once. Returns whether the document changed.
	pub fn apply_update(&mut self, update: &[u8]) -> Result<bool, HeadlessError> {
		Ok(self.integrate(update)?.doc_changed())
	}

	fn integrate(&mut self, update: &[u8]) -> Result<Update, HeadlessError> {
		let mut env = self.env();
		let binding = self.shared.as_mut().ok_or(HeadlessError::NotShared)?;
		self.editor.apply_external(&mut env, |doc| {
			let commit = binding.apply_update(doc, update, Encoding::V1)?;
			with_block(commit, doc, binding)
		})
	}

	/// The whole shared state as one v1 update, to persist or to send to a
	/// peer that is joining.
	pub fn encode_state(&self) -> Result<Vec<u8>, HeadlessError> {
		Ok(self.binding()?.encode_state(Encoding::V1))
	}

	/// The v1 state vector, which a peer answers with what this one lacks.
	pub fn state_vector(&self) -> Result<Vec<u8>, HeadlessError> {
		Ok(self.binding()?.state_vector())
	}

	/// Everything a peer at v1 `state_vector` is missing, as one v1 update.
	pub fn encode_diff(&self, state_vector: &[u8]) -> Result<Vec<u8>, HeadlessError> {
		Ok(self.binding()?.encode_diff(state_vector, Encoding::V1)?)
	}

	/// Drain the v1 updates this peer produced (local commands, undo, and
	/// repairs) for broadcasting.
	pub fn take_updates(&mut self) -> Result<Vec<Vec<u8>>, HeadlessError> {
		Ok(self.binding_mut()?.take_updates())
	}

	/// Check the shared state against the schema.
	pub fn validate(&self) -> Result<Validation, HeadlessError> {
		let binding = self.binding()?;

		Ok(Validation {
			rejected_blocks: binding.rejected_blocks(),
			waiting: binding.is_waiting(),
		})
	}

	/// The y-sync message to send a client when it connects: sync step 1,
	/// which asks for whatever this document is missing.
	pub fn sync_start(&self) -> Result<Vec<u8>, HeadlessError> {
		let state =
			StateVector::decode_v1(&self.binding()?.state_vector()).map_err(CollabError::from)?;
		Ok(Message::Sync(SyncMessage::SyncStep1(state)).encode_v1())
	}

	/// Handle one binary y-sync message (as y-websocket frames them) from a
	/// client connection.
	///
	/// - Sync step 1 is answered with the updates the client lacks.
	/// - Sync step 2 and updates are applied and relayed to the other
	///   connections.
	/// - Awareness (presence) is relayed without being interpreted.
	pub fn receive_sync(&mut self, message: &[u8]) -> Result<SyncReply, HeadlessError> {
		self.receive_sync_changes(message).map(|(sync, _)| sync)
	}

	/// [`Doc::receive_sync`], also returning the commit of every update that
	/// changed the document, in order. Interactive hosts pass each to their
	/// layout (`DocumentLayout::apply`) so only the changed blocks lay out
	/// again.
	pub fn receive_sync_changes(
		&mut self,
		message: &[u8],
	) -> Result<(SyncReply, Vec<Commit>), HeadlessError> {
		let mut sync = SyncReply::default();
		let mut commits = Vec::new();

		for message in decode(message)? {
			self.handle(message, &mut sync, &mut commits)?;
		}

		let repairs = self.flush_sync()?;
		sync.reply.extend(repairs.iter().cloned());
		sync.broadcast.extend(repairs);
		Ok((sync, commits))
	}

	/// The local changes since the last call as y-sync update messages for
	/// every connection. Call it after running commands on a shared
	/// document.
	pub fn flush_sync(&mut self) -> Result<Vec<Vec<u8>>, HeadlessError> {
		Ok(self
			.take_updates()?
			.into_iter()
			.map(|update| Message::Sync(SyncMessage::Update(update)).encode_v1())
			.collect())
	}

	fn handle(
		&mut self,
		message: Message,
		sync: &mut SyncReply,
		commits: &mut Vec<Commit>,
	) -> Result<(), HeadlessError> {
		match message {
			Message::Sync(SyncMessage::SyncStep1(state)) => {
				let diff = self.encode_diff(&state.encode_v1())?;
				sync.reply
					.push(Message::Sync(SyncMessage::SyncStep2(diff)).encode_v1());
			}
			Message::Sync(SyncMessage::SyncStep2(update) | SyncMessage::Update(update)) => {
				if update != EMPTY_UPDATE {
					let applied = self.integrate(&update)?;

					if applied.doc_changed() {
						sync.changed = true;
						commits.push(applied.commit);
					}

					sync.broadcast
						.push(Message::Sync(SyncMessage::Update(update)).encode_v1());
				}
			}
			Message::Awareness(update) => {
				sync.broadcast.push(Message::Awareness(update).encode_v1());
			}
			Message::AwarenessQuery | Message::Auth(_) => {}
			Message::Custom(tag, _) => return Err(CollabError::UnsupportedMessage(tag).into()),
		}

		Ok(())
	}

	/// Mirror a local update into the shared state, applying any follow-up
	/// the binding returns.
	pub(crate) fn mirror_local(&mut self, update: &Update) -> Result<(), HeadlessError> {
		let Some(binding) = self.shared.as_mut() else {
			return Ok(());
		};

		if !update.doc_changed() {
			return Ok(());
		}

		let mut env = canvist_editor::Env::headless(std::time::Duration::ZERO);
		self.editor.apply_external(&mut env, |doc| {
			let follow_up = binding.apply_local(doc, &update.commit, Track::Step)?;
			with_block(follow_up, doc, binding)
		})?;
		Ok(())
	}

	/// Undo or redo this peer's own changes through the shared history,
	/// which never reverts a collaborator's work.
	pub(crate) fn undo_shared(&mut self, redo: bool) -> Result<bool, HeadlessError> {
		let mut env = self.env();
		let binding = self.shared.as_mut().ok_or(HeadlessError::NotShared)?;
		let mut applied = false;

		self.editor.apply_external(&mut env, |doc| {
			let commit = if redo {
				binding.redo(doc)?
			} else {
				binding.undo(doc)?
			};
			applied = commit.is_some();
			let commit = commit.unwrap_or_else(|| doc.transaction().commit());
			with_block(commit, doc, binding)
		})?;

		Ok(applied)
	}

	fn binding(&self) -> Result<&Binding, HeadlessError> {
		self.shared.as_ref().ok_or(HeadlessError::NotShared)
	}

	fn binding_mut(&mut self) -> Result<&mut Binding, HeadlessError> {
		self.shared.as_mut().ok_or(HeadlessError::NotShared)
	}
}

/// Every y-protocols message in `data`.
fn decode(data: &[u8]) -> Result<Vec<Message>, HeadlessError> {
	let mut decoder = DecoderV1::new(Cursor::new(data));
	let messages = MessageReader::new(&mut decoder).collect::<Result<_, _>>();
	Ok(messages.map_err(CollabError::from)?)
}

/// `commit`, followed by the empty paragraph [`ensure_block`] adds when the
/// shared state left the document without blocks.
fn with_block(
	commit: Commit,
	doc: &mut Document,
	binding: &mut Binding,
) -> Result<Commit, HeadlessError> {
	Ok(match ensure_block(doc, binding)? {
		Some(added) => then(commit, added),
		None => commit,
	})
}

/// Give a document the shared state emptied one empty paragraph, mirrored
/// to the shared state without an undo step. The editor would otherwise
/// add it on its own, out of the binding's sight.
fn ensure_block(
	doc: &mut Document,
	binding: &mut Binding,
) -> Result<Option<Commit>, HeadlessError> {
	if !doc.children(None).is_empty() {
		return Ok(None);
	}

	let kind = doc.schema().default_text_block();
	let mut transaction = doc.transaction();
	let id = transaction.allocate_id()?;
	transaction.insert_blocks(None, 0, vec![BlockTree::text(id, kind, InlineText::new())])?;
	let added = transaction.commit();
	let follow_up = binding.apply_local(doc, &added, Track::Untracked)?;

	Ok(Some(then(added, follow_up)))
}

/// Two consecutive commits as one.
fn then(first: Commit, second: Commit) -> Commit {
	let mut touched = first.touched;
	touched.extend(second.touched);
	touched.sort_unstable();
	touched.dedup();

	Commit {
		steps: [first.steps, second.steps].concat(),
		inverses: [first.inverses, second.inverses].concat(),
		maps: [first.maps, second.maps].concat(),
		touched,
		structural: first.structural || second.structural,
		version: second.version.max(first.version),
	}
}
