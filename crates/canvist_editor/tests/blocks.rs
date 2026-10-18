//! Splitting, joining, block types, wrapping, lifting, and atomic blocks.

mod support;

use canvist_editor::ParagraphNavigator;
use canvist_editor::Unit;
use canvist_editor::commands;
use canvist_model::Attrs;
use support::after;
use support::dedent;
use support::kind;
use support::mark;
use support::render;
use support::state;

const LINES: ParagraphNavigator = ParagraphNavigator;

fn backspace(state: &mut canvist_editor::EditorState) -> canvist_editor::CommandResult {
	commands::delete_backward(state, Unit::Grapheme, &LINES)
}

fn delete(state: &mut canvist_editor::EditorState) -> canvist_editor::CommandResult {
	commands::delete_forward(state, Unit::Grapheme, &LINES)
}

#[test]
fn enter_splits_in_the_middle_keeping_the_type() {
	let result = after("heading level=2: Ti|tle", commands::split_block);

	assert_eq!(
		result,
		dedent(
			"
		heading level=2: Ti
		heading level=2: |tle
	"
		)
	);
}

#[test]
fn enter_at_the_end_of_a_heading_starts_a_paragraph() {
	let result = after("heading level=2: Title|", commands::split_block);

	assert_eq!(
		result,
		dedent(
			"
		heading level=2: Title
		paragraph: |
	"
		)
	);
}

#[test]
fn enter_at_the_start_inserts_a_paragraph_above() {
	let result = after("heading: |Title", commands::split_block);

	assert_eq!(
		result,
		dedent(
			"
		paragraph:
		heading: |Title
	"
		)
	);
}

#[test]
fn enter_replaces_the_selection_and_keeps_marks() {
	let mut state = state("paragraph: {bold:a^b}c|d");
	commands::split_block(&mut state).expect("split");
	commands::insert_text(&mut state, "x").expect("type");

	assert_eq!(
		render(&state),
		dedent(
			"
		paragraph: {bold:a}
		paragraph: {bold:x}|d
	"
		)
	);
}

#[test]
fn enter_does_nothing_on_a_block_selection() {
	let mut state = state("horizontalRule @");

	assert_eq!(commands::split_block(&mut state), Ok(None));
}

#[test]
fn backspace_joins_with_the_previous_text_block() {
	let result = after(
		"
		heading: one
		paragraph: |two
		",
		backspace,
	);

	assert_eq!(result, "heading: one|two");
}

#[test]
fn backspace_at_the_start_of_a_quote_lifts_out_of_it() {
	let result = after(
		"
		paragraph: a
		blockquote
		  paragraph: |b
		  paragraph: c
		",
		backspace,
	);

	assert_eq!(
		result,
		dedent(
			"
		paragraph: a
		paragraph: |b
		blockquote
		  paragraph: c
	"
		)
	);
}

#[test]
fn backspace_after_a_container_joins_its_last_text_block() {
	let result = after(
		"
		blockquote
		  paragraph: a
		paragraph: |b
		",
		backspace,
	);

	assert_eq!(
		result,
		dedent(
			"
		blockquote
		  paragraph: a|b
	"
		)
	);
}

#[test]
fn backspace_in_a_list_item_without_list_rules_joins_the_text_before() {
	let result = after(
		"
		paragraph: intro
		bulletList
		  listItem
		    paragraph: |one
		",
		backspace,
	);

	assert_eq!(result, "paragraph: intro|one");
}

#[test]
fn backspace_after_a_divider_selects_it_then_deletes_it() {
	let mut state = state(
		"
		paragraph: a
		horizontalRule
		paragraph: |b
		",
	);
	backspace(&mut state).expect("select");
	assert_eq!(
		render(&state),
		dedent(
			"
		paragraph: a
		horizontalRule @
		paragraph: b
	"
		)
	);

	backspace(&mut state).expect("delete");
	assert_eq!(
		render(&state),
		dedent(
			"
		paragraph: a|
		paragraph: b
	"
		)
	);
}

#[test]
fn backspace_in_an_empty_block_after_a_divider_removes_the_block() {
	let result = after(
		"
		horizontalRule
		paragraph: |
		",
		backspace,
	);

	assert_eq!(result, "horizontalRule @");
}

#[test]
fn backspace_at_the_document_start_resets_an_empty_heading() {
	let empty = after("heading level=3: |", backspace);
	let full = after("heading: |x", backspace);
	let mut plain = state("paragraph: |x");

	assert_eq!(empty, "paragraph: |");
	assert_eq!(full, "heading: |x");
	assert_eq!(backspace(&mut plain), Ok(None));
}

#[test]
fn delete_at_the_end_pulls_the_next_block_in() {
	let result = after(
		"
		paragraph: one|
		bulletList
		  listItem
		    paragraph: two
		  listItem
		    paragraph: three
		",
		delete,
	);

	assert_eq!(
		result,
		dedent(
			"
		paragraph: one|two
		bulletList
		  listItem
		    paragraph: three
	"
		)
	);
}

#[test]
fn delete_before_a_divider_selects_it_and_does_nothing_at_the_end() {
	let selected = after(
		"
		paragraph: a|
		horizontalRule
		",
		delete,
	);
	let mut last = state("paragraph: a|");

	assert_eq!(
		selected,
		dedent(
			"
		paragraph: a
		horizontalRule @
	"
		)
	);
	assert_eq!(delete(&mut last), Ok(None));
}

#[test]
fn deleting_a_range_across_list_items_joins_their_contents() {
	let result = after(
		"
		bulletList
		  listItem
		    paragraph: o^ne
		  listItem
		    paragraph: two
		  listItem
		    paragraph: th|ree
		    bulletList
		      listItem
		        paragraph: nested
		",
		commands::delete_selection,
	);

	assert_eq!(
		result,
		dedent(
			"
		bulletList
		  listItem
		    paragraph: o|ree
		    bulletList
		      listItem
		        paragraph: nested
	"
		)
	);
}

#[test]
fn deleting_every_block_leaves_an_empty_paragraph() {
	let mut state = state("horizontalRule @");
	commands::delete_selection(&mut state).expect("delete");

	assert_eq!(render(&state), "paragraph: |");
}

#[test]
fn setting_the_block_type_changes_every_selected_text_block() {
	let mut state = state(
		"
		paragraph: o^ne
		bulletList
		  listItem
		    paragraph: tw|o
		",
	);
	let heading = kind(&state, "heading");
	let level = Attrs::new().with("level", 2);
	commands::set_block_type(&mut state, heading, &level).expect("set");

	assert_eq!(
		render(&state),
		dedent(
			"
		heading level=2: o^ne
		bulletList
		  listItem
		    paragraph: tw|o
	"
		),
		"listItem does not accept headings"
	);
}

#[test]
fn toggling_a_block_type_switches_back_to_paragraphs() {
	let mut state = state("paragraph: x|");
	let heading = kind(&state, "heading");
	let level = Attrs::new().with("level", 2);

	commands::toggle_block_type(&mut state, heading, &level).expect("on");
	assert_eq!(render(&state), "heading level=2: x|");
	assert!(commands::is_block_active(&state, heading, &level));

	commands::toggle_block_type(&mut state, heading, &level).expect("off");
	assert_eq!(render(&state), "paragraph: x|");
}

#[test]
fn block_type_commands_reject_containers_and_leaves() {
	let mut state = state("paragraph: x|");
	let quote = kind(&state, "blockquote");

	assert_eq!(
		commands::set_block_type(&mut state, quote, &Attrs::new()),
		Ok(None)
	);
}

#[test]
fn block_attributes_merge_and_skip_no_ops() {
	let mut state = state(
		"
		taskList
		  taskItem
		    paragraph: |todo
		",
	);
	let item = state.doc().children(Some(state.doc().children(None)[0]))[0];
	let checked = Attrs::new().with("checked", true);
	commands::set_block_attrs(&mut state, item, &checked).expect("check");

	assert_eq!(
		render(&state),
		dedent(
			"
		taskList
		  taskItem checked=true
		    paragraph: |todo
	"
		)
	);
	assert_eq!(
		commands::set_block_attrs(&mut state, item, &checked),
		Ok(None)
	);
}

#[test]
fn toggling_a_blockquote_wraps_then_lifts() {
	let mut state = state(
		"
		paragraph: o^ne
		paragraph: tw|o
		paragraph: three
		",
	);
	let quote = kind(&state, "blockquote");

	commands::toggle_wrap(&mut state, quote, Attrs::new()).expect("wrap");
	assert_eq!(
		render(&state),
		dedent(
			"
		blockquote
		  paragraph: o^ne
		  paragraph: tw|o
		paragraph: three
	"
		)
	);

	commands::toggle_wrap(&mut state, quote, Attrs::new()).expect("lift");
	assert_eq!(
		render(&state),
		dedent(
			"
		paragraph: o^ne
		paragraph: tw|o
		paragraph: three
	"
		)
	);
}

#[test]
fn lifting_the_middle_of_a_quote_splits_it() {
	let result = after(
		"
		blockquote
		  paragraph: a
		  paragraph: |b
		  paragraph: c
		",
		commands::lift_block,
	);

	assert_eq!(
		result,
		dedent(
			"
		blockquote
		  paragraph: a
		paragraph: |b
		blockquote
		  paragraph: c
	"
		)
	);
}

#[test]
fn lifting_fails_where_the_parent_rejects_the_block() {
	let mut root = state("paragraph: |a");
	let mut item = state(
		"
		bulletList
		  listItem
		    paragraph: |a
		",
	);

	assert_eq!(commands::lift_block(&mut root), Ok(None));
	assert_eq!(commands::lift_block(&mut item), Ok(None));
}

#[test]
fn wrapping_checks_the_schema() {
	let mut state = state("paragraph: |a");
	let item = kind(&state, "listItem");

	assert_eq!(commands::wrap_in(&mut state, item, Attrs::new()), Ok(None));
}

#[test]
fn inserting_a_divider_splits_the_paragraph() {
	let middle = after("paragraph: ab|cd", |state| {
		commands::insert_leaf(state, kind(state, "horizontalRule"), Attrs::new())
	});
	let end = after("heading: ab|", |state| {
		commands::insert_leaf(state, kind(state, "horizontalRule"), Attrs::new())
	});
	let start = after("paragraph: |ab", |state| {
		commands::insert_leaf(state, kind(state, "horizontalRule"), Attrs::new())
	});

	assert_eq!(
		middle,
		dedent(
			"
		paragraph: ab
		horizontalRule
		paragraph: |cd
	"
		)
	);
	assert_eq!(
		end,
		dedent(
			"
		heading: ab
		horizontalRule
		paragraph: |
	"
		)
	);
	assert_eq!(
		start,
		dedent(
			"
		horizontalRule
		paragraph: |ab
	"
		)
	);
}

#[test]
fn inserting_a_non_leaf_as_a_leaf_does_nothing() {
	let mut state = state("paragraph: |ab");
	let paragraph = kind(&state, "paragraph");

	assert_eq!(
		commands::insert_leaf(&mut state, paragraph, Attrs::new()),
		Ok(None)
	);
}

#[test]
fn splitting_keeps_stored_marks_for_the_new_line() {
	let mut state = state("paragraph: a|");
	let bold = mark(&state, "bold");
	commands::toggle_mark(&mut state, &bold).expect("toggle");
	commands::split_block(&mut state).expect("split");
	commands::insert_text(&mut state, "b").expect("type");

	assert_eq!(
		render(&state),
		dedent(
			"
		paragraph: a
		paragraph: {bold:b}|
	"
		)
	);
}
