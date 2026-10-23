//! Deleting: the selection, by unit before or after the caret, and joining
//! blocks when the caret sits at a block boundary.

use canvist_model::BlockId;
use canvist_model::Point;
use canvist_model::Selection;

use super::CommandResult;
use super::insert::caret_after_removal;
use crate::Caret;
use crate::Direction;
use crate::EditKind;
use crate::EditorState;
use crate::LineNavigator;
use crate::Unit;
use crate::motion;
use crate::position;
use crate::position::Stop;
use crate::position::parent_of;
use crate::structure;

/// Delete a non-empty selection: a text range (merging the blocks at its
/// ends) or a block selection.
pub fn delete_selection(state: &mut EditorState) -> CommandResult {
	match state.selection() {
		Selection::Blocks { anchor, head } => delete_blocks(state, anchor, head),
		selection @ Selection::Text { .. } => {
			let Some(range) = position::text_range(state.doc(), selection) else {
				return Ok(None);
			};

			if range.is_empty() {
				return Ok(None);
			}

			let kind = if range.from.block == range.to.block {
				EditKind::Deleting
			} else {
				EditKind::Structure
			};

			state.change(kind, |tr| {
				let caret = structure::delete_range(tr, range.from, range.to)?;
				tr.set_selection(Selection::caret(caret));
				Ok(true)
			})
		}
	}
}

fn delete_blocks(state: &mut EditorState, anchor: BlockId, head: BlockId) -> CommandResult {
	let doc = state.doc();
	let Some((parent, range)) = position::sibling_range(doc, anchor, head)
		.or_else(|| position::sibling_range(doc, head, anchor))
	else {
		return Ok(None);
	};
	let caret = caret_after_removal(state, parent, &range);

	state.change(EditKind::Structure, |tr| {
		tr.remove_blocks(parent, range.start, range.len())?;
		structure::prune_empty(tr, parent)?;

		if let Some(caret) = caret {
			tr.set_selection(Selection::caret(caret));
		}

		Ok(true)
	})
}

/// Backspace: delete the selection, or one `unit` before the caret. At the
/// start of a block, join it with what comes before ([`join_backward`]).
pub fn delete_backward(
	state: &mut EditorState,
	unit: Unit,
	lines: &mut dyn LineNavigator,
) -> CommandResult {
	delete_by(state, Direction::Backward, unit, lines)
}

/// Delete: delete the selection, or one `unit` after the caret. At the end
/// of a block, join the next block into it ([`join_forward`]).
pub fn delete_forward(
	state: &mut EditorState,
	unit: Unit,
	lines: &mut dyn LineNavigator,
) -> CommandResult {
	delete_by(state, Direction::Forward, unit, lines)
}

fn delete_by(
	state: &mut EditorState,
	direction: Direction,
	unit: Unit,
	lines: &mut dyn LineNavigator,
) -> CommandResult {
	let Some(caret) = state.selection().as_caret() else {
		return delete_selection(state);
	};
	let len = position::text_len(state.doc(), caret.block);

	let at_boundary = match direction {
		Direction::Backward => caret.offset == 0,
		Direction::Forward => caret.offset == len,
	};

	if at_boundary {
		return match direction {
			Direction::Backward => join_backward(state),
			Direction::Forward => join_forward(state),
		};
	}

	let from_caret = Caret {
		point: caret,
		affinity: state.affinity(),
	};
	let Some((Stop::Text(target), _)) =
		motion::target(state.doc(), from_caret, direction, unit, lines)?
	else {
		return Ok(None);
	};

	let (from, to) = match direction {
		Direction::Backward => (target, caret),
		Direction::Forward => (caret, target),
	};
	let kind = if from.block == to.block {
		EditKind::Deleting
	} else {
		EditKind::Structure
	};

	state.change(kind, |tr| {
		let caret = structure::delete_range(tr, from, to)?;
		tr.set_selection(Selection::caret(caret));
		Ok(true)
	})
}

/// Backspace at the start of a text block.
///
/// - The first child of a container is lifted out of it when the
///   container's parent accepts it (leaving a blockquote, for example).
/// - After a text block, the two are joined.
/// - After a container, the block joins the container's last text block.
/// - After an atomic block, that block is selected (removing this block
///   first if it is empty), so a second Backspace deletes it.
/// - An empty non-default block at the very start becomes the default type.
pub fn join_backward(state: &mut EditorState) -> CommandResult {
	let Some(caret) = state.selection().as_caret() else {
		return Ok(None);
	};

	if caret.offset != 0 {
		return Ok(None);
	}

	let doc = state.doc();
	let block = caret.block;
	let parent = parent_of(doc, block);
	let first = doc.children(parent).first() == Some(&block);

	if first {
		if let Some(container) = parent.filter(|&id| structure::can_lift(doc, id, &(0..1))) {
			return state.change(EditKind::Structure, |tr| {
				structure::lift(tr, container, 0..1)?;
				Ok(true)
			});
		}

		if parent.is_none() {
			return reset_empty_block(state, block);
		}
	}

	match position::prev_stop(doc, block) {
		Some(Stop::Text(target)) => merge_into(state, target.block, block),
		Some(Stop::Leaf(leaf)) => select_leaf(state, leaf, block),
		None => Ok(None),
	}
}

/// Delete at the end of a text block: join the next text block into this
/// one, or select the atomic block that follows.
pub fn join_forward(state: &mut EditorState) -> CommandResult {
	let Some(caret) = state.selection().as_caret() else {
		return Ok(None);
	};
	let doc = state.doc();

	if caret.offset != position::text_len(doc, caret.block) {
		return Ok(None);
	}

	match position::next_stop(doc, caret.block) {
		Some(Stop::Text(next)) => merge_into(state, caret.block, next.block),
		Some(Stop::Leaf(leaf)) => select_leaf(state, leaf, caret.block),
		None => Ok(None),
	}
}

fn merge_into(state: &mut EditorState, target: BlockId, source: BlockId) -> CommandResult {
	state.change(EditKind::Structure, |tr| {
		let caret = structure::merge_text(tr, target, source)?;
		tr.set_selection(Selection::caret(caret));
		Ok(true)
	})
}

/// Select the atomic block `leaf`, removing the empty text block the caret
/// was in so the leaf takes its place.
fn select_leaf(state: &mut EditorState, leaf: BlockId, from: BlockId) -> CommandResult {
	let empty = position::text_len(state.doc(), from) == 0;

	state.change(EditKind::Structure, |tr| {
		if empty {
			let parent = parent_of(tr.doc(), from);
			structure::remove(tr, from)?;
			structure::prune_empty(tr, parent)?;
		}

		tr.set_selection(Selection::block(leaf));
		Ok(true)
	})
}

/// Turn an empty heading (or other non-default block) at the start of the
/// document into the default text block.
fn reset_empty_block(state: &mut EditorState, block: BlockId) -> CommandResult {
	let doc = state.doc();
	let default = doc.schema().default_text_block();
	let current = structure::kind_of(doc, block);

	if position::text_len(doc, block) != 0 || current == Some(default) {
		return Ok(None);
	}

	let attrs = structure::default_attrs(doc, default);

	state.change(EditKind::Structure, |tr| {
		tr.set_block(block, default, attrs)?;
		tr.set_selection(Selection::caret(Point::new(block, 0)));
		Ok(true)
	})
}
