//! ProseMirror-compatible JSON.
//!
//! canvist reads and writes the JSON shape used by ProseMirror and TipTap
//! (`{ "type": "doc", "content": [...] }`), so existing content migrates
//! without conversion. Block ids are stored in `attrs.id`, matching TipTap's
//! `UniqueID` extension. Blocks without an id, or whose id is not a canvist
//! id (such as TipTap's UUIDs), get a fresh id on import.

use std::sync::Arc;

use serde_json::Map;
use serde_json::Value as Json;
use serde_json::json;

use crate::Attrs;
use crate::BlockId;
use crate::BlockTree;
use crate::Document;
use crate::Embed;
use crate::InlineText;
use crate::Mark;
use crate::MarkSet;
use crate::Schema;
use crate::StepError;
use crate::TreeContent;
use crate::Value;
use crate::schema::ContentKind;

/// The attribute key that stores a block's id.
const ID_KEY: &str = "id";

/// Errors raised while importing JSON.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum JsonError {
	/// A node was not shaped as expected.
	#[error("expected {expected} in {context}")]
	Shape {
		/// What was expected.
		expected: &'static str,
		/// Where it was expected.
		context: String,
	},
	/// A node, mark, or embed type is not in the schema.
	#[error("unknown {kind} type `{name}`")]
	UnknownType {
		/// `block`, `mark`, or `inline`.
		kind: &'static str,
		/// The unknown name.
		name: String,
	},
	/// The content violated the schema.
	#[error(transparent)]
	Step(#[from] StepError),
}

fn shape(expected: &'static str, context: &str) -> JsonError {
	JsonError::Shape {
		expected,
		context: context.to_owned(),
	}
}

impl Document {
	/// Serialize to ProseMirror-compatible JSON.
	#[must_use]
	pub fn to_json(&self) -> Json {
		let content: Vec<Json> = self
			.children(None)
			.iter()
			.filter_map(|&id| self.tree(id))
			.map(|tree| tree_to_json(self.schema(), &tree))
			.collect();
		json!({ "type": "doc", "content": content })
	}

	/// Parse ProseMirror-compatible JSON, validating it against `schema`.
	pub fn from_json(schema: Arc<Schema>, client: u32, json: &Json) -> Result<Self, JsonError> {
		let mut doc = Self::empty(schema, client);

		if json.get("type").and_then(Json::as_str) != Some("doc") {
			return Err(shape("a `doc` node", "the root"));
		}

		let nodes = content_of(json);
		observe_ids(&mut doc, nodes);
		let blocks = nodes
			.iter()
			.map(|node| tree_from_json(&mut doc, node))
			.collect::<Result<Vec<_>, _>>()?;
		doc.apply_steps([crate::Step::InsertBlocks {
			parent: None,
			index: 0,
			blocks,
		}])?;
		Ok(doc)
	}
}

fn content_of(node: &Json) -> &[Json] {
	node.get("content")
		.and_then(Json::as_array)
		.map_or(&[], Vec::as_slice)
}

fn tree_to_json(schema: &Schema, tree: &BlockTree) -> Json {
	let mut attrs = attrs_to_json(&tree.attrs);
	attrs.insert(ID_KEY.to_owned(), Json::String(tree.id.to_string()));
	let mut node = Map::new();
	node.insert(
		"type".to_owned(),
		Json::String(schema.block(tree.kind).name.to_string()),
	);
	node.insert("attrs".to_owned(), Json::Object(attrs));

	let content = match &tree.content {
		TreeContent::Inline(text) => inline_to_json(schema, text),
		TreeContent::Blocks(children) => {
			children
				.iter()
				.map(|child| tree_to_json(schema, child))
				.collect()
		}
		TreeContent::Leaf => Vec::new(),
	};

	if !content.is_empty() {
		node.insert("content".to_owned(), Json::Array(content));
	}

	Json::Object(node)
}

fn inline_to_json(schema: &Schema, text: &InlineText) -> Vec<Json> {
	text.runs()
		.map(|run| {
			let mut node = match run.embed {
				Some(embed) => {
					json!({
						"type": schema.embed(embed.kind).name.as_ref(),
						"attrs": attrs_to_json(&embed.attrs),
					})
				}
				None => json!({ "type": "text", "text": run.text }),
			};
			if !run.marks.is_empty() {
				node["marks"] = marks_to_json(schema, run.marks);
			}

			node
		})
		.collect()
}

fn marks_to_json(schema: &Schema, marks: &MarkSet) -> Json {
	let marks = marks.as_slice().iter().map(|mark| {
		let mut node = json!({ "type": schema.mark(mark.kind).name.as_ref() });
		if !mark.attrs.is_empty() {
			node["attrs"] = Json::Object(attrs_to_json(&mark.attrs));
		}

		node
	});
	Json::Array(marks.collect())
}

fn attrs_to_json(attrs: &Attrs) -> Map<String, Json> {
	attrs
		.iter()
		.map(|(key, value)| (key.to_owned(), value_to_json(value)))
		.collect()
}

fn value_to_json(value: &Value) -> Json {
	match value {
		Value::Null => Json::Null,
		Value::Bool(value) => Json::Bool(*value),
		Value::Int(value) => Json::from(*value),
		Value::Float(value) => Json::from(*value),
		Value::String(value) => Json::String(value.to_string()),
		Value::Array(values) => Json::Array(values.iter().map(value_to_json).collect()),
	}
}

/// Reserve every explicit block id before allocating fresh ones, so fresh ids
/// never collide with ids that appear later in the document.
fn observe_ids(doc: &mut Document, nodes: &[Json]) {
	for node in nodes {
		let name = node.get("type").and_then(Json::as_str).unwrap_or_default();
		let Some(kind) = doc.schema().block_type(name) else {
			continue;
		};

		if let Some(id) = explicit_id(node) {
			doc.observe_id(id);
		}

		if matches!(doc.schema().block(kind).content, ContentKind::Blocks(_)) {
			observe_ids(doc, content_of(node));
		}
	}
}

fn explicit_id(node: &Json) -> Option<BlockId> {
	node.pointer("/attrs/id")
		.and_then(Json::as_str)?
		.parse()
		.ok()
}

fn tree_from_json(doc: &mut Document, node: &Json) -> Result<BlockTree, JsonError> {
	let schema = doc.schema().clone();
	let name = node
		.get("type")
		.and_then(Json::as_str)
		.ok_or_else(|| shape("a `type`", "a block"))?;
	let kind = schema.block_type(name).ok_or_else(|| {
		JsonError::UnknownType {
			kind: "block",
			name: name.to_owned(),
		}
	})?;

	let id = match explicit_id(node) {
		Some(id) => id,
		None => doc.allocate_id()?,
	};

	let mut attrs = attrs_from_json(node.get("attrs"));
	attrs.remove(ID_KEY);

	let content = match &schema.block(kind).content {
		ContentKind::Inline => TreeContent::Inline(inline_from_json(&schema, content_of(node))?),
		ContentKind::Blocks(_) => {
			let children = content_of(node)
				.iter()
				.map(|child| tree_from_json(doc, child));
			TreeContent::Blocks(children.collect::<Result<_, _>>()?)
		}
		ContentKind::Leaf => TreeContent::Leaf,
	};

	Ok(BlockTree {
		id,
		kind,
		attrs,
		content,
	})
}

fn inline_from_json(schema: &Schema, nodes: &[Json]) -> Result<InlineText, JsonError> {
	let mut text = InlineText::new();

	for node in nodes {
		let marks = marks_from_json(schema, node.get("marks"))?;
		let name = node
			.get("type")
			.and_then(Json::as_str)
			.ok_or_else(|| shape("a `type`", "inline content"))?;
		let piece = if name == "text" {
			let value = node
				.get("text")
				.and_then(Json::as_str)
				.ok_or_else(|| shape("a `text` string", "a text node"))?;
			InlineText::plain(value, marks)
		} else {
			let kind = schema.embed_type(name).ok_or_else(|| {
				JsonError::UnknownType {
					kind: "inline",
					name: name.to_owned(),
				}
			})?;

			let attrs = attrs_from_json(node.get("attrs"));
			InlineText::embed(Embed { kind, attrs }, marks)
		};

		text.append(piece);
	}

	Ok(text)
}

fn marks_from_json(schema: &Schema, marks: Option<&Json>) -> Result<MarkSet, JsonError> {
	let Some(marks) = marks.and_then(Json::as_array) else {
		return Ok(MarkSet::empty());
	};
	let marks = marks.iter().map(|mark| {
		let name = mark
			.get("type")
			.and_then(Json::as_str)
			.ok_or_else(|| shape("a `type`", "a mark"))?;
		let kind = schema.mark_type(name).ok_or_else(|| {
			JsonError::UnknownType {
				kind: "mark",
				name: name.to_owned(),
			}
		})?;
		Ok(Mark::with_attrs(kind, attrs_from_json(mark.get("attrs"))))
	});
	Ok(MarkSet::from_marks(
		marks.collect::<Result<Vec<_>, JsonError>>()?,
	))
}

fn attrs_from_json(attrs: Option<&Json>) -> Attrs {
	let Some(attrs) = attrs.and_then(Json::as_object) else {
		return Attrs::new();
	};
	attrs
		.iter()
		.map(|(key, value)| (key, value_from_json(value)))
		.collect()
}

/// Convert JSON to an attribute value. Nested objects are not representable
/// yet and become `Null`; none of TipTap's built-in extensions use them.
fn value_from_json(value: &Json) -> Value {
	match value {
		Json::Bool(value) => Value::Bool(*value),
		Json::Number(number) => {
			number.as_i64().map_or_else(
				|| Value::Float(number.as_f64().unwrap_or_default()),
				Value::Int,
			)
		}
		Json::String(value) => Value::from(value.as_str()),
		Json::Array(values) => Value::Array(values.iter().map(value_from_json).collect()),
		Json::Null | Json::Object(_) => Value::Null,
	}
}
