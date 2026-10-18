//! Hard line breaks inside a block.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::EditorError;
use canvist_editor::Extension;
use canvist_editor::Platform;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_editor::embed_type;
use canvist_model::Attrs;
use canvist_model::Embed;
use canvist_model::EmbedSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

/// A `hardBreak` inline embed: a line break that stays inside its block.
/// `Shift-Enter` and `Mod-Enter` insert one.
#[derive(Clone, Copy, Debug, Default)]
pub struct HardBreak;

impl HardBreak {
	fn insert(schema: &Schema) -> Result<Command, EditorError> {
		let kind = embed_type(schema, "hardBreak", "hardBreak")?;
		Ok(command(move |state| {
			let embed = Embed {
				kind,
				attrs: Attrs::new(),
			};
			commands::insert_embed(state, embed)
		}))
	}
}

impl Extension for HardBreak {
	fn name(&self) -> &'static str {
		"hardBreak"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.embed(EmbedSpec::new("hardBreak"))
	}

	fn commands(&self, schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
		Ok(vec![("setHardBreak", Self::insert(schema)?)])
	}

	fn keymap(&self, schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		let insert = Self::insert(schema)?;
		Ok(vec![("Shift-Enter", insert.clone()), ("Mod-Enter", insert)])
	}
}
