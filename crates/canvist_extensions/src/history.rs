//! Undo and redo keys.

use std::sync::Arc;

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::Extension;
use canvist_core::Platform;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_model::Schema;

/// `Mod-z` undoes; `Mod-Shift-z` and `Mod-y` redo. The history itself
/// belongs to the editor (see [`canvist_core::History`]), so a
/// collaborative history works with these keys too.
#[derive(Clone, Copy, Debug, Default)]
pub struct History;

/// Typed access to [`History`]. Ask the editor whether there is anything to
/// undo with [`Editor::can_undo`](canvist_core::Editor::can_undo).
#[derive(Clone, Copy, Debug)]
pub struct HistoryHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl HistoryHandle {
	/// Undo the last group of changes.
	#[must_use]
	pub fn undo(self) -> Command {
		undo()
	}

	/// Redo the last undone group.
	#[must_use]
	pub fn redo(self) -> Command {
		redo()
	}
}

fn undo() -> Command {
	Arc::new(canvist_core::undo)
}

fn redo() -> Command {
	Arc::new(canvist_core::redo)
}

impl Extension for History {
	fn name(&self) -> &'static str {
		"history"
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![
			CommandSpec::simple("undo", "Undo the last group of changes.", undo()),
			CommandSpec::simple("redo", "Redo the last undone group.", redo()),
		])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![
			("Mod-z", undo()),
			("Mod-Shift-z", redo()),
			("Mod-y", redo()),
		])
	}
}

impl TypedExtension for History {
	type Handle = HistoryHandle;

	fn handle(&self, registration: Registration) -> HistoryHandle {
		HistoryHandle {
			_registration: registration,
		}
	}
}
