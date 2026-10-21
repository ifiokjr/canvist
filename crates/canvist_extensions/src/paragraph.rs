//! The document root and the paragraph, which every schema needs.

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Platform;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

use crate::resolve;

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

impl TypedExtension for Document {
	type Handle = Registration;

	fn handle(&self, registration: Registration) -> Registration {
		registration
	}
}

/// The default text block. It registers first (priority 1000), so it is
/// what Enter creates and what an empty document starts with.
#[derive(Clone, Copy, Debug, Default)]
pub struct Paragraph;

/// Typed access to [`Paragraph`].
#[derive(Clone, Copy, Debug)]
pub struct ParagraphHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl ParagraphHandle {
	/// Turn the selected text blocks into paragraphs.
	#[must_use]
	pub fn set(self) -> Command {
		set()
	}

	/// Whether every selected text block is a paragraph.
	#[must_use]
	pub fn is_active(self, state: &EditorState) -> bool {
		is_active(state)
	}
}

fn set() -> Command {
	command(|state| {
		let paragraph = resolve::block(state, "paragraph", "paragraph")?;
		commands::set_block_type(state, paragraph, &Attrs::new())
	})
}

fn is_active(state: &EditorState) -> bool {
	let paragraph = state.schema().block_type("paragraph");
	paragraph.is_some_and(|kind| commands::is_block_active(state, kind, &Attrs::new()))
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

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Block, "paragraph")]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![CommandSpec::simple(
			"setParagraph",
			"Turn the selected text blocks into paragraphs.",
			set(),
		)])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let query = QuerySpec::new(
			"isParagraph",
			"Whether every selected text block is a paragraph.",
			ValueType::Bool,
			|state, _| Ok(Value::Bool(is_active(state))),
		);
		Ok(vec![query])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-Alt-0", set())])
	}
}

impl TypedExtension for Paragraph {
	type Handle = ParagraphHandle;

	fn handle(&self, registration: Registration) -> ParagraphHandle {
		ParagraphHandle {
			_registration: registration,
		}
	}
}
