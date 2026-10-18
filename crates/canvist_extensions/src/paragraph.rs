//! The document root and the paragraph, which every schema needs.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::CommandSpec;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::NodeKind;
use canvist_editor::NodeMeta;
use canvist_editor::Platform;
use canvist_editor::QuerySpec;
use canvist_editor::Registration;
use canvist_editor::TypedExtension;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::commands;
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
