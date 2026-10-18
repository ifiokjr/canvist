//! Marks are inline formatting such as bold, italic, or links.

use std::sync::Arc;

use crate::Attrs;
use crate::MarkTypeId;
use crate::Schema;

/// One mark instance: a type plus its attributes, e.g. `link { href }`.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
	/// The mark's type.
	pub kind: MarkTypeId,
	/// The mark's attributes.
	pub attrs: Attrs,
}

impl Mark {
	/// A mark with no attributes.
	#[must_use]
	pub const fn new(kind: MarkTypeId) -> Self {
		Self {
			kind,
			attrs: Attrs::new(),
		}
	}

	/// A mark with attributes.
	#[must_use]
	pub const fn with_attrs(kind: MarkTypeId, attrs: Attrs) -> Self {
		Self { kind, attrs }
	}
}

/// An immutable, shared, sorted set of marks.
///
/// Every run of text holds one `MarkSet`. Sets are reference counted so that
/// splitting a run or copying a slice never clones mark attributes, and
/// equality short-circuits on pointer identity for the common case of runs
/// that share a set.
#[derive(Clone, Debug, Default)]
pub struct MarkSet(Option<Arc<[Mark]>>);

impl MarkSet {
	/// The empty set.
	#[must_use]
	pub const fn empty() -> Self {
		Self(None)
	}

	/// Build a set from marks, sorting them and keeping the last mark of each
	/// type.
	#[must_use]
	pub fn from_marks(marks: impl IntoIterator<Item = Mark>) -> Self {
		let mut marks: Vec<Mark> = marks.into_iter().collect();
		marks.sort_by_key(|mark| mark.kind);
		marks.reverse();
		marks.dedup_by_key(|mark| mark.kind);
		marks.reverse();
		Self::from_sorted(marks)
	}

	/// The marks in type order.
	#[must_use]
	pub fn as_slice(&self) -> &[Mark] {
		self.0.as_deref().unwrap_or_default()
	}

	/// Whether the set holds no marks.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.0.is_none()
	}

	/// The mark of type `kind`, if present.
	#[must_use]
	pub fn get(&self, kind: MarkTypeId) -> Option<&Mark> {
		let marks = self.as_slice();
		let index = marks.binary_search_by_key(&kind, |mark| mark.kind).ok()?;
		Some(&marks[index])
	}

	/// Whether a mark of type `kind` is present.
	#[must_use]
	pub fn contains(&self, kind: MarkTypeId) -> bool {
		self.get(kind).is_some()
	}

	/// A new set with `mark` added, removing any mark it excludes.
	#[must_use]
	pub fn with(&self, mark: &Mark, schema: &Schema) -> Self {
		let excludes = &schema.mark(mark.kind).excludes;
		let mut marks: Vec<Mark> = self
			.as_slice()
			.iter()
			.filter(|existing| excludes.binary_search(&existing.kind).is_err())
			.cloned()
			.collect();
		let index = marks.partition_point(|existing| existing.kind < mark.kind);
		marks.insert(index, mark.clone());
		Self::from_sorted(marks)
	}

	/// A new set without any mark of type `kind`.
	#[must_use]
	pub fn without(&self, kind: MarkTypeId) -> Self {
		if !self.contains(kind) {
			return self.clone();
		}

		let marks = self
			.as_slice()
			.iter()
			.filter(|mark| mark.kind != kind)
			.cloned();
		Self::from_sorted(marks.collect())
	}

	fn from_sorted(marks: Vec<Mark>) -> Self {
		if marks.is_empty() {
			return Self::empty();
		}

		Self(Some(marks.into()))
	}
}

impl PartialEq for MarkSet {
	fn eq(&self, other: &Self) -> bool {
		match (&self.0, &other.0) {
			(Some(left), Some(right)) => Arc::ptr_eq(left, right) || left == right,
			(None, None) => true,
			_ => false,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::MarkSpec;
	use crate::schema::BlockSpec;

	fn schema() -> Schema {
		Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.mark(MarkSpec::new("bold"))
			.mark(MarkSpec::new("italic"))
			.mark(MarkSpec::new("code").excludes(&["bold", "italic"]))
			.mark(MarkSpec::new("link"))
			.build()
			.expect("valid schema")
	}

	fn kind(schema: &Schema, name: &str) -> MarkTypeId {
		schema.mark_type(name).expect("mark exists")
	}

	#[test]
	fn keeps_marks_sorted_by_type() {
		let schema = schema();
		let set = MarkSet::empty()
			.with(&Mark::new(kind(&schema, "italic")), &schema)
			.with(&Mark::new(kind(&schema, "bold")), &schema);

		let kinds: Vec<_> = set.as_slice().iter().map(|mark| mark.kind).collect();
		assert_eq!(kinds, [kind(&schema, "bold"), kind(&schema, "italic")]);
	}

	#[test]
	fn adding_replaces_same_type_and_drops_excluded() {
		let schema = schema();
		let link = kind(&schema, "link");
		let first = Mark::with_attrs(link, Attrs::new().with("href", "a"));
		let second = Mark::with_attrs(link, Attrs::new().with("href", "b"));
		let set =
			MarkSet::from_marks([Mark::new(kind(&schema, "bold")), first]).with(&second, &schema);

		assert_eq!(set.get(link), Some(&second));
		let coded = set.with(&Mark::new(kind(&schema, "code")), &schema);
		assert!(!coded.contains(kind(&schema, "bold")));
		assert!(coded.contains(link));
	}

	#[test]
	fn removing_the_last_mark_yields_the_empty_set() {
		let schema = schema();
		let bold = kind(&schema, "bold");
		let set = MarkSet::from_marks([Mark::new(bold)]).without(bold);

		assert!(set.is_empty());
		assert_eq!(set, MarkSet::empty());
	}
}
