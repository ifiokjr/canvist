//! Visual lines: the interface between the editor and layout.
//!
//! Lines exist only after layout, so the editor asks a [`LineNavigator`]
//! (implemented by `canvist_layout`) where the line above, below, or around
//! a caret is. The trait lives here so the editor never depends on layout,
//! and headless users get the [`ParagraphNavigator`] fallback.

use canvist_model::Document;
use canvist_model::Point;

use crate::position;
use crate::text;

/// Which visual line a caret at a soft line break belongs to: the offset
/// where one line wraps into the next is both the end of the first line
/// and the start of the second.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Affinity {
	/// The end of the earlier line.
	Upstream,
	/// The start of the later line, which typing and most motion produce.
	#[default]
	Downstream,
}

/// A caret: a point and the line it belongs to at a soft wrap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Caret {
	/// Where the caret is.
	pub point: Point,
	/// Which line it shows on at a soft wrap.
	pub affinity: Affinity,
}

impl Caret {
	/// A downstream caret at `point`.
	#[must_use]
	pub const fn new(point: Point) -> Self {
		Self {
			point,
			affinity: Affinity::Downstream,
		}
	}
}

/// Up or down a visual line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VerticalDirection {
	/// The line above.
	Up,
	/// The line below.
	Down,
}

/// The logical start or end of a visual line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineEdge {
	/// Where the line starts.
	Start,
	/// Where the line ends.
	End,
}

/// The result of moving up or down a line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerticalMotion {
	/// Where the caret lands.
	pub caret: Caret,
	/// The horizontal position to aim for on the next vertical move, in
	/// the navigator's own units (document pixels for layout).
	pub goal_x: f32,
}

/// Why a navigator could not answer, e.g. a block it had to lay out could
/// not be measured.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct NavigationError(String);

impl NavigationError {
	/// Wrap a navigator's own error.
	#[must_use]
	pub fn new(error: &impl std::fmt::Display) -> Self {
		Self(error.to_string())
	}
}

/// Answers questions about visual lines, which only layout knows.
///
/// Methods take `&mut self` so an implementation can lay out and measure
/// blocks it has not seen yet, and are fallible because measuring can fail.
pub trait LineNavigator {
	/// The caret one visual line up or down from `from`, closest to the
	/// horizontal position `goal_x` (or `from`'s own position when `None`).
	/// `Ok(None)` on the first or last line. Atomic blocks are skipped.
	fn vertical(
		&mut self,
		doc: &Document,
		from: Caret,
		direction: VerticalDirection,
		goal_x: Option<f32>,
	) -> Result<Option<VerticalMotion>, NavigationError>;

	/// The start or end of the visual line holding `at`.
	fn line_edge(
		&mut self,
		doc: &Document,
		at: Caret,
		edge: LineEdge,
	) -> Result<Caret, NavigationError>;
}

/// A [`LineNavigator`] that treats each text block as a single line and
/// measures horizontal position in grapheme columns, for headless use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParagraphNavigator;

impl LineNavigator for ParagraphNavigator {
	fn vertical(
		&mut self,
		doc: &Document,
		from: Caret,
		direction: VerticalDirection,
		goal_x: Option<f32>,
	) -> Result<Option<VerticalMotion>, NavigationError> {
		let point = from.point;
		let Some(current) = doc.inline(point.block) else {
			return Ok(None);
		};
		let goal_x = goal_x.unwrap_or_else(|| text::column(current.as_str(), point.offset) as f32);
		let next = match direction {
			VerticalDirection::Up => doc.prev_text_block(point.block),
			VerticalDirection::Down => doc.next_text_block(point.block),
		};
		let Some((next, text)) = next.and_then(|id| Some((id, doc.inline(id)?.as_str()))) else {
			return Ok(None);
		};
		let offset = text::offset_at_column(text, goal_x.max(0.0) as usize);

		Ok(Some(VerticalMotion {
			caret: Caret::new(Point::new(next, offset)),
			goal_x,
		}))
	}

	fn line_edge(
		&mut self,
		doc: &Document,
		at: Caret,
		edge: LineEdge,
	) -> Result<Caret, NavigationError> {
		let point = match edge {
			LineEdge::Start => Point::new(at.point.block, 0),
			LineEdge::End => position::end_of(doc, at.point.block),
		};

		Ok(Caret::new(point))
	}
}
