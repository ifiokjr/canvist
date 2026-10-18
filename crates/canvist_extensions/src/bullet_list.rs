//! Bullet lists.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::InputRule;
use canvist_editor::Platform;
use canvist_editor::RuleMatch;
use canvist_editor::commands;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

use crate::lists;

const TYPES: (&str, &str) = ("bulletList", "listItem");

/// A `bulletList` of `listItem`s. `- `, `+ `, or `* ` starts one, and
/// `Mod-Shift-8` toggles one. Needs [`ListItem`](crate::ListItem).
#[derive(Clone, Copy, Debug, Default)]
pub struct BulletList;

impl BulletList {
	fn toggle(schema: &Schema) -> Result<Command, EditorError> {
		Ok(lists::toggle(
			schema,
			lists::list_type(schema, "bulletList", TYPES)?,
		))
	}
}

impl Extension for BulletList {
	fn name(&self) -> &'static str {
		"bulletList"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::container("bulletList", &["listItem"]))
	}

	fn commands(&self, schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
		Ok(vec![("toggleBulletList", Self::toggle(schema)?)])
	}

	fn keymap(&self, schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-Shift-8", Self::toggle(schema)?)])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let target = lists::list_type(schema, self.name(), TYPES)?;
		let wrap =
			move |state: &mut EditorState, _: &RuleMatch| commands::wrap_in_list(state, target);
		Ok(vec![InputRule::wrap(r"^\s*([-+*])\s$", wrap)?])
	}
}
