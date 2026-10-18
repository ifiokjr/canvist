//! Selection motion and selection commands.

mod support;

use canvist_editor::Affinity;
use canvist_editor::Caret;
use canvist_editor::Direction;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::LineEdge;
use canvist_editor::LineNavigator;
use canvist_editor::NavigationError;
use canvist_editor::ParagraphNavigator;
use canvist_editor::Unit;
use canvist_editor::VerticalDirection;
use canvist_editor::VerticalMotion;
use canvist_editor::commands;
use canvist_editor::motion;
use canvist_model::Document;
use canvist_model::Point;
use support::after;
use support::dedent;
use support::render;
use support::state;

fn step(
	direction: Direction,
	unit: Unit,
	extend: bool,
) -> impl FnOnce(&mut EditorState) -> canvist_editor::CommandResult {
	move |state| motion::move_caret(state, direction, unit, extend, &mut ParagraphNavigator)
}

fn vertical(
	direction: Direction,
	extend: bool,
) -> impl FnOnce(&mut EditorState) -> canvist_editor::CommandResult {
	move |state| motion::move_vertical(state, direction, extend, &mut ParagraphNavigator)
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
			&mut ParagraphNavigator
		),
		Ok(None)
	);
	assert_eq!(
		motion::move_caret(
			&mut home,
			Direction::Backward,
			Unit::Line,
			false,
			&mut ParagraphNavigator
		),
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
	let inside_char = canvist_model::Selection::caret(Point::new(block, 1));
	commands::set_selection(&mut state, inside_char).expect("repair");

	assert_eq!(render(&state), "paragraph: |é");
	assert_eq!(commands::set_selection(&mut state, inside_char), Ok(None));
}

/// Soft-wraps every text block into lines of `width` bytes, so the offset
/// where two lines meet is ambiguous without an affinity.
struct Wrapped {
	width: u32,
}

impl Wrapped {
	fn line(&self, doc: &Document, at: Caret) -> (u32, u32) {
		let len = doc
			.inline(at.point.block)
			.map_or(0, canvist_model::InlineText::len);
		let offset = at.point.offset;
		let upstream =
			at.affinity == Affinity::Upstream && offset > 0 && offset.is_multiple_of(self.width);
		let index = if upstream {
			offset / self.width - 1
		} else {
			offset / self.width
		};
		let start = (index * self.width).min(len);

		(start, (start + self.width).min(len))
	}
}

impl LineNavigator for Wrapped {
	fn vertical(
		&mut self,
		doc: &Document,
		from: Caret,
		direction: VerticalDirection,
		goal_x: Option<f32>,
	) -> Result<Option<VerticalMotion>, NavigationError> {
		let (start, end) = self.line(doc, from);
		let len = doc
			.inline(from.point.block)
			.map_or(0, canvist_model::InlineText::len);
		let goal_x = goal_x.unwrap_or((from.point.offset - start) as f32);
		let line_start = match direction {
			VerticalDirection::Up if start > 0 => start - self.width,
			VerticalDirection::Down if end < len => end,
			_ => return Ok(None),
		};
		let offset = (line_start + (goal_x as u32).min(self.width)).min(len);
		let affinity = if offset > line_start && offset.is_multiple_of(self.width) {
			Affinity::Upstream
		} else {
			Affinity::Downstream
		};
		let caret = Caret {
			point: Point::new(from.point.block, offset),
			affinity,
		};

		Ok(Some(VerticalMotion { caret, goal_x }))
	}

	fn line_edge(
		&mut self,
		doc: &Document,
		at: Caret,
		edge: LineEdge,
	) -> Result<Caret, NavigationError> {
		let (start, end) = self.line(doc, at);
		let len = doc
			.inline(at.point.block)
			.map_or(0, canvist_model::InlineText::len);

		Ok(match edge {
			LineEdge::Start => Caret::new(Point::new(at.point.block, start)),
			LineEdge::End if end < len => {
				Caret {
					point: Point::new(at.point.block, end),
					affinity: Affinity::Upstream,
				}
			}
			LineEdge::End => Caret::new(Point::new(at.point.block, end)),
		})
	}
}

struct Unmeasurable;

impl LineNavigator for Unmeasurable {
	fn vertical(
		&mut self,
		_: &Document,
		_: Caret,
		_: VerticalDirection,
		_: Option<f32>,
	) -> Result<Option<VerticalMotion>, NavigationError> {
		Err(NavigationError::new(&"font not loaded"))
	}

	fn line_edge(&mut self, _: &Document, _: Caret, _: LineEdge) -> Result<Caret, NavigationError> {
		Err(NavigationError::new(&"font not loaded"))
	}
}

#[test]
fn line_end_at_a_soft_wrap_stays_on_its_line() {
	let mut wrapped = Wrapped { width: 4 };
	let mut state = state("paragraph: a|bcdefghij");

	motion::move_caret(
		&mut state,
		Direction::Forward,
		Unit::Line,
		false,
		&mut wrapped,
	)
	.expect("end");
	assert_eq!(render(&state), "paragraph: abcd|efghij");
	assert_eq!(state.affinity(), Affinity::Upstream);

	// The upstream caret is still on the first line, so Home goes to 0
	// rather than staying at the start of the second line.
	motion::move_caret(
		&mut state,
		Direction::Backward,
		Unit::Line,
		false,
		&mut wrapped,
	)
	.expect("home");
	assert_eq!(render(&state), "paragraph: |abcdefghij");
	assert_eq!(state.affinity(), Affinity::Downstream);
}

#[test]
fn typing_resets_affinity_to_downstream() {
	let mut wrapped = Wrapped { width: 4 };
	let mut state = state("paragraph: a|bcdefghij");

	motion::move_caret(
		&mut state,
		Direction::Forward,
		Unit::Line,
		false,
		&mut wrapped,
	)
	.expect("end");
	commands::insert_text(&mut state, "x").expect("type");

	assert_eq!(state.affinity(), Affinity::Downstream);
}

#[test]
fn vertical_motion_keeps_its_goal_across_short_lines() {
	let mut wrapped = Wrapped { width: 4 };
	let mut state = state("paragraph: abc|defghij");

	motion::move_vertical(&mut state, Direction::Forward, false, &mut wrapped).expect("down");
	assert_eq!(render(&state), "paragraph: abcdefg|hij");
	assert_eq!(state.goal(), Some(3.0));

	motion::move_vertical(&mut state, Direction::Forward, false, &mut wrapped).expect("down");
	assert_eq!(render(&state), "paragraph: abcdefghij|");
	assert_eq!(state.goal(), Some(3.0));
}

#[test]
fn navigator_errors_reach_the_caller() {
	let mut state = state("paragraph: ab|c");

	let line = motion::move_caret(
		&mut state,
		Direction::Forward,
		Unit::Line,
		false,
		&mut Unmeasurable,
	);
	let vertical = motion::move_vertical(&mut state, Direction::Forward, false, &mut Unmeasurable);

	let expected = EditorError::Navigation(NavigationError::new(&"font not loaded"));
	assert_eq!(line, Err(expected.clone()));
	assert_eq!(vertical, Err(expected));
	assert_eq!(render(&state), "paragraph: ab|c");
}
