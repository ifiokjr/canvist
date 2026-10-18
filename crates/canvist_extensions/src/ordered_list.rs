//! Ordered lists.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::InputRule;
use canvist_editor::Platform;
use canvist_editor::RuleMatch;
use canvist_editor::commands::ListType;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;

use crate::lists;

const TYPES: (&str, &str) = ("orderedList", "listItem");

/// An `orderedList` of `listItem`s with a `start` number. `1. ` (or any
/// number) starts one at that number, and `Mod-Shift-7` toggles one. Needs
/// [`ListItem`](crate::ListItem).
#[derive(Clone, Copy, Debug, Default)]
pub struct OrderedList;

impl OrderedList {
	fn toggle(schema: &Schema) -> Result<Command, EditorError> {
		Ok(lists::toggle(
			schema,
			lists::list_type(schema, "orderedList", TYPES)?,
		))
	}
}

impl Extension for OrderedList {
	fn name(&self) -> &'static str {
		"orderedList"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let attrs = Attrs::new().with("start", 1);
		schema.block(BlockSpec::container("orderedList", &["listItem"]).attrs(attrs))
	}

	fn commands(&self, schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
		Ok(vec![("toggleOrderedList", Self::toggle(schema)?)])
	}

	fn keymap(&self, schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-Shift-7", Self::toggle(schema)?)])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let target = lists::list_type(schema, self.name(), TYPES)?;
		let wrap = move |state: &mut EditorState, found: &RuleMatch| {
			let start = lists::start_number(found);
			lists::wrap_and(state, target, |state| set_start(state, target, start))
		};
		Ok(vec![InputRule::wrap(r"^(\d+)\.\s$", wrap)?])
	}
}

/// Number a freshly wrapped list from `start`, unless it joined a list that
/// already had items before the caret's.
fn set_start(state: &mut EditorState, target: ListType, start: i64) -> CommandResult {
	let Some((item, list)) = lists::caret_item(state, target.item) else {
		return Ok(None);
	};
	let first = state.doc().children(Some(list)).first() == Some(&item);

	if !first || start == 1 {
		return Ok(None);
	}

	lists::set_attrs(state, list, &Attrs::new().with("start", start))
}
