//! List items.

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
use canvist_core::commands;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

use crate::lists;

const NAME: &str = "listItem";

/// A `listItem` container for bullet and ordered lists. Enter splits it,
/// Tab and Shift-Tab indent and outdent it, and Backspace at its start
/// joins it with the item before.
#[derive(Clone, Copy, Debug, Default)]
pub struct ListItem;

/// Typed access to [`ListItem`].
#[derive(Clone, Copy, Debug)]
pub struct ListItemHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl ListItemHandle {
	/// Split the item at the caret (Enter).
	#[must_use]
	pub fn split(self) -> Command {
		lists::item_command(NAME, NAME, commands::split_list_item)
	}

	/// Nest the selected items in the item before them (Tab).
	#[must_use]
	pub fn sink(self) -> Command {
		lists::item_command(NAME, NAME, commands::sink_list_item)
	}

	/// Lift the selected items one level (Shift-Tab).
	#[must_use]
	pub fn lift(self) -> Command {
		lists::item_command(NAME, NAME, commands::lift_list_item)
	}
}

impl Extension for ListItem {
	fn name(&self) -> &'static str {
		NAME
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::container(NAME, &["block"]).groups(&[]))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Block, NAME)]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		Ok(lists::item_specs(
			NAME,
			NAME,
			["splitListItem", "sinkListItem", "liftListItem"],
		))
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(lists::item_bindings(NAME, NAME))
	}
}

impl TypedExtension for ListItem {
	type Handle = ListItemHandle;

	fn handle(&self, registration: Registration) -> ListItemHandle {
		ListItemHandle {
			_registration: registration,
		}
	}
}
