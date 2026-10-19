//! The binding between one model document and one yrs document.

use std::sync::Arc;
use std::sync::Mutex;

use canvist_model::Commit;
use canvist_model::Document;
use canvist_model::Schema;
use yrs::Doc;
use yrs::OffsetKind;
use yrs::Options;
use yrs::Origin;
use yrs::ReadTxn;
use yrs::StateVector;
use yrs::Transact;
use yrs::UndoManager;
use yrs::Update;
use yrs::block::ClientID;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;

use crate::CollabError;
use crate::keys;
use crate::mirror::LOCAL;
use crate::mirror::Mirror;
use crate::mirror::OBSERVER;
use crate::mirror::REMOTE;
use crate::mirror::UNTRACKED;
use crate::mirror::lock;
use crate::registry::Registry;
use crate::remote::Repair;
use crate::reorder::Reorder;

/// How a local commit enters the shared undo history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Track {
	/// Start a new undo step.
	#[default]
	Step,
	/// Join the previous undo step, e.g. the next keystroke of a word.
	Merge,
	/// Leave undo history alone, e.g. for edits an extension makes on its
	/// own.
	Untracked,
}

/// A Yjs update encoding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
	/// lib0 v1, used by the y-sync protocol, y-websocket, and Hocuspocus.
	#[default]
	V1,
	/// lib0 v2, smaller for documents with long editing histories.
	V2,
}

/// Mirrors one [`Document`] into a yrs document and back.
///
/// Local commits go in through [`Binding::apply_local`]; updates from other
/// peers come in through [`Binding::apply_update`], which returns the model
/// commit that mirrors them. Updates for other peers, including undo and
/// automatic repairs, collect in an outbox drained by
/// [`Binding::take_updates`].
///
/// The binding tracks exactly one document. Changing that document other than
/// through commits passed to [`Binding::apply_local`] makes the two diverge;
/// [`Binding::document`] rebuilds the document from the shared state.
pub struct Binding {
	doc: Doc,
	mirror: Mirror,
	outbox: Arc<Mutex<Vec<Vec<u8>>>>,
	undo: UndoManager,
	reorder: Reorder,
}

impl std::fmt::Debug for Binding {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Binding")
			.field("client_id", &self.doc.client_id())
			.field("mirror", &self.mirror)
			.finish_non_exhaustive()
	}
}

impl Binding {
	/// A binding over an empty shared document with a random yrs client id.
	pub fn new(schema: Arc<Schema>) -> Result<Self, CollabError> {
		Self::with_client_id(schema, ClientID::random().get())
	}

	/// A binding over an empty shared document with the given yrs client id,
	/// which must be unique among peers and never reused with lost state.
	pub fn with_client_id(schema: Arc<Schema>, client_id: u64) -> Result<Self, CollabError> {
		let doc = shared_doc(client_id);
		let root = doc.get_or_insert_array(keys::ROOT);
		let outbox = observe_updates(&doc)?;
		let undo = undo_manager(&doc, &root);

		Ok(Self {
			mirror: Mirror::new(root, schema),
			doc,
			outbox,
			undo,
			reorder: Reorder::default(),
		})
	}

	/// The schema documents of this binding use.
	pub fn schema(&self) -> &Arc<Schema> {
		self.mirror.schema()
	}

	/// The underlying yrs document, for awareness and custom transports.
	pub fn yrs(&self) -> &Doc {
		&self.doc
	}

	/// Write every block of `doc` into the empty shared document, making
	/// `doc` the tracked document. The first peer of a new document calls
	/// this; peers that join call [`Binding::document`] after syncing.
	pub fn seed(&mut self, doc: &Document) -> Result<(), CollabError> {
		let mut txn = self.doc.transact_mut_with(UNTRACKED);
		self.mirror.seed(&mut txn, doc)
	}

	/// Build a document from the shared state, making it the tracked document.
	/// `client` seeds block id allocation as in [`Document::new`].
	pub fn document(&mut self, client: u32) -> Result<Document, CollabError> {
		let txn = self.doc.transact();
		let (doc, repairs) = self.mirror.document(&txn, client)?;
		drop(txn);
		self.repair(&repairs);
		Ok(doc)
	}

	/// Mirror a local commit of the tracked document, which `doc` must be
	/// in its state right after `commit`.
	///
	/// Returns a follow-up commit, usually empty: retyping a container can
	/// make children visible that the shared document holds but the old
	/// type did not allow (see [the crate docs](crate#the-yjs-schema)).
	/// Hosts apply it like a remote change.
	pub fn apply_local(
		&mut self,
		doc: &mut Document,
		commit: &Commit,
		track: Track,
	) -> Result<Commit, CollabError> {
		if commit.is_empty() {
			return Ok(doc.transaction().commit());
		}

		let origin = match track {
			Track::Step => {
				self.undo.reset();
				LOCAL
			}
			Track::Merge => LOCAL,
			Track::Untracked => UNTRACKED,
		};
		let mut txn = self.doc.transact_mut_with(origin);
		let retyped = self.mirror.translate(&mut txn, doc, commit)?;
		drop(txn);

		if retyped.is_empty() {
			return Ok(doc.transaction().commit());
		}

		let txn = self.doc.transact();
		let (follow_up, repairs) = self.mirror.reveal(&txn, doc, &retyped)?;
		drop(txn);
		self.repair(&repairs);
		Ok(follow_up)
	}

	/// Integrate an update from another peer and mirror it into `doc`,
	/// returning the commit that did so.
	///
	/// Updates may arrive in any order and more than once. An update that
	/// builds on changes not yet received waits inside the binding and is
	/// applied, with an empty commit returned for now, once they arrive.
	///
	/// On error the binding may no longer match `doc`; rebuild it with
	/// [`Binding::document`].
	pub fn apply_update(
		&mut self,
		doc: &mut Document,
		update: &[u8],
		encoding: Encoding,
	) -> Result<Commit, CollabError> {
		self.reorder.hold(decode_update(update, encoding)?);

		if !integrate_ready(&self.doc, &mut self.reorder)? {
			return Ok(doc.transaction().commit());
		}

		self.mirror(doc)
	}

	/// Whether updates are waiting for earlier updates that have not arrived.
	/// Syncing with any peer that has them, as the y-sync handshake does,
	/// fills the gap.
	pub fn is_waiting(&self) -> bool {
		self.reorder.is_waiting()
	}

	/// How many items in the shared document's block arrays the model hides
	/// because they break the schema: an unknown type, an id that does not parse, the
	/// wrong content for their type, or a parent that does not allow them
	/// (see [the crate docs](crate#the-yjs-schema)). Blocks inside a hidden
	/// block are not counted, nor is inline content shown as U+FFFD.
	///
	/// A server checks this after merging updates to find peers writing
	/// content its schema does not know.
	pub fn rejected_blocks(&self) -> u32 {
		self.mirror.rejected_blocks()
	}

	/// Translate the recorded yrs events into one model commit.
	pub(crate) fn mirror(&mut self, doc: &mut Document) -> Result<Commit, CollabError> {
		let txn = self.doc.transact();
		let (commit, repairs) = self.mirror.mirror(&txn, doc)?;
		drop(txn);
		self.repair(&repairs);
		Ok(commit)
	}

	/// Delete block maps that lost their block id to another map.
	fn repair(&mut self, repairs: &[Repair]) {
		if repairs.is_empty() {
			return;
		}

		let mut txn = self.doc.transact_mut_with(UNTRACKED);
		self.mirror.repair(&mut txn, repairs);
	}

	/// The state vector of the shared document, v1 encoded, for the first
	/// step of a sync.
	pub fn state_vector(&self) -> Vec<u8> {
		self.doc.transact().state_vector().encode_v1()
	}

	/// Everything the peer whose v1 state vector is `state_vector` is missing.
	pub fn encode_diff(
		&self,
		state_vector: &[u8],
		encoding: Encoding,
	) -> Result<Vec<u8>, CollabError> {
		let state_vector = StateVector::decode_v1(state_vector)?;
		Ok(self.encode_diff_from(&state_vector, encoding))
	}

	/// Everything a peer at `state_vector` is missing.
	pub(crate) fn encode_diff_from(
		&self,
		state_vector: &StateVector,
		encoding: Encoding,
	) -> Vec<u8> {
		encode_diff(&self.doc, state_vector, encoding)
	}

	/// The whole shared document as one update.
	pub fn encode_state(&self, encoding: Encoding) -> Vec<u8> {
		encode_diff(&self.doc, &StateVector::default(), encoding)
	}

	/// Drain the v1 updates this peer produced (local commits, undo, redo,
	/// and repairs) for broadcasting.
	pub fn take_updates(&mut self) -> Vec<Vec<u8>> {
		std::mem::take(&mut *lock(&self.outbox))
	}

	/// Undo the latest local undo step, returning the commit that mirrors it.
	pub fn undo(&mut self, doc: &mut Document) -> Result<Option<Commit>, CollabError> {
		if !self.undo.undo_blocking() {
			return Ok(None);
		}

		self.mirror(doc).map(Some)
	}

	/// Redo the latest undone step, returning the commit that mirrors it.
	pub fn redo(&mut self, doc: &mut Document) -> Result<Option<Commit>, CollabError> {
		if !self.undo.redo_blocking() {
			return Ok(None);
		}

		self.mirror(doc).map(Some)
	}

	/// Whether there is a local step to undo.
	pub fn can_undo(&self) -> bool {
		self.undo.can_undo()
	}

	/// Whether there is an undone step to redo.
	pub fn can_redo(&self) -> bool {
		self.undo.can_redo()
	}

	pub(crate) fn registry(&self) -> &Registry {
		self.mirror.registry()
	}
}

impl Drop for Binding {
	fn drop(&mut self) {
		// The observer only touches state this binding owns. Unsubscribing can
		// only fail while a transaction is open, which no `&mut self` method
		// leaves behind, so there is nothing to report here.
		let _ = self.doc.unobserve_update_v1(OBSERVER);
	}
}

/// A yrs document with the options every canvist binding uses.
pub(crate) fn shared_doc(client_id: u64) -> Doc {
	let options = Options {
		offset_kind: OffsetKind::Utf16,
		cleanup_formatting: false,
		..Options::with_client_id(ClientID::new(client_id))
	};
	Doc::with_options(options)
}

/// Collect every update this peer produces, except integrated remote ones.
pub(crate) fn observe_updates(doc: &Doc) -> Result<Arc<Mutex<Vec<Vec<u8>>>>, CollabError> {
	let outbox = Arc::new(Mutex::new(Vec::new()));
	let sink = outbox.clone();
	doc.observe_update_v1(OBSERVER, move |txn, event| {
		if txn.origin() != Some(&Origin::from(REMOTE)) {
			lock(&sink).push(event.update.clone());
		}
	})
	.map_err(|_| CollabError::Busy)?;
	Ok(outbox)
}

/// An undo manager over `scope` that tracks only local commits.
pub(crate) fn undo_manager<S: AsRef<yrs::branch::Branch>>(doc: &Doc, scope: &S) -> UndoManager {
	let options = yrs::undo::Options {
		// Steps group until `Track::Step` resets the manager, so time plays
		// no part: the clock is constant and the timeout never expires.
		capture_timeout_millis: u64::MAX,
		tracked_origins: [Origin::from(LOCAL)].into_iter().collect(),
		capture_transaction: None,
		timestamp: Arc::new(|| 1),
		init_undo_stack: Vec::new(),
		init_redo_stack: Vec::new(),
	};
	let mut undo = UndoManager::with_options(options);
	undo.expand_scope(doc, scope);
	undo
}

pub(crate) fn decode_update(update: &[u8], encoding: Encoding) -> Result<Update, CollabError> {
	Ok(match encoding {
		Encoding::V1 => Update::decode_v1(update)?,
		Encoding::V2 => Update::decode_v2(update)?,
	})
}

/// Integrate every held update whose dependencies are present, in one yrs
/// transaction. Returns whether any were.
pub(crate) fn integrate_ready(doc: &Doc, reorder: &mut Reorder) -> Result<bool, CollabError> {
	let mut txn = doc.transact_mut_with(REMOTE);
	let mut integrated = false;

	// One at a time against what the transaction has integrated so far, so
	// an update yrs keeps pending cannot let a later one overtake it.
	while let Some(update) = reorder.take_next(&txn.state_vector()) {
		txn.apply_update(update)?;
		integrated = true;
	}

	Ok(integrated)
}

pub(crate) fn encode_diff(doc: &Doc, state_vector: &StateVector, encoding: Encoding) -> Vec<u8> {
	let txn = doc.transact();

	match encoding {
		Encoding::V1 => txn.encode_state_as_update_v1(state_vector),
		Encoding::V2 => txn.encode_state_as_update_v2(state_vector),
	}
}

/// Concatenate two commits made one after the other on `doc`.
pub(crate) fn compose(mut first: Commit, second: Commit, doc: &Document) -> Commit {
	first.steps.extend(second.steps);
	first.inverses.extend(second.inverses);
	first.maps.extend(second.maps);
	first.touched.extend(second.touched);
	first.touched.retain(|&id| doc.contains(id));
	first.touched.sort_unstable();
	first.touched.dedup();
	first.structural |= second.structural;

	first.version = second.version;
	first
}
