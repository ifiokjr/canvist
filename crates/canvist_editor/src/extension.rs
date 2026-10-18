//! Extensions: the unit of "pay for what you use".

use canvist_model::BlockTypeId;
use canvist_model::EmbedTypeId;
use canvist_model::MarkTypeId;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

use crate::Command;
use crate::CommandResult;
use crate::CommandSpec;
use crate::EditorError;
use crate::EditorState;
use crate::InputRule;
use crate::NodeMeta;
use crate::PasteRule;
use crate::Platform;
use crate::QuerySpec;
use crate::Update;

/// A key binding contributed by an extension.
pub type Binding = (&'static str, Command);

/// A feature: schema items plus the behaviour that goes with them.
///
/// The editor first lets every extension add to the schema, builds it, and
/// then asks each extension for commands, keymaps, and rules, which can
/// resolve type ids from the finished schema. Every method has a default,
/// so an extension implements only what it contributes. Extensions with a
/// higher [`Extension::priority`] get their key bindings and rules tried
/// first.
pub trait Extension: Send + Sync {
	/// A unique name, used in errors and for debugging.
	fn name(&self) -> &'static str;

	/// Ordering: higher runs first. The default is 100, as in TipTap.
	fn priority(&self) -> i32 {
		100
	}

	/// Register blocks, marks, and embeds.
	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema
	}

	/// Named commands with typed arguments. They are what dynamic hosts
	/// run with [`Editor::run_named`](crate::Editor::run_named), and their
	/// metadata is what bindings generate typed APIs from.
	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(Vec::new())
	}

	/// Named questions about the state, such as whether a mark is active
	/// or a block's attribute value.
	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(Vec::new())
	}

	/// The blocks, marks, and embeds [`Extension::schema`] registers, with
	/// typed attributes, for generated bindings.
	fn nodes(&self) -> Vec<NodeMeta> {
		Vec::new()
	}

	/// Key bindings, in ProseMirror notation such as `Mod-b`.
	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(Vec::new())
	}

	/// Rules that rewrite text as it is typed.
	fn input_rules(&self, _schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		Ok(Vec::new())
	}

	/// Rules that format pasted text.
	fn paste_rules(&self, _schema: &Schema) -> Result<Vec<PasteRule>, EditorError> {
		Ok(Vec::new())
	}

	/// Text block types that hold code: input and paste rules never run
	/// inside them and pasted text keeps its line breaks.
	fn code_blocks(&self, _schema: &Schema) -> Result<Vec<BlockTypeId>, EditorError> {
		Ok(Vec::new())
	}

	/// Fix up the state after a local update, e.g. keep a trailing
	/// paragraph. The result is folded into the same update and undo step.
	fn after_update(&self, _state: &mut EditorState, _update: &Update) -> CommandResult {
		Ok(None)
	}
}

/// Look up a block type an extension depends on.
pub fn block_type(
	schema: &Schema,
	extension: &'static str,
	name: &'static str,
) -> Result<BlockTypeId, EditorError> {
	schema.block_type(name).ok_or(EditorError::MissingType {
		extension,
		kind: "block",
		name,
	})
}

/// Look up a mark type an extension depends on.
pub fn mark_type(
	schema: &Schema,
	extension: &'static str,
	name: &'static str,
) -> Result<MarkTypeId, EditorError> {
	schema.mark_type(name).ok_or(EditorError::MissingType {
		extension,
		kind: "mark",
		name,
	})
}

/// Look up an embed type an extension depends on.
pub fn embed_type(
	schema: &Schema,
	extension: &'static str,
	name: &'static str,
) -> Result<EmbedTypeId, EditorError> {
	schema.embed_type(name).ok_or(EditorError::MissingType {
		extension,
		kind: "embed",
		name,
	})
}
