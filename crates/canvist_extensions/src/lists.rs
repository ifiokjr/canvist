//! Lists: bullet, ordered, and task lists, and the items inside them.
//!
//! Every list type shares one set of item commands from
//! [`canvist_editor::commands`]; the extensions here only name the types,
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

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::RuleMatch;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_editor::commands::ListType;
use canvist_editor::position;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTypeId;
use canvist_model::Schema;

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
		list: canvist_editor::block_type(schema, extension, list)?,
		item: canvist_editor::block_type(schema, extension, item)?,
	})
}

/// A command toggling `target`, converting any other known list.
pub(crate) fn toggle(schema: &Schema, target: ListType) -> Command {
	let known = known_lists(schema);
	command(move |state| commands::toggle_list(state, target, &known))
}

/// Enter, Tab, Shift-Tab, and Backspace inside items of type `item`.
pub(crate) fn item_bindings(item: BlockTypeId) -> Vec<Binding> {
	vec![
		(
			"Enter",
			command(move |state| commands::split_list_item(state, item)),
		),
		(
			"Tab",
			command(move |state| commands::sink_list_item(state, item)),
		),
		(
			"Shift-Tab",
			command(move |state| commands::lift_list_item(state, item)),
		),
		(
			"Backspace",
			command(move |state| commands::join_list_item_backward(state, item)),
		),
	]
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
