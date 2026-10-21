//! The document as one string of UTF-16 code units, the way platform text
//! input sees it.
//!
//! `UITextInput` and Android's `InputConnection` address text by offsets
//! into a single string, while the model addresses it by block and UTF-8
//! byte. The editor's text is every text block in document order, joined by
//! a line feed: one code unit between consecutive blocks. Inline embeds
//! count as one code unit, as in [`DocPosition`](crate::DocPosition).
//!
//! The index keeps each text block's start, so converting either way costs a
//! binary search plus a scan of one block. It is patched from each commit:
//! typing re-measures only the touched block and shifts the starts after
//! it, and structural changes rebuild the order while reusing the lengths of
//! blocks whose revision did not change.

use canvist_headless::canvist_model::BlockId;
use canvist_headless::canvist_model::Commit;
use canvist_headless::canvist_model::Document;
use canvist_headless::canvist_model::InlineText;
use canvist_headless::canvist_model::Point;
use rustc_hash::FxHashMap;

use crate::OffsetRange;

/// The code unit joining consecutive text blocks.
const BLOCK_SEPARATOR: char = '\n';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
	block: BlockId,
	/// The block's revision when `length` was counted.
	rev: u32,
	/// Where the block's text starts in the editor's text.
	start: u32,
	/// The block's text length in UTF-16 code units.
	length: u32,
}

/// Offsets of every text block in the editor's text.
#[derive(Clone, Debug, Default)]
pub(crate) struct TextIndex {
	entries: Vec<Entry>,
	positions: FxHashMap<BlockId, usize>,
}

impl TextIndex {
	pub(crate) fn new(doc: &Document) -> Self {
		let mut index = Self::default();
		index.rebuild(doc);
		index
	}

	/// The editor's text length in UTF-16 code units.
	pub(crate) fn len(&self) -> u32 {
		self.entries
			.last()
			.map_or(0, |entry| entry.start + entry.length)
	}

	/// Account for `commit`, which `doc` already reflects.
	pub(crate) fn apply(&mut self, doc: &Document, commit: &Commit) {
		if commit.structural {
			self.rebuild(doc);

			return;
		}

		let mut first = None;

		for block in &commit.touched {
			let Some(&index) = self.positions.get(block) else {
				continue;
			};

			if let Some((rev, length)) = measure(doc, *block) {
				let entry = &mut self.entries[index];
				entry.rev = rev;
				entry.length = length;
				first = Some(first.map_or(index, |first: usize| first.min(index)));
			}
		}

		if let Some(first) = first {
			self.restart_from(first);
		}
	}

	/// Recount the block order, keeping lengths of unchanged blocks.
	pub(crate) fn rebuild(&mut self, doc: &Document) {
		let old: FxHashMap<BlockId, Entry> = self
			.entries
			.drain(..)
			.map(|entry| (entry.block, entry))
			.collect();
		self.positions.clear();

		for block in doc.text_blocks() {
			let id = block.id();
			let rev = block.rev();
			let known = old.get(&id).filter(|entry| entry.rev == rev);
			let length = known.map_or_else(
				|| block.inline().map_or(0, |text| utf16_len(text.as_str())),
				|entry| entry.length,
			);

			self.positions.insert(id, self.entries.len());
			self.entries.push(Entry {
				block: id,
				rev,
				start: 0,
				length,
			});
		}

		self.restart_from(0);
	}

	fn restart_from(&mut self, first: usize) {
		let mut start = match first.checked_sub(1) {
			Some(previous) => {
				let entry = &self.entries[previous];
				entry.start + entry.length + 1
			}
			None => 0,
		};

		for entry in &mut self.entries[first..] {
			entry.start = start;
			start += entry.length + 1;
		}
	}

	/// The offset of model `point`, or `None` when its block is not a text
	/// block of the document. Offsets inside a character snap back to its
	/// start.
	pub(crate) fn offset(&self, doc: &Document, point: Point) -> Option<u32> {
		let entry = self.entries[*self.positions.get(&point.block)?];
		let text = doc.inline(point.block)?.as_str();
		Some(entry.start + utf16_len(prefix(text, point.offset)))
	}

	/// The model point at `offset`, clamped into the text. The line feed
	/// after a block is the end of that block.
	pub(crate) fn point(&self, doc: &Document, offset: u32) -> Option<Point> {
		let entry = self.entry_at(offset)?;
		let text = doc.inline(entry.block)?.as_str();
		let local = offset.saturating_sub(entry.start).min(entry.length);
		Some(Point::new(entry.block, utf8_offset(text, local)))
	}

	/// The range of the text block holding `offset`.
	pub(crate) fn block_range(&self, offset: u32) -> Option<OffsetRange> {
		self.entry_at(offset).map(|entry| {
			OffsetRange {
				start: entry.start,
				end: entry.start + entry.length,
			}
		})
	}

	/// The range of `block`'s text, if it is a text block.
	pub(crate) fn range_of(&self, block: BlockId) -> Option<OffsetRange> {
		let entry = self.entries[*self.positions.get(&block)?];
		Some(OffsetRange {
			start: entry.start,
			end: entry.start + entry.length,
		})
	}

	/// The editor's text in `range`, clamped to the text.
	pub(crate) fn text(&self, doc: &Document, range: OffsetRange) -> String {
		let end = range.end.min(self.len());
		let mut out = String::new();
		let Some(first) = self.entry_index_at(range.start) else {
			return out;
		};

		for entry in &self.entries[first..] {
			if entry.start > end {
				break;
			}

			let Some(text) = doc.inline(entry.block).map(InlineText::as_str) else {
				continue;
			};
			let from = range.start.saturating_sub(entry.start).min(entry.length);
			let to = end.saturating_sub(entry.start).min(entry.length);
			out.push_str(&text[utf8_offset(text, from) as usize..utf8_offset(text, to) as usize]);

			if end > entry.start + entry.length {
				out.push(BLOCK_SEPARATOR);
			}
		}

		out
	}

	fn entry_at(&self, offset: u32) -> Option<&Entry> {
		self.entry_index_at(offset)
			.map(|index| &self.entries[index])
	}

	/// The index of the entry whose text, or the line feed after it, holds
	/// `offset`; the last block for offsets past the end.
	fn entry_index_at(&self, offset: u32) -> Option<usize> {
		if self.entries.is_empty() {
			return None;
		}

		let after = self.entries.partition_point(|entry| entry.start <= offset);
		Some(after.saturating_sub(1))
	}
}

/// The revision and UTF-16 length of text block `block`.
fn measure(doc: &Document, block: BlockId) -> Option<(u32, u32)> {
	let rev = doc.block(block)?.rev();
	let text = doc.inline(block)?.as_str();
	Some((rev, utf16_len(text)))
}

fn utf16_len(text: &str) -> u32 {
	text.chars().map(|c| c.len_utf16() as u32).sum()
}

/// `text` up to byte `offset`, snapped back to a character boundary.
fn prefix(text: &str, offset: u32) -> &str {
	let mut end = (offset as usize).min(text.len());

	while !text.is_char_boundary(end) {
		end -= 1;
	}

	&text[..end]
}

/// The byte offset of UTF-16 offset `units` in `text`. An offset inside a
/// surrogate pair snaps back to the pair's start.
pub(crate) fn utf8_offset(text: &str, units: u32) -> u32 {
	let mut counted = 0;

	for (byte, character) in text.char_indices() {
		counted += character.len_utf16() as u32;

		if counted > units {
			return byte as u32;
		}
	}

	text.len() as u32
}

/// The UTF-16 offset of byte `offset` in `text`.
pub(crate) fn utf16_offset(text: &str, offset: u32) -> u32 {
	utf16_len(prefix(text, offset))
}

#[cfg(test)]
mod tests {
	use canvist_headless::Engine;
	use canvist_headless::Format;
	use canvist_headless::Position;

	use super::*;

	fn index(markdown: &str) -> (canvist_headless::Doc, TextIndex) {
		let doc = Engine::starter_kit()
			.parse(Format::Markdown, markdown)
			.expect("valid markdown");
		let index = TextIndex::new(doc.document());
		(doc, index)
	}

	#[test]
	fn joins_text_blocks_with_line_feeds() {
		let (doc, index) = index("# Title\n\nSome 😀 text\n\n- one\n- two");

		assert_eq!(index.len(), 5 + 1 + 12 + 1 + 3 + 1 + 3);
		assert_eq!(
			index.text(
				doc.document(),
				OffsetRange {
					start: 0,
					end: index.len()
				}
			),
			"Title\nSome 😀 text\none\ntwo"
		);
		assert_eq!(
			index.text(doc.document(), OffsetRange { start: 3, end: 9 }),
			"le\nSom"
		);
	}

	#[test]
	fn converts_offsets_to_points_and_back() {
		let (doc, index) = index("ab\n\nc😀d");
		let document = doc.document();

		for offset in [0, 1, 2, 3, 4, 6, 7] {
			let point = index.point(document, offset).expect("a point");
			assert_eq!(
				index.offset(document, point),
				Some(offset),
				"offset {offset}"
			);
		}

		let second = doc.point(&Position::new([1], 3)).expect("after the emoji");
		assert_eq!(index.point(document, 6), Some(second));
		assert_eq!(
			index.point(document, 5),
			index.point(document, 4),
			"inside a surrogate pair snaps back"
		);
		assert_eq!(
			index.point(document, 99),
			index.point(document, 7),
			"past the end clamps"
		);
	}

	#[test]
	fn follows_typing_and_structural_changes() {
		let (mut doc, mut index) = index("ab\n\ncd");
		doc.record_commits();

		doc.select_range(&Position::new([0], 2), &Position::new([0], 2))
			.expect("select");
		doc.insert_text("xyz").expect("type");
		doc.press("Enter").expect("split");
		doc.insert_text("new").expect("type");

		for commit in doc.take_commits() {
			index.apply(doc.document(), &commit);
		}

		assert_eq!(
			index.text(
				doc.document(),
				OffsetRange {
					start: 0,
					end: index.len()
				}
			),
			"abxyz\nnew\ncd"
		);
		assert_eq!(index.len(), TextIndex::new(doc.document()).len());
	}

	#[test]
	fn converts_utf16_offsets_within_a_block() {
		let text = "a😀b";

		assert_eq!(utf8_offset(text, 0), 0);
		assert_eq!(utf8_offset(text, 1), 1);
		assert_eq!(utf8_offset(text, 2), 1, "inside the pair");
		assert_eq!(utf8_offset(text, 3), 5);
		assert_eq!(utf8_offset(text, 9), 6);
		assert_eq!(utf16_offset(text, 5), 3);
		assert_eq!(utf16_offset(text, 3), 1, "inside the emoji's bytes");
	}
}
