//! Local scene steps become writes to the shared node map.

use canvist_model::Commit;
use canvist_scene::NodeId;
use canvist_scene::NodeRecord;
use canvist_scene::Prop;
use canvist_scene::Props;
use canvist_scene::Scene;
use canvist_scene::SceneCommit;
use canvist_scene::Step;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use yrs::ArrayPrelim;
use yrs::Map;
use yrs::MapPrelim;
use yrs::MapRef;
use yrs::Out;
use yrs::ReadTxn;
use yrs::TransactionMut;

use super::codec::Pos;
use super::codec::json_to_any;
use super::index::Entry;
use super::index::Index;
use super::keys;
use crate::CollabError;

/// What a commit's text steps need once its node writes are done.
#[derive(Debug, Default)]
pub(crate) struct TextWork {
	/// Text frames created by the commit: their arrays are seeded from the
	/// final documents.
	pub(crate) created: Vec<NodeId>,
	/// Text frames removed by the commit: their mirrors are dropped.
	pub(crate) removed: Vec<NodeId>,
	/// Text steps on frames that already existed, grouped per frame in order.
	pub(crate) edits: FxHashMap<NodeId, Commit>,
}

/// Translates one scene commit into one yrs transaction.
pub(crate) struct Writer<'a, 'txn> {
	pub(crate) txn: &'a mut TransactionMut<'txn>,
	pub(crate) nodes: &'a MapRef,
	pub(crate) scene: &'a Scene,
	pub(crate) index: &'a mut Index,
	pub(crate) clock: &'a mut u64,
}

impl Writer<'_, '_> {
	/// Write every step of `commit`, returning the text work left for later.
	pub(crate) fn commit(mut self, commit: &SceneCommit) -> Result<TextWork, CollabError> {
		let mut work = TextWork::default();
		let mut created = FxHashSet::default();

		for (step, inverse) in commit.steps.iter().zip(&commit.inverses) {
			self.step(step, inverse, &mut work, &mut created)?;
		}

		work.created.retain(|node| self.scene.contains(*node));
		work.edits.retain(|node, _| self.scene.contains(*node));
		Ok(work)
	}

	/// Write one step. Text steps on frames this commit created are skipped:
	/// those frames are seeded from their final documents.
	fn step(
		&mut self,
		step: &Step,
		inverse: &Step,
		work: &mut TextWork,
		created: &mut FxHashSet<NodeId>,
	) -> Result<(), CollabError> {
		match step {
			Step::InsertNodes { nodes } => {
				for record in nodes {
					self.insert(record, work, created);
				}
			}
			Step::RemoveNodes { .. } => self.remove(inverse, work),
			Step::Move { node, parent, key } => self.move_node(*node, *parent, key.clone())?,
			Step::SetProp { node, prop } => self.set_prop(*node, prop)?,
			Step::Text { node, step } if !created.contains(node) => {
				push_text(work, *node, step.clone(), inverse);
			}
			Step::Text { .. } => {}
		}

		Ok(())
	}

	fn insert(
		&mut self,
		record: &NodeRecord,
		work: &mut TextWork,
		created: &mut FxHashSet<NodeId>,
	) {
		let schema = self.scene.schema();
		let spec = schema.spec(record.kind);
		let pos = self.next_pos(record.parent, record.key.clone(), record.parent);
		let map: MapRef = self
			.nodes
			.insert(self.txn, record.id.to_string(), MapPrelim::default());
		map.insert(self.txn, keys::TYPE, spec.name());
		map.insert(self.txn, keys::POS, pos.to_any());
		let props: MapRef = map.insert(self.txn, keys::PROPS, MapPrelim::default());
		write_props(self.txn, &props, &record.props, spec.kind());

		if spec.kind() == canvist_scene::NodeKind::Text {
			let _: yrs::ArrayRef = map.insert(self.txn, keys::TEXT, ArrayPrelim::default());
			work.created.push(record.id);
			created.insert(record.id);
		}

		self.index.set(
			record.id,
			Some(Entry {
				kind: Some(record.kind),
				pos: Some(pos),
			}),
		);
	}

	/// Delete every removed node's map. The inverse lists them all,
	/// descendants included.
	fn remove(&mut self, inverse: &Step, work: &mut TextWork) {
		let Step::InsertNodes { nodes } = inverse else {
			return;
		};

		for record in nodes {
			self.nodes.remove(self.txn, &record.id.to_string());
			self.index.set(record.id, None);
			work.removed.push(record.id);
		}
	}

	fn move_node(
		&mut self,
		node: NodeId,
		parent: NodeId,
		key: canvist_scene::FracKey,
	) -> Result<(), CollabError> {
		let map = self.node_map(node)?;
		let previous = self
			.index
			.get(node)
			.and_then(|entry| entry.pos.as_ref())
			.map_or(parent, |pos| pos.parent);
		let pos = self.next_pos(parent, key, previous);
		map.insert(self.txn, keys::POS, pos.to_any());
		let kind = self.index.get(node).and_then(|entry| entry.kind);
		self.index.set(
			node,
			Some(Entry {
				kind,
				pos: Some(pos),
			}),
		);
		Ok(())
	}

	fn set_prop(&mut self, node: NodeId, prop: &Prop) -> Result<(), CollabError> {
		let props =
			props_of(&self.node_map(node)?, self.txn).ok_or(CollabError::SceneNode(node))?;
		let key = prop.key().to_string();

		match prop {
			Prop::Custom(_, None) | Prop::ClearOverride(..) => {
				props.remove(self.txn, &key);
			}
			_ => {
				props.insert(self.txn, key, json_to_any(&prop.to_json()));
			}
		}

		Ok(())
	}

	fn node_map(&self, node: NodeId) -> Result<MapRef, CollabError> {
		node_map(self.nodes, self.txn, node).ok_or(CollabError::SceneNode(node))
	}

	fn next_pos(&mut self, parent: NodeId, key: canvist_scene::FracKey, previous: NodeId) -> Pos {
		*self.clock += 1;

		Pos {
			parent,
			key,
			clock: *self.clock,
			client: self.scene.client(),
			previous,
		}
	}
}

fn push_text(work: &mut TextWork, node: NodeId, step: canvist_model::Step, inverse: &Step) {
	let Step::Text { step: inverse, .. } = inverse else {
		return;
	};
	let commit = work.edits.entry(node).or_insert_with(|| {
		Commit {
			steps: Vec::new(),
			inverses: Vec::new(),
			maps: Vec::new(),
			touched: Vec::new(),
			structural: false,
			version: 0,
		}
	});
	commit.steps.push(step);
	commit.inverses.push(inverse.clone());
}

/// Write every prop that differs from its kind's default into `map`, in key
/// order.
///
/// A `MapPrelim` would insert them in hash order, which differs between
/// processes. Two peers that seed the same scene under the same client id
/// must give every key the same item id, or a later write, which names the
/// item it replaces, lands on the wrong key for the other peer.
pub(crate) fn write_props(
	txn: &mut TransactionMut<'_>,
	map: &MapRef,
	props: &Props,
	kind: canvist_scene::NodeKind,
) {
	for prop in props.changed_from(&Props::for_kind(kind)) {
		map.insert(txn, prop.key().to_string(), json_to_any(&prop.to_json()));
	}
}

/// The map of node `id`.
pub(crate) fn node_map<T: ReadTxn>(nodes: &MapRef, txn: &T, id: NodeId) -> Option<MapRef> {
	match nodes.get(txn, &id.to_string())? {
		Out::YMap(map) => Some(map),
		_ => None,
	}
}

/// A node map's props map.
pub(crate) fn props_of<T: ReadTxn>(map: &MapRef, txn: &T) -> Option<MapRef> {
	match map.get(txn, keys::PROPS)? {
		Out::YMap(props) => Some(props),
		_ => None,
	}
}

/// A node map's text array.
pub(crate) fn text_of<T: ReadTxn>(map: &MapRef, txn: &T) -> Option<yrs::ArrayRef> {
	match map.get(txn, keys::TEXT)? {
		Out::YArray(text) => Some(text),
		_ => None,
	}
}
