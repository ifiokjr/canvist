//! The implementation of each step.

use std::ops::Range;

use super::Applied;
use super::Step;
use super::StepError;
use super::StepMap;
use crate::Attrs;
use crate::BlockId;
use crate::BlockTree;
use crate::BlockTypeId;
use crate::Content;
use crate::ContentKind;
use crate::Document;
use crate::InlineError;
use crate::InlineText;
use crate::MarkSet;
use crate::TreeContent;
use crate::selection::Point;

fn text_of(doc: &Document, block: BlockId) -> Result<&InlineText, StepError> {
	let found = doc.block(block).ok_or(StepError::UnknownBlock(block))?;
	found.inline().ok_or(StepError::NotTextBlock(block))
}

pub(super) fn insert_inline(
	doc: &mut Document,
	block: BlockId,
	offset: u32,
	content: InlineText,
) -> Result<Applied, StepError> {
	text_of(doc, block)?.check_offset(offset)?;
	let inserted = content.len();
	doc.inline_mut(block)?.insert(offset, content)?;

	let inverse = Step::RemoveInline {
		block,
		range: offset..offset + inserted,
	};
	let map = StepMap::Inline {
		block,
		offset,
		removed: 0,
		inserted,
	};
	Ok(Applied::inline(inverse, map, block))
}

pub(super) fn remove_inline(
	doc: &mut Document,
	block: BlockId,
	range: Range<u32>,
) -> Result<Applied, StepError> {
	text_of(doc, block)?.check_range(&range)?;
	let removed = doc.inline_mut(block)?.remove(range.clone())?;

	let map = StepMap::Inline {
		block,
		offset: range.start,
		removed: range.end - range.start,
		inserted: 0,
	};
	let inverse = Step::InsertInline {
		block,
		offset: range.start,
		content: removed,
	};
	Ok(Applied::inline(inverse, map, block))
}

pub(super) fn format(
	doc: &mut Document,
	block: BlockId,
	range: Range<u32>,
	change: impl FnOnce(&mut InlineText, Range<u32>) -> Result<(), InlineError>,
) -> Result<Applied, StepError> {
	let runs = text_of(doc, block)?.mark_runs(range.clone())?;
	change(doc.inline_mut(block)?, range.clone())?;

	let inverse = Step::RestoreMarks {
		block,
		offset: range.start,
		runs,
	};
	Ok(Applied::inline(inverse, StepMap::Identity, block))
}

pub(super) fn restore_marks(
	doc: &mut Document,
	block: BlockId,
	offset: u32,
	runs: &[(u32, MarkSet)],
) -> Result<Applied, StepError> {
	let len: u32 = runs.iter().map(|(len, _)| len).sum();
	format(doc, block, offset..offset + len, |text, _| {
		text.restore_marks(offset, runs)
	})
}

pub(super) fn set_block(
	doc: &mut Document,
	block: BlockId,
	kind: BlockTypeId,
	attrs: Attrs,
) -> Result<Applied, StepError> {
	let current = doc.block(block).ok_or(StepError::UnknownBlock(block))?;
	check_allowed(doc, current.parent(), kind)?;

	if !content_fits(doc, kind, current.content()) {
		return Err(StepError::ContentMismatch(block));
	}

	let target = doc.block_mut(block)?;
	let inverse = Step::SetBlock {
		block,
		kind: std::mem::replace(&mut target.kind, kind),
		attrs: std::mem::replace(&mut target.attrs, attrs),
	};
	target.touch();
	Ok(Applied::inline(inverse, StepMap::Identity, block))
}

fn content_fits(doc: &Document, kind: BlockTypeId, content: &Content) -> bool {
	match (&doc.schema().block(kind).content, content) {
		(ContentKind::Inline, Content::Inline(_)) | (ContentKind::Leaf, Content::Leaf) => true,
		(ContentKind::Blocks(allowed), Content::Blocks(children)) => {
			children
				.iter()
				.filter_map(|&child| doc.block(child))
				.all(|child| allowed.binary_search(&child.kind()).is_ok())
		}
		_ => false,
	}
}

fn check_allowed(
	doc: &Document,
	parent: Option<BlockId>,
	kind: BlockTypeId,
) -> Result<(), StepError> {
	let parent_kind = match parent {
		None => None,
		Some(id) => Some(doc.block(id).ok_or(StepError::UnknownBlock(id))?.kind()),
	};

	if doc.schema().allows_child(parent_kind, kind) {
		Ok(())
	} else {
		Err(StepError::NotAllowed { parent, kind })
	}
}

pub(super) fn split(
	doc: &mut Document,
	block: BlockId,
	offset: u32,
	tail: BlockTree,
) -> Result<Applied, StepError> {
	text_of(doc, block)?.check_offset(offset)?;
	let parent = doc.block(block).and_then(crate::Block::parent);
	let index = doc
		.index_in_parent(block)
		.ok_or(StepError::UnknownBlock(block))?;
	check_new_tree(doc, parent, &tail)?;

	let new_block = tail.id;
	let rest = doc.inline_mut(block)?.split_off(offset)?;
	let tail = BlockTree {
		content: TreeContent::Inline(rest),
		..tail
	};
	doc.insert_tree(parent, index + 1, tail);

	let inverse = Step::Join {
		block,
		next: new_block,
	};
	let map = StepMap::Split {
		block,
		offset,
		new_block,
	};
	Ok(Applied::structural(inverse, map, vec![block, new_block]))
}

pub(super) fn join(
	doc: &mut Document,
	block: BlockId,
	next: BlockId,
) -> Result<Applied, StepError> {
	let offset = text_of(doc, block)?.len();
	text_of(doc, next)?;
	let parent = doc.block(block).and_then(crate::Block::parent);
	let index = doc
		.index_in_parent(block)
		.ok_or(StepError::UnknownBlock(block))?;

	if doc.children(parent).get(index + 1) != Some(&next) {
		return Err(StepError::NotAdjacent { block, next });
	}

	doc.children_mut(parent)?.remove(index + 1);
	let removed = doc.detach(next)?;

	if let TreeContent::Inline(text) = removed.content {
		doc.inline_mut(block)?.append(text);
	}

	let inverse = Step::Split {
		block,
		offset,
		new_block: next,
		kind: removed.kind,
		attrs: removed.attrs,
	};
	let map = StepMap::Join {
		block,
		next,
		offset,
	};
	Ok(Applied::structural(inverse, map, vec![block]))
}

fn check_index(
	doc: &Document,
	parent: Option<BlockId>,
	start: usize,
	end: usize,
) -> Result<(), StepError> {
	if let Some(id) = parent {
		let block = doc.block(id).ok_or(StepError::UnknownBlock(id))?;

		if !matches!(block.content(), Content::Blocks(_)) {
			return Err(StepError::NotContainer(id));
		}
	}

	let len = doc.children(parent).len();

	if start > end || end > len {
		return Err(StepError::OutOfBounds { start, end, len });
	}

	Ok(())
}

/// Validate a tree that is about to be inserted under `parent`.
fn check_new_tree(
	doc: &Document,
	parent: Option<BlockId>,
	tree: &BlockTree,
) -> Result<(), StepError> {
	check_allowed(doc, parent, tree.kind)?;
	check_tree(doc, tree)
}

/// Validate a tree's ids and content against the document and schema.
fn check_tree(doc: &Document, tree: &BlockTree) -> Result<(), StepError> {
	if doc.contains(tree.id) {
		return Err(StepError::DuplicateId(tree.id));
	}

	match (&doc.schema().block(tree.kind).content, &tree.content) {
		(ContentKind::Inline, TreeContent::Inline(_)) | (ContentKind::Leaf, TreeContent::Leaf) => {
			Ok(())
		}
		(ContentKind::Blocks(allowed), TreeContent::Blocks(children)) => {
			children.iter().try_for_each(|child| {
				if allowed.binary_search(&child.kind).is_err() {
					return Err(StepError::NotAllowed {
						parent: Some(tree.id),
						kind: child.kind,
					});
				}

				check_tree(doc, child)
			})
		}
		_ => Err(StepError::ContentMismatch(tree.id)),
	}
}

fn check_unique_ids(blocks: &[BlockTree]) -> Result<(), StepError> {
	let mut seen = rustc_hash::FxHashSet::default();
	let mut duplicate = None;

	for tree in blocks {
		tree.walk(&mut |node| {
			if !seen.insert(node.id) {
				duplicate.get_or_insert(node.id);
			}
		});
	}

	duplicate.map_or(Ok(()), |id| Err(StepError::DuplicateId(id)))
}

pub(super) fn insert_blocks(
	doc: &mut Document,
	parent: Option<BlockId>,
	index: usize,
	blocks: Vec<BlockTree>,
) -> Result<Applied, StepError> {
	check_index(doc, parent, index, index)?;
	check_unique_ids(&blocks)?;

	for tree in &blocks {
		check_new_tree(doc, parent, tree)?;
	}

	let mut touched = Vec::new();

	for tree in &blocks {
		tree.walk(&mut |node| touched.push(node.id));
	}

	let count = blocks.len();

	for (offset, tree) in blocks.into_iter().enumerate() {
		doc.insert_tree(parent, index + offset, tree);
	}

	let inverse = Step::RemoveBlocks {
		parent,
		index,
		count,
	};
	Ok(Applied::structural(inverse, StepMap::Identity, touched))
}

pub(super) fn remove_blocks(
	doc: &mut Document,
	parent: Option<BlockId>,
	index: usize,
	count: usize,
) -> Result<Applied, StepError> {
	check_index(doc, parent, index, index + count)?;
	let ids: Vec<BlockId> = doc.children(parent)[index..index + count].to_vec();
	let fallback = removal_fallback(doc, &ids);

	doc.children_mut(parent)?.drain(index..index + count);
	let blocks = ids
		.into_iter()
		.map(|id| doc.detach(id))
		.collect::<Result<Vec<_>, _>>()?;

	let mut removed = Vec::new();

	for tree in &blocks {
		tree.walk(&mut |node| removed.push(node.id));
	}

	let map = StepMap::Removed {
		blocks: removed,
		fallback,
	};
	let inverse = Step::InsertBlocks {
		parent,
		index,
		blocks,
	};
	Ok(Applied::structural(inverse, map, Vec::new()))
}

/// The closest text position outside the blocks about to be removed: the
/// end of the previous text block, else the start of the next one.
fn removal_fallback(doc: &Document, ids: &[BlockId]) -> Option<Point> {
	let first = *ids.first()?;

	if let Some(previous) = doc.prev_text_block(first) {
		let len = doc.inline(previous).map_or(0, InlineText::len);

		return Some(Point::new(previous, len));
	}

	let last = *ids.last()?;
	let mut after = doc.next_block(last);

	while let Some(id) = after {
		if !doc.is_ancestor(last, id) && doc.inline(id).is_some() {
			return Some(Point::new(id, 0));
		}

		after = doc.next_block(id);
	}

	None
}

pub(super) type Source = (Option<BlockId>, usize, usize);
pub(super) type Target = (Option<BlockId>, usize);

pub(super) fn move_blocks(
	doc: &mut Document,
	(parent, index, count): Source,
	(to_parent, to_index): Target,
) -> Result<Applied, StepError> {
	check_index(doc, parent, index, index + count)?;
	check_index(doc, to_parent, 0, 0)?;
	let ids: Vec<BlockId> = doc.children(parent)[index..index + count].to_vec();

	for &id in &ids {
		if Some(id) == to_parent || to_parent.is_some_and(|target| doc.is_ancestor(id, target)) {
			return Err(StepError::MoveIntoSelf(id));
		}

		let kind = doc.block(id).ok_or(StepError::UnknownBlock(id))?.kind();
		check_allowed(doc, to_parent, kind)?;
	}
	let remaining = doc.children(to_parent).len() - if parent == to_parent { count } else { 0 };

	if to_index > remaining {
		return Err(StepError::OutOfBounds {
			start: to_index,
			end: to_index,
			len: remaining,
		});
	}

	doc.children_mut(parent)?.drain(index..index + count);
	doc.children_mut(to_parent)?
		.splice(to_index..to_index, ids.iter().copied());

	for &id in &ids {
		doc.set_parent(id, to_parent)?;
	}

	let inverse = Step::MoveBlocks {
		parent: to_parent,
		index: to_index,
		count,
		to_parent: parent,
		to_index: index,
	};
	Ok(Applied::structural(inverse, StepMap::Identity, Vec::new()))
}
