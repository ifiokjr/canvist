//! Pasting a [`Slice`] over the selection.

use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::BlockTypeId;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Selection;
use canvist_model::TreeContent;

use super::CommandResult;
use super::Slice;
use super::delete_selection;
use super::insert::insert_at;
use super::slice::inline_of;
use crate::EditKind;
use crate::EditorError;
use crate::EditorState;
use crate::EditorTransaction;
use crate::position;
use crate::position::parent_of;
use crate::structure;

/// Paste `slice` over the selection. Open text blocks at either end merge
/// into the text around the caret; everything else is inserted as blocks,
/// converted or flattened where the destination does not accept them.
pub fn replace_selection(state: &mut EditorState, slice: Slice) -> CommandResult {
	if slice.is_empty() {
		return Ok(None);
	}

	if matches!(state.selection(), Selection::Blocks { .. }) {
		return replace_blocks(state, slice);
	}

	let Some(range) = position::text_range(state.doc(), state.selection()) else {
		return Ok(None);
	};

	state.change(EditKind::Paste, |tr| {
		let caret = structure::delete_range(tr, range.from, range.to)?;
		let blocks = slice
			.into_blocks()
			.into_iter()
			.map(|tree| fresh_ids(tr, tree))
			.collect::<Result<Vec<_>, _>>()?;
		let Some(caret) = paste_at(tr, caret, blocks)? else {
			return Ok(false);
		};

		tr.set_selection(Selection::caret(caret));
		Ok(true)
	})
}

/// Delete the selected blocks, then paste at the caret that leaves, as one
/// paste update.
fn replace_blocks(state: &mut EditorState, slice: Slice) -> CommandResult {
	let Some(cleared) = delete_selection(state)? else {
		return Ok(None);
	};
	let pasted = replace_selection(state, slice)?;

	let mut update = match pasted {
		Some(pasted) => cleared.then(pasted),
		None => cleared,
	};

	update.kind = EditKind::Paste;
	Ok(Some(update))
}

/// Give a tree and its descendants fresh ids.
fn fresh_ids(tr: &mut EditorTransaction<'_>, tree: BlockTree) -> Result<BlockTree, EditorError> {
	let id = tr.allocate_id()?;

	let content = match tree.content {
		TreeContent::Blocks(children) => {
			let children = children
				.into_iter()
				.map(|child| fresh_ids(tr, child))
				.collect::<Result<_, _>>()?;
			TreeContent::Blocks(children)
		}
		other => other,
	};

	Ok(BlockTree {
		id,
		content,
		..tree
	})
}

/// A slice taken apart: the open text at either end and the whole blocks
/// between them.
struct Parts {
	head: Option<InlineText>,
	middle: Vec<BlockTree>,
	tail: Option<InlineText>,
}

impl Parts {
	fn new(mut blocks: Vec<BlockTree>) -> Self {
		let last = blocks.len().saturating_sub(1);
		let tail = if last > 0 {
			take_open(&mut blocks, last)
		} else {
			None
		};

		let head = take_open(&mut blocks, 0);

		Self {
			head,
			middle: blocks,
			tail,
		}
	}
}

/// Remove the text block at `index` and return its text, if it is one.
fn take_open(blocks: &mut Vec<BlockTree>, index: usize) -> Option<InlineText> {
	let text = blocks.get(index).and_then(inline_of).cloned()?;
	blocks.remove(index);
	Some(text)
}

fn paste_at(
	tr: &mut EditorTransaction<'_>,
	caret: Point,
	blocks: Vec<BlockTree>,
) -> Result<Option<Point>, EditorError> {
	if let [only] = blocks.as_slice()
		&& let Some(text) = inline_of(only)
	{
		adopt_type(tr, caret.block, only.kind, only.attrs.clone())?;

		return insert_at(tr, caret, text.clone()).map(Some);
	}

	paste_blocks(tr, caret, Parts::new(blocks))
}

/// Split the block at `caret`, merge the open ends into the two halves,
/// and insert the whole blocks between them. An empty head left by pasting
/// at the very start is removed so the pasted blocks take its place.
fn paste_blocks(
	tr: &mut EditorTransaction<'_>,
	caret: Point,
	parts: Parts,
) -> Result<Option<Point>, EditorError> {
	let doc = tr.doc();
	let parent = parent_of(doc, caret.block);
	let index = structure::index_of(doc, caret.block)?;
	let Some((kind, attrs)) = structure::type_of(doc, caret.block) else {
		return Ok(None);
	};
	let head_empty = parts.head.is_none() && caret.offset == 0;
	let after = tr.split(caret, kind, attrs)?;

	if let Some(head) = parts.head {
		insert_at(tr, caret, head)?;
	}

	let end = match parts.tail {
		Some(tail) => insert_at(tr, after, tail)?,
		None => after,
	};

	let fitted: Vec<BlockTree> = parts
		.middle
		.into_iter()
		.flat_map(|tree| fit(tr.doc(), parent, tree))
		.collect();
	let last_text = fitted.last().and_then(last_text_in);
	tr.insert_blocks(parent, index + 1, fitted)?;

	if head_empty {
		structure::remove(tr, caret.block)?;
	}

	settle_caret(tr, end, after, last_text).map(Some)
}

/// Keep the caret after the pasted tail text, or, when the tail half is
/// empty, drop it and end at the last pasted text block instead.
fn settle_caret(
	tr: &mut EditorTransaction<'_>,
	end: Point,
	after: Point,
	last_text: Option<BlockId>,
) -> Result<Point, EditorError> {
	let tail_empty = position::text_len(tr.doc(), after.block) == 0;
	let Some(last) = last_text.filter(|_| tail_empty && end == after) else {
		return Ok(end);
	};

	structure::remove(tr, after.block)?;
	Ok(position::end_of(tr.doc(), last))
}

/// Retype an empty target block to match pasted text, e.g. pasting part of
/// a heading into an empty paragraph makes a heading.
fn adopt_type(
	tr: &mut EditorTransaction<'_>,
	block: BlockId,
	kind: BlockTypeId,
	attrs: Attrs,
) -> Result<(), EditorError> {
	let doc = tr.doc();
	let empty = position::text_len(doc, block) == 0;
	let differs = structure::kind_of(doc, block) != Some(kind);

	if empty && differs && structure::allows(doc, parent_of(doc, block), kind) {
		tr.set_block(block, kind, attrs)?;
	}

	Ok(())
}

/// The last text block in a tree.
fn last_text_in(tree: &BlockTree) -> Option<BlockId> {
	let mut last = None;
	tree.walk(&mut |node| last = inline_of(node).map_or(last, |_| Some(node.id)));
	last
}

/// Make `tree` acceptable inside `parent`: keep it if allowed, retype text
/// blocks to the parent's text type, and flatten containers otherwise.
fn fit(doc: &Document, parent: Option<BlockId>, tree: BlockTree) -> Vec<BlockTree> {
	if structure::allows(doc, parent, tree.kind) {
		return vec![tree];
	}

	match tree.content {
		TreeContent::Inline(_) => {
			let Some(kind) = structure::default_text_type(doc, parent) else {
				return Vec::new();
			};
			let attrs = structure::default_attrs(doc, kind);
			vec![BlockTree {
				kind,
				attrs,
				..tree
			}]
		}
		TreeContent::Blocks(children) => {
			children
				.into_iter()
				.flat_map(|child| fit(doc, parent, child))
				.collect()
		}
		TreeContent::Leaf => Vec::new(),
	}
}
