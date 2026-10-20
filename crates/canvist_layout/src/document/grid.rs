//! Table grids: cells side by side.
//!
//! A table's cells flatten into ordinary rows, cell by cell, so measuring,
//! painting, and caret geometry treat their text like any other. Only the
//! rows' positions differ: each one's top comes from the grid rather than
//! from the rows before it. In the row extents the table's first row carries
//! the space above the grid and its last row the grid's height, and the rows
//! between carry nothing, so prefix sums stay monotonic and the document
//! height counts the grid once.
//!
//! Grid lines follow CSS's collapsed borders: a line of width `w` is centred
//! on each cell edge, so a cell's content box sits half a line inside its
//! edges, and the whole grid is `w` wider and taller than its columns and
//! rows. The HTML renderer writes the same table with `border-collapse`.

use std::ops::Range;

use canvist_model::Block;
use canvist_model::Document;
#[cfg(feature = "tables")]
use canvist_model::TableMap;
use canvist_model::Value;

#[cfg(feature = "tables")]
use super::rows::Row;
use crate::style::attr;

#[cfg(feature = "tables")]
/// The fewest pixels a column without a `colwidth` is given before every
/// column shrinks to fit.
pub(crate) const MIN_COLUMN_WIDTH: f32 = 24.0;

/// A laid-out table.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Grid {
	/// Space from the bottom of whatever precedes the table down to the top
	/// of its grid, including the table's own top padding.
	pub(crate) lead: i64,
	/// The grid's height, outer lines included.
	pub(crate) height: i64,
	/// The grid line width.
	pub(crate) rule: i64,
	/// Each table row's top line, relative to the grid's top, plus one more
	/// for the bottom line: `rows + 1` entries, at line centres.
	pub(crate) lines: Vec<i64>,
	/// The cells, in document order.
	pub(crate) cells: Vec<CellBox>,
}

/// One cell's place in its grid.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CellBox {
	/// The layout rows it holds, relative to the table's first row.
	pub(crate) rows: Range<u32>,
	/// The space owed after its last row: its bottom padding.
	pub(crate) bottom: i64,
	/// The table row it starts in, and how many it spans.
	pub(crate) row: u32,
	pub(crate) rowspan: u32,
	/// Its left and right edges, at line centres.
	pub(crate) left: f32,
	pub(crate) right: f32,
}

#[cfg_attr(
	not(feature = "tables"),
	allow(dead_code, reason = "only the `tables` feature builds grids")
)]
impl Grid {
	#[cfg(feature = "tables")]
	/// Place every row of the table: `rows` are the table's layout rows, the
	/// first being its first. Cells' content stacks from half a line below
	/// their top edge; each table row is as tall as its tallest single-row
	/// cell, and a cell spanning rows grows the last of them if it needs to.
	pub(crate) fn solve(&mut self, rows: &mut [Row]) {
		let heights = self.row_heights(rows);
		let half = self.rule / 2;
		let mut line = half;

		for (slot, height) in self.lines.iter_mut().zip(heights.iter().chain([&0])) {
			*slot = line;
			line += height;
		}

		self.height = self.lines.last().map_or(0, |last| last + half);

		for cell in &self.cells {
			let mut y = self.lines[cell.row as usize] + half;

			for row in &mut rows[cell.range()] {
				y += row.lead;
				row.grid_y = y;
				y += row.height;
			}
		}
	}

	#[cfg(feature = "tables")]
	/// How tall each table row is, from line centre to line centre.
	fn row_heights(&self, rows: &[Row]) -> Vec<i64> {
		let mut heights = vec![0; self.lines.len().saturating_sub(1)];
		let mut spanning = Vec::new();

		for cell in &self.cells {
			let stacked: i64 = rows[cell.range()]
				.iter()
				.map(|row| row.lead + row.height)
				.sum();
			let height = stacked + cell.bottom + self.rule;

			if cell.rowspan == 1 {
				let slot = &mut heights[cell.row as usize];
				*slot = (*slot).max(height);
			} else {
				spanning.push((cell.spanned(), height));
			}
		}

		// Spanning cells grow their last row once the rows above are known,
		// shortest reach first, as browsers distribute them.
		spanning.sort_by_key(|(spanned, _)| spanned.end);

		for (spanned, height) in spanning {
			let have: i64 = heights[spanned.clone()].iter().sum();
			heights[spanned.end - 1] += (height - have).max(0);
		}

		heights
	}

	#[cfg(feature = "tables")]
	/// The cell holding layout row `row`, relative to the table's first row.
	pub(crate) fn cell_of(&self, row: u32) -> Option<&CellBox> {
		let index = self.cells.partition_point(|cell| cell.rows.end <= row);
		self.cells
			.get(index)
			.filter(|cell| cell.rows.contains(&row))
	}

	/// The top and bottom of table rows `rows`, between their line centres.
	pub(crate) fn band(&self, rows: Range<usize>) -> (i64, i64) {
		let top = self.lines.get(rows.start).copied().unwrap_or(0);
		let bottom = self.lines.get(rows.end).copied().unwrap_or(top);
		(top, bottom)
	}
}

#[cfg_attr(
	not(feature = "tables"),
	allow(dead_code, reason = "only the `tables` feature builds grids")
)]
impl CellBox {
	/// Its layout rows as a slice range.
	pub(crate) fn range(&self) -> Range<usize> {
		self.rows.start as usize..self.rows.end as usize
	}

	/// The table rows it spans.
	pub(crate) fn spanned(&self) -> Range<usize> {
		self.row as usize..(self.row + self.rowspan) as usize
	}
}

#[cfg(feature = "tables")]
/// A table's rows, each with its cells, in document order.
pub(crate) fn table_cells<'d>(
	doc: &'d Document,
	table: &Block,
) -> Vec<(&'d Block, Vec<&'d Block>)> {
	let blocks = |parent: &Block| -> Vec<&'d Block> {
		doc.children(Some(parent.id()))
			.iter()
			.filter_map(|&id| doc.block(id))
			.collect()
	};

	blocks(table)
		.into_iter()
		.map(|row| (row, blocks(row)))
		.collect()
}

#[cfg(feature = "tables")]
/// The grid `cells` make, and each cell's `colwidth` in the same order.
pub(crate) fn table_map(
	doc: &Document,
	cells: &[(&Block, Vec<&Block>)],
) -> (TableMap, Vec<Option<Vec<f32>>>) {
	let map = TableMap::new(cells.iter().map(|(_, row)| {
		row.iter().map(|cell| {
			(
				span_attr(doc, cell, "colspan"),
				span_attr(doc, cell, "rowspan"),
			)
		})
	}));
	let hints = cells
		.iter()
		.flat_map(|(_, row)| row.iter().map(|cell| colwidth(cell)))
		.collect();

	(map, hints)
}

/// A cell's `colspan` or `rowspan`, at least one.
pub(crate) fn span_attr(doc: &Document, cell: &Block, key: &str) -> usize {
	let defaults = &doc.schema().block(cell.kind()).attrs;

	attr(cell.attrs(), defaults, key)
		.and_then(Value::as_int)
		.and_then(|value| usize::try_from(value).ok())
		.unwrap_or(1)
		.max(1)
}

#[cfg(feature = "tables")]
/// A cell's `colwidth`: the pixel width it gives each column it spans, as a
/// list of numbers, or `None` when it has none.
fn colwidth(cell: &Block) -> Option<Vec<f32>> {
	let Some(Value::Array(widths)) = cell.attrs().get("colwidth") else {
		return None;
	};

	Some(
		widths
			.iter()
			.map(|width| {
				match width {
					Value::Int(px) => *px as f32,
					Value::Float(px) => *px as f32,
					_ => 0.0,
				}
			})
			.collect(),
	)
}

#[cfg(feature = "tables")]
/// The width of each column of `map`, filling `width` where it can.
///
/// `hints` gives each cell's `colwidth`, one entry per column it spans, as
/// TipTap stores them; the first cell with a positive hint for a column
/// decides its width. Columns without one share what is left evenly. When
/// that leaves them less than [`MIN_COLUMN_WIDTH`], or the hinted widths
/// alone are too wide, every column scales to fit. Hinted columns that fall
/// short of `width` leave the table narrower, as a resized TipTap table is.
pub(crate) fn column_widths(map: &TableMap, hints: &[Option<Vec<f32>>], width: f32) -> Vec<f32> {
	let columns = hinted_columns(map, hints);
	let fixed: f32 = columns.iter().flatten().sum();
	let free = columns.iter().filter(|column| column.is_none()).count();
	let share = if free == 0 {
		0.0
	} else {
		(width - fixed) / free as f32
	};

	if free > 0 && share >= MIN_COLUMN_WIDTH {
		return columns
			.iter()
			.map(|column| column.unwrap_or(share))
			.collect();
	}

	let nominal: Vec<f32> = columns
		.iter()
		.map(|column| column.unwrap_or(MIN_COLUMN_WIDTH))
		.collect();
	let total: f32 = nominal.iter().sum();
	let scale = match free {
		0 if total <= width => 1.0,
		_ if total > 0.0 => width / total,
		_ => 0.0,
	};

	nominal.iter().map(|column| column * scale).collect()
}

#[cfg(feature = "tables")]
/// Each column's width from the first cell hinting a positive one.
fn hinted_columns(map: &TableMap, hints: &[Option<Vec<f32>>]) -> Vec<Option<f32>> {
	let mut columns: Vec<Option<f32>> = vec![None; map.width()];
	let hinted = map
		.cells()
		.iter()
		.zip(hints)
		.filter_map(|(cell, hint)| Some(cell.columns().zip(hint.as_ref()?.iter().copied())));

	for (column, px) in hinted.flatten() {
		if px > 0.0 && px.is_finite() {
			columns[column].get_or_insert(px);
		}
	}

	columns
}

/// Where a vertical move from a row lands.
#[cfg_attr(
	not(feature = "tables"),
	allow(dead_code, reason = "only the `tables` feature builds grids")
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GridStep {
	/// The row is not in a grid, so the move follows document order.
	Flow,
	/// The text row it lands on, or `None` at the document's edge.
	Row(Option<usize>),
}

#[cfg(all(test, feature = "tables"))]
#[allow(
	clippy::float_cmp,
	reason = "the widths compared are exactly representable"
)]
mod tests {
	use super::*;

	fn map(rows: &[&[(usize, usize)]]) -> TableMap {
		TableMap::new(rows.iter().map(|row| row.iter().copied()))
	}

	#[test]
	fn free_columns_share_the_width_left_by_hinted_ones() {
		let table = map(&[&[(1, 1), (1, 1), (1, 1)]]);
		let hints = [Some(vec![100.0]), None, None];

		assert_eq!(
			column_widths(&table, &hints, 500.0),
			vec![100.0, 200.0, 200.0]
		);
	}

	#[test]
	fn a_spanning_cell_hints_every_column_it_covers() {
		let table = map(&[&[(2, 1)], &[(1, 1), (1, 1)]]);
		let hints = [Some(vec![120.0, 80.0]), None, None];

		assert_eq!(column_widths(&table, &hints, 400.0), vec![120.0, 80.0]);
	}

	#[test]
	fn too_wide_hints_scale_down_to_fit() {
		let table = map(&[&[(1, 1), (1, 1)]]);
		let hints = [Some(vec![300.0]), Some(vec![500.0])];

		assert_eq!(column_widths(&table, &hints, 400.0), vec![150.0, 250.0]);
	}

	#[test]
	fn squeezed_free_columns_keep_a_minimum_share() {
		let table = map(&[&[(1, 1), (1, 1)]]);
		let hints = [Some(vec![390.0]), None];
		let widths = column_widths(&table, &hints, 400.0);

		assert!(widths[1] > 20.0, "{widths:?}");
		assert!((widths.iter().sum::<f32>() - 400.0).abs() < 0.01);
	}
}
