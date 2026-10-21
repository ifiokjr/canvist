//! Hard line breaks inside a block.

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::Extension;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Platform;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::embed_type;
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
