//! List items.

use canvist_editor::Binding;
use canvist_editor::EditorError;
use canvist_editor::Extension;
use canvist_editor::Platform;
use canvist_editor::block_type;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

use crate::lists;

/// A `listItem` container for bullet and ordered lists. Enter splits it,
/// Tab and Shift-Tab indent and outdent it, and Backspace at its start
/// joins it with the item before.
#[derive(Clone, Copy, Debug, Default)]
pub struct ListItem;

impl Extension for ListItem {
	fn name(&self) -> &'static str {
		"listItem"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema.block(BlockSpec::container("listItem", &["block"]).groups(&[]))
	}

	fn keymap(&self, schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(lists::item_bindings(block_type(
			schema,
			self.name(),
			"listItem",
		)?))
	}
}
