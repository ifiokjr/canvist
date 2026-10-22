//! Lists: bullet, ordered, and task lists, and the items inside them.
//!
//! Every list type shares one set of item commands from
//! [`canvist_core::commands`]; the extensions here only name the types,
//! bind keys, and add input rules.

#![cfg_attr(
	not(all(
		feature = "bullet-list",
		feature = "ordered-list",
		feature = "task-list"
	)),
	allow(
		dead_code,
		reason = "each list extension uses a different subset of these helpers"
	)
)]

use canvist_core::Binding;
use canvist_core::Command;
use canvist_core::CommandResult;
use canvist_core::CommandSpec;
use canvist_core::EditorError;
use canvist_core::EditorState;
use canvist_core::QuerySpec;
use canvist_core::RuleMatch;
use canvist_core::ValueType;
use canvist_core::command;
use canvist_core::commands;
use canvist_core::commands::ListType;
use canvist_core::position;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTypeId;
use canvist_model::Schema;
use canvist_model::Value;

/// Every list and item pair canvist knows, by TipTap name.
const LIST_TYPES: [(&str, &str); 3] = [
	("bulletList", "listItem"),
	("orderedList", "listItem"),
	("taskList", "taskItem"),
];

/// The list types registered in `schema`, so toggling one list type can
/// convert any other in place.
pub(crate) fn known_lists(schema: &Schema) -> Vec<ListType> {
	LIST_TYPES
		.iter()
		.filter_map(|&(list, item)| {
			Some(ListType {
				list: schema.block_type(list)?,
				item: schema.block_type(item)?,
			})
		})
		.collect()
}

/// The list type named `list` holding items named `item`.
pub(crate) fn list_type(
	schema: &Schema,
	extension: &'static str,
	(list, item): (&'static str, &'static str),
) -> Result<ListType, EditorError> {
	Ok(ListType {
		list: canvist_core::block_type(schema, extension, list)?,
		item: canvist_core::block_type(schema, extension, item)?,
	})
}

/// A command toggling the list `types` names, converting any other known
/// list. Types resolve when it runs.
pub(crate) fn toggle(extension: &'static str, types: (&'static str, &'static str)) -> Command {
	command(move |state| {
		let target = list_type(state.schema(), extension, types)?;
		let known = known_lists(state.schema());
		commands::toggle_list(state, target, &known)
	})
}

/// An item command for items named `item`, resolving the type when it runs.
pub(crate) fn item_command(
	extension: &'static str,
	item: &'static str,
	run: fn(&mut EditorState, BlockTypeId) -> CommandResult,
) -> Command {
	command(move |state| {
		let kind = canvist_core::block_type(state.schema(), extension, item)?;
		run(state, kind)
	})
}

/// Enter, Tab, Shift-Tab, and Backspace inside items named `item`.
pub(crate) fn item_bindings(extension: &'static str, item: &'static str) -> Vec<Binding> {
	vec![
		(
			"Enter",
			item_command(extension, item, commands::split_list_item),
		),
		(
			"Tab",
			item_command(extension, item, commands::sink_list_item),
		),
		(
			"Shift-Tab",
			item_command(extension, item, commands::lift_list_item),
		),
		(
			"Backspace",
			item_command(extension, item, commands::join_list_item_backward),
		),
	]
}

/// The split, sink, and lift commands for items named `item`, under
/// `names` (for example `splitListItem`, `sinkListItem`, `liftListItem`).
pub(crate) fn item_specs(
	extension: &'static str,
	item: &'static str,
	names: [&'static str; 3],
) -> Vec<CommandSpec> {
	let [split, sink, lift] = names;
	vec![
		CommandSpec::simple(
			split,
			"Split the item at the caret.",
			item_command(extension, item, commands::split_list_item),
		),
		CommandSpec::simple(
			sink,
			"Nest the selected items in the item before them.",
			item_command(extension, item, commands::sink_list_item),
		),
		CommandSpec::simple(
			lift,
			"Lift the selected items one level.",
			item_command(extension, item, commands::lift_list_item),
		),
	]
}

/// A query answering whether the selection is inside a `list` block.
pub(crate) fn inside_query(name: &'static str, list: &'static str, doc: &'static str) -> QuerySpec {
	QuerySpec::new(name, doc, ValueType::Bool, move |state, _| {
		Ok(Value::Bool(crate::resolve::around(state, list).is_some()))
	})
}

/// The item of type `item` holding the caret, and that item's list.
pub(crate) fn caret_item(state: &EditorState, item: BlockTypeId) -> Option<(BlockId, BlockId)> {
	let caret = state.selection().as_caret()?;
	let doc = state.doc();
	let found = position::ancestors_or_self(doc, caret.block)
		.find(|&id| doc.block(id).is_some_and(|block| block.kind() == item))?;
	let list = position::parent_of(doc, found)?;
	Some((found, list))
}

/// Wrap the caret's block in a `target` list, then let `adjust` set
/// attributes from the match, e.g. an ordered list's start number.
pub(crate) fn wrap_and(
	state: &mut EditorState,
	target: ListType,
	adjust: impl FnOnce(&mut EditorState) -> CommandResult,
) -> CommandResult {
	let Some(wrapped) = commands::wrap_in_list(state, target)? else {
		return Ok(None);
	};
	let adjusted = adjust(state)?;

	Ok(Some(match adjusted {
		Some(adjusted) => wrapped.then(adjusted),
		None => wrapped,
	}))
}

/// Set `attrs` on block `id` as part of an input rule.
pub(crate) fn set_attrs(state: &mut EditorState, id: BlockId, attrs: &Attrs) -> CommandResult {
	commands::set_block_attrs(state, id, attrs)
}

/// The number typed before `. `, as an ordered list's start.
pub(crate) fn start_number(found: &RuleMatch) -> i64 {
	found
		.group(1)
		.and_then(|digits| digits.parse().ok())
		.unwrap_or(1)
}
