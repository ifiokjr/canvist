//! Block commands: changing text block types, wrapping and lifting, block
//! attributes, and inserting atomic blocks.

use std::ops::Range;

use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTypeId;
use canvist_model::ContentKind;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Selection;

use super::CommandResult;
use super::split;
use crate::EditKind;
use crate::EditorState;
use crate::position;
use crate::position::parent_of;
use crate::structure;
use crate::structure::kind_of;

/// The text blocks the selection touches, in document order.
#[must_use]
pub fn selected_text_blocks(state: &EditorState) -> Vec<BlockId> {
	position::selected_text(state.doc(), state.selection())
		.into_iter()
		.map(|(block, _)| block)
		.collect()
}

/// Whether every selected text block has type `kind` and carries `attrs`.
#[must_use]
pub fn is_block_active(state: &EditorState, kind: BlockTypeId, attrs: &Attrs) -> bool {
	let blocks = selected_text_blocks(state);
	let doc = state.doc();

	!blocks.is_empty()
		&& blocks.iter().all(|&id| {
			doc.block(id).is_some_and(|block| {
				block.kind() == kind
					&& attrs
						.iter()
						.all(|(key, value)| block.attrs().get(key) == Some(value))
			})
		})
}

/// Set every selected text block that may take it to type `kind` with
/// `attrs`, e.g. a heading of level 2.
pub fn set_block_type(state: &mut EditorState, kind: BlockTypeId, attrs: &Attrs) -> CommandResult {
	let doc = state.doc();

	if doc.schema().block(kind).content != ContentKind::Inline {
		return Ok(None);
	}

	let blocks: Vec<BlockId> = selected_text_blocks(state)
		.into_iter()
		.filter(|&id| structure::allows(doc, parent_of(doc, id), kind))
		.collect();

	if blocks.is_empty() {
		return Ok(None);
	}

	state.change(EditKind::Structure, |tr| {
		for block in blocks {
			tr.set_block(block, kind, attrs.clone())?;
		}

		Ok(true)
	})
}

/// Set the selected text blocks to `kind` with `attrs`, or back to the
/// default text block if they all already are.
pub fn toggle_block_type(
	state: &mut EditorState,
	kind: BlockTypeId,
	attrs: &Attrs,
) -> CommandResult {
	if !is_block_active(state, kind, attrs) {
		return set_block_type(state, kind, attrs);
	}

	let default = state.schema().default_text_block();
	let default_attrs = structure::default_attrs(state.doc(), default);
	set_block_type(state, default, &default_attrs)
}

/// Merge `attrs` into block `block`'s attributes, e.g. checking a task item.
pub fn set_block_attrs(state: &mut EditorState, block: BlockId, attrs: &Attrs) -> CommandResult {
	let Some(current) = state.doc().block(block) else {
		return Ok(None);
	};
	let kind = current.kind();
	let mut merged = current.attrs().clone();

	for (key, value) in attrs.iter() {
		merged.set(key, value.clone());
	}

	if &merged == current.attrs() {
		return Ok(None);
	}

	state.change(EditKind::Structure, |tr| {
		tr.set_block(block, kind, merged)?;
		Ok(true)
	})
}

/// The sibling blocks a selection covers.
fn selected_siblings(state: &EditorState) -> Option<(Option<BlockId>, Range<usize>)> {
	let doc = state.doc();

	match state.selection() {
		Selection::Blocks { anchor, head } => {
			position::sibling_range(doc, anchor, head)
				.or_else(|| position::sibling_range(doc, head, anchor))
		}
		selection @ Selection::Text { .. } => {
			let range = position::text_range(doc, selection)?;
			position::sibling_range(doc, range.from.block, range.to.block)
		}
	}
}

/// Wrap the selected sibling blocks in a new `kind` container.
pub fn wrap_in(state: &mut EditorState, kind: BlockTypeId, attrs: Attrs) -> CommandResult {
	let Some((parent, range)) = selected_siblings(state) else {
		return Ok(None);
	};

	if !structure::can_wrap(state.doc(), parent, &range, kind) {
		return Ok(None);
	}

	let selection = state.selection();

	state.change(EditKind::Structure, |tr| {
		structure::wrap(tr, parent, range, kind, attrs)?;
		tr.set_selection(selection);
		Ok(true)
	})
}

/// Enter in an empty text block inside a container (such as a quote):
/// lift the block out instead of adding another empty line.
pub fn lift_empty_block(state: &mut EditorState) -> CommandResult {
	let Some(caret) = state.selection().as_caret() else {
		return Ok(None);
	};
	let doc = state.doc();
	let empty = doc
		.inline(caret.block)
		.is_some_and(canvist_model::InlineText::is_empty);
	let container = parent_of(doc, caret.block).filter(|_| empty);
	let Some((container, index)) = container.zip(doc.index_in_parent(caret.block)) else {
		return Ok(None);
	};

	lift_from(state, container, index..index + 1)
}

/// Lift the selected sibling blocks out of their container.
pub fn lift_block(state: &mut EditorState) -> CommandResult {
	let Some((Some(container), range)) = selected_siblings(state) else {
		return Ok(None);
	};

	lift_from(state, container, range)
}

fn lift_from(state: &mut EditorState, container: BlockId, range: Range<usize>) -> CommandResult {
	if !structure::can_lift(state.doc(), container, &range) {
		return Ok(None);
	}

	let selection = state.selection();

	state.change(EditKind::Structure, |tr| {
		structure::lift(tr, container, range)?;
		tr.set_selection(selection);
		Ok(true)
	})
}

/// Lift the selection out of the nearest `kind` container around it, or
/// wrap it in a new one, e.g. toggling a blockquote.
pub fn toggle_wrap(state: &mut EditorState, kind: BlockTypeId, attrs: Attrs) -> CommandResult {
	let doc = state.doc();
	let Some((parent, range)) = selected_siblings(state) else {
		return Ok(None);
	};
	let first = doc.children(parent)[range.start];
	let wrapper = position::ancestors_or_self(doc, first)
		.skip(1)
		.find(|&id| kind_of(doc, id) == Some(kind));

	let Some(wrapper) = wrapper else {
		return wrap_in(state, kind, attrs);
	};
	let last = doc.children(parent)[range.end - 1];
	let start = child_index(doc, wrapper, first);
	let end = child_index(doc, wrapper, last);

	match (start, end) {
		(Some(start), Some(end)) => lift_from(state, wrapper, start..end + 1),
		_ => Ok(None),
	}
}

/// The index of the child of `ancestor` that contains `id`.
fn child_index(doc: &Document, ancestor: BlockId, id: BlockId) -> Option<usize> {
	position::ancestors_or_self(doc, id)
		.find(|&block| parent_of(doc, block) == Some(ancestor))
		.and_then(|child| doc.index_in_parent(child))
}

/// Insert an atomic block such as a horizontal rule at the caret, splitting
/// the text block if the caret is inside it. The caret ends up in the text
/// block after the new block, which is created if needed.
pub fn insert_leaf(state: &mut EditorState, kind: BlockTypeId, attrs: Attrs) -> CommandResult {
	let doc = state.doc();
	let Some(range) = position::text_range(doc, state.selection()) else {
		return Ok(None);
	};
	let parent = parent_of(doc, range.from.block);
	let is_leaf = doc.schema().block(kind).content == ContentKind::Leaf;

	if !is_leaf || !structure::allows(doc, parent, kind) {
		return Ok(None);
	}

	state.change(EditKind::Structure, |tr| {
		let at = structure::delete_range(tr, range.from, range.to)?;
		let len = position::text_len(tr.doc(), at.block);
		let index = structure::index_of(tr.doc(), at.block)?;
		let leaf = structure::fresh_block(tr, kind)?.with_attrs(attrs);

		if at.offset == 0 {
			tr.insert_blocks(parent, index, vec![leaf])?;
			tr.set_selection(Selection::caret(at));
			return Ok(true);
		}

		let caret = if at.offset == len {
			let Some(caret) = split::split_text_block(tr, at)? else {
				return Ok(false);
			};
			caret
		} else {
			let block = tr.doc().block(at.block);
			let Some((kind, attrs)) = block.map(|block| (block.kind(), block.attrs().clone()))
			else {
				return Ok(false);
			};
			tr.split(at, kind, attrs)?
		};

		tr.insert_blocks(parent, index + 1, vec![leaf])?;
		tr.set_selection(Selection::caret(Point::new(caret.block, 0)));
		Ok(true)
	})
}
