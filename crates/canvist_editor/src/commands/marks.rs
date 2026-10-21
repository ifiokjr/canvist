//! Mark commands: toggling, setting, and querying inline formatting.

use std::ops::Range;

use canvist_model::Attrs;
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

/// A text block, the offset its selected range starts at, and the
/// `(len, marks)` runs of that range.
type BlockRuns = (BlockId, u32, Vec<(u32, MarkSet)>);

/// Rewrite the attributes of mark `kind` across the selection, run by run,
/// so a selection spanning differently styled text keeps each run's other
/// attributes. `update` receives the run's current attributes (`None` for a
/// run without the mark) and returns the new ones, or `None` to remove the
/// mark. At a caret, it rewrites the stored marks instead.
///
/// TipTap's `setColor` and `unsetFontSize` work this way: they change one
/// attribute of the `textStyle` mark and leave the others alone.
pub fn update_mark_attrs(
	state: &mut EditorState,
	kind: MarkTypeId,
	update: &dyn Fn(Option<&Attrs>) -> Option<Attrs>,
) -> CommandResult {
	let ranges = non_empty_ranges(state);
	let schema = state.schema().clone();
	let rewrite = |marks: &MarkSet| {
		let current = marks.get(kind).map(|mark| &mark.attrs);

		match update(current) {
			Some(attrs) => marks.with(&Mark::with_attrs(kind, attrs), &schema),
			None => marks.without(kind),
		}
	};

	if ranges.is_empty() {
		let marks = rewrite(&active_marks(state));

		return store_marks(state, marks);
	}

	let runs: Vec<BlockRuns> = ranges
		.iter()
		.filter_map(|(block, range)| {
			let text = state.doc().inline(*block)?;
			Some((*block, range.start, text.mark_runs(range.clone()).ok()?))
		})
		.collect();

	state.change(EditKind::Formatting, |tr| {
		for (block, start, runs) in runs {
			let mut offset = start;

			for (len, marks) in runs {
				let range = offset..offset + len;
				offset += len;

				match rewrite(&marks).get(kind) {
					Some(mark) => tr.add_mark(block, range, mark.clone())?,
					None => tr.remove_mark(block, range, kind)?,
				}
			}
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
