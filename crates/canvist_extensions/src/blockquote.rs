//! Block quotes.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::EditorError;
use canvist_editor::Extension;
use canvist_editor::InputRule;
use canvist_editor::Platform;
use canvist_editor::block_type;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

/// A `blockquote` container for any blocks. `> ` at the start of a block
/// wraps it, and `Mod-Shift-b` toggles one.
#[derive(Clone, Copy, Debug, Default)]
pub struct Blockquote;

impl Blockquote {
	fn toggle(schema: &Schema) -> Result<Command, EditorError> {
		let quote = block_type(schema, "blockquote", "blockquote")?;
		Ok(command(move |state| {
			commands::toggle_wrap(state, quote, Attrs::new())
		}))
	}
}

impl Extension for Blockquote {
	fn name(&self) -> &'static str {
		"blockquote"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::container("blockquote", &["block"]))
	}

	fn commands(&self, schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
		Ok(vec![("toggleBlockquote", Self::toggle(schema)?)])
	}

	fn keymap(&self, schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-Shift-b", Self::toggle(schema)?)])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let quote = block_type(schema, self.name(), "blockquote")?;
		let wrap = move |state: &mut canvist_editor::EditorState, _: &canvist_editor::RuleMatch| {
			commands::wrap_in(state, quote, Attrs::new())
		};
		Ok(vec![InputRule::wrap(r"^\s*>\s$", wrap)?])
	}
}
