//! Hard line breaks inside a block.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::CommandSpec;
use canvist_editor::EditorError;
use canvist_editor::Extension;
use canvist_editor::NodeKind;
use canvist_editor::NodeMeta;
use canvist_editor::Platform;
use canvist_editor::Registration;
use canvist_editor::TypedExtension;
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

/// Typed access to [`HardBreak`].
#[derive(Clone, Copy, Debug)]
pub struct HardBreakHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl HardBreakHandle {
	/// Insert a line break at the selection.
	#[must_use]
	pub fn insert(self) -> Command {
		insert()
	}
}

fn insert() -> Command {
	command(|state| {
		let kind = embed_type(state.schema(), "hardBreak", "hardBreak")?;
		let embed = Embed {
			kind,
			attrs: Attrs::new(),
		};
		commands::insert_embed(state, embed)
	})
}

impl Extension for HardBreak {
	fn name(&self) -> &'static str {
		"hardBreak"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.embed(EmbedSpec::new("hardBreak"))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Embed, "hardBreak")]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![CommandSpec::simple(
			"setHardBreak",
			"Insert a line break at the selection.",
			insert(),
		)])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Shift-Enter", insert()), ("Mod-Enter", insert())])
	}
}

impl TypedExtension for HardBreak {
	type Handle = HardBreakHandle;

	fn handle(&self, registration: Registration) -> HardBreakHandle {
		HardBreakHandle {
			_registration: registration,
		}
	}
}
