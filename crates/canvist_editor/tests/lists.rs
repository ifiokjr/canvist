//! List commands: split, sink, lift, join, and toggle.

mod support;

use canvist_editor::EditorState;
use canvist_editor::commands;
use canvist_editor::commands::ListType;
use support::after;
use support::dedent;
use support::kind;
use support::render;
use support::state;

fn bullet(state: &EditorState) -> ListType {
	ListType {
		list: kind(state, "bulletList"),
		item: kind(state, "listItem"),
	}
}

fn ordered(state: &EditorState) -> ListType {
	ListType {
		list: kind(state, "orderedList"),
		item: kind(state, "listItem"),
	}
}

fn task(state: &EditorState) -> ListType {
	ListType {
		list: kind(state, "taskList"),
		item: kind(state, "taskItem"),
	}
}

fn known(state: &EditorState) -> Vec<ListType> {
	vec![bullet(state), ordered(state), task(state)]
}

fn split(state: &mut EditorState) -> canvist_editor::CommandResult {
	let item = kind(state, "listItem");
	commands::split_list_item(state, item)
}

fn sink(state: &mut EditorState) -> canvist_editor::CommandResult {
	let item = kind(state, "listItem");
	commands::sink_list_item(state, item)
}

fn lift(state: &mut EditorState) -> canvist_editor::CommandResult {
	let item = kind(state, "listItem");
	commands::lift_list_item(state, item)
}

fn join(state: &mut EditorState) -> canvist_editor::CommandResult {
	let item = kind(state, "listItem");
	commands::join_list_item_backward(state, item)
}

fn toggle(
	state: &mut EditorState,
	target: fn(&EditorState) -> ListType,
) -> canvist_editor::CommandResult {
	let target = target(state);
	let known = known(state);
	commands::toggle_list(state, target, &known)
}

#[test]
fn enter_splits_an_item() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: on|e
		",
		split,
	);

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: on
		  listItem
		    paragraph: |e
	"
		)
	);
}

#[test]
fn enter_at_the_end_of_an_item_moves_nested_lists_along() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: one|
		    bulletList
		      listItem
		        paragraph: nested
		",
		split,
	);

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: one
		  listItem
		    paragraph: |
		    bulletList
		      listItem
		        paragraph: nested
	"
		)
	);
}

#[test]
fn enter_in_an_empty_item_leaves_the_list() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: one
		  listItem
		    paragraph: |
		  listItem
		    paragraph: three
		",
		split,
	);

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: one
		paragraph: |
		bulletList
		  listItem
		    paragraph: three
	"
		)
	);
}

#[test]
fn enter_in_an_empty_nested_item_outdents_it() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: one
		    bulletList
		      listItem
		        paragraph: |
		",
		split,
	);

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: one
		  listItem
		    paragraph: |
	"
		)
	);
}

#[test]
fn task_items_split_unchecked() {
	let mut state = state(
		"
		taskList
		  taskItem checked=true
		    paragraph: done|
		",
	);
	let item = kind(&state, "taskItem");
	commands::split_list_item(&mut state, item).expect("split");

	assert_eq!(
		render(&state),
		dedent(
			"
		taskList
		  taskItem checked=true
		    paragraph: done
		  taskItem
		    paragraph: |
	"
		)
	);
}

#[test]
fn splitting_outside_an_item_does_nothing() {
	let mut state = state("paragraph: a|");

	assert_eq!(split(&mut state), Ok(None));
}

#[test]
fn tab_nests_an_item_under_the_previous_one() {
	let mut state = state(
		"
		bulletList
		  listItem
		    paragraph: one
		  listItem
		    paragraph: tw|o
		  listItem
		    paragraph: three
		",
	);
	sink(&mut state).expect("sink");
	assert_eq!(
		render(&state),
		dedent(
			"
		bulletList
		  listItem
		    paragraph: one
		    bulletList
		      listItem
		        paragraph: tw|o
		  listItem
		    paragraph: three
	"
		)
	);

	let mut next = state;
	let last = next.doc().last_text_block().expect("text");
	let caret = canvist_model::Selection::caret(canvist_model::Point::new(last, 0));
	commands::set_selection(&mut next, caret).expect("move");
	sink(&mut next).expect("sink into existing sublist");
	assert_eq!(
		render(&next),
		dedent(
			"
		bulletList
		  listItem
		    paragraph: one
		    bulletList
		      listItem
		        paragraph: two
		      listItem
		        paragraph: |three
	"
		)
	);
}

#[test]
fn tab_on_the_first_item_does_nothing() {
	let mut state = state(
		"
		bulletList
		  listItem
		    paragraph: |one
		",
	);

	assert_eq!(sink(&mut state), Ok(None));
}

#[test]
fn shift_tab_lifts_a_nested_item_and_keeps_followers_nested() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: one
		    bulletList
		      listItem
		        paragraph: tw|o
		      listItem
		        paragraph: three
		",
		lift,
	);

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: one
		  listItem
		    paragraph: tw|o
		    bulletList
		      listItem
		        paragraph: three
	"
		)
	);
}

#[test]
fn shift_tab_on_a_top_level_range_unwraps_the_items() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: one
		  listItem
		    paragraph: t^wo
		  listItem
		    paragraph: th|ree
		  listItem
		    paragraph: four
		",
		lift,
	);

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: one
		paragraph: t^wo
		paragraph: th|ree
		bulletList
		  listItem
		    paragraph: four
	"
		)
	);
}

#[test]
fn backspace_at_an_item_start_joins_the_previous_item() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: one
		  listItem
		    paragraph: |two
		    bulletList
		      listItem
		        paragraph: nested
		",
		join,
	);

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: one|two
		    bulletList
		      listItem
		        paragraph: nested
	"
		)
	);
}

#[test]
fn backspace_on_the_first_item_lifts_it() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: |one
		  listItem
		    paragraph: two
		",
		join,
	);

	assert_eq!(
		result,
		dedent(
			"
		paragraph: |one
		bulletList
		  listItem
		    paragraph: two
	"
		)
	);
}

#[test]
fn backspace_after_an_item_with_a_sublist_lifts_instead_of_joining() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: one
		    bulletList
		      listItem
		        paragraph: nested
		  listItem
		    paragraph: |two
		",
		join,
	);

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: one
		    bulletList
		      listItem
		        paragraph: nested
		paragraph: |two
	"
		)
	);
}

#[test]
fn backspace_list_rule_ignores_carets_inside_text() {
	let mut state = state(
		"
		bulletList
		  listItem
		    paragraph: o|ne
		",
	);

	assert_eq!(join(&mut state), Ok(None));
}

#[test]
fn toggling_a_list_wraps_and_joins_neighbours() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: one
		paragraph: tw|o
		bulletList
		  listItem
		    paragraph: three
		",
		|state| toggle(state, bullet),
	);

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: one
		  listItem
		    paragraph: tw|o
		  listItem
		    paragraph: three
	"
		)
	);
}

#[test]
fn toggling_a_list_converts_headings_to_paragraphs() {
	let result = after("heading: ti|tle", |state| toggle(state, bullet));

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: ti|tle
	"
		)
	);
}

#[test]
fn toggling_the_same_list_lifts_out() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: on|e
		",
		|state| toggle(state, bullet),
	);

	assert_eq!(result, "paragraph: on|e");
}

#[test]
fn toggling_another_list_with_the_same_items_retypes_it() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: on|e
		",
		|state| toggle(state, ordered),
	);

	assert_eq!(
		result,
		dedent(
			"
		orderedList
		  listItem
		    paragraph: on|e
	"
		)
	);
}

#[test]
fn toggling_a_task_list_converts_the_items() {
	let mut state = state(
		"
		bulletList
		  listItem
		    paragraph: on|e
		  listItem
		    paragraph: two
		",
	);
	let ids_before: Vec<_> = state.doc().iter().map(canvist_model::Block::id).collect();
	toggle(&mut state, task).expect("convert");
	let ids_after: Vec<_> = state.doc().iter().map(canvist_model::Block::id).collect();

	assert_eq!(
		render(&state),
		dedent(
			"
		taskList
		  taskItem
		    paragraph: on|e
		  taskItem
		    paragraph: two
	"
		)
	);
	assert_eq!(ids_before, ids_after, "conversion keeps block ids");
}

#[test]
fn toggling_a_list_on_a_divider_does_nothing() {
	let mut state = state("horizontalRule @");

	assert_eq!(toggle(&mut state, bullet), Ok(None));
}
