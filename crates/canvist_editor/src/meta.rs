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
	/// A list of values of the inner type, such as the headings of a table
	/// of contents.
	List(Box<ValueType>),
	/// A map with these fields, such as an active suggestion's trigger and
	/// query. Bindings generate a typed record from it.
	Record(Vec<Field>),
}

impl ValueType {
	/// The inner type or null.
	#[must_use]
	pub fn nullable(inner: Self) -> Self {
		Self::Nullable(Box::new(inner))
	}

	/// A list of values of the inner type.
	#[must_use]
	pub fn list(inner: Self) -> Self {
		Self::List(Box::new(inner))
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
			(Self::List(inner), Value::Array(items)) => {
				items.iter().all(|item| inner.accepts(item))
			}
			(Self::Record(fields), Value::Map(map)) => accepts_record(fields, map),
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
			// Two items, so lists of pairs (such as an ink stroke's points)
			// get a valid sample too.
			Self::List(inner) => Value::from(vec![inner.sample(), inner.sample()]),
			Self::Record(fields) => {
				let map: canvist_model::Attrs = fields
					.iter()
					.map(|field| (field.name, field.ty.sample()))
					.collect();
				Value::from(map)
			}
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
			Self::List(inner) => json!({ "type": "list", "of": inner.to_json() }),
			Self::Record(fields) => {
				let fields: Vec<Json> = fields.iter().map(Field::to_json).collect();
				json!({ "type": "record", "fields": fields })
			}
		}
	}
}

/// A short description for error messages, such as `an integer from 1 to
/// 6`. Written by hand rather than derived: `Debug` would format the
/// floats in fields' defaults, which pulls float formatting into every
/// WebAssembly build.
impl std::fmt::Display for ValueType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Bool => f.write_str("a boolean"),
			Self::Int => f.write_str("an integer"),
			Self::IntRange { min, max } => write!(f, "an integer from {min} to {max}"),
			Self::Float => f.write_str("a number"),
			Self::String => f.write_str("a string"),
			Self::OneOf(options) => write!(f, "one of {}", options.join(", ")),
			Self::Nullable(inner) => write!(f, "{inner} or null"),
			Self::List(inner) => write!(f, "a list, each item {inner}"),
			Self::Record(fields) => {
				let names: Vec<&str> = fields.iter().map(|field| field.name).collect();
				write!(f, "a record of {}", names.join(", "))
			}
		}
	}
}

/// Whether `map` has every field of a record, each of its type, and no
/// others. A field with a default may be missing.
fn accepts_record(fields: &[Field], map: &canvist_model::Attrs) -> bool {
	let known = map
		.iter()
		.all(|(key, _)| fields.iter().any(|field| field.name == key));

	known
		&& fields.iter().all(|field| {
			match map.get(field.name) {
				Some(value) => field.ty.accepts(value),
				None => field.default.is_some(),
			}
		})
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
		Value::Map(map) => {
			// Inserting one key at a time, rather than collecting, keeps the
			// stable sort a collected `BTreeMap` needs out of every binary.
			let mut object = serde_json::Map::new();

			for (key, value) in map.iter() {
				object.insert(key.to_owned(), value_to_json(value));
			}

			Json::Object(object)
		}
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
	fn describes_types_for_error_messages() {
		let level = ValueType::nullable(ValueType::IntRange { min: 1, max: 6 });
		let tags = ValueType::list(ValueType::OneOf(vec!["a", "b"]));

		assert_eq!(level.to_string(), "an integer from 1 to 6 or null");
		assert_eq!(tags.to_string(), "a list, each item one of a, b");
	}

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

		let names = ValueType::list(ValueType::String);
		assert!(names.accepts(&Value::from(vec![Value::from("a")])));
		assert!(!names.accepts(&Value::from(vec![Value::Int(1)])));

		let record = ValueType::Record(vec![
			Field::required("query", ValueType::String, "The text."),
			Field::optional("count", ValueType::Int, 0, "How many."),
		]);
		let query = canvist_model::Attrs::new().with("query", "ad");
		assert!(
			record.accepts(&Value::from(query.clone())),
			"defaults may be left out"
		);
		assert!(!record.accepts(&Value::from(query.with("extra", 1))));
		assert!(!record.accepts(&Value::from(canvist_model::Attrs::new())));
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
			ValueType::list(ValueType::String),
			ValueType::Record(vec![
				Field::required("query", ValueType::String, "The text."),
				Field::optional("count", ValueType::Int, 0, "How many."),
			]),
		];

		for ty in types {
			assert!(ty.accepts(&ty.sample()), "{ty:?}");
		}
	}
}
