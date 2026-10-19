//! What the shared document holds, and where each node is shown.
//!
//! The shared state is a flat map of nodes that point at their parents, so
//! concurrent edits can leave nodes whose parent is gone (orphans) and moves
//! that form cycles. Where a node is shown is a deterministic function of
//! the shared state, so every peer shows the same tree:
//!
//! - A node is shown when its type is known, its position register is
//!   valid, its parent is shown (or is the root), and the parent's type
//!   accepts it. Everything else is hidden but kept, and reappears when it
//!   becomes valid, for example when an undo restores a deleted parent.
//! - When parents form a cycle, the member whose position has the greatest
//!   `(clock, client, id)` falls back to its previous parent: the later
//!   move loses. If that still cycles, it falls back to the first page.

use std::collections::BTreeSet;

use canvist_scene::FracKey;
use canvist_scene::NodeId;
use canvist_scene::NodeTypeId;
use canvist_scene::SceneSchema;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use super::codec::Pos;

/// One node map in the shared document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
	/// The node's type, or `None` when the schema does not know it.
	pub(crate) kind: Option<NodeTypeId>,
	/// The position register, or `None` when it is malformed.
	pub(crate) pos: Option<Pos>,
}

/// Every node map in the shared document, indexed by declared parent.
#[derive(Debug, Default)]
pub(crate) struct Index {
	entries: FxHashMap<NodeId, Entry>,
	children: FxHashMap<NodeId, FxHashSet<NodeId>>,
	/// Shared nodes declared directly under the root, by key: the page
	/// candidates for cycle fallbacks.
	top: BTreeSet<(FracKey, NodeId)>,
}

impl Index {
	/// The entry of node `id`.
	pub(crate) fn get(&self, id: NodeId) -> Option<&Entry> {
		self.entries.get(&id)
	}

	/// How many node maps the shared document holds.
	pub(crate) fn len(&self) -> usize {
		self.entries.len()
	}

	/// Replace (or with `None`, forget) the entry of node `id`.
	pub(crate) fn set(&mut self, id: NodeId, entry: Option<Entry>) {
		if let Some(old) = self.entries.remove(&id) {
			self.unlink(id, &old);
		}

		if let Some(entry) = entry {
			self.link(id, &entry);
			self.entries.insert(id, entry);
		}
	}

	fn link(&mut self, id: NodeId, entry: &Entry) {
		let Some(pos) = &entry.pos else {
			return;
		};

		self.children.entry(pos.parent).or_default().insert(id);

		if pos.parent.is_root() {
			self.top.insert((pos.key.clone(), id));
		}
	}

	fn unlink(&mut self, id: NodeId, entry: &Entry) {
		let Some(pos) = &entry.pos else {
			return;
		};

		if let Some(children) = self.children.get_mut(&pos.parent) {
			children.remove(&id);
		}

		self.top.remove(&(pos.key.clone(), id));
	}

	/// `roots` and every node declared under them, transitively.
	pub(crate) fn closure(&self, roots: impl IntoIterator<Item = NodeId>) -> FxHashSet<NodeId> {
		let mut out = FxHashSet::default();
		let mut stack: Vec<NodeId> = roots.into_iter().collect();

		while let Some(id) = stack.pop() {
			if !out.insert(id) {
				continue;
			}

			if let Some(children) = self.children.get(&id) {
				stack.extend(children.iter().copied());
			}
		}

		out
	}

	fn declared(&self, id: NodeId) -> Option<NodeId> {
		self.entries.get(&id)?.pos.as_ref().map(|pos| pos.parent)
	}

	/// The parent after the first rule: the greatest member of a cycle of
	/// declared parents falls back to its previous parent.
	fn first_parent(&self, id: NodeId) -> Option<NodeId> {
		let pos = self.entries.get(&id)?.pos.as_ref()?;

		match cycle_of(id, |node| self.declared(node)) {
			Some(cycle) if self.loser(&cycle) == id => Some(pos.previous),
			_ => Some(pos.parent),
		}
	}

	/// The member of `cycle` whose move loses.
	fn loser(&self, cycle: &[NodeId]) -> NodeId {
		cycle
			.iter()
			.copied()
			.max_by_key(|&member| self.rank(member))
			.unwrap_or(NodeId::ROOT)
	}

	/// The order in which cycle members lose: the greatest loses.
	fn rank(&self, id: NodeId) -> (u64, u32, NodeId) {
		let (clock, client) = self
			.entries
			.get(&id)
			.and_then(|entry| entry.pos.as_ref())
			.map_or((0, 0), Pos::rank);
		(clock, client, id)
	}
}

/// Resolves where nodes are shown, memoized for one pass over the index.
pub(crate) struct Placer<'a> {
	index: &'a Index,
	schema: &'a SceneSchema,
	shown: FxHashMap<NodeId, Option<(NodeId, FracKey)>>,
}

impl<'a> Placer<'a> {
	pub(crate) fn new(index: &'a Index, schema: &'a SceneSchema) -> Self {
		Self {
			index,
			schema,
			shown: FxHashMap::default(),
		}
	}

	/// Where node `id` is shown, as its parent and key, or `None` when it is
	/// hidden.
	pub(crate) fn placement(&mut self, id: NodeId) -> Option<(NodeId, FracKey)> {
		if let Some(known) = self.shown.get(&id) {
			return known.clone();
		}

		// Insert a placeholder first: the effective tree is acyclic, so it
		// is never read, but it bounds the recursion if that ever broke.
		self.shown.insert(id, None);
		let placed = self.resolve(id);
		self.shown.insert(id, placed.clone());
		placed
	}

	fn resolve(&mut self, id: NodeId) -> Option<(NodeId, FracKey)> {
		let entry = self.index.get(id)?;
		let kind = entry.kind?;
		let key = entry.pos.as_ref()?.key.clone();
		let parent = self.effective(id)?;
		let parent_kind = if parent.is_root() {
			None
		} else {
			self.placement(parent)?;
			Some(self.index.get(parent)?.kind?)
		};

		self.schema
			.accepts(parent_kind, kind)
			.then_some((parent, key))
	}

	/// The parent after both rules: the greatest member of a cycle left by
	/// the first rule falls back to the first page.
	fn effective(&self, id: NodeId) -> Option<NodeId> {
		let first = self.index.first_parent(id)?;

		match cycle_of(id, |node| self.index.first_parent(node)) {
			Some(cycle) if self.index.loser(&cycle) == id => self.first_page(id),
			_ => Some(first),
		}
	}

	/// The first page by key, other than `id` itself.
	fn first_page(&self, id: NodeId) -> Option<NodeId> {
		let page = self.schema.page();

		self.index
			.top
			.iter()
			.map(|(_, candidate)| *candidate)
			.find(|&candidate| {
				candidate != id
					&& self.index.get(candidate).and_then(|entry| entry.kind) == Some(page)
			})
	}
}

/// The members of the cycle through `start` in the graph `parent`, or
/// `None` when walking up from `start` never returns to it.
fn cycle_of(
	start: NodeId,
	mut parent: impl FnMut(NodeId) -> Option<NodeId>,
) -> Option<Vec<NodeId>> {
	let mut path = vec![start];
	let mut seen = FxHashSet::default();
	seen.insert(start);
	let mut current = parent(start);

	while let Some(node) = current {
		if node == start {
			return Some(path);
		}

		if node.is_root() || !seen.insert(node) {
			return None;
		}

		path.push(node);
		current = parent(node);
	}

	None
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use canvist_model::BlockSpec;
	use canvist_model::Schema;

	use super::*;

	fn schema() -> SceneSchema {
		let text = Arc::new(
			Schema::builder()
				.block(BlockSpec::text("paragraph"))
				.build()
				.expect("schema"),
		);
		SceneSchema::standard(text).expect("scene schema")
	}

	fn entry(
		schema: &SceneSchema,
		kind: &str,
		parent: NodeId,
		clock: u64,
		previous: NodeId,
	) -> Entry {
		Entry {
			kind: schema.node_type(kind),
			pos: Some(Pos {
				parent,
				key: FracKey::first(),
				clock,
				client: 1,
				previous,
			}),
		}
	}

	#[test]
	fn orphans_and_unknown_types_are_hidden() {
		let schema = schema();
		let mut index = Index::default();
		let page = NodeId::new(1, 0);
		let gone = NodeId::new(1, 9);
		let orphan = NodeId::new(2, 0);
		let unknown = NodeId::new(2, 1);
		index.set(
			page,
			Some(entry(&schema, "page", NodeId::ROOT, 1, NodeId::ROOT)),
		);
		index.set(orphan, Some(entry(&schema, "rectangle", gone, 1, gone)));
		index.set(unknown, Some(entry(&schema, "blob", page, 1, page)));
		let mut placer = Placer::new(&index, &schema);

		assert_eq!(
			placer.placement(page).map(|(parent, _)| parent),
			Some(NodeId::ROOT)
		);
		assert_eq!(placer.placement(orphan), None);
		assert_eq!(placer.placement(unknown), None);
	}

	#[test]
	fn the_later_move_of_a_cycle_loses() {
		let schema = schema();
		let mut index = Index::default();
		let page = NodeId::new(1, 0);
		let a = NodeId::new(1, 1);
		let b = NodeId::new(1, 2);
		index.set(
			page,
			Some(entry(&schema, "page", NodeId::ROOT, 1, NodeId::ROOT)),
		);
		// A moved into B at clock 5; B moved into A at clock 7, which loses.
		index.set(a, Some(entry(&schema, "frame", b, 5, page)));
		index.set(b, Some(entry(&schema, "frame", a, 7, page)));
		let mut placer = Placer::new(&index, &schema);

		assert_eq!(placer.placement(b).map(|(parent, _)| parent), Some(page));
		assert_eq!(placer.placement(a).map(|(parent, _)| parent), Some(b));
	}

	#[test]
	fn a_loser_whose_previous_parent_still_cycles_goes_to_the_first_page() {
		let schema = schema();
		let mut index = Index::default();
		let page = NodeId::new(1, 0);
		let a = NodeId::new(1, 1);
		let b = NodeId::new(1, 2);
		index.set(
			page,
			Some(entry(&schema, "page", NodeId::ROOT, 1, NodeId::ROOT)),
		);
		index.set(a, Some(entry(&schema, "frame", b, 5, b)));
		index.set(b, Some(entry(&schema, "frame", a, 7, a)));
		let mut placer = Placer::new(&index, &schema);

		assert_eq!(placer.placement(b).map(|(parent, _)| parent), Some(page));
		assert_eq!(placer.placement(a).map(|(parent, _)| parent), Some(b));
	}
}
