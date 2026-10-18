//! IME composition: preedit text that the input method may still replace.
//!
//! Hosts call [`set_composition`] for every preedit change and then
//! [`commit_composition`] or [`cancel_composition`]. Preedit updates share one
//! history kind so they collapse into a single undo step with the committed
//! text.

use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Selection;

use super::CommandResult;
use super::active_marks;
use super::insert::insert_at;
use crate::Composition;
use crate::EditKind;
use crate::EditorState;
use crate::position;
use crate::structure;

/// Start or update the preedit text, replacing the previous preedit (or the
/// selection, when composition starts).
pub fn set_composition(state: &mut EditorState, text: &str) -> CommandResult {
	replace_preedit(state, text, true)
}

/// Commit the final composed text and end the composition. Without an
/// active composition this simply types `text`.
pub fn commit_composition(state: &mut EditorState, text: &str) -> CommandResult {
	replace_preedit(state, text, false)
}

/// Remove the preedit text and end the composition.
pub fn cancel_composition(state: &mut EditorState) -> CommandResult {
	let Some(composition) = state.composition() else {
		return Ok(None);
	};

	state.change(EditKind::Composition, |tr| {
		let at = structure::delete_range(tr, composition.start_point(), composition.end_point())?;
		tr.set_composition(None);
		tr.set_selection(Selection::caret(at));
		Ok(true)
	})
}

fn replace_preedit(state: &mut EditorState, text: &str, keep: bool) -> CommandResult {
	let preedit = state
		.composition()
		.map(|composition| (composition.start_point(), composition.end_point()));
	let selected =
		position::text_range(state.doc(), state.selection()).map(|range| (range.from, range.to));
	let Some((from, to)) = preedit.or(selected) else {
		return Ok(None);
	};
	let marks = active_marks(state);
	let stored = state.stored_marks().cloned();
	let kind = if keep {
		EditKind::Composition
	} else {
		EditKind::Typing
	};

	state.change(kind, |tr| {
		let at = structure::delete_range(tr, from, to)?;
		let end = if text.is_empty() {
			at
		} else {
			insert_at(tr, at, InlineText::plain(text, marks))?
		};

		let composition = keep.then_some(Composition {
			block: at.block,
			start: at.offset,
			end: end.offset,
		});
		tr.set_composition(composition);
		tr.set_selection(Selection::caret(Point::new(end.block, end.offset)));

		if keep {
			tr.set_stored_marks(stored);
		}

		Ok(true)
	})
}
