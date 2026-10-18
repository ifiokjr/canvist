//! Bindable commands: what keymaps and named commands run.

use std::sync::Arc;

use canvist_model::Selection;
use canvist_model::Step;

use crate::CommandResult;
use crate::EditKind;
use crate::EditorState;
use crate::History;
use crate::LineNavigator;
use crate::Origin;
use crate::commands;

/// Everything a bound command may use: the state, the history (for undo
/// and redo), and the line navigator (for vertical and line motion).
pub struct Context<'a> {
	/// The editor state.
	pub state: &'a mut EditorState,
	/// The undo history.
	pub history: &'a mut dyn History,
	/// Visual line information from layout.
	pub lines: &'a dyn LineNavigator,
	/// The input rule that produced the last update, if nothing happened
	/// since.
	pub(crate) last_rule: Option<&'a AppliedRule>,
}

/// A command that keymaps, named-command tables, and hosts can run.
pub type Command = Arc<dyn Fn(&mut Context<'_>) -> CommandResult + Send + Sync>;

/// Wrap a plain state command, such as one from [`commands`], as a
/// bindable [`Command`].
pub fn command(run: impl Fn(&mut EditorState) -> CommandResult + Send + Sync + 'static) -> Command {
	Arc::new(move |context: &mut Context<'_>| run(context.state))
}

/// What an input rule replaced, so Backspace straight afterwards can
/// restore the typed text.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AppliedRule {
	/// The document version right after the rule.
	pub(crate) version: u64,
	/// The selection right after the rule.
	pub(crate) selection: Selection,
	/// The text whose typing triggered the rule.
	pub(crate) typed: String,
	/// The steps that revert the rule's update.
	pub(crate) undo_steps: Vec<Step>,
	/// The selection before the rule fired.
	pub(crate) selection_before: Selection,
}

/// Undo the last group of changes.
pub fn undo(context: &mut Context<'_>) -> CommandResult {
	context.history.undo(context.state)
}

/// Redo the last undone group.
pub fn redo(context: &mut Context<'_>) -> CommandResult {
	context.history.redo(context.state)
}

/// Revert an input rule that fired on the previous keystroke and type the
/// text that triggered it instead, so `## ` followed by Backspace leaves
/// the literal `## `. Does nothing once anything else has happened. The
/// result is a new edit in history, so undoing it reapplies the rule.
pub fn undo_input_rule(context: &mut Context<'_>) -> CommandResult {
	let Some(rule) = context.last_rule else {
		return Ok(None);
	};
	let fresh = rule.version == context.state.doc().version()
		&& rule.selection == context.state.selection();

	if !fresh {
		return Ok(None);
	}

	// Revert with the rule's own inverse steps rather than through the
	// history, so the history stays one unbroken chain of edits.
	let reverted = context.state.apply_steps(
		rule.undo_steps.iter().cloned(),
		EditKind::InputRule,
		Origin::Local,
		Some(rule.selection_before),
	)?;
	let typed = commands::insert_text(context.state, &rule.typed)?;

	Ok(Some(match typed {
		Some(typed) => reverted.then(typed),
		None => reverted,
	}))
}
