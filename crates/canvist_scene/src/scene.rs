//! The scene: a tree of nodes and the text documents of its text frames.

use std::cmp::Ordering;
use std::sync::Arc;

use canvist_model::Document;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use crate::FracKey;
use crate::Node;
use crate::NodeId;
use crate::NodeIdGenerator;
use crate::NodeKind;
use crate::NodeSpec;
use crate::Props;
use crate::SceneError;
use crate::SceneSchema;

/// A text frame's document, or a marker that an editor holds it.
#[derive(Clone, Debug)]
pub(crate) enum TextSlot {
	Owned(Document),
	Lent,
}

/// A tree of nodes under [`NodeId::ROOT`], whose children are pages.
///
/// Steps are the only way to change it (see [`Step`](crate::Step)), through
/// [`Scene::transaction`] or [`Scene::apply_steps`]. Reading is plain struct
/// access: rendering and hit testing read thousands of nodes per frame.
#[derive(Clone, Debug)]
pub struct Scene {
	schema: Arc<SceneSchema>,
	pub(crate) nodes: FxHashMap<NodeId, Node>,
	pub(crate) children: FxHashMap<NodeId, Vec<NodeId>>,
	pub(crate) texts: FxHashMap<NodeId, TextSlot>,
	ids: NodeIdGenerator,
	version: u64,
}

impl Scene {
	/// A scene with one empty page. `client` seeds node id allocation and
	/// must be unique among collaborating peers.
	pub fn new(schema: Arc<SceneSchema>, client: u32) -> Result<Self, SceneError> {
		let mut scene = Self::empty(schema, client);
		let mut transaction = scene.transaction();
		transaction.insert(NodeId::ROOT, None, "page", Props::default())?;
		let _ = transaction.commit();
		Ok(scene)
	}

	/// A scene with no pages, for loading content into.
	#[must_use]
	pub fn empty(schema: Arc<SceneSchema>, client: u32) -> Self {
		let mut children = FxHashMap::default();
		children.insert(NodeId::ROOT, Vec::new());

		Self {
			schema,
			nodes: FxHashMap::default(),
			children,
			texts: FxHashMap::default(),
			ids: NodeIdGenerator::new(client),
			version: 0,
		}
	}

	/// The schema.
	#[must_use]
	pub fn schema(&self) -> &Arc<SceneSchema> {
		&self.schema
	}

	/// The client id new nodes are allocated under.
	#[must_use]
	pub fn client(&self) -> u32 {
		self.ids.client()
	}

	/// Incremented by every applied step.
	#[must_use]
	pub fn version(&self) -> u64 {
		self.version
	}

	/// How many nodes the scene holds (the root is not a node).
	#[must_use]
	pub fn len(&self) -> usize {
		self.nodes.len()
	}

	/// Whether the scene has no nodes.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.nodes.is_empty()
	}

	/// Whether `id` is a node of this scene.
	#[must_use]
	pub fn contains(&self, id: NodeId) -> bool {
		self.nodes.contains_key(&id)
	}

	/// The node with this id.
	#[must_use]
	pub fn node(&self, id: NodeId) -> Option<&Node> {
		self.nodes.get(&id)
	}

	/// The declaration of the node's type.
	#[must_use]
	pub fn spec(&self, id: NodeId) -> Option<&NodeSpec> {
		self.node(id).map(|node| self.schema.spec(node.kind))
	}

	/// The built-in behaviour of the node's type.
	#[must_use]
	pub fn kind(&self, id: NodeId) -> Option<NodeKind> {
		self.spec(id).map(NodeSpec::kind)
	}

	/// The children of `parent` in order. [`NodeId::ROOT`]'s are the pages.
	#[must_use]
	pub fn children(&self, parent: NodeId) -> &[NodeId] {
		self.children.get(&parent).map_or(&[], Vec::as_slice)
	}

	/// The pages, in order.
	#[must_use]
	pub fn pages(&self) -> &[NodeId] {
		self.children(NodeId::ROOT)
	}

	/// Every node, in no particular order.
	pub fn nodes(&self) -> impl Iterator<Item = &Node> {
		self.nodes.values()
	}

	/// The node's index among its siblings.
	#[must_use]
	pub fn index_in_parent(&self, id: NodeId) -> Option<usize> {
		let node = self.node(id)?;
		sorted_position(&self.nodes, self.children(node.parent), node)
	}

	/// The node's ancestors, nearest first, ending before the root.
	pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
		let mut current = self.node(id).map(Node::parent);

		std::iter::from_fn(move || {
			let id = current.filter(|id| !id.is_root())?;
			current = self.node(id).map(Node::parent);
			Some(id)
		})
	}

	/// Whether `ancestor` is a strict ancestor of `id`. The root is an
	/// ancestor of every node.
	#[must_use]
	pub fn is_ancestor(&self, ancestor: NodeId, id: NodeId) -> bool {
		if ancestor.is_root() {
			return self.contains(id);
		}

		self.ancestors(id).any(|candidate| candidate == ancestor)
	}

	/// The page containing `id`.
	#[must_use]
	pub fn page_of(&self, id: NodeId) -> Option<NodeId> {
		std::iter::once(id)
			.chain(self.ancestors(id))
			.last()
			.filter(|&page| self.node(page).is_some_and(|node| node.parent.is_root()))
	}

	/// `id` and its descendants in pre-order (paint order).
	#[must_use]
	pub fn subtree(&self, id: NodeId) -> Vec<NodeId> {
		let mut out = Vec::new();
		let mut stack = vec![id];

		while let Some(next) = stack.pop() {
			if !next.is_root() {
				out.push(next);
			}

			stack.extend(self.children(next).iter().rev());
		}

		out
	}

	/// A text frame's document, unless an editor holds it.
	#[must_use]
	pub fn text(&self, id: NodeId) -> Option<&Document> {
		match self.texts.get(&id)? {
			TextSlot::Owned(doc) => Some(doc),
			TextSlot::Lent => None,
		}
	}

	/// Whether a text frame's document is lent to an editor.
	#[must_use]
	pub fn is_text_lent(&self, id: NodeId) -> bool {
		matches!(self.texts.get(&id), Some(TextSlot::Lent))
	}

	/// Hand a text frame's document to an editor, for example
	/// `EditorBuilder::build_with`. Until [`Scene::return_text`], `Text`
	/// steps on the frame fail and [`Scene::text`] returns `None`; the
	/// editor's own commits are the frame's changes.
	pub fn lend_text(&mut self, id: NodeId) -> Result<Document, SceneError> {
		match self.texts.insert(id, TextSlot::Lent) {
			Some(TextSlot::Owned(doc)) => Ok(doc),
			Some(TextSlot::Lent) => Err(SceneError::TextLent(id)),
			None => {
				self.texts.remove(&id);
				Err(SceneError::NotText(id))
			}
		}
	}

	/// Take back a lent document. The node's revision is bumped so caches
	/// relayout the frame.
	pub fn return_text(&mut self, id: NodeId, doc: Document) -> Result<(), SceneError> {
		if !self.is_text_lent(id) {
			return Err(SceneError::NotText(id));
		}

		self.texts.insert(id, TextSlot::Owned(doc));
		self.version += 1;
		self.touch(id);
		Ok(())
	}

	/// A key that places a new child of `parent` at `index` (`None` for the
	/// end), ignoring `moving` if it is already a child. When the neighbours
	/// share a key (concurrent inserts), the key lands after that run.
	pub fn key_at(
		&self,
		parent: NodeId,
		index: Option<usize>,
		moving: Option<NodeId>,
	) -> Result<FracKey, SceneError> {
		let children = self.children(parent);
		let skipped = moving
			.and_then(|id| self.node(id))
			.filter(|node| node.parent == parent)
			.and_then(|node| sorted_position(&self.nodes, children, node));
		let len = children.len() - usize::from(skipped.is_some());
		let index = index.unwrap_or(len).min(len);
		// The key at an index of the list with `moving` left out.
		let key = |at: usize| {
			let real = at + usize::from(skipped.is_some_and(|skip| at >= skip));
			self.nodes.get(&children[real]).map(Node::key)
		};
		let before = index.checked_sub(1).and_then(key);
		let after = (index..len)
			.filter_map(key)
			.find(|&candidate| Some(candidate) != before);

		Ok(FracKey::between(before, after)?)
	}

	/// Allocate a fresh node id.
	pub fn allocate_id(&mut self) -> Result<NodeId, SceneError> {
		self.ids.next_id().ok_or(SceneError::IdsExhausted)
	}

	/// Check every structural invariant, for tests and debugging: parents
	/// and child lists agree, child lists are sorted, every node is reachable
	/// from the root, types are accepted by their parents, props are valid,
	/// and exactly the text frames have documents.
	pub fn check(&self) -> Result<(), String> {
		for node in self.nodes.values() {
			self.check_node(node)?;
		}

		for (&parent, children) in &self.children {
			self.check_children(parent, children)?;
		}

		let reachable = self.subtree(NodeId::ROOT).len();

		if reachable != self.nodes.len() {
			return Err(format!("{} nodes, {reachable} reachable", self.nodes.len()));
		}

		Ok(())
	}

	fn check_node(&self, node: &Node) -> Result<(), String> {
		let parent_kind = (!node.parent.is_root())
			.then(|| self.node(node.parent).map(Node::kind))
			.map(|kind| kind.ok_or(format!("{} has a missing parent", node.id)))
			.transpose()?;
		let spec = self.schema.spec(node.kind);

		if !self.schema.accepts(parent_kind, node.kind) {
			return Err(format!("{} is not accepted by its parent", node.id));
		}

		if !node.props.shape.fits(spec.kind()) {
			return Err(format!("{} has a shape that does not fit", node.id));
		}

		if self.texts.contains_key(&node.id) != (spec.kind() == NodeKind::Text) {
			return Err(format!("{} has the wrong text state", node.id));
		}

		if !self.children(node.parent).contains(&node.id) {
			return Err(format!("{} is missing from its parent", node.id));
		}

		Ok(())
	}

	/// A child list must name existing nodes whose parent it is, in strictly
	/// increasing sibling order.
	fn check_children(&self, parent: NodeId, children: &[NodeId]) -> Result<(), String> {
		if !parent.is_root() && !self.contains(parent) {
			return Err(format!("children listed for missing {parent}"));
		}

		for pair in children.windows(2) {
			if self.order(pair[0], pair[1]) != Ordering::Less {
				return Err(format!("children of {parent} are out of order"));
			}
		}

		match children
			.iter()
			.find(|child| self.node(**child).is_none_or(|node| node.parent != parent))
		{
			Some(child) => Err(format!("{child} is listed under {parent} wrongly")),
			None => Ok(()),
		}
	}

	/// Sibling order: by key, then by id.
	pub(crate) fn order(&self, a: NodeId, b: NodeId) -> Ordering {
		let key = |id: NodeId| self.nodes.get(&id).map(Node::key);
		key(a).cmp(&key(b)).then(a.cmp(&b))
	}

	/// Insert `id` into its parent's child list at its sorted position.
	pub(crate) fn link(&mut self, id: NodeId) {
		let Some(node) = self.nodes.get(&id) else {
			return;
		};
		let parent = node.parent;
		let probe = (node.key.clone(), id);
		let nodes = &self.nodes;
		let children = self.children.entry(parent).or_default();
		let at = children.partition_point(|child| {
			nodes
				.get(child)
				.is_some_and(|node| (&node.key, *child) < (&probe.0, probe.1))
		});
		children.insert(at, id);
	}

	/// Remove `id` from its parent's child list. Call it before changing
	/// the node's key: the list is searched by key.
	pub(crate) fn unlink(&mut self, id: NodeId) {
		let Some(node) = self.nodes.get(&id) else {
			return;
		};
		let at = sorted_position(&self.nodes, self.children(node.parent), node);
		let parent = node.parent;

		if let (Some(at), Some(children)) = (at, self.children.get_mut(&parent)) {
			children.remove(at);
		}
	}

	pub(crate) fn touch(&mut self, id: NodeId) {
		let version = self.version;

		if let Some(node) = self.nodes.get_mut(&id) {
			node.rev = version;
		}
	}

	pub(crate) fn bump_version(&mut self) {
		self.version += 1;
	}

	pub(crate) fn observe_id(&mut self, id: NodeId) {
		self.ids.observe(id);
	}

	/// Ids from a set, keeping only those that are not descendants of others
	/// in the set.
	pub(crate) fn roots_of(&self, ids: &[NodeId]) -> Vec<NodeId> {
		let set: FxHashSet<NodeId> = ids.iter().copied().collect();
		let mut seen = FxHashSet::default();

		ids.iter()
			.copied()
			.filter(|&id| seen.insert(id))
			.filter(|&id| !self.ancestors(id).any(|ancestor| set.contains(&ancestor)))
			.collect()
	}
}

/// Where `node` sits in `children`, found by binary search on sibling order.
fn sorted_position(
	nodes: &FxHashMap<NodeId, Node>,
	children: &[NodeId],
	node: &Node,
) -> Option<usize> {
	let probe = (&node.key, node.id);
	let at = children.partition_point(|child| {
		nodes
			.get(child)
			.is_some_and(|sibling| (&sibling.key, *child) < probe)
	});

	(children.get(at) == Some(&node.id)).then_some(at)
}
