//! Geometry queries in document coordinates.

use std::ops::Range;

use canvist_model::BlockId;
use canvist_model::Point;
use canvist_model::Selection;

use super::DocumentLayout;
use super::Row;
use super::RowKind;
use crate::Affinity;
use crate::Caret;
use crate::Rect;
use crate::geometry::to_px;
use crate::text::Line;
use crate::text::TextLayout;

/// The top-left corner of a row's content box.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Origin {
	pub(crate) x: f32,
	pub(crate) y: f32,
}

impl DocumentLayout {
	/// The caret nearest to `(x, y)`, grapheme- and bidi-aware.
	///
	/// Points between blocks resolve to the nearest line of the block whose
	/// row contains `y`. Points on leaf blocks such as images resolve to the
	/// end of the previous text block, or the start of the next. Blocks that
	/// have not been measured resolve to their start.
	#[must_use]
	pub fn hit_test(&self, x: f32, y: f32) -> Option<Caret> {
		let index = self.row_at(y)?;
		let row = &self.rows()[index];

		if row.kind != RowKind::Text {
			return self.nearest_text_caret(index);
		}

		let Some(layout) = &row.layout else {
			return Some(Caret::new(Point::new(row.block, 0)));
		};
		let origin = self.origin(index);
		let line = layout.line_at(y - origin.y);
		let (offset, affinity) = layout.hit(line, x - origin.x);

		Some(Caret::with_affinity(
			Point::new(row.block, offset),
			affinity,
		))
	}

	/// The block whose row contains `(x, y)`, including leaf blocks.
	#[must_use]
	pub fn hit_block(&self, _x: f32, y: f32) -> Option<BlockId> {
		self.row_at(y).map(|index| self.rows()[index].block)
	}

	/// The rectangle to draw the caret in. Its horizontal centre is the caret
	/// position. Unmeasured blocks get a caret at their start.
	#[must_use]
	pub fn caret_rect(&self, caret: Caret) -> Option<Rect> {
		let index = self.row_index(caret.point.block)?;
		let row = &self.rows()[index];
		let width = self.theme().chrome().caret_width;
		let origin = self.origin(index);
		let Some(layout) = &row.layout else {
			let height = self.style_of(row).text.line_height_px();

			return Some(Rect::new(origin.x - width / 2.0, origin.y, width, height));
		};

		let offset = caret.point.offset.min(layout.text.len() as u32);
		let line_index = layout.line_index(offset, caret.affinity);
		let line = &layout.lines[line_index];
		let x = layout.caret_x(line_index, offset, caret.affinity);

		Some(Rect::new(
			origin.x + x - width / 2.0,
			origin.y + line.top,
			width,
			line.height,
		))
	}

	/// The caret's x position in document coordinates.
	pub(crate) fn caret_x(&self, caret: Caret) -> Option<f32> {
		let index = self.row_index(caret.point.block)?;
		let origin = self.origin(index);
		let Some(layout) = &self.rows()[index].layout else {
			return Some(origin.x);
		};
		let offset = caret.point.offset.min(layout.text.len() as u32);
		let line = layout.line_index(offset, caret.affinity);

		Some(origin.x + layout.caret_x(line, offset, caret.affinity))
	}

	/// The smallest rectangle around `selection`, for anchoring a bubble menu
	/// or toolbar to it, or `None` for a collapsed or unknown selection.
	#[must_use]
	pub fn selection_bounds(&self, selection: &Selection) -> Option<Rect> {
		self.selection_rects(selection)
			.into_iter()
			.reduce(|bounds, rect| bounds.union(&rect))
	}

	/// Rectangles covering `selection`, one or more per line, extending to
	/// the line's end edge where the selection continues onto the next line.
	#[must_use]
	pub fn selection_rects(&self, selection: &Selection) -> Vec<Rect> {
		match *selection {
			Selection::Text { anchor, head } => self.text_selection_rects(anchor, head),
			Selection::Blocks { anchor, head } => self.block_selection_rects(anchor, head),
		}
	}

	fn text_selection_rects(&self, anchor: Point, head: Point) -> Vec<Rect> {
		let rows = self.row_index(anchor.block).zip(self.row_index(head.block));
		let Some((anchor_row, head_row)) = rows else {
			return Vec::new();
		};

		let anchor = (anchor_row, anchor.offset);
		let head = (head_row, head.offset);
		let start = anchor.min(head);
		let end = anchor.max(head);
		let mut rects = Vec::new();

		for index in start.0..=end.0 {
			self.push_row_selection_rects(&mut rects, index, start, end);
		}

		rects
	}

	/// Push row `index`'s share of a text selection running from `start` to
	/// `end`, each a `(row, offset)` pair.
	fn push_row_selection_rects(
		&self,
		rects: &mut Vec<Rect>,
		index: usize,
		start: (usize, u32),
		end: (usize, u32),
	) {
		let Some(layout) = &self.rows()[index].layout else {
			// An unmeasured row has no lines to select within, so it is
			// covered whole only when the selection passes right through it.
			if index != start.0 && index != end.0 {
				rects.push(self.row_rect(index));
			}

			return;
		};
		let from = if index == start.0 { start.1 } else { 0 };
		let to = if index == end.0 { end.1 } else { u32::MAX };

		self.push_line_rects(rects, index, layout, from..to, index < end.0);
	}

	/// Push the rectangles selecting `range` on each line of row `index`.
	///
	/// Where the selection carries on past a line (`continues`, or `range`
	/// ending beyond it), the highlight also runs to the line's end edge so a
	/// multi-line selection reads as one block, as in native text fields.
	fn push_line_rects(
		&self,
		rects: &mut Vec<Rect>,
		index: usize,
		layout: &TextLayout,
		range: Range<u32>,
		continues: bool,
	) {
		let origin = self.origin(index);

		for (line_index, line) in layout.lines.iter().enumerate() {
			if !selects_line(line, &range) {
				continue;
			}

			let top = origin.y + line.top;
			let spans = layout.spans(line_index, range.start, range.end);
			let extends = continues || range.end > line.caret_end;

			for (left, right) in &spans {
				rects.push(Rect::new(origin.x + left, top, right - left, line.height));
			}

			if extends && let Some((left, right)) = end_edge(layout, line_index, &spans) {
				rects.push(Rect::new(origin.x + left, top, right - left, line.height));
			}
		}
	}

	fn block_selection_rects(&self, anchor: BlockId, head: BlockId) -> Vec<Rect> {
		let range = |id| {
			if let Some(index) = self.row_index(id) {
				return Some((index, index));
			}

			let span = &self.spans()[self.span_index(id)?];
			Some((span.first_row as usize, span.last_row as usize))
		};
		let (Some(a), Some(b)) = (range(anchor), range(head)) else {
			return Vec::new();
		};
		let first = a.0.min(b.0);
		let last = a.1.max(b.1);

		(first..=last).map(|index| self.row_rect(index)).collect()
	}

	/// The caret for a click on the non-text row `index`.
	fn nearest_text_caret(&self, index: usize) -> Option<Caret> {
		let rows = self.rows();
		let before = rows[..index]
			.iter()
			.rev()
			.find(|row| row.kind == RowKind::Text);

		if let Some(row) = before {
			return Some(end_caret(row));
		}

		rows[index + 1..]
			.iter()
			.find(|row| row.kind == RowKind::Text)
			.map(|row| Caret::new(Point::new(row.block, 0)))
	}

	/// The top-left of row `index`'s content box.
	pub(crate) fn origin(&self, index: usize) -> Origin {
		let row = &self.rows()[index];
		let style = self.style_of(row);

		Origin {
			x: row.x + style.inset_left(),
			y: to_px(self.row_top(index)) + style.padding.top,
		}
	}
}

/// The caret at the end of a text row's last line.
pub(crate) fn end_caret(row: &Row) -> Caret {
	let offset = row
		.layout
		.as_ref()
		.and_then(|layout| layout.lines.last())
		.map_or(0, |line| line.caret_end);

	Caret::new(Point::new(row.block, offset))
}

/// Whether a selection over `range` touches `line`. A soft-wrapped line ends
/// where the next one starts, so a selection starting there belongs to the
/// next line.
fn selects_line(line: &Line, range: &Range<u32>) -> bool {
	let starts_after = range.start > line.caret_end || (line.soft && range.start >= line.caret_end);

	line.range.start <= range.end && !starts_after
}

/// The gap between a line's selected `spans` and its end edge (its left edge
/// when right to left), or `None` when the spans already reach it.
fn end_edge(layout: &TextLayout, line_index: usize, spans: &[(f32, f32)]) -> Option<(f32, f32)> {
	let caret_end = layout.lines[line_index].caret_end;
	let line_end = layout.caret_x(line_index, caret_end, Affinity::Upstream);
	let (left, right) = if layout.rtl {
		(
			0.0,
			spans.iter().map(|span| span.0).fold(line_end, f32::min),
		)
	} else {
		(
			spans.iter().map(|span| span.1).fold(line_end, f32::max),
			layout.width,
		)
	};

	(right > left).then_some((left, right))
}
