//! Block quotes.

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::Extension;
use canvist_core::InputRule;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Platform;
use canvist_core::QuerySpec;
use canvist_core::Registration;
use canvist_core::RuleMatch;
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

/// A `blockquote` container for any blocks. `> ` at the start of a block
/// wraps it, and `Mod-Shift-b` toggles one.
#[derive(Clone, Copy, Debug, Default)]
pub struct Blockquote;

/// Typed access to [`Blockquote`].
#[derive(Clone, Copy, Debug)]
pub struct BlockquoteHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl BlockquoteHandle {
	/// Wrap the selected blocks in a quote, or lift them out of one.
	#[must_use]
	pub fn toggle(self) -> Command {
		toggle()
	}

	/// Whether the selection is inside a quote.
	#[must_use]
	pub fn is_active(self, state: &EditorState) -> bool {
		resolve::around(state, "blockquote").is_some()
	}
}

fn toggle() -> Command {
	command(|state| {
		let quote = resolve::block(state, "blockquote", "blockquote")?;
		commands::toggle_wrap(state, quote, Attrs::new())
	})
}

impl Extension for Blockquote {
	fn name(&self) -> &'static str {
		"blockquote"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::container("blockquote", &["block"]))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Block, "blockquote")]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let toggle_spec = CommandSpec::simple(
			"toggleBlockquote",
			"Wrap the selected blocks in a quote, or lift them out of one.",
			toggle(),
		);
		Ok(vec![toggle_spec])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let query = QuerySpec::new(
			"isBlockquote",
			"Whether the selection is inside a quote.",
			ValueType::Bool,
			|state, _| Ok(Value::Bool(resolve::around(state, "blockquote").is_some())),
		);
		Ok(vec![query])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-Shift-b", toggle())])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let quote = canvist_core::block_type(schema, self.name(), "blockquote")?;
		let wrap = move |state: &mut EditorState, _: &RuleMatch| {
			commands::wrap_in(state, quote, Attrs::new())
		};
		Ok(vec![InputRule::wrap(r"^\s*>\s$", wrap)?])
	}
}

impl TypedExtension for Blockquote {
	type Handle = BlockquoteHandle;

	fn handle(&self, registration: Registration) -> BlockquoteHandle {
		BlockquoteHandle {
			_registration: registration,
		}
	}
}
