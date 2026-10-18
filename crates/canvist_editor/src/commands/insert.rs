//! Inserting text and inline embeds at the selection.

use canvist_model::BlockId;
use canvist_model::Embed;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Selection;
use canvist_model::Step;

use super::CommandResult;
use super::active_marks;
use crate::EditKind;
use crate::EditorError;
use crate::EditorState;
use crate::EditorTransaction;
use crate::position;
use crate::structure;

/// Type `text` over the selection with the active marks, honouring stored
/// marks and mark inclusivity. Over a block selection, the blocks are
/// replaced by a new text block holding `text`.
pub fn insert_text(state: &mut EditorState, text: &str) -> CommandResult {
	if text.is_empty() {
		return Ok(None);
	}

	let marks = active_marks(state);
	insert_inline(state, InlineText::plain(text, marks))
}

/// Insert an inline embed (a mention, an inline image, a hard break) over
/// the selection, carrying the active marks.
pub fn insert_embed(state: &mut EditorState, embed: Embed) -> CommandResult {
	let marks = active_marks(state);
	insert_inline(state, InlineText::embed(embed, marks))
}

fn insert_inline(state: &mut EditorState, content: InlineText) -> CommandResult {
	if let Selection::Blocks { anchor, head } = state.selection() {
		return replace_blocks(state, anchor, head, content);
	}

	let Some(range) = position::text_range(state.doc(), state.selection()) else {
		return Ok(None);
	};
	state.change(EditKind::Typing, |tr| {
		let at = structure::delete_range(tr, range.from, range.to)?;
		let caret = insert_at(tr, at, content)?;
		tr.set_selection(Selection::caret(caret));
		Ok(true)
	})
}

/// Insert `content` at `at`, returning the point after it.
pub(crate) fn insert_at(
	tr: &mut EditorTransaction<'_>,
	at: Point,
	content: InlineText,
) -> Result<Point, EditorError> {
	let end = at.offset + content.len();
	tr.step(Step::InsertInline {
		block: at.block,
		offset: at.offset,
		content,
	})?;
	Ok(Point::new(at.block, end))
}

/// Replace selected blocks with one new text block holding `content`.
fn replace_blocks(
	state: &mut EditorState,
	anchor: BlockId,
	head: BlockId,
	content: InlineText,
) -> CommandResult {
	let doc = state.doc();
	let Some((parent, range)) = position::sibling_range(doc, anchor, head)
		.or_else(|| position::sibling_range(doc, head, anchor))
	else {
		return Ok(None);
	};
	let Some(kind) = structure::default_text_type(doc, parent) else {
		return Ok(None);
	};

	state.change(EditKind::Typing, |tr| {
		tr.remove_blocks(parent, range.start, range.len())?;
		let mut block = structure::fresh_block(tr, kind)?;
		let caret = Point::new(block.id, content.len());
		block.content = canvist_model::TreeContent::Inline(content);
		tr.insert_blocks(parent, range.start, vec![block])?;
		tr.set_selection(Selection::caret(caret));
		structure::prune_empty(tr, parent)?;
		Ok(true)
	})
}

/// The caret to place after removing blocks `range` of `parent`: the end of
/// the text before them, else the start of the text after them.
pub(crate) fn caret_after_removal(
	state: &EditorState,
	parent: Option<BlockId>,
	range: &std::ops::Range<usize>,
) -> Option<Point> {
	let doc = state.doc();
	let children = doc.children(parent);
	let first = *children.get(range.start)?;
	let last = *children.get(range.end.checked_sub(1)?)?;

	if let Some(before) = doc.prev_text_block(first) {
		return Some(position::end_of(doc, before));
	}

	let after = doc.next_text_block(position::last_descendant(doc, last))?;
	Some(Point::new(after, 0))
}
