//! Task lists with checkable items.

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::CommandSpec;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::InputRule;
use canvist_editor::NodeKind;
use canvist_editor::NodeMeta;
use canvist_editor::Platform;
use canvist_editor::QuerySpec;
use canvist_editor::Registration;
use canvist_editor::RuleMatch;
use canvist_editor::TypedExtension;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::BlockTypeId;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

use crate::lists;
use crate::resolve;

const NAME: &str = "taskList";
const ITEM: &str = "taskItem";
const TYPES: (&str, &str) = (NAME, ITEM);

/// A `taskList` of `taskItem`s, each with a `checked` attribute. `[ ] ` or
/// `[x] ` starts one, and `Mod-Shift-9` toggles one. Items behave like list
/// items under Enter, Tab, Shift-Tab, and Backspace.
#[derive(Clone, Copy, Debug, Default)]
pub struct TaskList;

/// Typed access to [`TaskList`].
#[derive(Clone, Copy, Debug)]
pub struct TaskListHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl TaskListHandle {
	/// Wrap the selection in a task list, convert another list into one,
	/// or lift the selection out of one.
	#[must_use]
	pub fn toggle(self) -> Command {
		lists::toggle(NAME, TYPES)
	}

	/// Check or uncheck the task at the caret.
	#[must_use]
	pub fn toggle_checked(self) -> Command {
		toggle_checked()
	}

	/// Check (`true`) or uncheck the task at the caret.
	#[must_use]
	pub fn set_checked(self, checked: bool) -> Command {
		set_checked(checked)
	}

	/// Split the task at the caret (Enter).
	#[must_use]
	pub fn split(self) -> Command {
		lists::item_command(NAME, ITEM, commands::split_list_item)
	}

	/// Nest the selected tasks in the task before them (Tab).
	#[must_use]
	pub fn sink(self) -> Command {
		lists::item_command(NAME, ITEM, commands::sink_list_item)
	}

	/// Lift the selected tasks one level (Shift-Tab).
	#[must_use]
	pub fn lift(self) -> Command {
		lists::item_command(NAME, ITEM, commands::lift_list_item)
	}

	/// Whether the selection is inside a task list.
	#[must_use]
	pub fn is_active(self, state: &EditorState) -> bool {
		resolve::around(state, NAME).is_some()
	}

	/// Whether the task around the selection is checked.
	#[must_use]
	pub fn is_checked(self, state: &EditorState) -> Option<bool> {
		checked(state)
	}
}

fn checked(state: &EditorState) -> Option<bool> {
	resolve::attr_around(state, ITEM, "checked").and_then(|checked| checked.as_bool())
}

fn set_checked(checked: bool) -> Command {
	command(move |state| check_around(state, checked))
}

fn toggle_checked() -> Command {
	command(|state| {
		let Some(current) = checked(state) else {
			return Ok(None);
		};
		check_around(state, !current)
	})
}

/// Check or uncheck the task around the selection.
fn check_around(state: &mut EditorState, checked: bool) -> CommandResult {
	let Some(item) = resolve::around(state, ITEM) else {
		return Ok(None);
	};

	lists::set_attrs(state, item, &Attrs::new().with("checked", checked))
}

/// Check the caret's new task item when the rule was typed as `[x]`.
fn check(state: &mut EditorState, item: BlockTypeId, checked: bool) -> CommandResult {
	let Some((found, _)) = lists::caret_item(state, item).filter(|_| checked) else {
		return Ok(None);
	};

	lists::set_attrs(state, found, &Attrs::new().with("checked", true))
}

fn checked_field() -> Field {
	Field::optional(
		"checked",
		ValueType::Bool,
		false,
		"Whether the task is done.",
	)
}

impl Extension for TaskList {
	fn name(&self) -> &'static str {
		NAME
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let item = BlockSpec::container(ITEM, &["block"])
			.groups(&[])
			.attrs(Attrs::new().with("checked", false));
		schema
			.block(BlockSpec::container(NAME, &[ITEM]))
			.block(item)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![
			NodeMeta::new(NodeKind::Block, NAME),
			NodeMeta::new(NodeKind::Block, ITEM).attr(checked_field()),
		]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let mut specs = vec![
			CommandSpec::simple(
				"toggleTaskList",
				"Wrap the selection in a task list, convert another list into one, or lift it out.",
				lists::toggle(NAME, TYPES),
			),
			CommandSpec::simple(
				"toggleTask",
				"Check or uncheck the task at the caret.",
				toggle_checked(),
			),
			CommandSpec::new(
				"setTaskChecked",
				"Check or uncheck the task at the caret.",
				|args| Ok(set_checked(args.bool("checked")?)),
			)
			.arg(Field::required(
				"checked",
				ValueType::Bool,
				"Whether the task is done.",
			)),
		];
		specs.extend(lists::item_specs(
			NAME,
			ITEM,
			["splitTaskItem", "sinkTaskItem", "liftTaskItem"],
		));
		Ok(specs)
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let checked_query = QuerySpec::new(
			"isTaskChecked",
			"Whether the task around the selection is checked, or null outside tasks.",
			ValueType::nullable(ValueType::Bool),
			|state, _| Ok(checked(state).map_or(Value::Null, Value::Bool)),
		);
		Ok(vec![
			lists::inside_query(
				"isTaskList",
				NAME,
				"Whether the selection is inside a task list.",
			),
			checked_query,
		])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		let mut bindings = lists::item_bindings(NAME, ITEM);
		bindings.push(("Mod-Shift-9", lists::toggle(NAME, TYPES)));
		Ok(bindings)
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let target = lists::list_type(schema, NAME, TYPES)?;
		let wrap = move |state: &mut EditorState, found: &RuleMatch| {
			let checked = found.group(2) == Some("x");
			lists::wrap_and(state, target, |state| check(state, target.item, checked))
		};
		Ok(vec![InputRule::wrap(r"^\s*(\[([ x])?\])\s$", wrap)?])
	}
}

impl TypedExtension for TaskList {
	type Handle = TaskListHandle;

	fn handle(&self, registration: Registration) -> TaskListHandle {
		TaskListHandle {
			_registration: registration,
		}
	}
}
