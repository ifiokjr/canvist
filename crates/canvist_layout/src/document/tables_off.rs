//! Without the `tables` feature no table is laid out on a grid: its cells
//! stack like any container's children, and every query that would consult
//! a grid follows the plain flow of rows.

use std::ops::Range;

use super::DocumentLayout;
use super::grid::GridStep;
use super::rows::Flattener;
use crate::Rect;

impl Flattener<'_> {
	/// The leading space of row `index`.
	pub(super) fn flow_lead(&self, index: usize) -> Option<i64> {
		self.flat.rows.get(index).map(|row| row.lead)
	}
}

#[allow(
	clippy::unused_self,
	reason = "the same methods as the grid-aware ones the `tables` feature builds"
)]
impl DocumentLayout {
	/// No grids to place.
	pub(super) fn resolve_grid(&mut self, _table: u32) -> i64 {
		0
	}

	/// Where row `index` starts: its own top.
	pub(crate) fn flow_top(&self, index: usize) -> i64 {
		self.row_top(index)
	}

	/// Where row `index` ends: its own bottom.
	pub(super) fn flow_bottom(&self, index: usize) -> i64 {
		self.row_top(index) + self.flat.rows.get(index).map_or(0, |row| row.height)
	}

	/// No grid bands.
	pub(super) fn grid_band(&self, _table: u32, _rows: Range<usize>, _inset: i64) -> (i64, i64) {
		(0, 0)
	}

	/// Row `index` itself.
	pub(super) fn grid_hit(&self, index: usize, _x: f32, _y: f32) -> usize {
		index
	}

	/// No grid lines.
	pub(crate) fn grid_lines(&self, _table: u32, _bounds: Rect) -> Vec<Rect> {
		Vec::new()
	}

	/// Document order.
	pub(crate) fn grid_step(&self, _index: usize, _down: bool, _goal_x: f32) -> GridStep {
		GridStep::Flow
	}
}
