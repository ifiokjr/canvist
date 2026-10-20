//! Scene JSON: a flat list of nodes in paint order.
//!
//! Writing produces the canonical form: every node with its id, type,
//! parent, key, the props that differ from their defaults, and a text
//! frame's document in ProseMirror JSON. Reading also accepts a friendlier
//! form for people and agents: `id` and `key` may be omitted (ids are
//! allocated, keys follow list order), and nodes may nest their children in
//! a `children` array instead of naming a `parent`.

use std::sync::Arc;

use canvist_model::Document;
use rustc_hash::FxHashMap;
use serde_json::Map;
use serde_json::Value as Json;
use serde_json::json;

use crate::FracKey;
use crate::NodeId;
use crate::NodeKind;
use crate::NodeRecord;
use crate::Prop;
use crate::PropKey;
use crate::Props;
use crate::Scene;
use crate::SceneError;
use crate::SceneSchema;
use crate::Shape;
use crate::Step;

/// The `type` of a scene JSON document.
pub const SCENE_TYPE: &str = "canvist-scene";
/// The format version this crate writes.
pub const SCENE_VERSION: u64 = 1;

impl Scene {
	/// Serialize to canonical scene JSON.
	#[must_use]
	pub fn to_json(&self) -> Json {
		let mut defaults = FxHashMap::default();
		let nodes: Vec<Json> = self
			.subtree(NodeId::ROOT)
			.into_iter()
			.filter_map(|id| self.write_node(id, &mut defaults))
			.collect();

		json!({ "type": SCENE_TYPE, "version": SCENE_VERSION, "nodes": nodes })
	}

	/// The JSON of one node, without its children.
	#[must_use]
	pub fn node_json(&self, id: NodeId) -> Option<Json> {
		self.write_node(id, &mut FxHashMap::default())
	}

	/// One node's JSON, reusing each kind's default props across calls.
	fn write_node(&self, id: NodeId, defaults: &mut FxHashMap<NodeKind, Props>) -> Option<Json> {
		let node = self.node(id)?;
		let kind = self.schema().spec(node.kind).kind();
		let defaults = defaults
			.entry(kind)
			.or_insert_with(|| Props::for_kind(kind));
		let mut out = Map::new();
		out.insert("id".into(), json!(id.to_string()));
		out.insert("type".into(), json!(self.schema().spec(node.kind).name()));
		out.insert("parent".into(), json!(node.parent.to_string()));
		out.insert("key".into(), json!(node.key.as_str()));
		out.insert("props".into(), props_json(&node.props, defaults));

		if let Some(doc) = self.text(id) {
			out.insert("text".into(), doc.to_json());
		}

		Some(Json::Object(out))
	}

	/// Parse scene JSON, validating it against `schema`. `client` seeds id
	/// allocation for nodes without ids and for later edits.
	pub fn from_json(
		schema: Arc<SceneSchema>,
		client: u32,
		json: &Json,
	) -> Result<Self, SceneError> {
		if json.get("type").and_then(Json::as_str) != Some(SCENE_TYPE) {
			return Err(SceneError::json(
				"type",
				format!("expected \"{SCENE_TYPE}\""),
			));
		}

		let nodes = json
			.get("nodes")
			.and_then(Json::as_array)
			.ok_or_else(|| SceneError::json("nodes", "expected an array"))?;
		let mut scene = Self::empty(schema, client);
		let records = Reader::new(&mut scene).read(nodes)?;
		scene.apply_steps([Step::InsertNodes { nodes: records }])?;
		Ok(scene)
	}
}

/// Every prop that differs from its default for a node of `kind`, keyed by
/// name. Reading the result with [`Props::for_kind`] as the base restores
/// `props` exactly.
#[must_use]
pub fn props_to_json(props: &Props, kind: NodeKind) -> Json {
	props_json(props, &Props::for_kind(kind))
}

fn props_json(props: &Props, defaults: &Props) -> Json {
	let map = props
		.changed_from(defaults)
		.into_iter()
		.map(|prop| (prop.key().to_string(), prop.to_json()))
		.collect();

	Json::Object(map)
}

/// A flattened node before ids, parents, and keys are resolved.
struct Pending<'json> {
	path: String,
	node: &'json Map<String, Json>,
	id: Option<NodeId>,
	parent: Option<usize>,
}

struct Reader<'scene> {
	scene: &'scene mut Scene,
}

impl<'scene> Reader<'scene> {
	fn new(scene: &'scene mut Scene) -> Self {
		Self { scene }
	}

	/// Flatten nested nodes, reserve explicit ids before allocating the rest,
	/// resolve each parent (nesting or `parent`) and key (given, or after the
	/// previous sibling in list order), then order the records parents first.
	fn read(self, nodes: &[Json]) -> Result<Vec<NodeRecord>, SceneError> {
		let mut pending = Vec::new();
		flatten(nodes, "nodes", None, &mut pending)?;

		for item in &mut pending {
			item.id = optional_id(item.node, &item.path)?;

			if let Some(id) = item.id {
				self.scene.observe_id(id);
			}
		}

		let ids = pending
			.iter()
			.map(|item| item.id.map_or_else(|| self.scene.allocate_id(), Ok))
			.collect::<Result<Vec<_>, _>>()?;
		let mut records = Vec::with_capacity(pending.len());
		let mut last_keys: FxHashMap<NodeId, FracKey> = FxHashMap::default();

		for (item, &id) in pending.iter().zip(&ids) {
			let parent = match item.parent {
				Some(index) => ids[index],
				None => parent_of(item.node, &item.path)?,
			};
			let key = key_of(item.node, &item.path, last_keys.get(&parent))?;
			last_keys.insert(parent, key.clone());
			records.push(self.record(item, id, parent, key)?);
		}

		order_parents_first(records)
	}

	fn record(
		&self,
		item: &Pending<'_>,
		id: NodeId,
		parent: NodeId,
		key: FracKey,
	) -> Result<NodeRecord, SceneError> {
		let schema = self.scene.schema();
		let type_name = item
			.node
			.get("type")
			.and_then(Json::as_str)
			.ok_or_else(|| SceneError::json(format!("{}.type", item.path), "expected a string"))?;
		let kind = schema
			.node_type(type_name)
			.ok_or_else(|| SceneError::UnknownType(type_name.to_owned()))?;
		let behaviour = schema.spec(kind).kind();
		let props = props_from_json(item.node.get("props"), behaviour, &item.path)?;
		let text = match item.node.get("text") {
			Some(text) if behaviour == NodeKind::Text => Some(self.text_blocks(id, text)?),
			Some(_) => return Err(SceneError::NotText(id)),
			None => None,
		};

		Ok(NodeRecord {
			id,
			kind,
			parent,
			key,
			props,
			text,
		})
	}

	fn text_blocks(
		&self,
		id: NodeId,
		text: &Json,
	) -> Result<Vec<canvist_model::BlockTree>, SceneError> {
		let schema = self.scene.schema().text().ok_or(SceneError::NotText(id))?;
		let doc = Document::from_json(schema.clone(), self.scene.client(), text)
			.map_err(|source| SceneError::TextJson { node: id, source })?;

		Ok(doc
			.children(None)
			.iter()
			.filter_map(|&block| doc.tree(block))
			.collect())
	}
}

fn flatten<'json>(
	nodes: &'json [Json],
	path: &str,
	parent: Option<usize>,
	out: &mut Vec<Pending<'json>>,
) -> Result<(), SceneError> {
	for (index, node) in nodes.iter().enumerate() {
		let path = format!("{path}[{index}]");
		let map = node
			.as_object()
			.ok_or_else(|| SceneError::json(&path, "expected an object"))?;
		let me = out.len();
		out.push(Pending {
			path: path.clone(),
			node: map,
			id: None,
			parent,
		});

		if let Some(children) = map.get("children") {
			let children = children
				.as_array()
				.ok_or_else(|| SceneError::json(format!("{path}.children"), "expected an array"))?;
			flatten(children, &format!("{path}.children"), Some(me), out)?;
		}
	}

	Ok(())
}

fn optional_id(node: &Map<String, Json>, path: &str) -> Result<Option<NodeId>, SceneError> {
	node.get("id")
		.map(|id| {
			id.as_str()
				.and_then(|id| id.parse::<NodeId>().ok())
				.filter(|id| !id.is_root())
				.ok_or_else(|| {
					SceneError::json(format!("{path}.id"), "expected a `client.seq` node id")
				})
		})
		.transpose()
}

fn parent_of(node: &Map<String, Json>, path: &str) -> Result<NodeId, SceneError> {
	match node.get("parent") {
		Some(parent) => {
			parent
				.as_str()
				.and_then(|parent| parent.parse().ok())
				.ok_or_else(|| {
					SceneError::json(format!("{path}.parent"), "expected a node id or `root`")
				})
		}
		None if node.get("type").and_then(Json::as_str) == Some("page") => Ok(NodeId::ROOT),
		None => {
			Err(SceneError::json(
				format!("{path}.parent"),
				"missing: nest the node in `children` or name its parent",
			))
		}
	}
}

fn key_of(
	node: &Map<String, Json>,
	path: &str,
	previous: Option<&FracKey>,
) -> Result<FracKey, SceneError> {
	match node.get("key") {
		Some(key) => {
			let text = key
				.as_str()
				.ok_or_else(|| SceneError::json(format!("{path}.key"), "expected a string"))?;
			FracKey::parse(text)
				.map_err(|error| SceneError::json(format!("{path}.key"), error.to_string()))
		}
		None => Ok(FracKey::between(previous, None)?),
	}
}

fn props_from_json(props: Option<&Json>, kind: NodeKind, path: &str) -> Result<Props, SceneError> {
	let mut out = Props::for_kind(kind);
	let Some(props) = props else {
		return Ok(out);
	};
	let map = props
		.as_object()
		.ok_or_else(|| SceneError::json(format!("{path}.props"), "expected an object"))?;

	for (name, value) in map {
		let at = || format!("{path}.props.{name}");
		let key = PropKey::parse(name).ok_or_else(|| SceneError::json(at(), "unknown property"))?;
		let prop = Prop::from_json(&key, value)
			.map_err(|error| SceneError::json(at(), error.to_string()))?;
		out.set(prop);
	}

	if out.shape == Shape::None {
		out.shape = Shape::default_for(kind);
	}

	Ok(out)
}

/// Sort records so each comes after its parent, keeping list order among
/// independent records.
fn order_parents_first(records: Vec<NodeRecord>) -> Result<Vec<NodeRecord>, SceneError> {
	let index: FxHashMap<NodeId, usize> = records
		.iter()
		.enumerate()
		.map(|(at, record)| (record.id, at))
		.collect();
	let depth = |start: usize| -> Result<usize, SceneError> {
		let mut depth = 0;
		let mut current = records[start].parent;

		while let Some(&at) = index.get(&current) {
			depth += 1;

			if depth > records.len() {
				return Err(SceneError::Cycle(records[start].id));
			}

			current = records[at].parent;
		}

		Ok(depth)
	};
	let depths = (0..records.len())
		.map(depth)
		.collect::<Result<Vec<_>, _>>()?;
	let mut keyed: Vec<(usize, NodeRecord)> = depths.into_iter().zip(records).collect();
	keyed.sort_by_key(|(depth, _)| *depth);

	Ok(keyed.into_iter().map(|(_, record)| record).collect())
}
