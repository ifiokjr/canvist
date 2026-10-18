//! Selection motion: by grapheme, word, line, and document, plus vertical
//! motion. Line and vertical motion ask a [`LineNavigator`], because only
//! layout knows where visual lines are.

use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Selection;

use crate::Affinity;
use crate::Caret;
use crate::CommandResult;
use crate::EditKind;
use crate::EditorError;
use crate::EditorState;
use crate::LineEdge;
use crate::LineNavigator;
use crate::VerticalDirection;
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

/// Where moving one `unit` in `direction` from `from` lands, and the
/// affinity the caret has there. Crossing a block boundary lands at the
/// adjacent text block or atomic block.
pub fn target(
	doc: &Document,
	from: Caret,
	direction: Direction,
	unit: Unit,
	lines: &mut dyn LineNavigator,
) -> Result<Option<(Stop, Affinity)>, EditorError> {
	let point = from.point;
	let Some(content) = doc
		.inline(point.block)
		.map(canvist_model::InlineText::as_str)
	else {
		return Ok(None);
	};
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
		(Unit::Line, _) => return line_edge(doc, from, direction, lines),
		(Unit::Document, _) => {
			let boundary = document_boundary(doc, point, direction);
			return Ok(boundary.map(|stop| (stop, Affinity::Downstream)));
		}
	};
	let stop = match (within, direction) {
		(Some(offset), _) => Some(Stop::Text(Point::new(point.block, offset))),
		(None, Direction::Backward) => position::prev_stop(doc, point.block),
		(None, Direction::Forward) => position::next_stop(doc, point.block),
	};

	Ok(stop.map(|stop| (stop, Affinity::Downstream)))
}

/// The start or end of the visual line, or `None` if the caret is there.
fn line_edge(
	doc: &Document,
	from: Caret,
	direction: Direction,
	lines: &mut dyn LineNavigator,
) -> Result<Option<(Stop, Affinity)>, EditorError> {
	let edge = match direction {
		Direction::Backward => LineEdge::Start,
		Direction::Forward => LineEdge::End,
	};
	let caret = lines.line_edge(doc, from, edge)?;
	let moved = caret.point != from.point || caret.affinity != from.affinity;

	Ok(moved.then_some((Stop::Text(caret.point), caret.affinity)))
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
	lines: &mut dyn LineNavigator,
) -> CommandResult {
	let doc = state.doc();
	let mut affinity = Affinity::Downstream;
	let next = match state.selection() {
		Selection::Text { anchor, head } => {
			let collapse = !extend && anchor != head && unit == Unit::Grapheme;

			if collapse {
				collapse_range(doc, state.selection(), direction)
			} else {
				let from = Caret {
					point: head,
					affinity: state.affinity(),
				};
				let found = target(doc, from, direction, unit, lines)?;
				found.and_then(|(stop, landed)| {
					affinity = landed;
					text_motion(doc, anchor, stop, direction, extend)
				})
			}
		}
		Selection::Blocks { anchor, head } => leave_blocks(doc, anchor, head, direction),
	};

	select(state, next, affinity)
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
	lines: &mut dyn LineNavigator,
) -> CommandResult {
	let Selection::Text { anchor, head } = state.selection() else {
		return move_caret(state, direction, Unit::Grapheme, extend, lines);
	};
	let from = Caret {
		point: head,
		affinity: state.affinity(),
	};
	let vertical = match direction {
		Direction::Backward => VerticalDirection::Up,
		Direction::Forward => VerticalDirection::Down,
	};

	let Some(motion) = lines.vertical(state.doc(), from, vertical, state.goal())? else {
		return move_caret(state, direction, Unit::Document, extend, lines);
	};
	let point = motion.caret.point;
	let selection = if extend {
		Selection::text(anchor, point)
	} else {
		Selection::caret(point)
	};

	state.change(EditKind::Selection, |tr| {
		tr.set_selection(selection);
		tr.set_goal(motion.goal_x);
		tr.set_affinity(motion.caret.affinity);
		Ok(true)
	})
}

fn select(
	state: &mut EditorState,
	selection: Option<Selection>,
	affinity: Affinity,
) -> CommandResult {
	let unchanged = selection.is_none_or(|selection| selection == state.selection());

	if unchanged && affinity == state.affinity() {
		return Ok(None);
	}

	let selection = selection.unwrap_or_else(|| state.selection());

	state.change(EditKind::Selection, |tr| {
		tr.set_selection(selection);
		tr.set_affinity(affinity);
		Ok(true)
	})
}
