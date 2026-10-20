//! Step implementations. Each validates fully before mutating, so a failed
//! step leaves the scene unchanged.

use std::sync::Arc;

use canvist_model::Document;
use canvist_model::Schema;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use super::Applied;
use super::Step;
use crate::FracKey;
use crate::Node;
use crate::NodeId;
use crate::NodeKind;
use crate::NodeRecord;
use crate::NodeTypeId;
use crate::Prop;
use crate::Props;
use crate::Scene;
use crate::SceneError;
use crate::scene::TextSlot;

pub(super) fn insert_nodes(
	scene: &mut Scene,
	records: Vec<NodeRecord>,
) -> Result<Applied, SceneError> {
	let texts = validate_inserts(scene, &records)?;
	let batch: FxHashSet<NodeId> = records.iter().map(|record| record.id).collect();
	let mut roots = Vec::new();
	let mut touched = Vec::with_capacity(records.len() + 1);

	for (record, text) in records.into_iter().zip(texts) {
		if !batch.contains(&record.parent) {
			roots.push(record.id);
			push_parent(&mut touched, record.parent);
		}

		touched.push(record.id);
		insert_one(scene, record, text);
	}

	Ok(Applied {
		inverse: Step::RemoveNodes { ids: roots },
		touched,
		structural: !batch.is_empty(),
	})
}

fn insert_one(scene: &mut Scene, record: NodeRecord, text: Option<Document>) {
	let id = record.id;
	scene.observe_id(id);
	scene.nodes.insert(
		id,
		Node {
			id,
			kind: record.kind,
			parent: record.parent,
			key: record.key,
			props: record.props,
			rev: 0,
		},
	);
	scene.link(id);

	if let Some(text) = text {
		scene.texts.insert(id, TextSlot::Owned(text));
	}
}

/// Check every record and build text frames' documents, before anything
/// changes.
fn validate_inserts(
	scene: &Scene,
	records: &[NodeRecord],
) -> Result<Vec<Option<Document>>, SceneError> {
	let mut inserted: FxHashMap<NodeId, NodeTypeId> = FxHashMap::default();
	let mut texts = Vec::with_capacity(records.len());

	for record in records {
		if scene.contains(record.id) || record.id.is_root() || inserted.contains_key(&record.id) {
			return Err(SceneError::DuplicateNode(record.id));
		}

		let parent_kind = parent_type(scene, &inserted, record.parent)?;
		check_accepts(scene, parent_kind, record.kind)?;
		let kind = scene.schema().spec(record.kind).kind();
		check_props(record.id, &record.props, kind)?;
		texts.push(build_text(scene, record, kind)?);
		inserted.insert(record.id, record.kind);
	}

	Ok(texts)
}

fn parent_type(
	scene: &Scene,
	inserted: &FxHashMap<NodeId, NodeTypeId>,
	parent: NodeId,
) -> Result<Option<NodeTypeId>, SceneError> {
	if parent.is_root() {
		return Ok(None);
	}

	inserted
		.get(&parent)
		.copied()
		.or_else(|| scene.node(parent).map(Node::kind))
		.map(Some)
		.ok_or(SceneError::UnknownNode(parent))
}

fn check_accepts(
	scene: &Scene,
	parent: Option<NodeTypeId>,
	child: NodeTypeId,
) -> Result<(), SceneError> {
	let schema = scene.schema();

	if schema.accepts(parent, child) {
		return Ok(());
	}

	Err(SceneError::NotAccepted {
		parent: parent.map_or_else(
			|| "the root".to_owned(),
			|kind| format!("a `{}`", schema.spec(kind).name()),
		),
		child: schema.spec(child).name().to_owned(),
	})
}

fn build_text(
	scene: &Scene,
	record: &NodeRecord,
	kind: NodeKind,
) -> Result<Option<Document>, SceneError> {
	if kind != NodeKind::Text {
		return match record.text {
			Some(_) => Err(SceneError::NotText(record.id)),
			None => Ok(None),
		};
	}

	let schema = scene
		.schema()
		.text()
		.ok_or(SceneError::NotText(record.id))?;
	text_document(schema, scene.client(), record).map(Some)
}

fn text_document(
	schema: &Arc<Schema>,
	client: u32,
	record: &NodeRecord,
) -> Result<Document, SceneError> {
	let Some(blocks) = &record.text else {
		return Ok(Document::new(schema.clone(), client));
	};

	let mut doc = Document::empty(schema.clone(), client);
	doc.apply_steps([canvist_model::Step::InsertBlocks {
		parent: None,
		index: 0,
		blocks: blocks.clone(),
	}])
	.map_err(|source| {
		SceneError::Text {
			node: record.id,
			source,
		}
	})?;

	Ok(doc)
}

pub(super) fn remove_nodes(scene: &mut Scene, ids: &[NodeId]) -> Result<Applied, SceneError> {
	for &id in ids {
		check_editable(scene, id)?;
	}

	let roots = scene.roots_of(ids);
	let records = roots
		.iter()
		.flat_map(|&root| scene.subtree(root))
		.map(|id| record_of(scene, id))
		.collect::<Result<Vec<_>, _>>()?;
	let mut touched = Vec::new();

	for &root in &roots {
		push_parent(&mut touched, scene.nodes[&root].parent);
		scene.unlink(root);
	}

	for record in &records {
		scene.nodes.remove(&record.id);
		scene.children.remove(&record.id);
		scene.texts.remove(&record.id);
	}

	Ok(Applied {
		inverse: Step::InsertNodes { nodes: records },
		touched,
		structural: !roots.is_empty(),
	})
}

/// A record that re-inserts `id` exactly as it is.
fn record_of(scene: &Scene, id: NodeId) -> Result<NodeRecord, SceneError> {
	let node = scene.node(id).ok_or(SceneError::UnknownNode(id))?;
	let text = match scene.texts.get(&id) {
		None => None,
		Some(TextSlot::Lent) => return Err(SceneError::TextLent(id)),
		Some(TextSlot::Owned(doc)) => {
			Some(
				doc.children(None)
					.iter()
					.filter_map(|&block| doc.tree(block))
					.collect(),
			)
		}
	};

	Ok(NodeRecord {
		id,
		kind: node.kind,
		parent: node.parent,
		key: node.key.clone(),
		props: node.props.clone(),
		text,
	})
}

pub(super) fn move_node(
	scene: &mut Scene,
	id: NodeId,
	parent: NodeId,
	key: FracKey,
) -> Result<Applied, SceneError> {
	check_editable(scene, id)?;

	if !parent.is_root() && !scene.contains(parent) {
		return Err(SceneError::UnknownNode(parent));
	}

	if parent == id || scene.is_ancestor(id, parent) {
		return Err(SceneError::Cycle(id));
	}

	let kind = scene.nodes[&id].kind;
	let parent_kind = scene.node(parent).map(Node::kind);
	check_accepts(scene, parent_kind, kind)?;

	scene.unlink(id);
	let node = scene
		.nodes
		.get_mut(&id)
		.ok_or(SceneError::UnknownNode(id))?;
	let old_parent = std::mem::replace(&mut node.parent, parent);
	let old_key = std::mem::replace(&mut node.key, key);
	scene.link(id);

	let mut touched = vec![id];
	push_parent(&mut touched, old_parent);
	push_parent(&mut touched, parent);

	Ok(Applied {
		inverse: Step::Move {
			node: id,
			parent: old_parent,
			key: old_key,
		},
		touched,
		structural: true,
	})
}

pub(super) fn set_prop(scene: &mut Scene, id: NodeId, prop: Prop) -> Result<Applied, SceneError> {
	check_editable(scene, id)?;
	let kind = scene.kind(id).ok_or(SceneError::UnknownNode(id))?;
	check_prop(id, &prop, kind)?;
	let node = scene
		.nodes
		.get_mut(&id)
		.ok_or(SceneError::UnknownNode(id))?;
	let previous = node.props.set(prop);

	Ok(Applied {
		inverse: Step::SetProp {
			node: id,
			prop: previous,
		},
		touched: vec![id],
		structural: false,
	})
}

pub(super) fn text(
	scene: &mut Scene,
	id: NodeId,
	step: canvist_model::Step,
) -> Result<Applied, SceneError> {
	check_editable(scene, id)?;
	let doc = match scene.texts.get_mut(&id) {
		Some(TextSlot::Owned(doc)) => doc,
		Some(TextSlot::Lent) => return Err(SceneError::TextLent(id)),
		None => return Err(SceneError::NotText(id)),
	};
	let applied = step
		.apply(doc)
		.map_err(|source| SceneError::Text { node: id, source })?;

	Ok(Applied {
		inverse: Step::Text {
			node: id,
			step: applied.inverse,
		},
		touched: vec![id],
		structural: false,
	})
}

fn check_editable(scene: &Scene, id: NodeId) -> Result<(), SceneError> {
	if id.is_root() {
		return Err(SceneError::Root);
	}

	if !scene.contains(id) {
		return Err(SceneError::UnknownNode(id));
	}

	Ok(())
}

fn push_parent(touched: &mut Vec<NodeId>, parent: NodeId) {
	if !parent.is_root() && !touched.contains(&parent) {
		touched.push(parent);
	}
}

/// Check a whole set of props against the node's kind.
fn check_props(node: NodeId, props: &Props, kind: NodeKind) -> Result<(), SceneError> {
	match props.invalid_key(kind) {
		None => Ok(()),
		Some(key) => {
			Err(SceneError::InvalidProp {
				node,
				key: key.to_string(),
				reason: "the value is out of range or does not fit the node's type",
			})
		}
	}
}

/// Check one property against the node's kind.
fn check_prop(node: NodeId, prop: &Prop, kind: NodeKind) -> Result<(), SceneError> {
	if !prop.is_valid() {
		return Err(invalid(node, prop, "the value is out of range"));
	}

	match prop {
		Prop::Shape(shape) if !shape.fits(kind) => {
			Err(invalid(
				node,
				prop,
				"the shape does not fit the node's type",
			))
		}
		Prop::Override(..) | Prop::ClearOverride(..) if kind != NodeKind::Instance => {
			Err(invalid(node, prop, "only instances have overrides"))
		}
		_ => Ok(()),
	}
}

fn invalid(node: NodeId, prop: &Prop, reason: &'static str) -> SceneError {
	SceneError::InvalidProp {
		node,
		key: prop.key().to_string(),
		reason,
	}
}
