//! Positions in document order: comparing points, ordering selections,
//! and finding the blocks a selection covers.

use std::cmp::Ordering;
use std::ops::Range;

use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::Content;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Selection;

/// A text selection ordered so that `from` comes first in the document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextRange {
	/// The earlier end.
	pub from: Point,
	/// The later end.
	pub to: Point,
}

impl TextRange {
	/// Whether the range is a collapsed caret.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.from == self.to
	}
}

/// The ordered range of a text selection, or `None` for block selections.
#[must_use]
pub fn text_range(doc: &Document, selection: Selection) -> Option<TextRange> {
	let Selection::Text { anchor, head } = selection else {
		return None;
	};

	if compare_points(doc, anchor, head) == Ordering::Greater {
		return Some(TextRange {
			from: head,
			to: anchor,
		});
	}

	Some(TextRange {
		from: anchor,
		to: head,
	})
}

/// Compare two points in document order.
#[must_use]
pub fn compare_points(doc: &Document, a: Point, b: Point) -> Ordering {
	if a.block == b.block {
		return a.offset.cmp(&b.offset);
	}

	compare_blocks(doc, a.block, b.block)
}

/// Compare two blocks in document (pre-)order: an ancestor comes before its
/// descendants. Costs `O(depth × siblings)`, so it is meant for selection
/// handling, not per-keystroke paths.
#[must_use]
pub fn compare_blocks(doc: &Document, a: BlockId, b: BlockId) -> Ordering {
	path(doc, a).cmp(&path(doc, b))
}

/// The child indexes leading from the root to `id`.
fn path(doc: &Document, id: BlockId) -> Vec<usize> {
	let mut indexes: Vec<usize> = ancestors_or_self(doc, id)
		.filter_map(|block| doc.index_in_parent(block))
		.collect();
	indexes.reverse();
	indexes
}

/// `id` followed by each of its ancestors, innermost first.
pub fn ancestors_or_self(doc: &Document, id: BlockId) -> impl Iterator<Item = BlockId> + '_ {
	std::iter::successors(Some(id), |&current| {
		doc.block(current).and_then(Block::parent)
	})
	.filter(|&current| doc.contains(current))
}

/// The parent of `id` (`None` for root children and unknown blocks).
#[must_use]
pub fn parent_of(doc: &Document, id: BlockId) -> Option<BlockId> {
	doc.block(id).and_then(Block::parent)
}

/// The length of a text block's content, or zero for other blocks.
#[must_use]
pub fn text_len(doc: &Document, id: BlockId) -> u32 {
	doc.inline(id).map_or(0, InlineText::len)
}

/// The point at the end of text block `id`.
#[must_use]
pub fn end_of(doc: &Document, id: BlockId) -> Point {
	Point::new(id, text_len(doc, id))
}

/// Whether `id` is a leaf (atomic) block.
#[must_use]
pub fn is_leaf(doc: &Document, id: BlockId) -> bool {
	doc.block(id)
		.is_some_and(|block| matches!(block.content(), Content::Leaf))
}

/// The deepest last descendant of `id`, or `id` itself.
#[must_use]
pub fn last_descendant(doc: &Document, id: BlockId) -> BlockId {
	let mut current = id;

	while let Some(&last) = doc.children(Some(current)).last() {
		current = last;
	}

	current
}

/// The caret at the start of the document, or a block selection of the
/// first block when the document has no text.
#[must_use]
pub fn start_selection(doc: &Document) -> Selection {
	if let Some(first) = doc.first_text_block() {
		return Selection::caret(Point::new(first, 0));
	}

	let first = doc.children(None).first().copied();
	first.map_or_else(
		|| Selection::caret(Point::new(BlockId::new(0, 0), 0)),
		Selection::block,
	)
}

/// The caret at the end of the document's last text block.
#[must_use]
pub fn end_selection(doc: &Document) -> Selection {
	doc.last_text_block().map_or_else(
		|| start_selection(doc),
		|last| Selection::caret(end_of(doc, last)),
	)
}

/// Repair `selection` so that it refers to blocks and offsets that exist.
///
/// Offsets are clamped to the text and moved back to a character boundary;
/// selections whose blocks vanished fall back to the start of the document.
#[must_use]
pub fn valid_selection(doc: &Document, selection: Selection) -> Selection {
	match selection {
		Selection::Text { anchor, head } => {
			match (valid_point(doc, anchor), valid_point(doc, head)) {
				(Some(anchor), Some(head)) => Selection::Text { anchor, head },
				(Some(point), None) | (None, Some(point)) => Selection::caret(point),
				(None, None) => start_selection(doc),
			}
		}
		Selection::Blocks { anchor, head } => {
			let siblings = doc.contains(anchor)
				&& doc.contains(head)
				&& parent_of(doc, anchor) == parent_of(doc, head);

			if siblings {
				selection
			} else {
				start_selection(doc)
			}
		}
	}
}

fn valid_point(doc: &Document, point: Point) -> Option<Point> {
	let text = doc.inline(point.block)?.as_str();
	let mut offset = (point.offset as usize).min(text.len());

	while !text.is_char_boundary(offset) {
		offset -= 1;
	}

	Some(Point::new(point.block, offset as u32))
}

/// The children of a common parent that cover two blocks: the parent (or
/// `None` for the root) and the child index range, end exclusive.
///
/// For two points inside one list item this is the item's children; for
/// points in two items of one list, the list's items.
#[must_use]
pub fn sibling_range(
	doc: &Document,
	first: BlockId,
	last: BlockId,
) -> Option<(Option<BlockId>, Range<usize>)> {
	let first_chain: Vec<BlockId> = ancestors_or_self(doc, first).collect();

	for candidate in ancestors_or_self(doc, last) {
		let parent = parent_of(doc, candidate);
		let start = first_chain
			.iter()
			.find(|&&block| parent_of(doc, block) == parent)
			.copied();

		if let Some(start) = start {
			let start = doc.index_in_parent(start)?;
			let end = doc.index_in_parent(candidate)?;

			return (start <= end).then(|| (parent, start..end + 1));
		}
	}

	None
}

/// The text blocks from `range.from` to `range.to` with the part of each
/// that the range covers, in document order.
#[must_use]
pub fn covered_text(doc: &Document, range: TextRange) -> Vec<(BlockId, Range<u32>)> {
	let mut covered = Vec::new();
	let mut current = Some(range.from.block);

	while let Some(block) = current {
		let start = if block == range.from.block {
			range.from.offset
		} else {
			0
		};

		if block == range.to.block {
			covered.push((block, start..range.to.offset));
			break;
		}

		covered.push((block, start..text_len(doc, block)));
		current = doc.next_text_block(block);
	}

	covered
}

/// Every text block inside the selected blocks, with its full range.
#[must_use]
pub fn covered_blocks(
	doc: &Document,
	anchor: BlockId,
	head: BlockId,
) -> Vec<(BlockId, Range<u32>)> {
	let Some((parent, range)) = sibling_range(doc, anchor, head) else {
		return Vec::new();
	};
	let mut covered = Vec::new();

	for &child in &doc.children(parent)[range] {
		collect_text(doc, child, &mut covered);
	}

	covered
}

fn collect_text(doc: &Document, id: BlockId, covered: &mut Vec<(BlockId, Range<u32>)>) {
	if doc.inline(id).is_some() {
		covered.push((id, 0..text_len(doc, id)));
	}

	for &child in doc.children(Some(id)) {
		collect_text(doc, child, covered);
	}
}

/// The text ranges a selection covers, whichever kind it is.
#[must_use]
pub fn selected_text(doc: &Document, selection: Selection) -> Vec<(BlockId, Range<u32>)> {
	match selection {
		Selection::Text { .. } => {
			text_range(doc, selection).map_or_else(Vec::new, |range| covered_text(doc, range))
		}
		Selection::Blocks { anchor, head } => covered_blocks(doc, anchor, head),
	}
}

/// A place the caret can stop: a point in text, or an atomic block that is
/// selected as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
	/// A text position.
	Text(Point),
	/// A leaf block such as a divider or image.
	Leaf(BlockId),
}

/// The first text block or leaf after `id` and its descendants.
#[must_use]
pub fn next_stop(doc: &Document, id: BlockId) -> Option<Stop> {
	let mut current = doc.next_block(last_descendant(doc, id));

	while let Some(block) = current {
		if doc.inline(block).is_some() {
			return Some(Stop::Text(Point::new(block, 0)));
		}

		if is_leaf(doc, block) {
			return Some(Stop::Leaf(block));
		}

		current = doc.next_block(block);
	}

	None
}

/// The last text block or leaf before `id`, outside its ancestors.
#[must_use]
pub fn prev_stop(doc: &Document, id: BlockId) -> Option<Stop> {
	let mut current = doc.prev_block(id);

	while let Some(block) = current {
		if doc.inline(block).is_some() {
			return Some(Stop::Text(end_of(doc, block)));
		}

		if is_leaf(doc, block) {
			return Some(Stop::Leaf(block));
		}

		current = doc.prev_block(block);
	}

	None
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::testing;

	#[test]
	fn orders_points_across_nesting() {
		let doc = testing::nested_doc();
		let texts: Vec<BlockId> = doc.text_blocks().map(Block::id).collect();

		for pair in texts.windows(2) {
			assert_eq!(compare_blocks(&doc, pair[0], pair[1]), Ordering::Less);
		}

		let list = doc.children(None)[1];
		assert_eq!(compare_blocks(&doc, list, texts[1]), Ordering::Less);
	}

	#[test]
	fn sibling_range_finds_the_common_parent() {
		let doc = testing::nested_doc();
		let texts: Vec<BlockId> = doc.text_blocks().map(Block::id).collect();
		let list = doc.children(None)[1];

		assert_eq!(
			sibling_range(&doc, texts[1], texts[2]),
			Some((Some(list), 0..2))
		);
		assert_eq!(sibling_range(&doc, texts[0], texts[3]), Some((None, 0..3)));
		assert_eq!(sibling_range(&doc, texts[3], texts[0]), None);
	}

	#[test]
	fn repairs_invalid_selections() {
		let doc = testing::nested_doc();
		let first = doc.first_text_block().expect("text");
		let broken = Selection::caret(Point::new(first, 99));

		assert_eq!(
			valid_selection(&doc, broken),
			Selection::caret(Point::new(first, 5))
		);
		let gone = Selection::caret(Point::new(BlockId::new(7, 7), 0));
		assert_eq!(valid_selection(&doc, gone), start_selection(&doc));
	}
}
