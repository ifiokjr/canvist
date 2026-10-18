//! Selection motion and selection commands.

mod support;

use canvist_editor::Direction;
use canvist_editor::EditorState;
use canvist_editor::ParagraphNavigator;
use canvist_editor::Unit;
use canvist_editor::commands;
use canvist_editor::motion;
use support::after;
use support::dedent;
use support::render;
use support::state;

const LINES: ParagraphNavigator = ParagraphNavigator;

fn step(
	direction: Direction,
	unit: Unit,
	extend: bool,
) -> impl FnOnce(&mut EditorState) -> canvist_editor::CommandResult {
	move |state| motion::move_caret(state, direction, unit, extend, &LINES)
}

fn vertical(
	direction: Direction,
	extend: bool,
) -> impl FnOnce(&mut EditorState) -> canvist_editor::CommandResult {
	move |state| motion::move_vertical(state, direction, extend, &LINES)
}

#[test]
fn moves_by_grapheme_across_blocks() {
	let doc = "
		paragraph: ab|
		paragraph: cd
	";
	let forward = after(doc, step(Direction::Forward, Unit::Grapheme, false));
	let backward = after(
		"paragraph: a👍🏽|",
		step(Direction::Backward, Unit::Grapheme, false),
	);

	assert_eq!(
		forward,
		dedent(
			"
		paragraph: ab
		paragraph: |cd
	"
		)
	);
	assert_eq!(backward, "paragraph: a|👍🏽");
}

#[test]
fn moving_without_shift_collapses_a_range() {
	let backward = after(
		"paragraph: a^bc|d",
		step(Direction::Backward, Unit::Grapheme, false),
	);
	let forward = after(
		"paragraph: a|bc^d",
		step(Direction::Forward, Unit::Grapheme, false),
	);

	assert_eq!(backward, "paragraph: a|bcd");
	assert_eq!(forward, "paragraph: abc|d");
}

#[test]
fn extending_moves_only_the_head() {
	let result = after(
		"paragraph: a|bcd",
		step(Direction::Forward, Unit::Word, true),
	);

	assert_eq!(result, "paragraph: a^bcd|");
}

#[test]
fn word_motion_crosses_blocks_at_their_edges() {
	let result = after(
		"
		paragraph: one
		paragraph: |two
		",
		step(Direction::Backward, Unit::Word, false),
	);

	assert_eq!(
		result,
		dedent(
			"
		paragraph: one|
		paragraph: two
	"
		)
	);
}

#[test]
fn line_and_document_motion() {
	let doc = "
		paragraph: one
		paragraph: tw|o
		paragraph: three
	";

	assert_eq!(
		after(doc, step(Direction::Backward, Unit::Line, false)),
		dedent(
			"
		paragraph: one
		paragraph: |two
		paragraph: three
	"
		)
	);
	assert_eq!(
		after(doc, step(Direction::Forward, Unit::Document, true)),
		dedent(
			"
		paragraph: one
		paragraph: tw^o
		paragraph: three|
	"
		)
	);
}

#[test]
fn motion_at_the_document_edge_does_nothing() {
	let mut start = state("paragraph: |a");
	let mut home = state("paragraph: |a");

	assert_eq!(
		motion::move_caret(
			&mut start,
			Direction::Backward,
			Unit::Grapheme,
			false,
			&LINES
		),
		Ok(None)
	);
	assert_eq!(
		motion::move_caret(&mut home, Direction::Backward, Unit::Line, false, &LINES),
		Ok(None)
	);
}

#[test]
fn arrowing_onto_a_divider_selects_it_and_arrowing_off_leaves_it() {
	let mut state = state(
		"
		paragraph: a|
		horizontalRule
		paragraph: b
		",
	);
	step(Direction::Forward, Unit::Grapheme, false)(&mut state).expect("onto");
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

	step(Direction::Forward, Unit::Grapheme, false)(&mut state).expect("off");
	assert_eq!(
		render(&state),
		dedent(
			"
		paragraph: a
		horizontalRule
		paragraph: |b
	"
		)
	);
}

#[test]
fn extending_skips_over_dividers() {
	let result = after(
		"
		paragraph: a|
		horizontalRule
		horizontalRule
		paragraph: b
		",
		step(Direction::Forward, Unit::Grapheme, true),
	);

	assert_eq!(
		result,
		dedent(
			"
		paragraph: a^
		horizontalRule
		horizontalRule
		paragraph: |b
	"
		)
	);
}

#[test]
fn leaving_a_text_block_selection_enters_its_text() {
	let mut state = state(
		"
		paragraph: one
		paragraph: two @
		",
	);
	step(Direction::Backward, Unit::Grapheme, false)(&mut state).expect("leave");

	assert_eq!(
		render(&state),
		dedent(
			"
		paragraph: one
		paragraph: |two
	"
		)
	);
}

#[test]
fn vertical_motion_keeps_its_goal_column() {
	let mut state = state(
		"
		paragraph: abcd|e
		paragraph: x
		paragraph: 12345
		",
	);
	vertical(Direction::Forward, false)(&mut state).expect("down");
	assert_eq!(state.goal(), Some(4.0));
	vertical(Direction::Forward, false)(&mut state).expect("down");

	assert_eq!(
		render(&state),
		dedent(
			"
		paragraph: abcde
		paragraph: x
		paragraph: 1234|5
	"
		)
	);
}

#[test]
fn vertical_motion_past_the_last_line_goes_to_the_end() {
	let result = after("paragraph: a|b", vertical(Direction::Forward, false));
	let extended = after(
		"
		paragraph: a|b
		paragraph: cd
		",
		vertical(Direction::Forward, true),
	);

	assert_eq!(result, "paragraph: ab|");
	assert_eq!(
		extended,
		dedent(
			"
		paragraph: a^b
		paragraph: c|d
	"
		)
	);
}

#[test]
fn any_other_change_resets_the_goal() {
	let mut state = state(
		"
		paragraph: abc|
		paragraph: x
		",
	);
	vertical(Direction::Forward, false)(&mut state).expect("down");
	commands::insert_text(&mut state, "y").expect("type");

	assert_eq!(state.goal(), None);
}

#[test]
fn select_all_spans_the_document() {
	let text = after(
		"
		paragraph: o|ne
		bulletList
		  listItem
		    paragraph: two
		",
		commands::select_all,
	);
	let blocks = after(
		"
		paragraph: o|ne
		horizontalRule
		",
		commands::select_all,
	);

	assert_eq!(
		text,
		dedent(
			"
		paragraph: ^one
		bulletList
		  listItem
		    paragraph: two|
	"
		)
	);
	assert_eq!(
		blocks,
		dedent(
			"
		paragraph: one @
		horizontalRule @
	"
		)
	);
}

#[test]
fn select_parent_grows_outwards() {
	let mut state = state(
		"
		bulletList
		  listItem
		    paragraph: o|ne
		",
	);
	commands::select_parent(&mut state).expect("paragraph");
	assert_eq!(
		render(&state),
		dedent(
			"
		bulletList
		  listItem
		    paragraph: one @
	"
		)
	);
	commands::select_parent(&mut state).expect("item");
	commands::select_parent(&mut state).expect("list");
	assert_eq!(
		render(&state),
		dedent(
			"
		bulletList @
		  listItem
		    paragraph: one
	"
		)
	);
	assert_eq!(commands::select_parent(&mut state), Ok(None));
}

#[test]
fn select_blocks_requires_siblings() {
	let mut state = state(
		"
		paragraph: |a
		bulletList
		  listItem
		    paragraph: b
		",
	);
	let root = state.doc().children(None).to_vec();
	let item = state.doc().children(Some(root[1]))[0];

	assert_eq!(commands::select_blocks(&mut state, root[0], item), Ok(None));
	commands::select_blocks(&mut state, root[0], root[1]).expect("siblings");
	assert_eq!(
		render(&state),
		dedent(
			"
		paragraph: a @
		bulletList @
		  listItem
		    paragraph: b
	"
		)
	);
}

#[test]
fn set_selection_repairs_offsets_and_ignores_no_ops() {
	let mut state = state("paragraph: é|");
	let block = state.doc().first_text_block().expect("text");
	let inside_char = canvist_model::Selection::caret(canvist_model::Point::new(block, 1));
	commands::set_selection(&mut state, inside_char).expect("repair");

	assert_eq!(render(&state), "paragraph: |é");
	assert_eq!(commands::set_selection(&mut state, inside_char), Ok(None));
}
