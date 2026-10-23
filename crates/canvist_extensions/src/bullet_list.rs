//! Bullet lists.

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
use canvist_core::commands;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

use crate::lists;

const NAME: &str = "bulletList";
const TYPES: (&str, &str) = ("bulletList", "listItem");

/// A `bulletList` of `listItem`s. `- `, `+ `, or `* ` starts one, and
/// `Mod-Shift-8` toggles one. Needs [`ListItem`](crate::ListItem).
#[derive(Clone, Copy, Debug, Default)]
pub struct BulletList;

/// Typed access to [`BulletList`].
#[derive(Clone, Copy, Debug)]
pub struct BulletListHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl BulletListHandle {
	/// Wrap the selection in a bullet list, convert another list into one,
	/// or lift the selection out of one.
	#[must_use]
	pub fn toggle(self) -> Command {
		lists::toggle(NAME, TYPES)
	}

	/// Whether the selection is inside a bullet list.
	#[must_use]
	pub fn is_active(self, state: &EditorState) -> bool {
		crate::resolve::around(state, NAME).is_some()
	}
}

impl Extension for BulletList {
	fn name(&self) -> &'static str {
		NAME
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::container(NAME, &["listItem"]))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Block, NAME)]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(vec![CommandSpec::simple(
			"toggleBulletList",
			"Wrap the selection in a bullet list, convert another list into one, or lift it out.",
			lists::toggle(NAME, TYPES),
		)])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![lists::inside_query(
			"isBulletList",
			NAME,
			"Whether the selection is inside a bullet list.",
		)])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-Shift-8", lists::toggle(NAME, TYPES))])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let target = lists::list_type(schema, NAME, TYPES)?;
		let wrap =
			move |state: &mut EditorState, _: &RuleMatch| commands::wrap_in_list(state, target);
		Ok(vec![InputRule::wrap(r"^\s*([-+*])\s$", wrap)?])
	}
}

impl TypedExtension for BulletList {
	type Handle = BulletListHandle;

	fn handle(&self, registration: Registration) -> BulletListHandle {
		BulletListHandle {
			_registration: registration,
		}
	}
}
