//! Machine-readable descriptions of what extensions add: their blocks,
//! marks, and embeds with attribute schemas, their commands with typed
//! arguments, and their queries with typed results.
//!
//! Bindings for other languages generate their types from this metadata,
//! so a TypeScript editor built with Bold offers `toggleBold()` and one
//! without it does not. Command and query metadata comes from the same
//! [`CommandSpec`](crate::CommandSpec) and [`QuerySpec`](crate::QuerySpec)
//! that run them, so it cannot drift from the real behaviour.

use canvist_model::Value;
use serde_json::Value as Json;
use serde_json::json;

/// The type of an argument, attribute, or query result.
#[derive(Clone, Debug, PartialEq)]
pub enum ValueType {
	/// `true` or `false`.
	Bool,
	/// Any integer.
	Int,
	/// An integer from `min` to `max`, inclusive, such as a heading level.
	IntRange {
		/// The smallest allowed value.
		min: i64,
		/// The largest allowed value.
		max: i64,
	},
	/// Any number.
	Float,
	/// Any string.
	String,
	/// One of a fixed set of strings.
	OneOf(Vec<&'static str>),
	/// The inner type or null.
	Nullable(Box<ValueType>),
}

impl ValueType {
	/// The inner type or null.
	#[must_use]
	pub fn nullable(inner: Self) -> Self {
		Self::Nullable(Box::new(inner))
	}

	/// Whether `value` has this type.
	#[must_use]
	pub fn accepts(&self, value: &Value) -> bool {
		match (self, value) {
			(Self::Nullable(_), Value::Null)
			| (Self::Bool, Value::Bool(_))
			| (Self::Int, Value::Int(_))
			| (Self::Float, Value::Float(_) | Value::Int(_))
			| (Self::String, Value::String(_)) => true,
			(Self::IntRange { min, max }, Value::Int(number)) => (min..=max).contains(&number),
			(Self::OneOf(options), Value::String(text)) => options.contains(&&**text),
			(Self::Nullable(inner), value) => inner.accepts(value),
			_ => false,
		}
	}

	/// A valid example value, used by tests that exercise every command.
	#[must_use]
	pub fn sample(&self) -> Value {
		match self {
			Self::Bool => Value::Bool(true),
			Self::Int => Value::Int(1),
			Self::IntRange { min, .. } => Value::Int(*min),
			Self::Float => Value::Float(1.0),
			Self::String => Value::from("sample"),
			Self::OneOf(options) => {
				options
					.first()
					.map_or(Value::Null, |&option| Value::from(option))
			}
			Self::Nullable(inner) => inner.sample(),
		}
	}

	/// The type as JSON, e.g. `{"type":"int","min":1,"max":6}`.
	#[must_use]
	pub fn to_json(&self) -> Json {
		match self {
			Self::Bool => json!({ "type": "bool" }),
			Self::Int => json!({ "type": "int" }),
			Self::IntRange { min, max } => json!({ "type": "int", "min": min, "max": max }),
			Self::Float => json!({ "type": "float" }),
			Self::String => json!({ "type": "string" }),
			Self::OneOf(options) => json!({ "type": "string", "enum": options }),
			Self::Nullable(inner) => json!({ "type": "nullable", "of": inner.to_json() }),
		}
	}
}

/// An attribute value as JSON.
#[must_use]
pub fn value_to_json(value: &Value) -> Json {
	match value {
		Value::Null => Json::Null,
		Value::Bool(flag) => Json::Bool(*flag),
		Value::Int(number) => json!(number),
		Value::Float(number) => json!(number),
		Value::String(text) => Json::String(text.to_string()),
		Value::Array(items) => Json::Array(items.iter().map(value_to_json).collect()),
	}
}

/// A named, typed value: a command argument or a node attribute.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
	/// The name.
	pub name: &'static str,
	/// The type.
	pub ty: ValueType,
	/// The value used when none is given; `None` makes an argument
	/// required.
	pub default: Option<Value>,
	/// What it means.
	pub doc: &'static str,
}

/// `text`, or `""` when built with `--cfg canvist_omit_docs`.
///
/// Descriptions are only read from metadata, which bindings generate their
/// types from. A build that never reads metadata (the browser editor's
/// WebAssembly module) drops them, and their bytes with them.
#[must_use]
pub(crate) const fn doc(text: &'static str) -> &'static str {
	if cfg!(canvist_omit_docs) { "" } else { text }
}

impl Field {
	/// A required argument.
	#[must_use]
	pub const fn required(name: &'static str, ty: ValueType, doc: &'static str) -> Self {
		Self {
			name,
			ty,
			default: None,
			doc: self::doc(doc),
		}
	}

	/// An optional argument or an attribute, with its default.
	#[must_use]
	pub fn optional(
		name: &'static str,
		ty: ValueType,
		default: impl Into<Value>,
		doc: &'static str,
	) -> Self {
		Self {
			name,
			ty,
			default: Some(default.into()),
			doc: self::doc(doc),
		}
	}

	fn to_json(&self) -> Json {
		let mut field = json!({ "name": self.name, "type": self.ty.to_json(), "doc": self.doc });

		if let (Some(default), Some(object)) = (&self.default, field.as_object_mut()) {
			object.insert("default".to_owned(), value_to_json(default));
		}

		field
	}
}

/// What a schema item is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeKind {
	/// A block type.
	Block,
	/// A mark type.
	Mark,
	/// An inline embed type.
	Embed,
}

/// A block, mark, or embed an extension registers, with its attributes.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeMeta {
	/// Block, mark, or embed.
	pub kind: NodeKind,
	/// The type name, e.g. `heading`.
	pub name: &'static str,
	/// The attributes, with their types and defaults.
	pub attrs: Vec<Field>,
}

impl NodeMeta {
	/// A schema item with no attributes.
	#[must_use]
	pub const fn new(kind: NodeKind, name: &'static str) -> Self {
		Self {
			kind,
			name,
			attrs: Vec::new(),
		}
	}

	/// Add an attribute.
	#[must_use]
	pub fn attr(mut self, attr: Field) -> Self {
		self.attrs.push(attr);
		self
	}

	fn to_json(&self) -> Json {
		let kind = match self.kind {
			NodeKind::Block => "block",
			NodeKind::Mark => "mark",
			NodeKind::Embed => "embed",
		};

		let attrs: Vec<Json> = self.attrs.iter().map(Field::to_json).collect();
		json!({ "kind": kind, "name": self.name, "attrs": attrs })
	}
}

/// A command: its name, what it does, and its arguments. Commands return
/// an update or nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandMeta {
	/// The name, e.g. `toggleHeading`.
	pub name: &'static str,
	/// What it does.
	pub doc: &'static str,
	/// Its arguments.
	pub args: Vec<Field>,
}

/// A query: its name, what it answers, its arguments, and its result type.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryMeta {
	/// The name, e.g. `headingLevel`.
	pub name: &'static str,
	/// What it answers.
	pub doc: &'static str,
	/// Its arguments.
	pub args: Vec<Field>,
	/// The type of its answer.
	pub returns: ValueType,
}

/// Everything one extension adds.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtensionMeta {
	/// The extension's name.
	pub name: &'static str,
	/// Its blocks, marks, and embeds.
	pub nodes: Vec<NodeMeta>,
	/// Its commands.
	pub commands: Vec<CommandMeta>,
	/// Its queries.
	pub queries: Vec<QueryMeta>,
}

impl ExtensionMeta {
	/// The description as JSON, the input for binding generators.
	#[must_use]
	pub fn to_json(&self) -> Json {
		let nodes: Vec<Json> = self.nodes.iter().map(NodeMeta::to_json).collect();
		let commands: Vec<Json> = self
			.commands
			.iter()
			.map(|command| {
				let args: Vec<Json> = command.args.iter().map(Field::to_json).collect();
				json!({ "name": command.name, "doc": command.doc, "args": args })
			})
			.collect();
		let queries: Vec<Json> = self
			.queries
			.iter()
			.map(|query| {
				let args: Vec<Json> = query.args.iter().map(Field::to_json).collect();
				json!({
					"name": query.name,
					"doc": query.doc,
					"args": args,
					"returns": query.returns.to_json(),
				})
			})
			.collect();

		json!({ "name": self.name, "nodes": nodes, "commands": commands, "queries": queries })
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn checks_values_against_types() {
		let level = ValueType::IntRange { min: 1, max: 6 };
		let language = ValueType::nullable(ValueType::String);
		let align = ValueType::OneOf(vec!["left", "right"]);

		assert!(level.accepts(&Value::Int(3)));
		assert!(!level.accepts(&Value::Int(7)));
		assert!(!level.accepts(&Value::from("3")));
		assert!(language.accepts(&Value::Null));
		assert!(language.accepts(&Value::from("rust")));
		assert!(align.accepts(&Value::from("left")));
		assert!(!align.accepts(&Value::from("middle")));
		assert!(ValueType::Float.accepts(&Value::Int(2)));
	}

	#[test]
	fn samples_are_accepted() {
		let types = [
			ValueType::Bool,
			ValueType::Int,
			ValueType::IntRange { min: 2, max: 4 },
			ValueType::Float,
			ValueType::String,
			ValueType::OneOf(vec!["a"]),
			ValueType::nullable(ValueType::Int),
		];

		for ty in types {
			assert!(ty.accepts(&ty.sample()), "{ty:?}");
		}
	}
}
