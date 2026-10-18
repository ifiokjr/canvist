//! The document root and the paragraph, which every schema needs.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::EditorError;
use canvist_editor::Extension;
use canvist_editor::Platform;
use canvist_editor::block_type;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

/// The document root, which holds any block in the `block` group.
#[derive(Clone, Copy, Debug, Default)]
pub struct Document;

impl Extension for Document {
	fn name(&self) -> &'static str {
		"doc"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.root(&["block"])
	}
}

/// The default text block. It registers first (priority 1000), so it is
/// what Enter creates and what an empty document starts with.
#[derive(Clone, Copy, Debug, Default)]
pub struct Paragraph;

impl Paragraph {
	fn set(schema: &Schema) -> Result<Command, EditorError> {
		let paragraph = block_type(schema, "paragraph", "paragraph")?;
		Ok(command(move |state| {
			commands::set_block_type(state, paragraph, &Attrs::new())
		}))
	}
}

impl Extension for Paragraph {
	fn name(&self) -> &'static str {
		"paragraph"
	}

	fn priority(&self) -> i32 {
		1000
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::text("paragraph"))
	}

	fn commands(&self, schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
		Ok(vec![("setParagraph", Self::set(schema)?)])
	}

	fn keymap(&self, schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-Alt-0", Self::set(schema)?)])
	}
}
