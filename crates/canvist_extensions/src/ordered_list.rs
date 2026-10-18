//! Ordered lists.

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
use canvist_editor::commands::ListType;
use canvist_model::Attrs;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Value;

use crate::lists;
use crate::resolve;

const NAME: &str = "orderedList";
const TYPES: (&str, &str) = ("orderedList", "listItem");

/// An `orderedList` of `listItem`s with a `start` number. `1. ` (or any
/// number) starts one at that number, and `Mod-Shift-7` toggles one. Needs
/// [`ListItem`](crate::ListItem).
#[derive(Clone, Copy, Debug, Default)]
pub struct OrderedList;

/// Typed access to [`OrderedList`].
#[derive(Clone, Copy, Debug)]
pub struct OrderedListHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl OrderedListHandle {
	/// Wrap the selection in an ordered list, convert another list into
	/// one, or lift the selection out of one.
	#[must_use]
	pub fn toggle(self) -> Command {
		lists::toggle(NAME, TYPES)
	}

	/// Number the ordered list around the selection from `start`.
	#[must_use]
	pub fn set_start(self, start: i64) -> Command {
		set_start_command(start)
	}

	/// Whether the selection is inside an ordered list.
	#[must_use]
	pub fn is_active(self, state: &EditorState) -> bool {
		resolve::around(state, NAME).is_some()
	}

	/// The start number of the ordered list around the selection.
	#[must_use]
	pub fn start(self, state: &EditorState) -> Option<i64> {
		resolve::attr_around(state, NAME, "start").and_then(|start| start.as_int())
	}
}

fn set_start_command(start: i64) -> Command {
	command(move |state| {
		let Some(list) = resolve::around(state, NAME) else {
			return Ok(None);
		};
		lists::set_attrs(state, list, &Attrs::new().with("start", start))
	})
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

fn start_field(required: bool) -> Field {
	let doc = "The number of the first item.";

	if required {
		Field::required("start", ValueType::Int, doc)
	} else {
		Field::optional("start", ValueType::Int, 1, doc)
	}
}

impl Extension for OrderedList {
	fn name(&self) -> &'static str {
		NAME
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		let attrs = Attrs::new().with("start", 1);
		schema.block(BlockSpec::container(NAME, &["listItem"]).attrs(attrs))
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		vec![NodeMeta::new(NodeKind::Block, NAME).attr(start_field(false))]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let toggle = CommandSpec::simple(
			"toggleOrderedList",
			"Wrap the selection in an ordered list, convert another list into one, or lift it out.",
			lists::toggle(NAME, TYPES),
		);
		let start = CommandSpec::new(
			"setOrderedListStart",
			"Number the ordered list around the selection from a start number.",
			|args| Ok(set_start_command(args.int("start")?)),
		);
		Ok(vec![toggle, start.arg(start_field(true))])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		let start = QuerySpec::new(
			"orderedListStart",
			"The start number of the ordered list around the selection, or null.",
			ValueType::nullable(ValueType::Int),
			|state, _| Ok(resolve::attr_around(state, NAME, "start").unwrap_or(Value::Null)),
		);
		Ok(vec![
			lists::inside_query(
				"isOrderedList",
				NAME,
				"Whether the selection is inside an ordered list.",
			),
			start,
		])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		Ok(vec![("Mod-Shift-7", lists::toggle(NAME, TYPES))])
	}

	fn input_rules(&self, schema: &Schema) -> Result<Vec<InputRule>, EditorError> {
		let target = lists::list_type(schema, NAME, TYPES)?;
		let wrap = move |state: &mut EditorState, found: &RuleMatch| {
			let start = lists::start_number(found);
			lists::wrap_and(state, target, |state| set_start(state, target, start))
		};
		Ok(vec![InputRule::wrap(r"^(\d+)\.\s$", wrap)?])
	}
}

impl TypedExtension for OrderedList {
	type Handle = OrderedListHandle;

	fn handle(&self, registration: Registration) -> OrderedListHandle {
		OrderedListHandle {
			_registration: registration,
		}
	}
}
