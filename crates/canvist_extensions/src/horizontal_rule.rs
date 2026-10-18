//! Horizontal rules between blocks.

use canvist_editor::Command;
use canvist_editor::EditorError;
use canvist_editor::Extension;
use canvist_editor::InputRule;
use canvist_editor::block_type;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

/// A `horizontalRule` atomic block. `---`, `___ `, or `*** ` on an empty
/// line inserts one.
#[derive(Clone, Copy, Debug, Default)]
pub struct HorizontalRule;

impl Extension for HorizontalRule {
	fn name(&self) -> &'static str {
		"horizontalRule"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::leaf("horizontalRule"))
	}

	fn commands(&self, schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
		let kind = block_type(schema, self.name(), "horizontalRule")?;
		let insert = command(move |state| commands::insert_leaf(state, kind, Attrs::new()));
		Ok(vec![("setHorizontalRule", insert)])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let kind = block_type(schema, self.name(), "horizontalRule")?;
		Ok(vec![InputRule::leaf(r"^(?:---|—-|___\s|\*\*\*\s)$", kind)?])
	}
}
