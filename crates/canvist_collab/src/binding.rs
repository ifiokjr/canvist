//! The binding between one model document and one yrs document.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use canvist_model::BlockId;
use canvist_model::Commit;
use canvist_model::Document;
use canvist_model::Schema;
use canvist_model::Step;
use yrs::Array;
use yrs::ArrayRef;
use yrs::DeepObservable;
use yrs::Doc;
use yrs::OffsetKind;
use yrs::Options;
use yrs::Origin;
use yrs::ReadTxn;
use yrs::StateVector;
use yrs::Transact;
use yrs::TransactionMut;
use yrs::UndoManager;
use yrs::Update;
use yrs::block::ClientID;
use yrs::types::Events;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;

use crate::CollabError;
use crate::claims::Owners;
use crate::keys;
use crate::local::Local;
use crate::local::position_of;
use crate::registry::Registration;
use crate::registry::Registry;
use crate::remote::Remote;
use crate::remote::Repair;
use crate::remote::record;
use crate::remote::record::Record;
use crate::reorder::Reorder;
use crate::tree;
use crate::tree::Parent;
use crate::tree::Reader;

/// Transactions created by [`Binding::apply_local`] with undo tracking.
const LOCAL: &str = "canvist:local";
/// Local transactions undo ignores: untracked edits and duplicate repairs.
const UNTRACKED: &str = "canvist:untracked";
/// Transactions that integrate updates from other peers.
const REMOTE: &str = "canvist:remote";
/// The key of the binding's own yrs observers.
const OBSERVER: &str = "canvist:binding";

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
	root: ArrayRef,
	schema: Arc<Schema>,
	registry: Registry,
	inbox: Arc<Mutex<Vec<Record>>>,
	outbox: Arc<Mutex<Vec<Vec<u8>>>>,
	undo: UndoManager,
	reorder: Reorder,
}

impl std::fmt::Debug for Binding {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Binding")
			.field("client_id", &self.doc.client_id())
			.field("registry", &self.registry)
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
		let options = Options {
			offset_kind: OffsetKind::Utf16,
			cleanup_formatting: false,
			..Options::with_client_id(ClientID::new(client_id))
		};
		let doc = Doc::with_options(options);
		let root = doc.get_or_insert_array(keys::ROOT);
		let inbox = Arc::new(Mutex::new(Vec::new()));
		let outbox = Arc::new(Mutex::new(Vec::new()));
		observe(&doc, &root, &schema, &inbox, &outbox)?;
		let undo = undo_manager(&doc, &root);

		Ok(Self {
			doc,
			root,
			schema,
			registry: Registry::default(),
			inbox,
			outbox,
			undo,
			reorder: Reorder::default(),
		})
	}

	/// The schema documents of this binding use.
	pub fn schema(&self) -> &Arc<Schema> {
		&self.schema
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

		if self.root.len(&txn) > 0 {
			return Err(CollabError::NotEmpty);
		}

		let mut registration = Registration::default();

		// Inserting at the front never walks the array, so writing the blocks
		// last to first keeps seeding linear in the document's size.
		for &block in doc.children(None).iter().rev() {
			let tree = doc.tree(block).ok_or(CollabError::NotShared(block))?;
			tree::write(
				&mut txn,
				&self.schema,
				(&self.root, 0, None),
				&tree,
				&mut registration,
			)?;
		}

		self.registry = Registry::default();
		self.registry.register(registration);
		Ok(())
	}

	/// Build a document from the shared state, making it the tracked document.
	/// `client` seeds block id allocation as in [`Document::new`].
	pub fn document(&mut self, client: u32) -> Result<Document, CollabError> {
		self.inbox().clear();
		let mut doc = Document::empty(self.schema.clone(), client);
		let txn = self.doc.transact();
		let mut owners = Owners::default();
		owners.scan_array(&self.root, &txn);
		let mut registration = Registration::default();
		let mut reader = Reader {
			txn: &txn,
			schema: &self.schema,
			claims: &mut owners,
		};
		let blocks = reader.children(&self.root, Parent::ROOT, &mut registration);

		drop(txn);

		doc.apply_steps([Step::InsertBlocks {
			parent: None,
			index: 0,
			blocks,
		}])?;
		let repairs = repairs_of(&registration);
		self.registry = Registry::default();
		self.registry.register(registration);
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
		let retyped = self.translate_local(doc, commit, origin)?;

		if retyped.is_empty() {
			return Ok(doc.transaction().commit());
		}

		self.reveal(doc, &retyped)
	}

	/// Translate `commit` in one yrs transaction tagged `origin`, returning
	/// the containers it retyped while they held hidden children.
	fn translate_local(
		&mut self,
		doc: &Document,
		commit: &Commit,
		origin: &str,
	) -> Result<Vec<BlockId>, CollabError> {
		let structural = commit
			.steps
			.iter()
			.filter(|step| is_structural(step))
			.count();
		let mut txn = self.doc.transact_mut_with(origin);
		let mut local = Local {
			txn: &mut txn,
			root: &self.root,
			schema: &self.schema,
			registry: &mut self.registry,
			doc,
			single_structural: structural <= 1,
			settled: crate::local::settled(&commit.steps),
			retyped: Vec::new(),
		};

		for (step, inverse) in commit.steps.iter().zip(&commit.inverses) {
			local.step(step, inverse)?;
		}

		Ok(local.retyped)
	}

	/// Show hidden children that the new types of `retyped` allow.
	fn reveal(&mut self, doc: &mut Document, retyped: &[BlockId]) -> Result<Commit, CollabError> {
		let txn = self.doc.transact();
		let remote = Remote::new(
			doc.transaction(),
			&txn,
			(&self.root, &self.schema),
			&mut self.registry,
		);
		let (follow_up, repairs) = remote.reveal(retyped)?;
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
		let update = match encoding {
			Encoding::V1 => Update::decode_v1(update)?,
			Encoding::V2 => Update::decode_v2(update)?,
		};

		self.reorder.hold(update);
		let mut combined: Option<Commit> = None;

		loop {
			let state = self.doc.transact().state_vector();
			let ready = self.reorder.take_ready(&state);

			if ready.is_empty() {
				break;
			}

			self.integrate(ready)?;
			let commit = self.mirror(doc)?;
			combined = Some(match combined {
				Some(first) => compose(first, commit, doc),
				None => commit,
			});
		}

		Ok(combined.unwrap_or_else(|| doc.transaction().commit()))
	}

	/// Integrate updates whose dependencies are all present, in one yrs
	/// transaction.
	fn integrate(&self, ready: Vec<Update>) -> Result<(), CollabError> {
		let mut txn = self.doc.transact_mut_with(REMOTE);

		for update in ready {
			txn.apply_update(update)?;
		}

		Ok(())
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
		self.registry.unmirrored_total()
	}

	/// Translate the recorded yrs events into one model commit.
	pub(crate) fn mirror(&mut self, doc: &mut Document) -> Result<Commit, CollabError> {
		let records = std::mem::take(&mut *self.inbox());
		let txn = self.doc.transact();
		let remote = Remote::new(
			doc.transaction(),
			&txn,
			(&self.root, &self.schema),
			&mut self.registry,
		);
		let (commit, repairs) = remote.apply(records)?;
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

		for repair in repairs {
			let array = match repair.parent {
				None => Some(self.root.clone()),
				Some(parent) => {
					self.registry
						.block(parent)
						.and_then(|found| crate::local::map_ref(&txn, &found.map))
						.and_then(|map| tree::children_of(&map, &txn))
				}
			};

			if let Some(array) = array
				&& let Some(index) = position_of(&array, &txn, &repair.map)
			{
				array.remove(&mut txn, index);
			}

			self.registry.remove_hidden(&repair.map);
		}
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
		let txn = self.doc.transact();

		match encoding {
			Encoding::V1 => txn.encode_state_as_update_v1(state_vector),
			Encoding::V2 => txn.encode_state_as_update_v2(state_vector),
		}
	}

	/// The whole shared document as one update.
	pub fn encode_state(&self, encoding: Encoding) -> Vec<u8> {
		let txn = self.doc.transact();
		let empty = StateVector::default();

		match encoding {
			Encoding::V1 => txn.encode_state_as_update_v1(&empty),
			Encoding::V2 => txn.encode_state_as_update_v2(&empty),
		}
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
		&self.registry
	}

	fn inbox(&self) -> std::sync::MutexGuard<'_, Vec<Record>> {
		lock(&self.inbox)
	}
}

impl Drop for Binding {
	fn drop(&mut self) {
		// The observers only touch state this binding owns. Unsubscribing can
		// only fail while a transaction is open, which no `&mut self` method
		// leaves behind, so there is nothing to report here.
		let _ = self.doc.unobserve_update_v1(OBSERVER);
		self.root.unobserve_deep(OBSERVER);
	}
}

/// Subscribe the inbox to remote changes and the outbox to local updates.
fn observe(
	doc: &Doc,
	root: &ArrayRef,
	schema: &Arc<Schema>,
	inbox: &Arc<Mutex<Vec<Record>>>,
	outbox: &Arc<Mutex<Vec<Vec<u8>>>>,
) -> Result<(), CollabError> {
	let schema = schema.clone();
	let inbox = inbox.clone();
	root.observe_deep(
		OBSERVER,
		move |txn: &TransactionMut<'_>, events: &Events<'_>| {
			if is_local(txn) {
				return;
			}

			let records = events
				.iter()
				.filter_map(|event| record::record(txn, event, &schema));
			lock(&inbox).extend(records);
		},
	);

	let outbox = outbox.clone();
	doc.observe_update_v1(OBSERVER, move |txn, event| {
		if txn.origin() != Some(&Origin::from(REMOTE)) {
			lock(&outbox).push(event.update.clone());
		}
	})
	.map_err(|_| CollabError::Busy)
}

/// Whether `txn` was made by this binding from local commits.
fn is_local(txn: &TransactionMut<'_>) -> bool {
	txn.origin()
		.is_some_and(|origin| *origin == Origin::from(LOCAL) || *origin == Origin::from(UNTRACKED))
}

fn undo_manager(doc: &Doc, root: &ArrayRef) -> UndoManager {
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
	undo.expand_scope(doc, root);
	undo
}

fn is_structural(step: &Step) -> bool {
	matches!(
		step,
		Step::Split { .. }
			| Step::Join { .. }
			| Step::InsertBlocks { .. }
			| Step::RemoveBlocks { .. }
			| Step::MoveBlocks { .. }
	)
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

fn repairs_of(registration: &Registration) -> Vec<Repair> {
	registration
		.losers
		.iter()
		.map(|&(map, parent)| Repair { parent, map })
		.collect()
}

/// Lock a mutex shared with a yrs observer. A panic inside an observer
/// leaves the data intact (it is only ever appended to), so poisoning is
/// ignored.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
	mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
