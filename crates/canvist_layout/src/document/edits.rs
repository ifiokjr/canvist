//! Reading a block's text edit out of a commit's steps.

use canvist_model::BlockId;
use canvist_model::Step;

use crate::text::TextEdit;

/// The combined edit `steps` made to the text of `block`, or `None` when a
/// step did more than edit its text (splitting, joining, or retyping it), so
/// its layout must be rebuilt.
pub(crate) fn text_edit(block: BlockId, steps: &[Step]) -> Option<TextEdit> {
	let mut combined: Option<TextEdit> = None;

	for step in steps {
		let edit = match step_edit(block, step) {
			Change::Edit(edit) => edit,
			Change::None => continue,
			Change::Rebuild => return None,
		};

		combined = Some(combined.map_or(edit, |combined| combined.then(edit)));
	}

	combined
}

/// What one step did to a block's text.
enum Change {
	/// Nothing.
	None,
	/// Edited it.
	Edit(TextEdit),
	/// Something an edit cannot describe.
	Rebuild,
}

/// What a step does, before asking which block it applies to.
enum Effect<'a> {
	/// Edits the text of one block.
	Edit(BlockId, TextEdit),
	/// Changes these blocks in a way an edit cannot describe.
	Rebuild(&'a [BlockId]),
	/// Leaves every block's text alone.
	Other,
}

fn step_edit(block: BlockId, step: &Step) -> Change {
	match effect(step) {
		Effect::Edit(target, edit) if target == block => Change::Edit(edit),
		Effect::Rebuild(targets) if targets.contains(&block) => Change::Rebuild,
		Effect::Edit(..) | Effect::Rebuild(_) | Effect::Other => Change::None,
	}
}

/// Which blocks `step` changes, and how.
fn effect(step: &Step) -> Effect<'_> {
	match step {
		Step::InsertInline {
			block,
			offset,
			content,
		} => Effect::Edit(*block, TextEdit::insert(*offset, content.len())),
		Step::RemoveInline { block, range } => Effect::Edit(*block, TextEdit::remove(range)),
		Step::AddMark { block, range, .. } | Step::RemoveMark { block, range, .. } => {
			Effect::Edit(*block, TextEdit::restyle(range))
		}
		Step::RestoreMarks {
			block,
			offset,
			runs,
		} => {
			let length: u32 = runs.iter().map(|(length, _)| *length).sum();

			Effect::Edit(*block, TextEdit::restyle(&(*offset..*offset + length)))
		}
		Step::SetBlock { block, .. } | Step::Split { block, .. } | Step::Join { block, .. } => {
			Effect::Rebuild(std::slice::from_ref(block))
		}
		_ => Effect::Other,
	}
}
