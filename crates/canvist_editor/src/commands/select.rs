//! Selection commands that do not move by units: select all, block
//! selections, and setting a selection from a pointer.

use canvist_model::BlockId;
use canvist_model::Selection;

use super::CommandResult;
use crate::EditKind;
use crate::EditorState;
use crate::position;
use crate::position::parent_of;

/// Replace the selection, repairing it if it points at missing blocks or
/// invalid offsets. Does nothing if the selection would not change.
pub fn set_selection(state: &mut EditorState, selection: Selection) -> CommandResult {
	let selection = position::valid_selection(state.doc(), selection);

	if selection == state.selection() {
		return Ok(None);
	}

	state.change(EditKind::Selection, |tr| {
		tr.set_selection(selection);
		Ok(true)
	})
}

/// Select the whole document: a text range when it starts and ends with
/// text, otherwise every top-level block.
pub fn select_all(state: &mut EditorState) -> CommandResult {
	let doc = state.doc();
	let roots = doc.children(None);
	let (Some(&first), Some(&last)) = (roots.first(), roots.last()) else {
		return Ok(None);
	};
	let first_text = doc
		.iter()
		.find(|block| block.children().is_empty())
		.filter(|block| block.inline().is_some());
	let last_text =
		Some(position::last_descendant(doc, last)).filter(|&id| doc.inline(id).is_some());

	let selection = match (first_text, last_text) {
		(Some(start), Some(end)) => {
			Selection::text(
				canvist_model::Point::new(start.id(), 0),
				position::end_of(doc, end),
			)
		}
		_ => {
			Selection::Blocks {
				anchor: first,
				head: last,
			}
		}
	};

	set_selection(state, selection)
}

/// Select the sibling blocks from `anchor` to `head` as objects.
pub fn select_blocks(state: &mut EditorState, anchor: BlockId, head: BlockId) -> CommandResult {
	let doc = state.doc();

	if !doc.contains(anchor) || parent_of(doc, anchor) != parent_of(doc, head) {
		return Ok(None);
	}

	set_selection(state, Selection::Blocks { anchor, head })
}

/// Escape-style selection growth: select the blocks the text selection
/// touches, then their parent container, and so on up to the root.
pub fn select_parent(state: &mut EditorState) -> CommandResult {
	let doc = state.doc();

	let selection = match state.selection() {
		Selection::Blocks { anchor, .. } => {
			let Some(parent) = parent_of(doc, anchor) else {
				return Ok(None);
			};
			Selection::block(parent)
		}
		text @ Selection::Text { .. } => {
			let Some(range) = position::text_range(doc, text) else {
				return Ok(None);
			};
			let Some((parent, range)) =
				position::sibling_range(doc, range.from.block, range.to.block)
			else {
				return Ok(None);
			};
			let children = doc.children(parent);
			Selection::Blocks {
				anchor: children[range.start],
				head: children[range.end - 1],
			}
		}
	};

	set_selection(state, selection)
}
