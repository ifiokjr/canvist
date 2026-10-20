//! Vertical caret motion and line boundaries for the editor engine.
//!
//! The editor owns grapheme and word motion; anything that depends on where
//! lines wrap goes through a [`LayoutNavigator`], from
//! [`DocumentLayout::navigator`]. The editor engine defines the navigation
//! trait it calls, so it stays free of layout; hosts adapt this type to it.

use canvist_model::Document;
use canvist_model::Point;

use crate::Affinity;
use crate::Caret;
use crate::DocumentLayout;
use crate::LayoutError;
use crate::TextMeasurer;
use crate::document::GridStep;
use crate::document::RowKind;

/// Up or down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VerticalDirection {
	/// Towards the start of the document.
	Up,
	/// Towards the end of the document.
	Down,
}

/// The start or end of a visual line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineEdge {
	/// The line's first caret offset.
	Start,
	/// The line's last caret offset.
	End,
}

/// The result of a vertical move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerticalMotion {
	/// Where the caret lands.
	pub caret: Caret,
	/// The x position to aim for on the next vertical move, in document
	/// pixels. Keep passing it back until the caret moves horizontally, so
	/// moving through a short line does not lose the column.
	pub goal_x: f32,
}

/// Line-aware caret motion backed by a [`DocumentLayout`]. Blocks it moves
/// into are laid out on demand, with one measurer call each.
pub struct LayoutNavigator<'a> {
	layout: &'a mut DocumentLayout,
	doc: &'a Document,
	measurer: &'a mut dyn TextMeasurer,
}

impl std::fmt::Debug for LayoutNavigator<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("LayoutNavigator").finish_non_exhaustive()
	}
}

impl DocumentLayout {
	/// A navigator over this layout.
	pub fn navigator<'a>(
		&'a mut self,
		doc: &'a Document,
		measurer: &'a mut dyn TextMeasurer,
	) -> LayoutNavigator<'a> {
		LayoutNavigator {
			layout: self,
			doc,
			measurer,
		}
	}
}

impl LayoutNavigator<'_> {
	/// The row of `caret`, laid out.
	fn row(&mut self, caret: Caret) -> Result<usize, LayoutError> {
		let index = self
			.layout
			.row_index(caret.point.block)
			.ok_or(LayoutError::OutOfSync(caret.point.block))?;
		self.layout.ensure_row(self.doc, self.measurer, index)?;
		Ok(index)
	}

	/// The next text row from `index` in `direction`, laid out.
	fn neighbour(
		&mut self,
		index: usize,
		direction: VerticalDirection,
		goal_x: f32,
	) -> Result<Option<usize>, LayoutError> {
		let rows = self.layout.rows();
		let step = self
			.layout
			.grid_step(index, direction == VerticalDirection::Down, goal_x);

		let found = match (step, direction) {
			(GridStep::Row(found), _) => found,
			(GridStep::Flow, VerticalDirection::Up) => {
				rows[..index]
					.iter()
					.rposition(|row| row.kind == RowKind::Text)
			}
			(GridStep::Flow, VerticalDirection::Down) => {
				rows[index + 1..]
					.iter()
					.position(|row| row.kind == RowKind::Text)
					.map(|offset| index + 1 + offset)
			}
		};

		if let Some(found) = found {
			self.layout.ensure_row(self.doc, self.measurer, found)?;
		}

		Ok(found)
	}

	/// The row and line one visual line away from `line` of row `index`.
	/// Within a block this is the neighbouring line; at a block's first or last
	/// line it is the nearest line of the neighbouring text row.
	fn adjacent_line(
		&mut self,
		index: usize,
		line: usize,
		direction: VerticalDirection,
		goal_x: f32,
	) -> Result<Option<(usize, usize)>, LayoutError> {
		let line_count = self.layout.rows()[index]
			.layout
			.as_ref()
			.map_or(0, |layout| layout.lines.len());

		let within = match direction {
			VerticalDirection::Up => line.checked_sub(1),
			VerticalDirection::Down => Some(line + 1).filter(|&next| next < line_count),
		};

		if let Some(within) = within {
			return Ok(Some((index, within)));
		}

		let Some(row) = self.neighbour(index, direction, goal_x)? else {
			return Ok(None);
		};
		let Some(layout) = self.layout.rows()[row].layout.as_ref() else {
			return Ok(None);
		};

		let entry = match direction {
			VerticalDirection::Up => layout.lines.len().checked_sub(1),
			VerticalDirection::Down => Some(0),
		};

		Ok(entry.map(|entry| (row, entry)))
	}
}

impl LayoutNavigator<'_> {
	/// Move one visual line up or down from `from`, aiming at `goal_x`
	/// (document pixels; `None` uses the caret's own position). Returns
	/// `Ok(None)` at the first or last line of the document. Leaf blocks such
	/// as images are skipped.
	pub fn vertical(
		&mut self,
		from: Caret,
		direction: VerticalDirection,
		goal_x: Option<f32>,
	) -> Result<Option<VerticalMotion>, LayoutError> {
		let index = self.row(from)?;
		let goal_x = goal_x.or_else(|| self.layout.caret_x(from)).unwrap_or(0.0);
		let Some(layout) = self.layout.rows()[index].layout.as_ref() else {
			return Ok(None);
		};
		let line = layout.line_index(from.point.offset, from.affinity);
		let Some((row, line)) = self.adjacent_line(index, line, direction, goal_x)? else {
			return Ok(None);
		};
		let origin = self.layout.origin(row);
		let Some(layout) = self.layout.rows()[row].layout.as_ref() else {
			return Ok(None);
		};
		let (offset, affinity) = layout.hit(line, goal_x - origin.x);
		let block = self.layout.rows()[row].block;

		Ok(Some(VerticalMotion {
			caret: Caret::with_affinity(Point::new(block, offset), affinity),
			goal_x,
		}))
	}

	/// The start or end of the visual line holding `at`.
	pub fn line_edge(&mut self, at: Caret, edge: LineEdge) -> Result<Caret, LayoutError> {
		let index = self.row(at)?;
		let Some(layout) = self.layout.rows()[index].layout.as_ref() else {
			return Ok(at);
		};
		let line = &layout.lines[layout.line_index(at.point.offset, at.affinity)];

		let (offset, affinity) = match edge {
			LineEdge::Start => (line.range.start, Affinity::Downstream),
			LineEdge::End if line.soft => (line.caret_end, Affinity::Upstream),
			LineEdge::End => (line.caret_end, Affinity::Downstream),
		};

		Ok(Caret::with_affinity(
			Point::new(at.point.block, offset),
			affinity,
		))
	}
}
