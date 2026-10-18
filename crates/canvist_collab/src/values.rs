//! Conversions between model values and Yjs values.
//!
//! See the [crate docs](crate#the-yjs-schema) for the shapes these produce.

use std::collections::HashMap;
use std::sync::Arc;

use canvist_model::Attrs;
use canvist_model::Embed;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Schema;
use canvist_model::Value;
use yrs::Any;
use yrs::any::Number;

/// Yjs formatting attributes: mark name to mark value.
pub(crate) type Format = yrs::types::Attrs;

/// Convert a model value to a Yjs value. Always lossless.
pub(crate) fn to_any(value: &Value) -> Any {
	match value {
		Value::Null => Any::Null,
		Value::Bool(value) => Any::Bool(*value),
		Value::Int(value) => Any::Number(Number::Int(*value)),
		Value::Float(value) => Any::Number(Number::Float(*value)),
		Value::String(value) => Any::String(value.clone()),
		Value::Array(values) => Any::Array(values.iter().map(to_any).collect()),
	}
}

/// Convert a Yjs value to a model value, or `None` for maps, buffers, and
/// `undefined`, which the model cannot represent.
pub(crate) fn from_any(any: &Any) -> Option<Value> {
	let value = match any {
		Any::Null => Value::Null,
		Any::Bool(value) => Value::Bool(*value),
		Any::Number(Number::Int(value)) => Value::Int(*value),
		Any::Number(Number::Float(value)) => Value::Float(*value),
		Any::String(value) => Value::String(value.clone()),
		Any::Array(values) => {
			let values: Option<Vec<Value>> = values.iter().map(from_any).collect();
			Value::from(values?)
		}
		Any::Undefined | Any::Buffer(_) | Any::Map(_) => return None,
	};

	Some(value)
}

/// Attributes as a Yjs map value.
pub(crate) fn attrs_to_any(attrs: &Attrs) -> Any {
	let entries: HashMap<String, Any> = attrs
		.iter()
		.map(|(key, value)| (key.to_owned(), to_any(value)))
		.collect();
	Any::Map(Arc::new(entries))
}

/// Attributes from Yjs `(key, value)` pairs, skipping unrepresentable values.
pub(crate) fn attrs_from<'a>(entries: impl IntoIterator<Item = (&'a str, &'a Any)>) -> Attrs {
	entries
		.into_iter()
		.filter_map(|(key, any)| Some((key, from_any(any)?)))
		.collect()
}

/// The formatting value of a mark: `true`, or a map of its attributes.
pub(crate) fn mark_value(mark: &Mark) -> Any {
	if mark.attrs.is_empty() {
		Any::Bool(true)
	} else {
		attrs_to_any(&mark.attrs)
	}
}

/// What one formatting attribute says about a mark.
pub(crate) enum MarkState {
	/// The mark is present.
	Set(Mark),
	/// The mark is absent: the value is `null` or `false`.
	Cleared,
}

/// Read one formatting attribute, or `None` for keys that are not marks in
/// `schema` (they are left untouched). A map value supplies the mark's
/// attributes; any other value marks it present without attributes.
pub(crate) fn mark_from(schema: &Schema, key: &str, value: &Any) -> Option<MarkState> {
	let kind = schema.mark_type(key)?;

	let state = match value {
		Any::Null | Any::Undefined | Any::Bool(false) => MarkState::Cleared,
		Any::Map(entries) => {
			let attrs = attrs_from(entries.iter().map(|(key, value)| (key.as_str(), value)));
			MarkState::Set(Mark::with_attrs(kind, attrs))
		}
		_ => MarkState::Set(Mark::new(kind)),
	};

	Some(state)
}

/// The marks described by a complete set of formatting attributes.
pub(crate) fn marks_from(schema: &Schema, format: Option<&Format>) -> MarkSet {
	let Some(format) = format else {
		return MarkSet::empty();
	};

	let marks = format.iter().filter_map(|(key, value)| {
		match mark_from(schema, key, value)? {
			MarkState::Set(mark) => Some(mark),
			MarkState::Cleared => None,
		}
	});
	MarkSet::from_marks(marks)
}

/// `marks` with the formatting changes in `format` applied, exactly as Yjs
/// stores them. Schema exclusions are deliberately not applied: the mirror
/// shows what the shared document holds.
pub(crate) fn apply_format(schema: &Schema, marks: &MarkSet, format: &Format) -> MarkSet {
	let mut result: Vec<Mark> = marks.as_slice().to_vec();

	for (key, value) in format {
		let Some(state) = mark_from(schema, key, value) else {
			continue;
		};
		let kind = schema.mark_type(key);
		result.retain(|mark| Some(mark.kind) != kind);

		if let MarkState::Set(mark) = state {
			result.push(mark);
		}
	}

	MarkSet::from_marks(result)
}

/// The formatting attributes that describe `marks` on inserted content.
pub(crate) fn format_of(schema: &Schema, marks: &MarkSet) -> Format {
	marks
		.as_slice()
		.iter()
		.map(|mark| (mark_name(schema, mark), mark_value(mark)))
		.collect()
}

/// Formatting that sets every mark in `schema` to its state in `marks`.
pub(crate) fn full_format(schema: &Schema, marks: &MarkSet) -> Format {
	schema
		.marks()
		.iter()
		.map(|kind| {
			let value = marks.get(kind.id).map_or(Any::Null, mark_value);
			(kind.name.clone(), value)
		})
		.collect()
}

/// Formatting that adds `mark` and clears the marks it excludes.
pub(crate) fn add_mark_format(schema: &Schema, mark: &Mark) -> Format {
	let spec = schema.mark(mark.kind);
	let mut format: Format = spec
		.excludes
		.iter()
		.filter(|&&excluded| excluded != mark.kind)
		.map(|&excluded| (schema.mark(excluded).name.clone(), Any::Null))
		.collect();
	format.insert(spec.name.clone(), mark_value(mark));
	format
}

fn mark_name(schema: &Schema, mark: &Mark) -> Arc<str> {
	schema.mark(mark.kind).name.clone()
}

/// An embed as a Yjs value: `{ "<embed type>": { ...attrs } }`.
pub(crate) fn embed_to_any(schema: &Schema, embed: &Embed) -> Any {
	let name = schema.embed(embed.kind).name.to_string();
	Any::Map(Arc::new(HashMap::from([(
		name,
		attrs_to_any(&embed.attrs),
	)])))
}

/// Read an embed value, or `None` if it is not a single-key map naming an
/// embed type in `schema`.
pub(crate) fn embed_from(schema: &Schema, any: &Any) -> Option<Embed> {
	let Any::Map(entries) = any else {
		return None;
	};
	let mut entries = entries.iter();
	let (name, value) = entries.next()?;

	if entries.next().is_some() {
		return None;
	}

	let attrs = match value {
		Any::Map(attrs) => attrs_from(attrs.iter().map(|(key, value)| (key.as_str(), value))),
		_ => Attrs::new(),
	};

	Some(Embed {
		kind: schema.embed_type(name)?,
		attrs,
	})
}

#[cfg(test)]
mod tests {
	use canvist_model::BlockSpec;
	use canvist_model::EmbedSpec;
	use canvist_model::MarkSpec;

	use super::*;

	fn schema() -> Schema {
		Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.mark(MarkSpec::new("bold"))
			.mark(MarkSpec::new("code").excludes(&["bold"]))
			.mark(MarkSpec::new("link"))
			.embed(EmbedSpec::new("mention"))
			.build()
			.expect("valid schema")
	}

	#[test]
	fn values_round_trip() {
		let values = [
			Value::Null,
			Value::Bool(true),
			Value::Int(-3),
			Value::Float(2.0),
			Value::from("text"),
			Value::from(vec![Value::Int(1), Value::from("two")]),
		];

		for value in values {
			assert_eq!(from_any(&to_any(&value)), Some(value));
		}

		assert_eq!(from_any(&Any::Undefined), None);
	}

	#[test]
	fn marks_round_trip_through_formatting() {
		let schema = schema();
		let link = schema.mark_type("link").expect("link");
		let bold = schema.mark_type("bold").expect("bold");
		let marks = MarkSet::from_marks([
			Mark::new(bold),
			Mark::with_attrs(link, Attrs::new().with("href", "https://canvist.dev")),
		]);

		let format = format_of(&schema, &marks);

		assert_eq!(marks_from(&schema, Some(&format)), marks);
	}

	#[test]
	fn applying_format_ignores_unknown_keys_and_exclusions() {
		let schema = schema();
		let bold = schema.mark_type("bold").expect("bold");
		let code = schema.mark_type("code").expect("code");
		let marks = MarkSet::from_marks([Mark::new(bold)]);
		let format = Format::from([
			(Arc::from("code"), Any::Bool(true)),
			(Arc::from("comment"), Any::Bool(true)),
		]);

		let applied = apply_format(&schema, &marks, &format);

		assert!(applied.contains(bold), "exclusions are not reapplied");
		assert!(applied.contains(code));
		assert_eq!(applied.as_slice().len(), 2);
	}

	#[test]
	fn adding_a_mark_clears_what_it_excludes() {
		let schema = schema();
		let code = Mark::new(schema.mark_type("code").expect("code"));

		let format = add_mark_format(&schema, &code);

		assert_eq!(format.get("bold"), Some(&Any::Null));
		assert_eq!(format.get("code"), Some(&Any::Bool(true)));
	}

	#[test]
	fn embeds_round_trip() {
		let schema = schema();
		let mention = Embed {
			kind: schema.embed_type("mention").expect("mention"),
			attrs: Attrs::new().with("id", "ifiok"),
		};

		let any = embed_to_any(&schema, &mention);

		assert_eq!(embed_from(&schema, &any), Some(mention));
		assert_eq!(embed_from(&schema, &Any::Bool(true)), None);
	}
}
