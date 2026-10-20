//! Collaboration for [`canvist_scene`] scenes: a step-to-yrs binding with
//! the same guarantees as the document binding.
//!
//! # The Yjs shape
//!
//! A scene is one Yjs document:
//!
//! - The root `Y.Map` named `"nodes"` ([`keys::NODES`]) maps each node id
//!   (`client.seq` in base 36) to a node `Y.Map`. The tree is flat: each
//!   node names its parent, so moving a node never copies it, and edits a
//!   collaborator makes inside a moved node survive.
//! - A node map holds:
//!   - `"type"`: the node type name, such as `"rectangle"`.
//!   - `"pos"`: `[parent, key, clock, client, previousParent]`, one atomic
//!     value, so a reparent never mixes one peer's parent with another's
//!     key. `parent` is a node id or `"root"`, `key` a fractional key, and
//!     `clock`, `client`, and `previousParent` decide which move loses when
//!     concurrent moves form a cycle.
//!   - `"props"`: a `Y.Map` from property name to its JSON value
//!     ([`canvist_scene`'s property names](canvist_scene#properties)). Each
//!     key is a last-writer-wins register, so concurrent edits to different
//!     properties of one node all survive.
//!   - `"text"`, for text frames: a `Y.Array` of block maps in exactly the
//!     shape of the [document binding](crate#the-yjs-schema). Text frames
//!     live inside the scene's document, so creating a frame and its first
//!     paragraph is one atomic change, and one undo manager covers shapes
//!     and the text inside them.
//!
//! # Concurrency
//!
//! Which nodes are shown, and where, is a deterministic function of the
//! shared state, so every peer converges on the same scene:
//!
//! - A node whose parent was deleted concurrently is hidden but kept; it
//!   reappears if the parent comes back, for example through undo.
//! - Nodes with an unknown type, a malformed position, or a parent whose
//!   type does not accept them are hidden the same way.
//! - When concurrent moves form a cycle, the member whose position has the
//!   greatest `(clock, client, id)` falls back to its previous parent, or to
//!   the first page if that still cycles: the later move loses.
//! - Property values this schema rejects are ignored and left in place.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//!
//! use canvist_collab::Encoding;
//! use canvist_collab::SceneBinding;
//! use canvist_collab::Track;
//! use canvist_model::BlockSpec;
//! use canvist_model::Schema;
//! use canvist_scene::Prop;
//! use canvist_scene::Props;
//! use canvist_scene::Scene;
//! use canvist_scene::SceneSchema;
//!
//! let text = Arc::new(Schema::builder().block(BlockSpec::text("paragraph")).build()?);
//! let schema = Arc::new(SceneSchema::standard(text)?);
//!
//! // Ada seeds the shared scene; Grace joins from Ada's state.
//! let mut ada_scene = Scene::new(schema.clone(), 1)?;
//! let mut ada = SceneBinding::with_client_id(schema.clone(), 1)?;
//! ada.seed(&ada_scene)?;
//! let mut grace = SceneBinding::with_client_id(schema.clone(), 2)?;
//! grace.apply_state(&ada.encode_state(Encoding::V1), Encoding::V1)?;
//! let mut grace_scene = grace.scene(2)?;
//!
//! // Ada draws a rectangle; Grace receives an ordinary scene commit.
//! let page = ada_scene.pages()[0];
//! let mut transaction = ada_scene.transaction();
//! let rect = transaction.insert(page, None, "rectangle", Props::default())?;
//! transaction.set(rect, Prop::Opacity(0.5))?;
//! let commit = transaction.commit();
//! ada.apply_local(&mut ada_scene, &commit, Track::Step)?;
//!
//! for update in ada.take_updates() {
//! 	grace.apply_update(&mut grace_scene, &update, Encoding::V1, None)?;
//! }
//!
//! assert_eq!(grace_scene.to_json(), ada_scene.to_json());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod codec;
mod index;
pub mod keys;
mod local;
mod remote;

use std::sync::Arc;
use std::sync::Mutex;

use canvist_model::BlockId;
use canvist_model::Commit;
use canvist_model::Document;
use canvist_scene::NodeId;
use canvist_scene::NodeRecord;
use canvist_scene::Scene;
use canvist_scene::SceneCommit;
use canvist_scene::SceneSchema;
use canvist_scene::Step;
use rustc_hash::FxHashMap;
use yrs::DeepObservable;
use yrs::Doc;
use yrs::Map;
use yrs::MapRef;
use yrs::ReadTxn;
use yrs::StateVector;
use yrs::Transact;
use yrs::TransactionMut;
use yrs::UndoManager;
use yrs::block::ClientID;
use yrs::types::Event;
use yrs::types::Events;
use yrs::types::PathSegment;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;

use self::index::Index;
use self::local::TextWork;
use self::local::Writer;
use self::remote::Dirty;
use self::remote::Reconciler;
use self::remote::TextFrame;
use crate::CollabError;
use crate::Encoding;
use crate::Track;
use crate::binding::decode_update;
use crate::binding::encode_diff;
use crate::binding::integrate_ready;
use crate::binding::observe_updates;
use crate::binding::shared_doc;
use crate::binding::undo_manager;
use crate::mirror::LOCAL;
use crate::mirror::Mirror;
use crate::mirror::OBSERVER;
use crate::mirror::UNTRACKED;
use crate::mirror::is_local;
use crate::mirror::lock;
use crate::reorder::Reorder;

/// A text frame whose document an editor holds while a collaborator's
/// update arrives (see [`Scene::lend_text`]). Hosts implement it over their
/// editor, typically with `Editor::apply_external`.
pub trait LentText {
	/// The text frame.
	fn node(&self) -> NodeId;

	/// Let `change` edit the frame's document as a collaborator's change.
	/// `change` returns the commit of exactly the steps it applied.
	fn apply(
		&mut self,
		change: &mut dyn FnMut(&mut Document) -> Result<Commit, CollabError>,
	) -> Result<Commit, CollabError>;
}

/// A lent document held directly, for hosts without an editor in between.
#[derive(Debug)]
pub struct LentDocument<'a> {
	/// The text frame.
	pub node: NodeId,
	/// Its document.
	pub doc: &'a mut Document,
}

impl LentText for LentDocument<'_> {
	fn node(&self) -> NodeId {
		self.node
	}

	fn apply(
		&mut self,
		change: &mut dyn FnMut(&mut Document) -> Result<Commit, CollabError>,
	) -> Result<Commit, CollabError> {
		change(self.doc)
	}
}

/// Mirrors one [`Scene`] into a yrs document and back.
///
/// It works like [`Binding`](crate::Binding): local commits go in through
/// [`SceneBinding::apply_local`], updates from peers come back from
/// [`SceneBinding::apply_update`] as ordinary scene commits, and updates for
/// peers collect in an outbox drained by [`SceneBinding::take_updates`].
/// Undo and redo only revert this peer's commits.
pub struct SceneBinding {
	doc: Doc,
	nodes: MapRef,
	schema: Arc<SceneSchema>,
	index: Index,
	texts: FxHashMap<NodeId, TextFrame>,
	dirty: Arc<Mutex<Dirty>>,
	outbox: Arc<Mutex<Vec<Vec<u8>>>>,
	undo: UndoManager,
	reorder: Reorder,
	clock: u64,
}

impl std::fmt::Debug for SceneBinding {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SceneBinding")
			.field("client_id", &self.doc.client_id())
			.field("nodes", &self.index.len())
			.field("texts", &self.texts.len())
			.finish_non_exhaustive()
	}
}

impl SceneBinding {
	/// A binding over an empty shared scene with a random yrs client id.
	pub fn new(schema: Arc<SceneSchema>) -> Result<Self, CollabError> {
		Self::with_client_id(schema, ClientID::random().get())
	}

	/// A binding over an empty shared scene with the given yrs client id,
	/// which must be unique among peers and never reused with lost state.
	pub fn with_client_id(schema: Arc<SceneSchema>, client_id: u64) -> Result<Self, CollabError> {
		// yrs 0.28 frees deleted nested types even when the undo manager still
		// references content inside them (it only keeps what tracked
		// transactions deleted), so undoing after a peer deletes a node
		// reads freed memory. Keeping deleted content avoids it; see the
		// canvas architecture's recorded tradeoffs.
		let doc = shared_doc(client_id, false);
		let nodes = doc.get_or_insert_map(keys::NODES);
		let outbox = observe_updates(&doc)?;
		let dirty = Arc::new(Mutex::new(Dirty::default()));
		observe_nodes(&nodes, &dirty, schema.text().cloned());
		let undo = undo_manager(&doc, &nodes);

		Ok(Self {
			doc,
			nodes,
			schema,
			index: Index::default(),
			texts: FxHashMap::default(),
			dirty,
			outbox,
			undo,
			reorder: Reorder::default(),
			clock: 0,
		})
	}

	/// The scene schema.
	pub fn schema(&self) -> &Arc<SceneSchema> {
		&self.schema
	}

	/// The underlying yrs document, for awareness and custom transports.
	pub fn yrs(&self) -> &Doc {
		&self.doc
	}

	/// Write every node of `scene` into the empty shared document. The first
	/// peer of a new scene calls this; peers that join call
	/// [`SceneBinding::scene`] after syncing.
	pub fn seed(&mut self, scene: &Scene) -> Result<(), CollabError> {
		if self.nodes.len(&self.doc.transact()) > 0 {
			return Err(CollabError::NotEmpty);
		}

		let records = scene
			.subtree(NodeId::ROOT)
			.into_iter()
			.filter_map(|id| record_of(scene, id))
			.collect();
		let commit = SceneCommit {
			steps: vec![Step::InsertNodes { nodes: records }],
			inverses: vec![Step::RemoveNodes { ids: Vec::new() }],
			touched: Vec::new(),
			structural: true,
			version: scene.version(),
		};
		let mut txn = self.doc.transact_mut_with(UNTRACKED);
		let work = Writer {
			txn: &mut txn,
			nodes: &self.nodes,
			scene,
			index: &mut self.index,
			clock: &mut self.clock,
		}
		.commit(&commit)?;
		finish_text(
			&mut txn,
			(&self.nodes, &self.schema),
			&mut self.texts,
			scene,
			&work,
		)?;
		drop(txn);
		Ok(())
	}

	/// Integrate a full state update before building a scene with
	/// [`SceneBinding::scene`], as a joining peer does.
	pub fn apply_state(&mut self, update: &[u8], encoding: Encoding) -> Result<(), CollabError> {
		self.reorder.hold(decode_update(update, encoding)?);
		integrate_ready(&self.doc, &mut self.reorder)?;
		*lock(&self.dirty) = Dirty::default();
		Ok(())
	}

	/// Build a scene from the shared state. `client` seeds node id allocation
	/// as in [`Scene::empty`].
	pub fn scene(&mut self, client: u32) -> Result<Scene, CollabError> {
		*lock(&self.dirty) = Dirty::default();
		self.texts.clear();
		self.index = Index::default();
		let mut scene = Scene::empty(self.schema.clone(), client);
		let txn = self.doc.transact();
		let ids: Vec<NodeId> = self
			.nodes
			.keys(&txn)
			.filter_map(|key| key.parse().ok())
			.collect();
		let mut reconciler = Reconciler {
			txn: &txn,
			nodes: &self.nodes,
			schema: &self.schema,
			index: &mut self.index,
			texts: &mut self.texts,
			clock: &mut self.clock,
			repairs: Vec::new(),
		};

		for &id in &ids {
			let entry = reconciler.read_entry(id);
			reconciler.index.set(id, entry);
		}

		let dirty = Dirty {
			nodes: ids.into_iter().collect(),
			..Dirty::default()
		};
		let mut transaction = scene.transaction();
		reconciler.run(&mut transaction, dirty, None)?;
		let repairs = std::mem::take(&mut reconciler.repairs);
		let _ = transaction.commit();
		drop(txn);
		self.repair(&repairs);
		Ok(scene)
	}

	/// Mirror a local commit of `scene`, which must be in its state right
	/// after `commit`. Returns a follow-up commit, usually empty, as
	/// [`Binding::apply_local`](crate::Binding::apply_local) does.
	pub fn apply_local(
		&mut self,
		scene: &mut Scene,
		commit: &SceneCommit,
		track: Track,
	) -> Result<SceneCommit, CollabError> {
		if commit.is_empty() {
			return Ok(scene.transaction().commit());
		}

		let origin = self.origin(track);
		let mut txn = self.doc.transact_mut_with(origin);
		let work = Writer {
			txn: &mut txn,
			nodes: &self.nodes,
			scene,
			index: &mut self.index,
			clock: &mut self.clock,
		}
		.commit(commit)?;
		let retyped = finish_text(
			&mut txn,
			(&self.nodes, &self.schema),
			&mut self.texts,
			scene,
			&work,
		)?;
		drop(txn);
		self.reveal(scene, retyped)
	}

	/// Mirror a local commit an editor made to a lent text frame's document
	/// (see [`Scene::lend_text`]), which `doc` must be in its state right
	/// after `commit`. Returns a follow-up commit for the editor, usually
	/// empty.
	pub fn apply_local_text(
		&mut self,
		node: NodeId,
		doc: &mut Document,
		commit: &Commit,
		track: Track,
	) -> Result<Commit, CollabError> {
		if commit.is_empty() {
			return Ok(doc.transaction().commit());
		}

		let origin = self.origin(track);
		let frame = self
			.texts
			.get_mut(&node)
			.ok_or(CollabError::SceneNode(node))?;
		let mut txn = self.doc.transact_mut_with(origin);
		let array = text_array(&self.nodes, &txn, node)?;
		let retyped = frame.mirror.translate(&mut txn, &array, doc, commit)?;
		drop(txn);

		if retyped.is_empty() {
			return Ok(doc.transaction().commit());
		}

		let txn = self.doc.transact();
		let array = text_array(&self.nodes, &txn, node)?;
		let (follow_up, repairs) = frame.mirror.reveal(&txn, &array, doc, &retyped)?;
		drop(txn);
		self.repair(&[(node, repairs)]);
		Ok(follow_up)
	}

	/// Integrate an update from another peer and mirror it into `scene`,
	/// returning the commit that did so. `lent` is the text frame an editor
	/// holds, if any: its text changes go to the editor instead of the
	/// returned commit.
	///
	/// Updates may arrive in any order and more than once; an update that
	/// builds on changes not yet received waits inside the binding.
	pub fn apply_update(
		&mut self,
		scene: &mut Scene,
		update: &[u8],
		encoding: Encoding,
		lent: Option<&mut dyn LentText>,
	) -> Result<SceneCommit, CollabError> {
		self.reorder.hold(decode_update(update, encoding)?);

		if !integrate_ready(&self.doc, &mut self.reorder)? {
			return Ok(scene.transaction().commit());
		}

		self.reconcile(scene, lent)
	}

	/// Undo this peer's latest undo step, returning the commit that mirrors
	/// it.
	pub fn undo(
		&mut self,
		scene: &mut Scene,
		lent: Option<&mut dyn LentText>,
	) -> Result<Option<SceneCommit>, CollabError> {
		if !self.undo.undo_blocking() {
			return Ok(None);
		}

		self.reconcile(scene, lent).map(Some)
	}

	/// Redo this peer's latest undone step, returning the commit that
	/// mirrors it.
	pub fn redo(
		&mut self,
		scene: &mut Scene,
		lent: Option<&mut dyn LentText>,
	) -> Result<Option<SceneCommit>, CollabError> {
		if !self.undo.redo_blocking() {
			return Ok(None);
		}

		self.reconcile(scene, lent).map(Some)
	}

	/// Whether there is a local step to undo.
	pub fn can_undo(&self) -> bool {
		self.undo.can_undo()
	}

	/// Whether there is an undone step to redo.
	pub fn can_redo(&self) -> bool {
		self.undo.can_redo()
	}

	/// Whether updates are waiting for earlier updates that have not arrived.
	pub fn is_waiting(&self) -> bool {
		self.reorder.is_waiting()
	}

	/// How many node maps the shared document holds that `scene` does not
	/// show: orphans, unknown types, and nodes their parent rejects.
	pub fn hidden_nodes(&self, scene: &Scene) -> usize {
		self.index.len().saturating_sub(scene.len())
	}

	/// The state vector of the shared document, v1 encoded.
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
		Ok(encode_diff(&self.doc, &state_vector, encoding))
	}

	/// The whole shared document as one update.
	pub fn encode_state(&self, encoding: Encoding) -> Vec<u8> {
		encode_diff(&self.doc, &StateVector::default(), encoding)
	}

	/// Drain the v1 updates this peer produced for broadcasting.
	pub fn take_updates(&mut self) -> Vec<Vec<u8>> {
		std::mem::take(&mut *lock(&self.outbox))
	}

	fn origin(&mut self, track: Track) -> &'static str {
		match track {
			Track::Step => {
				self.undo.reset();
				LOCAL
			}
			Track::Merge => LOCAL,
			Track::Untracked => UNTRACKED,
		}
	}

	/// Show hidden blocks that retyped containers now allow, as a follow-up.
	fn reveal(
		&mut self,
		scene: &mut Scene,
		retyped: Vec<(NodeId, Vec<BlockId>)>,
	) -> Result<SceneCommit, CollabError> {
		let txn = self.doc.transact();
		let mut transaction = scene.transaction();
		let mut repairs = Vec::new();

		for (node, blocks) in retyped {
			let array = text_array(&self.nodes, &txn, node)?;
			let frame = self
				.texts
				.get_mut(&node)
				.ok_or(CollabError::SceneNode(node))?;
			transaction.text_external(node, |doc| {
				let (commit, found) = frame.mirror.reveal(&txn, &array, doc, &blocks)?;
				repairs.push((node, found));
				Ok::<_, CollabError>(commit)
			})?;
		}

		let commit = transaction.commit();
		drop(txn);
		self.repair(&repairs);
		Ok(commit)
	}

	/// Translate everything the observers recorded into one scene commit.
	fn reconcile<'l>(
		&mut self,
		scene: &mut Scene,
		lent: Option<&mut (dyn LentText + 'l)>,
	) -> Result<SceneCommit, CollabError> {
		let dirty = std::mem::take(&mut *lock(&self.dirty));
		let has_text = self.texts.values().any(|frame| frame.mirror.has_changes());

		if dirty.is_empty() && !has_text {
			return Ok(scene.transaction().commit());
		}

		let txn = self.doc.transact();
		let mut reconciler = Reconciler {
			txn: &txn,
			nodes: &self.nodes,
			schema: &self.schema,
			index: &mut self.index,
			texts: &mut self.texts,
			clock: &mut self.clock,
			repairs: Vec::new(),
		};
		let mut transaction = scene.transaction();
		reconciler.run(&mut transaction, dirty, lent)?;
		let repairs = std::mem::take(&mut reconciler.repairs);
		let commit = transaction.commit();
		drop(txn);
		self.repair(&repairs);
		Ok(commit)
	}

	/// Apply text frames' duplicate-block repairs.
	fn repair(&mut self, repairs: &[(NodeId, Vec<crate::remote::Repair>)]) {
		if repairs.iter().all(|(_, found)| found.is_empty()) {
			return;
		}

		let mut txn = self.doc.transact_mut_with(UNTRACKED);

		for (node, found) in repairs {
			let Ok(array) = text_array(&self.nodes, &txn, *node) else {
				continue;
			};

			if let Some(frame) = self.texts.get_mut(node) {
				frame.mirror.repair(&mut txn, &array, found);
			}
		}
	}
}

impl Drop for SceneBinding {
	fn drop(&mut self) {
		self.nodes.unobserve_deep(OBSERVER);
		let _ = self.doc.unobserve_update_v1(OBSERVER);
	}
}

/// Create mirrors for new text frames, seed them, and translate edits to
/// existing ones, in the commit's own transaction so a frame and its text
/// arrive together. Returns the containers each frame's edits retyped.
fn finish_text(
	txn: &mut TransactionMut<'_>,
	(nodes, schema): (&MapRef, &SceneSchema),
	texts: &mut FxHashMap<NodeId, TextFrame>,
	scene: &Scene,
	work: &TextWork,
) -> Result<Vec<(NodeId, Vec<BlockId>)>, CollabError> {
	for node in &work.removed {
		texts.remove(node);
	}

	let mut retyped = Vec::new();

	for &node in &work.created {
		let text_schema = schema.text().ok_or(CollabError::SceneNode(node))?;
		let doc = scene.text(node).ok_or(CollabError::SceneNode(node))?;
		let array = text_array(nodes, txn, node)?;
		let mut mirror = Mirror::new(text_schema.clone());
		mirror.seed(txn, &array, doc)?;
		let array = crate::registry::branch_id(&array);
		texts.insert(node, TextFrame { mirror, array });
	}

	for (node, commit) in &work.edits {
		let doc = scene.text(*node).ok_or(CollabError::SceneNode(*node))?;
		let array = text_array(nodes, txn, *node)?;
		let frame = texts.get_mut(node).ok_or(CollabError::SceneNode(*node))?;
		retyped.push((*node, frame.mirror.translate(txn, &array, doc, commit)?));
	}

	retyped.retain(|(_, blocks)| !blocks.is_empty());
	Ok(retyped)
}

/// The text array of node `id`, looked up in this transaction.
fn text_array<T: ReadTxn>(
	nodes: &MapRef,
	txn: &T,
	id: NodeId,
) -> Result<yrs::ArrayRef, CollabError> {
	local::node_map(nodes, txn, id)
		.and_then(|map| local::text_of(&map, txn))
		.ok_or(CollabError::SceneNode(id))
}

/// A record that writes `id` as it is now; text is seeded separately.
fn record_of(scene: &Scene, id: NodeId) -> Option<NodeRecord> {
	let node = scene.node(id)?;

	Some(NodeRecord {
		id,
		kind: node.kind(),
		parent: node.parent(),
		key: node.key().clone(),
		props: node.props().clone(),
		text: None,
	})
}

/// Record which nodes and props remote transactions touch.
fn observe_nodes(
	nodes: &MapRef,
	dirty: &Arc<Mutex<Dirty>>,
	text: Option<Arc<canvist_model::Schema>>,
) {
	let dirty = dirty.clone();
	nodes.observe_deep(
		OBSERVER,
		move |txn: &TransactionMut<'_>, events: &Events<'_>| {
			if is_local(txn) {
				return;
			}

			let mut dirty = lock(&dirty);

			for event in events.iter() {
				note(txn, event, &mut dirty, text.as_deref());
			}
		},
	);
}

/// Classify one event by where it happened: inside a text frame's array
/// (its records go to that frame's mirror), the node map itself, one node
/// map, or one node's props.
fn note(
	txn: &TransactionMut<'_>,
	event: &Event,
	dirty: &mut Dirty,
	text: Option<&canvist_model::Schema>,
) {
	let path: Vec<PathSegment> = event.path().into_iter().collect();

	if let Some(schema) = text
		&& let Some(id) = text_frame_of(&path)
	{
		// An event on the text array itself names that array as the root.
		let root = (path.len() == 2).then(|| event_target_id(event)).flatten();

		if let Some(record) = crate::remote::record::record(txn, event, schema, root) {
			dirty.text.entry(id).or_default().push(record);
		}

		return;
	}

	if let Event::Map(event) = event {
		note_map(&path, event.keys(txn).keys(), dirty);
	}
}

/// The text frame whose array an event path points inside.
fn text_frame_of(path: &[PathSegment]) -> Option<NodeId> {
	match path {
		[PathSegment::Key(node), PathSegment::Key(field), ..] if &**field == keys::TEXT => {
			node.parse().ok()
		}
		_ => None,
	}
}

/// Record the nodes or props a map event at `path` changed.
fn note_map<'k>(path: &[PathSegment], keys: impl Iterator<Item = &'k Arc<str>>, dirty: &mut Dirty) {
	match path {
		[] => {
			dirty
				.nodes
				.extend(keys.filter_map(|key| key.parse::<NodeId>().ok()));
		}
		[PathSegment::Key(node)] => dirty.nodes.extend(node.parse::<NodeId>().ok()),
		[PathSegment::Key(node), PathSegment::Key(field)] if &**field == keys::PROPS => {
			if let Ok(id) = node.parse::<NodeId>() {
				dirty.props.entry(id).or_default().extend(keys.cloned());
			}
		}
		_ => {}
	}
}

fn event_target_id(event: &Event) -> Option<yrs::ID> {
	match event {
		Event::Array(event) => crate::registry::branch_id(event.target()),
		_ => None,
	}
}
