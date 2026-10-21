//! Clearing formatting: marks off the text, blocks back to paragraphs.

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandResult;
use canvist_core::CommandSpec;
use canvist_core::EditKind;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::Platform;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::Update;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::position;
use canvist_model::Attrs;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Selection;

/// The most containers `clearNodes` lifts the selection out of.
const MAX_DEPTH: usize = 16;

/// TipTap's `unsetAllMarks` and `clearNodes`, and Remirror's clear
/// formatting, which does both.
///
/// `unsetAllMarks` removes every mark from the selection (at a caret, from
/// what is typed next). `clearNodes` turns the selected text blocks into
/// paragraphs and lifts them out of quotes, callouts, and lists.
/// `clearFormatting` does both as one undo step, on `Mod-\`, as in Google
/// Docs.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClearFormatting;

/// Typed access to [`ClearFormatting`].
#[derive(Clone, Copy, Debug)]
pub struct ClearFormattingHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl ClearFormattingHandle {
	/// Remove every mark from the selection.
	#[must_use]
	pub fn unset_all_marks(self) -> Command {
		command(unset_all_marks)
	}

	/// Turn the selected blocks into plain paragraphs.
	#[must_use]
	pub fn clear_nodes(self) -> Command {
		command(clear_nodes)
	}

	/// Both.
	#[must_use]
	pub fn clear(self) -> Command {
		command(clear)
	}
}

/// Remove every mark from the selected text, or clear the stored marks at
/// a caret. Text with no marks changes nothing.
fn unset_all_marks(state: &mut EditorState) -> CommandResult {
	let mut ranges = position::selected_text(state.doc(), state.selection());
	ranges.retain(|(_, range)| !range.is_empty());

	if ranges.is_empty() {
		return state.change(EditKind::Selection, |tr| {
			tr.set_stored_marks(Some(MarkSet::empty()));
			Ok(true)
		});
	}

	let kinds: Vec<_> = (0..state.schema().marks().len())
		.map(|index| state.schema().marks()[index].id)
		.collect();
	let doc = state.doc();
	let marked = ranges.iter().any(|(block, range)| {
		doc.inline(*block)
			.and_then(|text| text.mark_runs(range.clone()).ok())
			.is_some_and(|runs| runs.iter().any(|(_, marks)| !marks.is_empty()))
	});

	if !marked {
		return Ok(None);
	}

	state.change(EditKind::Formatting, |tr| {
		for (block, range) in &ranges {
			for &kind in &kinds {
				tr.remove_mark(*block, range.clone(), kind)?;
			}
		}

		Ok(true)
	})
}

/// Paragraphs, then each selected block out of every container that lets
/// it go, one block at a time so a selection spanning containers still
/// unwraps.
fn clear_nodes(state: &mut EditorState) -> CommandResult {
	let paragraph = state.schema().default_text_block();
	let blocks = commands::selected_text_blocks(state);
	let selection = state.selection();

	let plain = Attrs::new();
	let doc = state.doc();
	let retype = blocks.iter().any(|&block| {
		doc.block(block)
			.is_some_and(|block| block.kind() != paragraph || *block.attrs() != plain)
	});

	state.atomic(|state| {
		let mut update = if retype {
			commands::set_block_type(state, paragraph, &plain)?
		} else {
			None
		};

		for &block in &blocks {
			let caret = Selection::caret(Point::new(block, 0));
			fold(&mut update, commands::set_selection(state, caret)?);
			fold(&mut update, lift_out(state)?);
		}

		// Moving the caret block to block is only the means: when no block
		// changed, nothing applied, and `atomic` puts the selection back.
		if !update.as_ref().is_some_and(Update::doc_changed) {
			return Ok(None);
		}

		let restored = position::valid_selection(state.doc(), selection);
		fold(&mut update, commands::set_selection(state, restored)?);

		Ok(update)
	})
}

/// Lift the caret's block out of every container and list item that lets
/// it go, at most `MAX_DEPTH` times.
fn lift_out(state: &mut EditorState) -> CommandResult {
	let mut update = None;

	for _ in 0..MAX_DEPTH {
		let Some(lifted) = lift(state)? else {
			break;
		};
		fold(&mut update, Some(lifted));
	}

	Ok(update)
}

/// Lift the caret's block out of its container or list item, once.
fn lift(state: &mut EditorState) -> CommandResult {
	if let Some(lifted) = commands::lift_block(state)? {
		return Ok(Some(lifted));
	}

	for name in ["listItem", "taskItem"] {
		let Some(item) = state.schema().block_type(name) else {
			continue;
		};

		if let Some(lifted) = commands::lift_list_item(state, item)? {
			return Ok(Some(lifted));
		}
	}

	Ok(None)
}

/// Append `next` to `update`.
fn fold(update: &mut Option<Update>, next: Option<Update>) {
	*update = match (update.take(), next) {
		(Some(first), Some(next)) => Some(first.then(next)),
		(first, next) => first.or(next),
	};
}

fn clear(state: &mut EditorState) -> CommandResult {
	state.atomic(|state| {
		let marks = unset_all_marks(state)?;
		let nodes = clear_nodes(state)?;

		Ok(match (marks, nodes) {
			(Some(marks), Some(nodes)) => Some(marks.then(nodes)),
			(marks, nodes) => marks.or(nodes),
		})
	})
}

impl Extension for ClearFormatting {
	fn name(&self) -> &'static str {
		"clearFormatting"
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![
			CommandSpec::simple(
				"unsetAllMarks",
				"Remove every mark from the selection.",
				command(unset_all_marks),
			),
			CommandSpec::simple(
				"clearNodes",
				"Turn the selected blocks into plain paragraphs, out of quotes and lists.",
				command(clear_nodes),
			),
			CommandSpec::simple(
				"clearFormatting",
				"Remove every mark and turn the selected blocks into plain paragraphs.",
				command(clear),
			),
		])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-\\", command(clear))])
	}
}

impl TypedExtension for ClearFormatting {
	type Handle = ClearFormattingHandle;

	fn handle(&self, registration: Registration) -> ClearFormattingHandle {
		ClearFormattingHandle {
			_registration: registration,
		}
	}
}
