//! Attribute values attached to blocks, marks, and inline embeds.

use std::sync::Arc;

/// A single attribute value.
///
/// The set of variants deliberately mirrors what JSON, Yjs maps, and every
/// host platform can represent without loss.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
	/// The absence of a value.
	#[default]
	Null,
	/// A boolean flag.
	Bool(bool),
	/// A signed integer, e.g. a heading level.
	Int(i64),
	/// A floating-point number, e.g. an image width.
	Float(f64),
	/// A shared, immutable string, e.g. a link href.
	String(Arc<str>),
	/// A shared, immutable list, e.g. table column widths.
	Array(Arc<[Value]>),
}

impl Value {
	/// The string payload, if this is a string.
	#[must_use]
	pub fn as_str(&self) -> Option<&str> {
		match self {
			Self::String(value) => Some(value),
			_ => None,
		}
	}

	/// The integer payload, if this is an integer.
	#[must_use]
	pub fn as_int(&self) -> Option<i64> {
		match self {
			Self::Int(value) => Some(*value),
			_ => None,
		}
	}

	/// The boolean payload, if this is a boolean.
	#[must_use]
	pub fn as_bool(&self) -> Option<bool> {
		match self {
			Self::Bool(value) => Some(*value),
			_ => None,
		}
	}
}

impl From<Vec<Value>> for Value {
	fn from(value: Vec<Value>) -> Self {
		Self::Array(value.into())
	}
}

impl From<bool> for Value {
	fn from(value: bool) -> Self {
		Self::Bool(value)
	}
}

impl From<i64> for Value {
	fn from(value: i64) -> Self {
		Self::Int(value)
	}
}

impl From<f64> for Value {
	fn from(value: f64) -> Self {
		Self::Float(value)
	}
}

impl From<&str> for Value {
	fn from(value: &str) -> Self {
		Self::String(value.into())
	}
}

impl From<String> for Value {
	fn from(value: String) -> Self {
		Self::String(value.into())
	}
}

/// An ordered set of named attributes.
///
/// Stored as a vector sorted by key: blocks and marks carry only a handful of
/// attributes, so a sorted vector is smaller and faster than a map.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Attrs(Vec<(Arc<str>, Value)>);

impl Attrs {
	/// An empty attribute set.
	#[must_use]
	pub const fn new() -> Self {
		Self(Vec::new())
	}

	/// Builder-style insert.
	#[must_use]
	pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
		self.set(key, value);
		self
	}

	/// The value for `key`, if present.
	#[must_use]
	pub fn get(&self, key: &str) -> Option<&Value> {
		let index = self.find(key).ok()?;
		Some(&self.0[index].1)
	}

	/// Insert or replace the value for `key`, returning the previous value.
	pub fn set(&mut self, key: &str, value: impl Into<Value>) -> Option<Value> {
		let value = value.into();

		match self.find(key) {
			Ok(index) => Some(std::mem::replace(&mut self.0[index].1, value)),
			Err(index) => {
				self.0.insert(index, (key.into(), value));
				None
			}
		}
	}

	/// Remove `key`, returning its value if it was present.
	pub fn remove(&mut self, key: &str) -> Option<Value> {
		let index = self.find(key).ok()?;
		Some(self.0.remove(index).1)
	}

	/// Iterate over `(key, value)` pairs in key order.
	pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
		self.0.iter().map(|(key, value)| (&**key, value))
	}

	/// The number of attributes.
	#[must_use]
	pub fn len(&self) -> usize {
		self.0.len()
	}

	/// Whether there are no attributes.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	fn find(&self, key: &str) -> Result<usize, usize> {
		self.0.binary_search_by(|(probe, _)| (**probe).cmp(key))
	}
}

impl<K: AsRef<str>, V: Into<Value>> FromIterator<(K, V)> for Attrs {
	fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
		let mut attrs = Self::new();

		for (key, value) in iter {
			attrs.set(key.as_ref(), value);
		}

		attrs
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn keeps_keys_sorted_and_unique() {
		let mut attrs = Attrs::new().with("level", 2).with("align", "center");
		let previous = attrs.set("level", 3);

		let keys: Vec<_> = attrs.iter().map(|(key, _)| key).collect();
		assert_eq!(keys, ["align", "level"]);
		assert_eq!(previous, Some(Value::Int(2)));
		assert_eq!(attrs.get("level").and_then(Value::as_int), Some(3));
	}

	#[test]
	fn remove_returns_the_old_value() {
		let mut attrs: Attrs = [("href", "https://canvist.dev")].into_iter().collect();

		assert_eq!(
			attrs.remove("href").as_ref().and_then(Value::as_str),
			Some("https://canvist.dev")
		);
		assert!(attrs.is_empty());
		assert_eq!(attrs.remove("href"), None);
	}
}
