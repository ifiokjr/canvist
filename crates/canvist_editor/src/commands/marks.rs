//! Mark commands: toggling, setting, and querying inline formatting.

use std::ops::Range;

use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::MarkTypeId;
use canvist_model::Point;

use super::CommandResult;
use crate::EditKind;
use crate::EditorState;
use crate::position;

/// The marks text typed at `point` receives, following ProseMirror's rules:
/// inside a run, that run's marks; at a run boundary, the marks before it,
/// minus non-inclusive marks (such as links) that do not continue after it.
#[must_use]
pub fn marks_at(doc: &Document, point: Point) -> MarkSet {
	let Some(text) = doc.inline(point.block) else {
		return MarkSet::empty();
	};
	let (main, other) = match (
		text.marks_before(point.offset),
		text.marks_after(point.offset),
	) {
		(Some(before), after) => (before, after),
		(None, Some(after)) => (after, None),

		(None, None) => return MarkSet::empty(),
	};
	let schema = doc.schema();

	main.as_slice()
		.iter()
		.filter(|mark| {
			!schema.mark(mark.kind).inclusive
				&& other.and_then(|set| set.get(mark.kind)) != Some(mark)
		})
		.fold(main.clone(), |marks, mark| marks.without(mark.kind))
}

/// The marks the next typed text receives: the stored marks if a mark was
/// toggled at the caret, otherwise the marks at the selection head.
#[must_use]
pub fn active_marks(state: &EditorState) -> MarkSet {
	if let Some(stored) = state.stored_marks() {
		return stored.clone();
	}

	match position::text_range(state.doc(), state.selection()) {
		Some(range) if range.is_empty() => marks_at(state.doc(), range.from),
		Some(range) => {
			state
				.doc()
				.inline(range.from.block)
				.and_then(|text| text.marks_after(range.from.offset))
				.cloned()
				.unwrap_or_default()
		}
		None => MarkSet::empty(),
	}
}

/// Whether mark `kind` is active: in the stored or caret marks for a caret,
/// or on every selected character for a range.
#[must_use]
pub fn is_mark_active(state: &EditorState, kind: MarkTypeId) -> bool {
	let ranges = non_empty_ranges(state);

	if ranges.is_empty() {
		return active_marks(state).contains(kind);
	}

	ranges
		.iter()
		.all(|(block, range)| covers(state.doc(), *block, range, kind))
}

/// Whether every character in `range` of `block` carries `kind`.
fn covers(doc: &Document, block: BlockId, range: &Range<u32>, kind: MarkTypeId) -> bool {
	let Some(text) = doc.inline(block) else {
		return false;
	};

	text.runs()
		.filter(|run| run.range.start < range.end && run.range.end > range.start)
		.all(|run| run.marks.contains(kind))
}

fn non_empty_ranges(state: &EditorState) -> Vec<(BlockId, Range<u32>)> {
	let mut ranges = position::selected_text(state.doc(), state.selection());
	ranges.retain(|(_, range)| !range.is_empty());
	ranges
}

/// Add `mark` to the selection, or remove its type if every selected
/// character already has it. At a caret, toggles the stored marks instead.
pub fn toggle_mark(state: &mut EditorState, mark: &Mark) -> CommandResult {
	if is_mark_active(state, mark.kind) {
		unset_mark(state, mark.kind)
	} else {
		set_mark(state, mark)
	}
}

/// Add `mark` to the selection, replacing marks of the same type (so it
/// also updates a link's `href`). At a caret, adds it to the stored marks.
pub fn set_mark(state: &mut EditorState, mark: &Mark) -> CommandResult {
	let ranges = non_empty_ranges(state);

	if ranges.is_empty() {
		let marks = active_marks(state).with(mark, state.schema());

		return store_marks(state, marks);
	}

	state.change(EditKind::Formatting, |tr| {
		for (block, range) in ranges {
			tr.add_mark(block, range, mark.clone())?;
		}

		Ok(true)
	})
}

/// Remove marks of type `kind` from the selection. At a caret, removes it
/// from the stored marks.
pub fn unset_mark(state: &mut EditorState, kind: MarkTypeId) -> CommandResult {
	let ranges = non_empty_ranges(state);

	if ranges.is_empty() {
		let marks = active_marks(state).without(kind);

		return store_marks(state, marks);
	}

	state.change(EditKind::Formatting, |tr| {
		for (block, range) in ranges {
			tr.remove_mark(block, range, kind)?;
		}

		Ok(true)
	})
}

fn store_marks(state: &mut EditorState, marks: MarkSet) -> CommandResult {
	if state.selection().as_caret().is_none() {
		return Ok(None);
	}

	state.change(EditKind::Selection, |tr| {
		tr.set_stored_marks(Some(marks));
		Ok(true)
	})
}
