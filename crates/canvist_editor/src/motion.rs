//! Selection motion: by grapheme, word, line, and document, plus vertical
//! motion through a [`LineNavigator`].
//!
//! Lines are a layout concept, so the editor never computes them itself.
//! `canvist_layout` implements [`LineNavigator`] from real line boxes; the
//! [`ParagraphNavigator`] fallback treats every text block as one line, which
//! is enough for headless tests and servers.

use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Selection;

use crate::CommandResult;
use crate::EditKind;
use crate::EditorState;
use crate::position;
use crate::position::Stop;
use crate::text;

/// Which way to move or delete.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
	/// Towards the start of the document.
	Backward,
	/// Towards the end of the document.
	Forward,
}

/// How far to move or delete.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
	/// One user-perceived character.
	Grapheme,
	/// To the next word boundary, skipping spaces and punctuation.
	Word,
	/// To the start or end of the visual line.
	Line,
	/// To the start or end of the document.
	Document,
}

/// Answers questions about visual lines, which only layout knows.
pub trait LineNavigator {
	/// The start (`Backward`) or end (`Forward`) of the visual line holding
	/// `point`.
	fn line_boundary(&self, doc: &Document, point: Point, direction: Direction) -> Point;

	/// The point on the line above (`Backward`) or below (`Forward`) closest
	/// to the horizontal position `goal`, or to `point`'s own position when
	/// `goal` is `None`. Returns the point and the goal to keep for the next
	/// vertical move, or `None` on the first or last line.
	fn vertical(
		&self,
		doc: &Document,
		point: Point,
		direction: Direction,
		goal: Option<f32>,
	) -> Option<(Point, f32)>;
}

/// A [`LineNavigator`] that treats each text block as a single line and
/// measures horizontal position in grapheme columns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParagraphNavigator;

impl LineNavigator for ParagraphNavigator {
	fn line_boundary(&self, doc: &Document, point: Point, direction: Direction) -> Point {
		match direction {
			Direction::Backward => Point::new(point.block, 0),
			Direction::Forward => position::end_of(doc, point.block),
		}
	}

	fn vertical(
		&self,
		doc: &Document,
		point: Point,
		direction: Direction,
		goal: Option<f32>,
	) -> Option<(Point, f32)> {
		let current = doc.inline(point.block)?.as_str();
		let goal = goal.unwrap_or_else(|| text::column(current, point.offset) as f32);

		let next = match direction {
			Direction::Backward => doc.prev_text_block(point.block),
			Direction::Forward => doc.next_text_block(point.block),
		}?;

		let next_text = doc.inline(next)?.as_str();
		let offset = text::offset_at_column(next_text, goal.max(0.0) as usize);

		Some((Point::new(next, offset), goal))
	}
}

/// Where moving one `unit` in `direction` from `point` lands. Crossing a
/// block boundary lands at the adjacent text block or atomic block.
#[must_use]
pub fn target(
	doc: &Document,
	point: Point,
	direction: Direction,
	unit: Unit,
	lines: &dyn LineNavigator,
) -> Option<Stop> {
	let content = doc.inline(point.block)?.as_str();
	let len = content.len() as u32;

	let within = match (unit, direction) {
		(Unit::Grapheme, Direction::Backward) => text::prev_grapheme(content, point.offset),
		(Unit::Grapheme, Direction::Forward) => text::next_grapheme(content, point.offset),
		(Unit::Word, Direction::Backward) => {
			text::prev_word(content, point.offset).or((point.offset > 0).then_some(0))
		}
		(Unit::Word, Direction::Forward) => {
			text::next_word(content, point.offset).or((point.offset < len).then_some(len))
		}
		(Unit::Line, _) => {
			let boundary = lines.line_boundary(doc, point, direction);
			return (boundary != point).then_some(Stop::Text(boundary));
		}
		(Unit::Document, _) => return document_boundary(doc, point, direction),
	};

	match (within, direction) {
		(Some(offset), _) => Some(Stop::Text(Point::new(point.block, offset))),
		(None, Direction::Backward) => position::prev_stop(doc, point.block),
		(None, Direction::Forward) => position::next_stop(doc, point.block),
	}
}

fn document_boundary(doc: &Document, point: Point, direction: Direction) -> Option<Stop> {
	let selection = match direction {
		Direction::Backward => position::start_selection(doc),
		Direction::Forward => position::end_selection(doc),
	};

	let boundary = selection.as_caret()?;
	(boundary != point).then_some(Stop::Text(boundary))
}

/// Move the caret, or extend the selection head, by `unit`.
///
/// Without `extend`, a range collapses to the side it is moved towards
/// (for grapheme moves), and moving onto an atomic block selects it. With
/// `extend`, the head skips over atomic blocks.
pub fn move_caret(
	state: &mut EditorState,
	direction: Direction,
	unit: Unit,
	extend: bool,
	lines: &dyn LineNavigator,
) -> CommandResult {
	let doc = state.doc();

	let next = match state.selection() {
		Selection::Text { anchor, head } => {
			let collapse = !extend && anchor != head && unit == Unit::Grapheme;

			if collapse {
				collapse_range(doc, state.selection(), direction)
			} else {
				let stop = target(doc, head, direction, unit, lines);
				stop.and_then(|stop| text_motion(doc, anchor, stop, direction, extend))
			}
		}
		Selection::Blocks { anchor, head } => leave_blocks(doc, anchor, head, direction),
	};

	select(state, next)
}

fn collapse_range(doc: &Document, selection: Selection, direction: Direction) -> Option<Selection> {
	let range = position::text_range(doc, selection)?;

	let point = match direction {
		Direction::Backward => range.from,
		Direction::Forward => range.to,
	};

	Some(Selection::caret(point))
}

fn text_motion(
	doc: &Document,
	anchor: Point,
	stop: Stop,
	direction: Direction,
	extend: bool,
) -> Option<Selection> {
	match (stop, extend) {
		(Stop::Text(head), true) => Some(Selection::text(anchor, head)),
		(Stop::Text(head), false) => Some(Selection::caret(head)),
		(Stop::Leaf(leaf), false) => Some(Selection::block(leaf)),
		(Stop::Leaf(leaf), true) => {
			let head = skip_leaves(doc, leaf, direction)?;
			Some(Selection::text(anchor, head))
		}
	}
}

/// The first text point past a run of atomic blocks.
fn skip_leaves(doc: &Document, leaf: BlockId, direction: Direction) -> Option<Point> {
	let mut current = leaf;

	loop {
		let stop = match direction {
			Direction::Backward => position::prev_stop(doc, current),
			Direction::Forward => position::next_stop(doc, current),
		}?;

		match stop {
			Stop::Text(point) => return Some(point),
			Stop::Leaf(next) => current = next,
		}
	}
}

/// Leave a block selection: into its text when it has some, otherwise past
/// it to the adjacent stop.
fn leave_blocks(
	doc: &Document,
	anchor: BlockId,
	head: BlockId,
	direction: Direction,
) -> Option<Selection> {
	let (parent, range) = position::sibling_range(doc, anchor, head)
		.or_else(|| position::sibling_range(doc, head, anchor))?;
	let children = doc.children(parent);
	let covered = position::covered_blocks(doc, anchor, head);

	let inside = match direction {
		Direction::Backward => covered.first().map(|(block, _)| Point::new(*block, 0)),
		Direction::Forward => {
			covered
				.last()
				.map(|(block, range)| Point::new(*block, range.end))
		}
	};

	if let Some(point) = inside {
		return Some(Selection::caret(point));
	}

	let stop = match direction {
		Direction::Backward => position::prev_stop(doc, children[range.start]),
		Direction::Forward => position::next_stop(doc, children[range.end - 1]),
	}?;

	Some(match stop {
		Stop::Text(point) => Selection::caret(point),
		Stop::Leaf(leaf) => Selection::block(leaf),
	})
}

/// Move the caret to the line above or below, keeping its horizontal goal.
/// On the first or last line it moves to the start or end of the document.
pub fn move_vertical(
	state: &mut EditorState,
	direction: Direction,
	extend: bool,
	lines: &dyn LineNavigator,
) -> CommandResult {
	let Selection::Text { anchor, head } = state.selection() else {
		return move_caret(state, direction, Unit::Grapheme, extend, lines);
	};
	let doc = state.doc();

	let Some((point, goal)) = lines.vertical(doc, head, direction, state.goal()) else {
		return move_caret(state, direction, Unit::Document, extend, lines);
	};
	let selection = if extend {
		Selection::text(anchor, point)
	} else {
		Selection::caret(point)
	};

	state.change(EditKind::Selection, |tr| {
		tr.set_selection(selection);
		tr.set_goal(goal);
		Ok(true)
	})
}

fn select(state: &mut EditorState, selection: Option<Selection>) -> CommandResult {
	let Some(selection) = selection.filter(|&selection| selection != state.selection()) else {
		return Ok(None);
	};

	state.change(EditKind::Selection, |tr| {
		tr.set_selection(selection);
		Ok(true)
	})
}
