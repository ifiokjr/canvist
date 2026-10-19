//! Remote changes to the shared node map become scene steps.
//!
//! The observer records which nodes and which props another peer (or undo)
//! touched. Reconciling reads only those nodes, plus the nodes declared
//! under them, so the cost follows the size of the change:
//!
//! 1. Refresh the index entries of touched nodes.
//! 2. Work out where each affected node is shown (see [`super::index`]).
//! 3. Insert nodes that became visible (parents first), move nodes whose
//!    placement changed (in an order that never forms a cycle), and remove
//!    nodes that became hidden.
//! 4. Set touched props, skipping values this schema rejects.
//! 5. Mirror text frames' documents through their own block mirrors.

use std::sync::Arc;

use canvist_model::BlockTree;
use canvist_model::Commit;
use canvist_model::Document;
use canvist_scene::FracKey;
use canvist_scene::NodeId;
use canvist_scene::NodeKind;
use canvist_scene::NodeRecord;
use canvist_scene::Prop;
use canvist_scene::PropKey;
use canvist_scene::Props;
use canvist_scene::Scene;
use canvist_scene::SceneSchema;
use canvist_scene::Step;
use canvist_scene::Transaction;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use yrs::Map;
use yrs::MapRef;
use yrs::Out;
use yrs::ReadTxn;

use super::LentText;
use super::codec::Pos;
use super::codec::any_to_json;
use super::index::Entry;
use super::index::Index;
use super::index::Placer;
use super::keys;
use super::local::node_map;
use super::local::props_of;
use super::local::text_of;
use crate::CollabError;
use crate::mirror::Mirror;
use crate::registry::branch_id;
use crate::remote::Repair;
use crate::remote::record::Record;

/// What the observer recorded since the last reconcile.
#[derive(Debug, Default)]
pub(crate) struct Dirty {
	/// Nodes whose map was added, removed, or had its type, position, props
	/// map, or text array replaced.
	pub(crate) nodes: FxHashSet<NodeId>,
	/// Props keys changed per node.
	pub(crate) props: FxHashMap<NodeId, FxHashSet<Arc<str>>>,
	/// Changes inside text frames' arrays, per frame, for their mirrors.
	pub(crate) text: FxHashMap<NodeId, Vec<Record>>,
}

/// A text frame's mirror and the item id of the array it mirrors, which
/// tells when the node's array is replaced.
#[derive(Debug)]
pub(crate) struct TextFrame {
	pub(crate) mirror: Mirror,
	pub(crate) array: Option<yrs::ID>,
}

impl Dirty {
	pub(crate) fn is_empty(&self) -> bool {
		self.nodes.is_empty() && self.props.is_empty() && self.text.is_empty()
	}
}

/// Where a node sits: its parent and key.
type Placement = (NodeId, FracKey);

/// Repairs each text frame's mirror must apply once reading is done.
pub(crate) type TextRepairs = Vec<(NodeId, Vec<Repair>)>;

/// Reconciles one batch of remote changes.
pub(crate) struct Reconciler<'a, T: ReadTxn> {
	pub(crate) txn: &'a T,
	pub(crate) nodes: &'a MapRef,
	pub(crate) schema: &'a Arc<SceneSchema>,
	pub(crate) index: &'a mut Index,
	pub(crate) texts: &'a mut FxHashMap<NodeId, TextFrame>,
	pub(crate) clock: &'a mut u64,
	pub(crate) repairs: TextRepairs,
}

/// The changes one reconcile makes, before they are applied.
#[derive(Debug, Default)]
struct Plan {
	inserts: Vec<NodeId>,
	moves: Vec<(NodeId, NodeId, FracKey)>,
	removes: Vec<NodeId>,
	/// Text frames whose text array was replaced: removed, then inserted.
	rebuild: Vec<NodeId>,
	placed: FxHashMap<NodeId, (NodeId, FracKey)>,
}

impl<T: ReadTxn> Reconciler<'_, T> {
	/// Apply `dirty` to `scene` in `transaction`.
	pub(crate) fn run<'l>(
		&mut self,
		transaction: &mut Transaction<'_>,
		dirty: Dirty,
		mut lent: Option<&mut (dyn LentText + 'l)>,
	) -> Result<(), CollabError> {
		for &id in &dirty.nodes {
			let entry = self.read_entry(id);
			self.index.set(id, entry);
		}

		let plan = self.plan(transaction.scene(), &dirty.nodes);

		if !plan.rebuild.is_empty() {
			transaction.remove(&plan.rebuild)?;
		}

		self.insert(transaction, &plan)?;
		move_all(transaction, plan.moves.clone())?;

		if !plan.removes.is_empty() {
			transaction.remove(&plan.removes)?;
		}

		for id in &plan.removes {
			self.texts.remove(id);
		}

		let inserted: FxHashSet<NodeId> = plan.inserts.iter().copied().collect();
		self.props(transaction, &dirty, &inserted)?;

		for (id, records) in dirty.text {
			if let Some(frame) = self.texts.get(&id).filter(|_| !inserted.contains(&id)) {
				frame.mirror.receive(records);
			}
		}

		self.text(transaction, &mut lent)
	}

	/// The index entry of node `id`, read from the shared state.
	pub(crate) fn read_entry(&mut self, id: NodeId) -> Option<Entry> {
		let map = node_map(self.nodes, self.txn, id)?;
		let kind = match map.get(self.txn, keys::TYPE) {
			Some(Out::Any(yrs::Any::String(name))) => self.schema.node_type(&name),
			_ => None,
		};
		let pos = match map.get(self.txn, keys::POS) {
			Some(Out::Any(any)) => Pos::from_any(&any),
			_ => None,
		};

		if let Some(pos) = &pos {
			*self.clock = (*self.clock).max(pos.clock);
		}

		Some(Entry { kind, pos })
	}

	/// Where every affected node should be, against where it is.
	fn plan(&self, scene: &Scene, dirty: &FxHashSet<NodeId>) -> Plan {
		let in_scene = dirty
			.iter()
			.filter(|id| scene.contains(**id))
			.flat_map(|&id| scene.subtree(id));
		let affected = self.index.closure(dirty.iter().copied().chain(in_scene));
		let mut placer = Placer::new(self.index, self.schema);
		let mut plan = Plan::default();
		let mut sorted: Vec<NodeId> = affected.into_iter().collect();
		sorted.sort_unstable();

		for id in sorted {
			let desired = placer.placement(id).filter(|_| self.has_text_if_needed(id));
			let current = scene
				.node(id)
				.map(|node| (node.parent(), node.key().clone()));

			self.classify(&mut plan, id, (desired, current), dirty);
		}

		plan
	}

	fn is_text(&self, id: NodeId) -> bool {
		self.index
			.get(id)
			.and_then(|entry| entry.kind)
			.is_some_and(|kind| self.schema.spec(kind).kind() == NodeKind::Text)
	}

	/// Add node `id` to the plan from where it should be and where it is.
	fn classify(
		&self,
		plan: &mut Plan,
		id: NodeId,
		(desired, current): (Option<Placement>, Option<Placement>),
		dirty: &FxHashSet<NodeId>,
	) {
		match (desired, current) {
			(Some(desired), None) => {
				plan.inserts.push(id);
				plan.placed.insert(id, desired);
			}
			(Some(desired), Some(_))
				if dirty.contains(&id) && self.is_text(id) && self.text_replaced(id) =>
			{
				plan.rebuild.push(id);
				plan.inserts.push(id);
				plan.placed.insert(id, desired);
			}
			(Some(desired), Some(current)) if desired != current => {
				plan.moves.push((id, desired.0, desired.1));
			}
			(None, Some(_)) => plan.removes.push(id),
			_ => {}
		}
	}

	/// Text frames need their text array; one without it stays hidden.
	fn has_text_if_needed(&self, id: NodeId) -> bool {
		!self.is_text(id)
			|| node_map(self.nodes, self.txn, id)
				.is_some_and(|map| text_of(&map, self.txn).is_some())
	}

	/// Insert newly visible nodes in one step, parents first.
	fn insert(
		&mut self,
		transaction: &mut Transaction<'_>,
		plan: &Plan,
	) -> Result<(), CollabError> {
		if plan.inserts.is_empty() {
			return Ok(());
		}

		let depth = |mut id: NodeId| {
			let mut depth = 0usize;

			while let Some((parent, _)) = plan.placed.get(&id) {
				depth += 1;
				id = *parent;
			}

			depth
		};
		let mut order = plan.inserts.clone();
		order.sort_by_key(|&id| (depth(id), id));
		let client = transaction.scene().client();
		let records = order
			.into_iter()
			.map(|id| self.record(id, &plan.placed[&id], client))
			.collect::<Result<Vec<_>, _>>()?;

		transaction.step(Step::InsertNodes { nodes: records })?;
		Ok(())
	}

	/// The record that inserts shared node `id` at `placement`.
	fn record(
		&mut self,
		id: NodeId,
		placement: &(NodeId, FracKey),
		client: u32,
	) -> Result<NodeRecord, CollabError> {
		let kind = self
			.index
			.get(id)
			.and_then(|entry| entry.kind)
			.ok_or(CollabError::SceneNode(id))?;
		let behaviour = self.schema.spec(kind).kind();
		let map = node_map(self.nodes, self.txn, id).ok_or(CollabError::SceneNode(id))?;
		let props = props_of(&map, self.txn).map_or_else(
			|| Props::for_kind(behaviour),
			|props| read_props(&props, self.txn, behaviour),
		);
		let text = match behaviour {
			NodeKind::Text => Some(self.text_record(id, &map, client)?),
			_ => None,
		};

		Ok(NodeRecord {
			id,
			kind,
			parent: placement.0,
			key: placement.1.clone(),
			props,
			text,
		})
	}

	/// A new mirror over a text frame's array, and the blocks it holds.
	fn text_record(
		&mut self,
		id: NodeId,
		map: &MapRef,
		client: u32,
	) -> Result<Vec<BlockTree>, CollabError> {
		let array = text_of(map, self.txn).ok_or(CollabError::SceneNode(id))?;
		let schema = self.schema.text().ok_or(CollabError::SceneNode(id))?;
		let mut mirror = Mirror::new(schema.clone());
		let (doc, repairs) = mirror.document(self.txn, &array, client)?;
		self.texts.insert(
			id,
			TextFrame {
				mirror,
				array: branch_id(&array),
			},
		);
		self.repairs.push((id, repairs));

		Ok(doc
			.children(None)
			.iter()
			.filter_map(|&block| doc.tree(block))
			.collect())
	}

	/// Set touched props on nodes that stayed visible.
	fn props(
		&mut self,
		transaction: &mut Transaction<'_>,
		dirty: &Dirty,
		inserted: &FxHashSet<NodeId>,
	) -> Result<(), CollabError> {
		let mut targets: Vec<NodeId> = dirty
			.nodes
			.iter()
			.chain(dirty.props.keys())
			.copied()
			.filter(|id| !inserted.contains(id) && transaction.scene().contains(*id))
			.collect();
		targets.sort_unstable();
		targets.dedup();

		for id in targets {
			for prop in self.prop_changes(transaction.scene(), id, dirty) {
				transaction.set(id, prop)?;
			}
		}

		Ok(())
	}

	/// The props of `id` whose shared value differs from the scene's.
	fn prop_changes(&self, scene: &Scene, id: NodeId, dirty: &Dirty) -> Vec<Prop> {
		let (Some(node), Some(kind)) = (scene.node(id), scene.kind(id)) else {
			return Vec::new();
		};
		let shared = node_map(self.nodes, self.txn, id)
			.and_then(|map| props_of(&map, self.txn))
			.map_or_else(
				|| Props::for_kind(kind),
				|props| read_props(&props, self.txn, kind),
			);
		let keys: Vec<PropKey> = if dirty.nodes.contains(&id) {
			all_keys(node.props(), &shared)
		} else {
			dirty
				.props
				.get(&id)
				.into_iter()
				.flatten()
				.filter_map(|name| PropKey::parse(name))
				.collect()
		};

		keys.into_iter()
			.map(|key| shared.get(&key))
			.filter(|prop| *prop != node.props().get(&prop.key()))
			.collect()
	}

	/// Mirror each text frame's recorded changes.
	fn text<'l>(
		&mut self,
		transaction: &mut Transaction<'_>,
		lent: &mut Option<&mut (dyn LentText + 'l)>,
	) -> Result<(), CollabError> {
		let mut pending: Vec<NodeId> = self
			.texts
			.iter()
			.filter(|(_, frame)| frame.mirror.has_changes())
			.map(|(&id, _)| id)
			.collect();
		pending.sort_unstable();

		for id in pending {
			let txn = self.txn;
			let Some(array) = node_map(self.nodes, txn, id).and_then(|map| text_of(&map, txn))
			else {
				continue;
			};
			let Some(frame) = self.texts.get_mut(&id) else {
				continue;
			};
			let mut repairs = Vec::new();
			let mut change = |doc: &mut Document| -> Result<Commit, CollabError> {
				let (commit, found) = frame.mirror.mirror(txn, &array, doc)?;
				repairs = found;
				Ok(commit)
			};

			match lent.as_deref_mut() {
				Some(holder) if holder.node() == id => {
					holder.apply(&mut change)?;
				}
				_ => transaction.text_external(id, change)?,
			}

			self.repairs.push((id, repairs));
		}

		Ok(())
	}

	/// Whether the text frame's mirror still reads the node's current array.
	pub(crate) fn text_replaced(&self, id: NodeId) -> bool {
		let current = node_map(self.nodes, self.txn, id)
			.and_then(|map| text_of(&map, self.txn))
			.and_then(|array| branch_id(&array));
		let mirrored = self.texts.get(&id).and_then(|frame| frame.array);

		current != mirrored
	}
}

/// Read a shared props map, keeping the values this schema accepts for
/// `kind` and leaving the rest at their defaults.
pub(crate) fn read_props<T: ReadTxn>(map: &MapRef, txn: &T, kind: NodeKind) -> Props {
	let mut props = Props::for_kind(kind);

	for (name, value) in map.iter(txn) {
		let Out::Any(any) = value else {
			continue;
		};
		let Some(key) = PropKey::parse(name) else {
			continue;
		};

		if let Ok(prop) = Prop::from_json(&key, &any_to_json(&any))
			&& prop.fits(kind)
		{
			props.set(prop);
		}
	}

	props
}

/// Every key either side has a non-default value for.
fn all_keys(a: &Props, b: &Props) -> Vec<PropKey> {
	let mut keys: Vec<PropKey> = PropKey::CORE.to_vec();
	keys.extend(
		a.custom
			.keys()
			.chain(b.custom.keys())
			.map(|name| PropKey::Custom(name.clone())),
	);
	keys.extend(
		a.overrides
			.keys()
			.chain(b.overrides.keys())
			.map(|(target, key)| PropKey::Override(*target, Box::new(key.clone()))),
	);
	keys.sort();
	keys.dedup();
	keys
}

/// Apply moves in an order that never makes a node its own ancestor: a
/// move whose target is not inside the moving node always exists, because
/// the final tree is acyclic.
fn move_all(
	transaction: &mut Transaction<'_>,
	mut moves: Vec<(NodeId, NodeId, FracKey)>,
) -> Result<(), CollabError> {
	while !moves.is_empty() {
		let scene = transaction.scene();
		let ready = moves
			.iter()
			.position(|&(node, parent, _)| parent != node && !scene.is_ancestor(node, parent))
			.ok_or(CollabError::Diverged)?;
		let (node, parent, key) = moves.swap_remove(ready);
		transaction.step(Step::Move { node, parent, key })?;
	}

	Ok(())
}
