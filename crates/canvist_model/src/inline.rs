//! Inline content: text, marks, and embeds inside a text block.
//!
//! Inline content is stored the way Quill deltas and Yjs `Y.Text` store it:
//! one UTF-8 string plus a run-length list of formatting. That is far more
//! compact than a tree of inline nodes, makes typing an `O(runs)` splice, and
//! maps one-to-one onto `Y.Text` insert/format/embed operations.
//!
//! Inline embeds (mentions, inline images, hard breaks) occupy a single
//! [`EMBED_CHAR`] in the string so that every offset stays a plain byte offset.

use std::ops::Range;
use std::sync::Arc;

use crate::Attrs;
use crate::EmbedTypeId;
use crate::Mark;
use crate::MarkSet;
use crate::MarkTypeId;
use crate::Schema;

/// The object replacement character that stands in for an embed.
pub const EMBED_CHAR: char = '\u{FFFC}';

/// The UTF-8 length of [`EMBED_CHAR`].
const EMBED_LEN: u32 = EMBED_CHAR.len_utf8() as u32;

/// An atomic inline object such as a mention or an inline image.
#[derive(Clone, Debug, PartialEq)]
pub struct Embed {
	/// The embed's type.
	pub kind: EmbedTypeId,
	/// The embed's attributes, e.g. the mentioned user id.
	pub attrs: Attrs,
}

/// Errors raised by inline edits with invalid offsets.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InlineError {
	/// The offset is past the end of the text.
	#[error("offset {offset} is out of bounds for inline text of length {len}")]
	OutOfBounds {
		/// The requested offset.
		offset: u32,
		/// The text length.
		len: u32,
	},
	/// The offset falls inside a multi-byte character.
	#[error("offset {0} is not on a character boundary")]
	NotCharBoundary(u32),
	/// The range end is before its start.
	#[error("range {start}..{end} is reversed")]
	Reversed {
		/// The range start.
		start: u32,
		/// The range end.
		end: u32,
	},
}

/// One formatting run.
#[derive(Clone, Debug, PartialEq)]
struct Run {
	len: u32,
	marks: MarkSet,
	embed: Option<Arc<Embed>>,
}

impl Run {
	fn text(len: u32, marks: MarkSet) -> Self {
		Self {
			len,
			marks,
			embed: None,
		}
	}

	fn merges_with(&self, next: &Self) -> bool {
		self.embed.is_none() && next.embed.is_none() && self.marks == next.marks
	}
}

/// A borrowed view of one run, yielded by [`InlineText::runs`].
#[derive(Clone, Debug, PartialEq)]
pub struct RunRef<'a> {
	/// The byte range covered by the run.
	pub range: Range<u32>,
	/// The run's text (the embed character for embeds).
	pub text: &'a str,
	/// The run's marks.
	pub marks: &'a MarkSet,
	/// The embed, if this run is one.
	pub embed: Option<&'a Embed>,
}

/// The inline content of a text block.
///
/// Invariants: run lengths sum to the text length, no run is empty, embed runs
/// cover exactly one [`EMBED_CHAR`], and adjacent text runs have different
/// marks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InlineText {
	text: String,
	runs: Vec<Run>,
}

impl InlineText {
	/// Empty inline content.
	#[must_use]
	pub const fn new() -> Self {
		Self {
			text: String::new(),
			runs: Vec::new(),
		}
	}

	/// Plain text with one set of marks. Any [`EMBED_CHAR`] in `text` is
	/// removed, because embeds can only be created with [`Self::embed`].
	#[must_use]
	pub fn plain(text: &str, marks: MarkSet) -> Self {
		let text = if text.contains(EMBED_CHAR) {
			text.replace(EMBED_CHAR, "")
		} else {
			text.to_owned()
		};
		let runs = if text.is_empty() {
			Vec::new()
		} else {
			vec![Run::text(text.len() as u32, marks)]
		};

		Self { text, runs }
	}

	/// A single embed with the given marks.
	#[must_use]
	pub fn embed(embed: Embed, marks: MarkSet) -> Self {
		Self {
			text: EMBED_CHAR.to_string(),
			runs: vec![Run {
				len: EMBED_LEN,
				marks,
				embed: Some(Arc::new(embed)),
			}],
		}
	}

	/// The length in UTF-8 bytes.
	#[must_use]
	pub fn len(&self) -> u32 {
		self.text.len() as u32
	}

	/// Whether there is no content.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.text.is_empty()
	}

	/// The raw text, with embeds shown as [`EMBED_CHAR`].
	#[must_use]
	pub fn as_str(&self) -> &str {
		&self.text
	}

	/// Iterate over formatting runs in order.
	pub fn runs(&self) -> impl Iterator<Item = RunRef<'_>> {
		let mut start = 0;
		self.runs.iter().map(move |run| {
			let range = start..start + run.len;
			start = range.end;
			RunRef {
				text: &self.text[range.start as usize..range.end as usize],
				range,
				marks: &run.marks,
				embed: run.embed.as_deref(),
			}
		})
	}

	/// The number of runs.
	#[must_use]
	pub fn run_count(&self) -> usize {
		self.runs.len()
	}

	/// The marks of the character ending at `offset`, if any.
	#[must_use]
	pub fn marks_before(&self, offset: u32) -> Option<&MarkSet> {
		self.run_containing(offset.checked_sub(1)?)
			.map(|run| &run.marks)
	}

	/// The marks of the character starting at `offset`, if any.
	#[must_use]
	pub fn marks_after(&self, offset: u32) -> Option<&MarkSet> {
		self.run_containing(offset).map(|run| &run.marks)
	}

	/// Check that `offset` is a valid insertion point.
	pub fn check_offset(&self, offset: u32) -> Result<(), InlineError> {
		if offset > self.len() {
			return Err(InlineError::OutOfBounds {
				offset,
				len: self.len(),
			});
		}

		if !self.text.is_char_boundary(offset as usize) {
			return Err(InlineError::NotCharBoundary(offset));
		}

		Ok(())
	}

	/// Check that `range` is ordered and both ends are valid offsets.
	pub fn check_range(&self, range: &Range<u32>) -> Result<(), InlineError> {
		if range.end < range.start {
			return Err(InlineError::Reversed {
				start: range.start,
				end: range.end,
			});
		}

		self.check_offset(range.start)?;
		self.check_offset(range.end)
	}

	/// Insert `other` at `offset`.
	pub fn insert(&mut self, offset: u32, other: Self) -> Result<(), InlineError> {
		self.check_offset(offset)?;
		let index = self.split_at(offset);
		let inserted = other.runs.len();
		self.text.insert_str(offset as usize, &other.text);
		self.runs.splice(index..index, other.runs);
		self.merge_at(index + inserted);
		self.merge_at(index);
		Ok(())
	}

	/// Remove `range` and return the removed content.
	pub fn remove(&mut self, range: Range<u32>) -> Result<Self, InlineError> {
		self.check_range(&range)?;
		let (first, last) = self.split_range(&range);
		let runs = self.runs.drain(first..last).collect();
		let text = self
			.text
			.drain(range.start as usize..range.end as usize)
			.collect();
		self.merge_at(first);
		Ok(Self { text, runs })
	}

	/// Copy `range` without modifying `self`.
	pub fn slice(&self, range: Range<u32>) -> Result<Self, InlineError> {
		let mut copy = self.clone();
		copy.check_range(&range)?;
		copy.text.truncate(range.end as usize);
		copy.split_at(range.end);
		copy.runs.truncate(copy.run_index_at(range.end));
		copy.remove(0..range.start)?;
		Ok(copy)
	}

	/// Split at `offset`, keeping the head and returning the tail.
	pub fn split_off(&mut self, offset: u32) -> Result<Self, InlineError> {
		self.remove(offset..self.len())
	}

	/// Append `other` to the end.
	pub fn append(&mut self, other: Self) {
		let index = self.runs.len();
		self.text.push_str(&other.text);
		self.runs.extend(other.runs);
		self.merge_at(index);
	}

	/// Add `mark` to every run in `range`, honouring the schema's exclusions.
	pub fn add_mark(
		&mut self,
		range: Range<u32>,
		mark: &Mark,
		schema: &Schema,
	) -> Result<(), InlineError> {
		self.map_marks(range, |marks| marks.with(mark, schema))
	}

	/// Remove marks of type `kind` from every run in `range`.
	pub fn remove_mark(&mut self, range: Range<u32>, kind: MarkTypeId) -> Result<(), InlineError> {
		self.map_marks(range, |marks| marks.without(kind))
	}

	/// The marks of each run in `range`, as `(len, marks)` pairs that
	/// [`Self::restore_marks`] can reapply.
	pub fn mark_runs(&self, range: Range<u32>) -> Result<Vec<(u32, MarkSet)>, InlineError> {
		let slice = self.slice(range)?;
		Ok(slice
			.runs
			.into_iter()
			.map(|run| (run.len, run.marks))
			.collect())
	}

	/// Reapply marks captured by [`Self::mark_runs`], starting at `offset`.
	pub fn restore_marks(
		&mut self,
		offset: u32,
		runs: &[(u32, MarkSet)],
	) -> Result<(), InlineError> {
		let mut start = offset;

		for (len, marks) in runs {
			let end = start + len;
			self.map_marks(start..end, |_| marks.clone())?;
			start = end;
		}

		Ok(())
	}

	fn map_marks(
		&mut self,
		range: Range<u32>,
		mut map: impl FnMut(&MarkSet) -> MarkSet,
	) -> Result<(), InlineError> {
		self.check_range(&range)?;
		let (first, last) = self.split_range(&range);

		for run in &mut self.runs[first..last] {
			run.marks = map(&run.marks);
		}

		self.normalize();
		Ok(())
	}

	fn run_containing(&self, offset: u32) -> Option<&Run> {
		let mut start = 0;
		self.runs.iter().find(|run| {
			start += run.len;
			offset < start
		})
	}

	/// The index of the first run starting at or after `offset`.
	fn run_index_at(&self, offset: u32) -> usize {
		let mut start = 0;
		self.runs
			.iter()
			.position(|run| {
				let found = start >= offset;
				start += run.len;
				found
			})
			.unwrap_or(self.runs.len())
	}

	/// Ensure a run boundary exists at `offset` and return the index of the
	/// run that starts there. `offset` must be a checked character boundary.
	fn split_at(&mut self, offset: u32) -> usize {
		let mut start = 0;

		for index in 0..self.runs.len() {
			let len = self.runs[index].len;

			if offset == start {
				return index;
			}

			if offset < start + len {
				let head = offset - start;
				let mut tail = self.runs[index].clone();

				tail.len = len - head;
				self.runs[index].len = head;
				self.runs.insert(index + 1, tail);

				return index + 1;
			}

			start += len;
		}

		self.runs.len()
	}

	fn split_range(&mut self, range: &Range<u32>) -> (usize, usize) {
		let first = self.split_at(range.start);
		let last = self.split_at(range.end);
		(first, last)
	}

	/// Merge the runs on either side of `boundary` if their marks match.
	fn merge_at(&mut self, boundary: usize) {
		if boundary == 0 || boundary >= self.runs.len() {
			return;
		}

		if self.runs[boundary - 1].merges_with(&self.runs[boundary]) {
			let next = self.runs.remove(boundary);
			self.runs[boundary - 1].len += next.len;
		}
	}

	fn normalize(&mut self) {
		let mut merged: Vec<Run> = Vec::with_capacity(self.runs.len());

		for run in self.runs.drain(..).filter(|run| run.len > 0) {
			match merged.last_mut() {
				Some(previous) if previous.merges_with(&run) => previous.len += run.len,
				_ => merged.push(run),
			}
		}

		self.runs = merged;
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::BlockSpec;
	use crate::EmbedSpec;
	use crate::MarkSpec;

	fn schema() -> Schema {
		Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.mark(MarkSpec::new("bold"))
			.mark(MarkSpec::new("italic"))
			.embed(EmbedSpec::new("mention"))
			.build()
			.expect("valid schema")
	}

	fn bold(schema: &Schema) -> Mark {
		Mark::new(schema.mark_type("bold").expect("bold exists"))
	}

	fn summary(text: &InlineText) -> Vec<(&str, usize)> {
		text.runs()
			.map(|run| (run.text, run.marks.as_slice().len()))
			.collect()
	}

	#[test]
	fn inserting_with_equal_marks_merges_runs() {
		let mut text = InlineText::plain("helo", MarkSet::empty());
		text.insert(3, InlineText::plain("l", MarkSet::empty()))
			.expect("valid offset");

		assert_eq!(text.as_str(), "hello");
		assert_eq!(text.run_count(), 1);
	}

	#[test]
	fn add_mark_splits_and_remove_mark_merges() {
		let schema = schema();
		let mut text = InlineText::plain("hello world", MarkSet::empty());

		text.add_mark(6..11, &bold(&schema), &schema)
			.expect("valid range");
		assert_eq!(summary(&text), [("hello ", 0), ("world", 1)]);

		text.remove_mark(0..11, bold(&schema).kind)
			.expect("valid range");
		assert_eq!(summary(&text), [("hello world", 0)]);
	}

	#[test]
	fn remove_returns_formatted_content() {
		let schema = schema();
		let mut text = InlineText::plain("abcdef", MarkSet::empty());
		text.add_mark(2..4, &bold(&schema), &schema)
			.expect("valid range");

		let removed = text.remove(1..5).expect("valid range");

		assert_eq!(text.as_str(), "af");
		assert_eq!(text.run_count(), 1);
		assert_eq!(summary(&removed), [("b", 0), ("cd", 1), ("e", 0)]);
	}

	#[test]
	fn split_off_and_append_round_trip() {
		let schema = schema();
		let mut text = InlineText::plain("hello", MarkSet::empty());
		text.add_mark(0..5, &bold(&schema), &schema)
			.expect("valid range");
		let original = text.clone();

		let tail = text.split_off(2).expect("valid offset");
		text.append(tail);

		assert_eq!(text, original);
	}

	#[test]
	fn slice_copies_without_mutating() {
		let schema = schema();
		let mut text = InlineText::plain("hello world", MarkSet::empty());
		text.add_mark(4..7, &bold(&schema), &schema)
			.expect("valid range");

		let slice = text.slice(2..9).expect("valid range");

		assert_eq!(summary(&slice), [("ll", 0), ("o w", 1), ("or", 0)]);
		assert_eq!(text.as_str(), "hello world");
	}

	#[test]
	fn embeds_never_merge_and_are_atomic() {
		let schema = schema();
		let mention = Embed {
			kind: schema.embed_type("mention").expect("mention exists"),
			attrs: Attrs::new().with("id", "ifiok"),
		};
		let mut text = InlineText::plain("hi ", MarkSet::empty());
		text.insert(3, InlineText::embed(mention.clone(), MarkSet::empty()))
			.expect("valid offset");
		text.insert(3, InlineText::embed(mention, MarkSet::empty()))
			.expect("valid offset");

		assert_eq!(text.run_count(), 3);
		assert_eq!(text.check_offset(4), Err(InlineError::NotCharBoundary(4)));
		assert_eq!(text.runs().filter(|run| run.embed.is_some()).count(), 2);
	}

	#[test]
	fn plain_text_cannot_smuggle_embeds() {
		let text = InlineText::plain("a\u{FFFC}b", MarkSet::empty());

		assert_eq!(text.as_str(), "ab");
	}

	#[test]
	fn rejects_invalid_offsets() {
		let mut text = InlineText::plain("é", MarkSet::empty());

		assert_eq!(text.check_offset(1), Err(InlineError::NotCharBoundary(1)));
		assert!(matches!(
			text.remove(0..9),
			Err(InlineError::OutOfBounds { .. })
		));
		let (start, end) = (2, 0);
		assert!(matches!(
			text.remove(start..end),
			Err(InlineError::Reversed { .. })
		));
	}

	#[test]
	fn mark_snapshots_restore_formatting() {
		let schema = schema();
		let mut text = InlineText::plain("abcdef", MarkSet::empty());
		text.add_mark(1..3, &bold(&schema), &schema)
			.expect("valid range");
		let before = text.clone();
		let snapshot = text.mark_runs(0..6).expect("valid range");

		text.add_mark(0..6, &bold(&schema), &schema)
			.expect("valid range");
		text.restore_marks(0, &snapshot).expect("valid offset");

		assert_eq!(text, before);
	}

	#[test]
	fn reports_marks_on_either_side_of_an_offset() {
		let schema = schema();
		let mut text = InlineText::plain("ab", MarkSet::empty());
		text.add_mark(0..1, &bold(&schema), &schema)
			.expect("valid range");

		assert!(text.marks_before(1).is_some_and(|marks| !marks.is_empty()));
		assert!(text.marks_after(1).is_some_and(MarkSet::is_empty));
		assert!(text.marks_before(0).is_none());
		assert!(text.marks_after(2).is_none());
	}
}
