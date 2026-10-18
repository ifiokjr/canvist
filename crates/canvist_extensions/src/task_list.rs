//! Task lists with checkable items.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::InputRule;
use canvist_editor::Platform;
use canvist_editor::RuleMatch;
use canvist_editor::command;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::BlockTypeId;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

use crate::lists;

const TYPES: (&str, &str) = ("taskList", "taskItem");

/// A `taskList` of `taskItem`s, each with a `checked` attribute. `[ ] ` or
/// `[x] ` starts one, `Mod-Shift-9` toggles one, and the `toggleTask`
/// command checks or unchecks the item at the caret. Items behave like list
/// items under Enter, Tab, Shift-Tab, and Backspace.
#[derive(Clone, Copy, Debug, Default)]
pub struct TaskList;

impl TaskList {
	fn toggle(schema: &Schema) -> Result<Command, EditorError> {
		Ok(lists::toggle(
			schema,
			lists::list_type(schema, "taskList", TYPES)?,
		))
	}

	fn toggle_checked(item: BlockTypeId) -> Command {
		command(move |state| {
			let Some((found, _)) = lists::caret_item(state, item) else {
				return Ok(None);
			};
			let checked = state
				.doc()
				.block(found)
				.and_then(|block| block.attrs().get("checked"))
				.and_then(Value::as_bool)
				.unwrap_or(false);
			lists::set_attrs(state, found, &Attrs::new().with("checked", !checked))
		})
	}
}

impl Extension for TaskList {
	fn name(&self) -> &'static str {
		"taskList"
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let item = BlockSpec::container("taskItem", &["block"])
			.groups(&[])
			.attrs(Attrs::new().with("checked", false));
		schema
			.block(BlockSpec::container("taskList", &["taskItem"]))
			.block(item)
	}

	fn commands(&self, schema: &Schema) -> Result<Vec<(&'static str, Command)>, EditorError> {
		let target = lists::list_type(schema, self.name(), TYPES)?;
		Ok(vec![
			("toggleTaskList", Self::toggle(schema)?),
			("toggleTask", Self::toggle_checked(target.item)),
		])
	}

	fn keymap(&self, schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		let target = lists::list_type(schema, self.name(), TYPES)?;
		let mut bindings = lists::item_bindings(target.item);
		bindings.push(("Mod-Shift-9", Self::toggle(schema)?));
		Ok(bindings)
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let target = lists::list_type(schema, self.name(), TYPES)?;
		let wrap = move |state: &mut EditorState, found: &RuleMatch| {
			let checked = found.group(2) == Some("x");
			lists::wrap_and(state, target, |state| check(state, target.item, checked))
		};
		Ok(vec![InputRule::wrap(r"^\s*(\[([ x])?\])\s$", wrap)?])
	}
}

/// Check the caret's new task item when the rule was typed as `[x]`.
fn check(state: &mut EditorState, item: BlockTypeId, checked: bool) -> CommandResult {
	let Some((found, _)) = lists::caret_item(state, item).filter(|_| checked) else {
		return Ok(None);
	};

	lists::set_attrs(state, found, &Attrs::new().with("checked", true))
}
