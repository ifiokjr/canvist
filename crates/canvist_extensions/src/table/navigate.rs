//! Moving between cells, and the keys that must not cross a cell's edge.

use canvist_editor::CommandResult;
use canvist_editor::EditKind;
use canvist_editor::EditorState;
use canvist_editor::position;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Selection;

use super::context::cell_around;
use super::context::current;
use super::context::selected;
use super::edit::add_row;
use super::edit::first_text;
use super::edit::is_empty_cell;

/// The text blocks inside `block`, in document order.
fn text_blocks(doc: &Document, block: BlockId) -> Vec<BlockId> {
	let mut found = Vec::new();
	let mut stack = vec![block];

	while let Some(next) = stack.pop() {
		match doc.block(next) {
			Some(found_block) if found_block.inline().is_some() => found.push(next),
			Some(_) => stack.extend(doc.children(Some(next)).iter().rev()),
			None => {}
		}
	}

	found
}

/// Select the whole content of the next cell (`forward`) or the previous
/// one, in document order, as TipTap's `goToNextCell` does. Does nothing at
/// the table's first or last cell.
pub(crate) fn go_to_cell(state: &mut EditorState, forward: bool) -> CommandResult {
	let Some((context, index)) = current(state) else {
		return Ok(None);
	};
	let target = if forward {
		index + 1
	} else {
		let Some(previous) = index.checked_sub(1) else {
			return Ok(None);
		};
		previous
	};

	if target >= context.map.cells().len() {
		return Ok(None);
	}

	let blocks = text_blocks(state.doc(), context.block(target));
	let (Some(&first), Some(&last)) = (blocks.first(), blocks.last()) else {
		return Ok(None);
	};
	let end = position::end_of(state.doc(), last);

	state.change(EditKind::Selection, |tr| {
		tr.set_selection(Selection::text(Point::new(first, 0), end));
		Ok(true)
	})
}

/// Tab: the next cell, or past the last one a new row and its first cell.
pub(crate) fn tab(state: &mut EditorState) -> CommandResult {
	if let Some(update) = go_to_cell(state, true)? {
		return Ok(Some(update));
	}

	if current(state).is_none() {
		return Ok(None);
	}

	state.atomic(|state| {
		let added = add_row(state, true)?;
		let moved = go_to_cell(state, true)?;

		Ok(added.zip(moved).map(|(added, moved)| added.then(moved)))
	})
}

/// Backspace at the start of a cell, or Delete at its end: swallow the key,
/// so text never joins across a cell's edge. A selection spanning cells
/// clears them instead.
pub(crate) fn guard_edge(state: &mut EditorState, forward: bool) -> CommandResult {
	if let Some(update) = clear_cells(state)? {
		return Ok(Some(update));
	}

	let Some(caret) = state.selection().as_caret() else {
		return Ok(None);
	};
	let doc = state.doc();
	let Some((cell, _)) = cell_around(doc, caret.block) else {
		return Ok(None);
	};
	let blocks = text_blocks(doc, cell);
	let at_edge = if forward {
		blocks.last() == Some(&caret.block) && caret.offset == position::text_len(doc, caret.block)
	} else {
		blocks.first() == Some(&caret.block) && caret.offset == 0
	};

	if !at_edge {
		return Ok(None);
	}

	let selection = state.selection();
	state.change(EditKind::Selection, |tr| {
		tr.set_selection(selection);
		Ok(true)
	})
}

/// Empty every cell of a selection that spans more than one, leaving each
/// with one empty paragraph and the caret in the first, as deleting a
/// TipTap cell selection does.
pub(crate) fn clear_cells(state: &mut EditorState) -> CommandResult {
	let Some((context, rect)) = selected(state) else {
		return Ok(None);
	};
	let cells = context.blocks_in(rect);

	if cells.len() < 2 {
		return Ok(None);
	}

	let paragraph = state.doc().schema().default_text_block();

	state.change(EditKind::Structure, |tr| {
		for &cell in &cells {
			if is_empty_cell(tr.doc(), cell) {
				continue;
			}

			let count = tr.doc().children(Some(cell)).len();
			tr.remove_blocks(Some(cell), 0, count)?;
			let id = tr.allocate_id()?;
			tr.insert_blocks(
				Some(cell),
				0,
				vec![BlockTree::text(id, paragraph, InlineText::new())],
			)?;
		}

		let start = first_text(tr.doc(), cells[0]);
		tr.set_selection(Selection::caret(Point::new(start, 0)));
		Ok(true)
	})
}
