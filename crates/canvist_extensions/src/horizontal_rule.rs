//! Horizontal rules between blocks.

use canvist_core::Command;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::Extension;
use canvist_core::InputRule;
use canvist_core::NodeKind;
use canvist_core::NodeMeta;
use canvist_core::Registration;
use canvist_core::TypedExtension;
use canvist_core::block_type;
use canvist_core::command;
use canvist_core::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

use crate::resolve;

/// A `horizontalRule` atomic block. `---`, `___ `, or `*** ` on an empty
/// line inserts one.
#[derive(Clone, Copy, Debug, Default)]
pub struct HorizontalRule;

/// Typed access to [`HorizontalRule`].
#[derive(Clone, Copy, Debug)]
pub struct HorizontalRuleHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl HorizontalRuleHandle {
	/// Insert a horizontal rule at the selection.
	#[must_use]
	pub fn insert(self) -> Command {
		insert()
	}
}

fn insert() -> Command {
	command(|state| {
		let kind = resolve::block(state, "horizontalRule", "horizontalRule")?;
		commands::insert_leaf(state, kind, Attrs::new())
	})
}

impl Extension for HorizontalRule {
	fn name(&self) -> &'static str {
		"horizontalRule"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::leaf("horizontalRule"))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Block, "horizontalRule")]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![CommandSpec::simple(
			"setHorizontalRule",
			"Insert a horizontal rule at the selection.",
			insert(),
		)])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let kind = block_type(schema, self.name(), "horizontalRule")?;
		Ok(vec![InputRule::leaf(r"^(?:---|—-|___\s|\*\*\*\s)$", kind)?])
	}
}

impl TypedExtension for HorizontalRule {
	type Handle = HorizontalRuleHandle;

	fn handle(&self, registration: Registration) -> HorizontalRuleHandle {
		HorizontalRuleHandle {
			_registration: registration,
		}
	}
}
