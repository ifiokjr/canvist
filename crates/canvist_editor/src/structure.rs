//! Structural building blocks shared by commands: splitting containers,
//! lifting and wrapping sibling ranges, merging text blocks across
//! containers, and removing what becomes empty.
//!
//! Each helper emits model steps through an [`EditorTransaction`] and reads
//! the document as changed so far, so commands can chain them freely.

use std::ops::Range;

use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::BlockTypeId;
use canvist_model::ContentKind;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Step;
use canvist_model::StepError;
use canvist_model::TreeContent;

use crate::EditorError;
use crate::EditorTransaction;
use crate::position;
use crate::position::parent_of;

/// The kind of block `id`.
pub(crate) fn kind_of(doc: &Document, id: BlockId) -> Option<BlockTypeId> {
	doc.block(id).map(canvist_model::Block::kind)
}

/// Whether `parent` (`None` is the root) may hold a block of `kind`.
pub(crate) fn allows(doc: &Document, parent: Option<BlockId>, kind: BlockTypeId) -> bool {
	let parent_kind = match parent {
		Some(id) => {
			let Some(kind) = kind_of(doc, id) else {
				return false;
			};
			Some(kind)
		}
		None => None,
	};

	doc.schema().allows_child(parent_kind, kind)
}

/// The first text block type `parent` accepts: what Enter creates there.
pub(crate) fn default_text_type(doc: &Document, parent: Option<BlockId>) -> Option<BlockTypeId> {
	match parent {
		Some(id) => text_type_in(doc.schema(), Some(kind_of(doc, id)?)),
		None => text_type_in(doc.schema(), None),
	}
}

/// The first text block type a `parent` type accepts (`None` is the root).
pub(crate) fn text_type_in(schema: &Schema, parent: Option<BlockTypeId>) -> Option<BlockTypeId> {
	let Some(parent) = parent else {
		return Some(schema.default_text_block());
	};
	let ContentKind::Blocks(allowed) = &schema.block(parent).content else {
		return None;
	};

	allowed
		.iter()
		.copied()
		.find(|&kind| schema.block(kind).content == ContentKind::Inline)
}

/// The default attributes of a block type.
pub(crate) fn default_attrs(doc: &Document, kind: BlockTypeId) -> Attrs {
	doc.schema().block(kind).attrs.clone()
}

/// An empty block tree of `kind` with a fresh id and default attributes.
pub(crate) fn fresh_block(
	tr: &mut EditorTransaction<'_>,
	kind: BlockTypeId,
) -> Result<BlockTree, EditorError> {
	let id = tr.allocate_id()?;
	let attrs = default_attrs(tr.doc(), kind);

	let content = match tr.doc().schema().block(kind).content {
		ContentKind::Inline => TreeContent::Inline(canvist_model::InlineText::new()),
		ContentKind::Blocks(_) => TreeContent::Blocks(Vec::new()),
		ContentKind::Leaf => TreeContent::Leaf,
	};

	Ok(BlockTree {
		id,
		kind,
		attrs,
		content,
	})
}

/// An empty copy of container `id` (same type and attributes, fresh id).
pub(crate) fn empty_copy(
	tr: &mut EditorTransaction<'_>,
	id: BlockId,
) -> Result<BlockTree, EditorError> {
	let block = tr.doc().block(id).ok_or(StepError::UnknownBlock(id))?;
	let (kind, attrs) = (block.kind(), block.attrs().clone());
	let id = tr.allocate_id()?;
	Ok(BlockTree::container(id, kind, Vec::new()).with_attrs(attrs))
}

/// The position of `id` among its siblings.
pub(crate) fn index_of(doc: &Document, id: BlockId) -> Result<usize, EditorError> {
	Ok(doc.index_in_parent(id).ok_or(StepError::UnknownBlock(id))?)
}

/// Move `container`'s children from `at` onwards into a new copy of it
/// placed right after it. Returns the copy, or `None` if nothing moved.
pub(crate) fn split_container(
	tr: &mut EditorTransaction<'_>,
	container: BlockId,
	at: usize,
) -> Result<Option<BlockId>, EditorError> {
	let len = tr.doc().children(Some(container)).len();

	if at >= len {
		return Ok(None);
	}

	let parent = parent_of(tr.doc(), container);
	let index = index_of(tr.doc(), container)?;
	let copy = empty_copy(tr, container)?;
	let copy_id = copy.id;
	tr.insert_blocks(parent, index + 1, vec![copy])?;
	tr.move_blocks((Some(container), at, len - at), (Some(copy_id), 0))?;
	Ok(Some(copy_id))
}

/// Whether the children `range` of `container` may move into its parent.
pub(crate) fn can_lift(doc: &Document, container: BlockId, range: &Range<usize>) -> bool {
	let Some(block) = doc.block(container) else {
		return false;
	};
	let target = block.parent();
	let children = doc.children(Some(container));

	children.get(range.clone()).is_some_and(|moved| {
		moved
			.iter()
			.filter_map(|&child| kind_of(doc, child))
			.all(|kind| allows(doc, target, kind))
	})
}

/// Lift the children `range` of `container` out into its parent, splitting
/// the container around them. The caller checks [`can_lift`].
pub(crate) fn lift(
	tr: &mut EditorTransaction<'_>,
	container: BlockId,
	range: Range<usize>,
) -> Result<(), EditorError> {
	let target = parent_of(tr.doc(), container);
	let index = index_of(tr.doc(), container)?;
	split_container(tr, container, range.end)?;
	tr.move_blocks(
		(Some(container), range.start, range.len()),
		(target, index + 1),
	)?;

	if tr.doc().children(Some(container)).is_empty() {
		tr.remove_blocks(target, index, 1)?;
	}

	Ok(())
}

/// Remove block `id` and its descendants.
pub(crate) fn remove(tr: &mut EditorTransaction<'_>, id: BlockId) -> Result<(), EditorError> {
	let parent = parent_of(tr.doc(), id);
	let index = index_of(tr.doc(), id)?;
	tr.remove_blocks(parent, index, 1)?;
	Ok(())
}

/// Remove `container` and each ancestor that is left without children.
pub(crate) fn prune_empty(
	tr: &mut EditorTransaction<'_>,
	container: Option<BlockId>,
) -> Result<(), EditorError> {
	let mut current = container;

	while let Some(id) = current {
		let empty = tr.doc().contains(id)
			&& tr.doc().inline(id).is_none()
			&& !position::is_leaf(tr.doc(), id)
			&& tr.doc().children(Some(id)).is_empty();

		if !empty {
			break;
		}

		current = parent_of(tr.doc(), id);
		remove(tr, id)?;
	}

	Ok(())
}

/// Append the content of text block `source` to text block `target`, which
/// comes before it in the document, and remove `source`.
///
/// Uses a `Join` step whenever the blocks can become siblings, so points in
/// `source` map onto `target`. When `source`'s old container has the same
/// type as `target`'s (two list items, two quotes), its remaining children
/// follow into `target`'s container, the way joining list items works in
/// every word processor. Returns the point where the two texts meet.
pub(crate) fn merge_text(
	tr: &mut EditorTransaction<'_>,
	target: BlockId,
	source: BlockId,
) -> Result<Point, EditorError> {
	let doc = tr.doc();
	let meet = position::end_of(doc, target);
	let target_parent = parent_of(doc, target);
	let source_parent = parent_of(doc, source);
	let source_kind = kind_of(doc, source).ok_or(StepError::UnknownBlock(source))?;
	let target_index = index_of(doc, target)?;
	let source_index = index_of(doc, source)?;

	if target_parent == source_parent && source_index == target_index + 1 {
		tr.join(target, source)?;
	} else if allows(doc, target_parent, source_kind) {
		tr.move_blocks(
			(source_parent, source_index, 1),
			(target_parent, target_index + 1),
		)?;
		tr.join(target, source)?;
	} else {
		let content = doc.inline(source).cloned().unwrap_or_default();
		tr.step(Step::InsertInline {
			block: target,
			offset: meet.offset,
			content,
		})?;
		remove(tr, source)?;
	}

	adopt_children(tr, target_parent, source_parent)?;
	prune_empty(tr, source_parent)?;
	Ok(meet)
}

/// Move the children of `from` to the end of `into` when both containers
/// share a type, then drop `from`.
fn adopt_children(
	tr: &mut EditorTransaction<'_>,
	into: Option<BlockId>,
	from: Option<BlockId>,
) -> Result<(), EditorError> {
	let (Some(into), Some(from)) = (into, from) else {
		return Ok(());
	};
	let doc = tr.doc();
	let same_kind = into != from && doc.contains(from) && kind_of(doc, into) == kind_of(doc, from);
	let count = doc.children(Some(from)).len();

	if !same_kind || count == 0 || doc.is_ancestor(from, into) {
		return Ok(());
	}

	let end = doc.children(Some(into)).len();
	tr.move_blocks((Some(from), 0, count), (Some(into), end))?;
	Ok(())
}

/// Delete everything between two text points, merging their blocks.
/// `from` must come before `to`. Returns where the caret belongs.
pub(crate) fn delete_range(
	tr: &mut EditorTransaction<'_>,
	from: Point,
	to: Point,
) -> Result<Point, EditorError> {
	if from.block == to.block {
		if from.offset < to.offset {
			tr.delete_text(from.block, from.offset..to.offset)?;
		}

		return Ok(from);
	}

	let covered = covered_subtrees(tr.doc(), from.block, to.block);
	tr.delete_text(to.block, 0..to.offset)?;
	let from_end = position::text_len(tr.doc(), from.block);
	tr.delete_text(from.block, from.offset..from_end)?;

	for id in covered.into_iter().rev() {
		remove(tr, id)?;
	}

	merge_text(tr, from.block, to.block)
}

/// The largest subtrees lying entirely between text blocks `first` and
/// `last`, in document order.
fn covered_subtrees(doc: &Document, first: BlockId, last: BlockId) -> Vec<BlockId> {
	let mut covered = Vec::new();
	let mut current = doc.next_block(first);

	while let Some(block) = current {
		if block == last {
			break;
		}

		if doc.is_ancestor(block, last) {
			current = doc.next_block(block);
			continue;
		}

		covered.push(block);
		current = doc.next_block(position::last_descendant(doc, block));
	}

	covered
}

#[cfg(test)]
mod tests {
	use canvist_model::Block;
	use canvist_model::Selection;

	use super::*;
	use crate::EditKind;
	use crate::testing;

	fn texts(doc: &Document) -> Vec<BlockId> {
		doc.text_blocks().map(Block::id).collect()
	}

	#[test]
	fn deleting_across_a_list_merges_the_ends() {
		let mut state = testing::nested_state();
		let ids = texts(state.doc());
		let update = state
			.change(EditKind::Deleting, |tr| {
				let caret = delete_range(tr, Point::new(ids[0], 2), Point::new(ids[3], 1))?;
				tr.set_selection(Selection::caret(caret));
				Ok(true)
			})
			.expect("valid range")
			.expect("applied");

		assert_eq!(state.doc().plain_text(), "inutro");
		assert_eq!(state.doc().children(None).len(), 1);
		assert_eq!(update.selection, Selection::caret(Point::new(ids[0], 2)));
	}
}
