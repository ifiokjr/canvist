//! Undo and redo keys.

use std::sync::Arc;

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::EditorError;
use canvist_editor::Extension;
use canvist_editor::Platform;
use canvist_model::Schema;

/// `Mod-z` undoes; `Mod-Shift-z` and `Mod-y` redo. The history itself
/// belongs to the editor (see [`canvist_editor::History`]), so a
/// collaborative history works with these keys too.
#[derive(Clone, Copy, Debug, Default)]
pub struct History;

fn undo() -> Command {
	Arc::new(canvist_editor::undo)
}

fn redo() -> Command {
	Arc::new(canvist_editor::redo)
}

impl Extension for History {
	fn name(&self) -> &'static str {
		"history"
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
		Ok(vec![("undo", undo()), ("redo", redo())])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![
			("Mod-z", undo()),
			("Mod-Shift-z", redo()),
			("Mod-y", redo()),
		])
	}
}
