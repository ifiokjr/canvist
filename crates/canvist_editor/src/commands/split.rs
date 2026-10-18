//! Enter: splitting a text block in two.

use canvist_model::Point;
use canvist_model::Selection;

use super::CommandResult;
use super::active_marks;
use crate::EditKind;
use crate::EditorError;
use crate::EditorState;
use crate::EditorTransaction;
use crate::position;
use crate::position::parent_of;
use crate::structure;

/// Split the text block at the caret, replacing any selected text first.
///
/// - In the middle, both halves keep the block's type, so a heading stays a
///   heading on both sides.
/// - At the end, the new block is the parent's default text block, so Enter
///   after a heading starts a paragraph.
/// - At the start of a non-empty block, an empty default block is inserted
///   above and the caret stays with the content.
///
/// The active marks carry over, so typing continues in the same style.
pub fn split_block(state: &mut EditorState) -> CommandResult {
	let Some(range) = position::text_range(state.doc(), state.selection()) else {
		return Ok(None);
	};
	let marks = active_marks(state);

	state.change(EditKind::Structure, |tr| {
		let at = structure::delete_range(tr, range.from, range.to)?;
		let Some(caret) = split_text_block(tr, at)? else {
			return Ok(false);
		};
		tr.set_selection(Selection::caret(caret));

		if !marks.is_empty() {
			tr.set_stored_marks(Some(marks));
		}

		Ok(true)
	})
}

/// Split the text block at `at`, returning the caret position afterwards,
/// or `None` if the parent accepts no text block to split into.
pub(crate) fn split_text_block(
	tr: &mut EditorTransaction<'_>,
	at: Point,
) -> Result<Option<Point>, EditorError> {
	let doc = tr.doc();
	let parent = parent_of(doc, at.block);
	let index = structure::index_of(doc, at.block)?;
	let len = position::text_len(doc, at.block);
	let Some(block) = doc.block(at.block) else {
		return Ok(None);
	};
	let (kind, attrs) = (block.kind(), block.attrs().clone());
	let default = structure::default_text_type(doc, parent);

	if at.offset == len {
		let Some(default) = default else {
			return Ok(None);
		};
		let fresh = structure::fresh_block(tr, default)?;
		let caret = Point::new(fresh.id, 0);
		tr.insert_blocks(parent, index + 1, vec![fresh])?;

		return Ok(Some(caret));
	}

	if at.offset == 0 {
		let fresh = structure::fresh_block(tr, default.unwrap_or(kind))?;
		tr.insert_blocks(parent, index, vec![fresh])?;

		return Ok(Some(at));
	}

	Ok(Some(tr.split(at, kind, attrs)?))
}
