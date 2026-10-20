//! Tables in the flattened document: placing each cell's content in the
//! columns it spans, and the geometry that depends on the grid, from row
//! positions and container boxes to hit testing, vertical motion, and the
//! grid lines.

use canvist_model::Block;
use canvist_model::TableMap;

use super::DocumentLayout;
use super::RowKind;
use super::grid::CellBox;
use super::grid::Grid;
use super::grid::GridStep;
use super::grid::column_widths;
use super::grid::table_cells;
use super::grid::table_map;
use super::rows::Flat;
use super::rows::Flattener;
use super::rows::Frame;
use super::rows::GridPart;
use super::rows::Placement;
use crate::Rect;
use crate::geometry::to_px;
use crate::geometry::to_units;
use crate::style::StyleId;

/// What placing one table's cells needs.
struct TableFrame<'t> {
	/// The table's span.
	span: u32,
	/// The table's content box.
	inner: Frame,
	/// Column edges at line centres: one more than there are columns.
	edges: Vec<f32>,
	/// The grid line width.
	rule: f32,
	map: &'t TableMap,
	/// The table's first layout row.
	first_row: usize,
}

/// A table row being placed: its block, style, span, and index.
#[derive(Clone, Copy)]
struct RowFrame<'b> {
	block: &'b Block,
	style: StyleId,
	span: u32,
	index: usize,
}

impl Flattener<'_> {
	/// The leading space of row `index` in the flow of the blocks around it:
	/// its own, or for the first row of a table, the table's.
	pub(super) fn flow_lead(&self, index: usize) -> Option<i64> {
		let row = self.flat.rows.get(index)?;
		let table = row
			.table
			.and_then(|table| self.flat.spans.get(table as usize));

		Some(match table.map(|span| (span.first_row, &span.grid)) {
			Some((first, Some(GridPart::Table(grid)))) if first as usize == index => grid.lead,
			_ => row.lead,
		})
	}

	/// Flatten a table: its rows and cells become spans, each cell's content
	/// becomes rows placed in the cell's columns, and the grid is solved so
	/// every row knows its top. `rule` is the grid line width. A table with
	/// no cells is an ordinary container.
	pub(super) fn table(&mut self, block: &Block, placement: Placement, rule: f32) {
		let doc = self.doc;
		let cells = table_cells(doc, block);
		let (map, hints) = table_map(doc, &cells);

		if map.cells().is_empty() {
			self.container(block, placement);
			return;
		}

		let Placement {
			style,
			frame,
			marker,
			..
		} = placement;
		let resolved = self.styles.get(style);
		let (inset_left, inset_right) = (resolved.inset_left(), resolved.inset_right());
		let (padding_top, padding_bottom) = (resolved.padding.top, resolved.padding.bottom);
		let span = self.push_span(block, style, frame, marker);
		let inner = Frame {
			x: frame.x + inset_left,
			width: (frame.width - inset_left - inset_right).max(0.0),
			span: Some(span),
			lists: frame.lists,
		};
		let widths = column_widths(&map, &hints, (inner.width - rule).max(0.0));
		let table = TableFrame {
			span,
			inner,
			edges: column_edges(&widths, inner.x + rule / 2.0),
			rule,
			map: &map,
			first_row: self.flat.rows.len(),
		};

		self.pending += to_units(padding_top);
		let mut grid = Grid {
			lead: self.pending,
			rule: to_units(rule),
			lines: vec![0; map.height() + 1],
			..Grid::default()
		};
		self.in_table = true;

		for (index, (row_block, row_cells)) in cells.iter().enumerate() {
			let style = self
				.styles
				.resolve(row_block, Some(block.kind()), Some(style));
			let row = RowFrame {
				block: row_block,
				style,
				span: self.push_span(row_block, style, inner, None),
				index,
			};
			self.table_row(&table, &mut grid, row, row_cells);
		}

		self.in_table = false;
		self.pending = to_units(padding_bottom);
		self.finish_table(&table, grid, to_units(padding_top));
	}

	/// Place the cells of one table row.
	fn table_row(
		&mut self,
		table: &TableFrame<'_>,
		grid: &mut Grid,
		row: RowFrame<'_>,
		cells: &[&Block],
	) {
		self.flat.spans[row.span as usize].grid = Some(GridPart::Row {
			table: table.span,
			row: row.index as u32,
		});

		for (child, cell) in cells.iter().enumerate() {
			self.table_cell(table, grid, row, (child, cell));
		}

		let last = (self.flat.rows.len() as u32).saturating_sub(1);
		let span = &mut self.flat.spans[row.span as usize];
		span.last_row = last.max(span.first_row);
	}

	/// Flatten cell `child` of `row` into the columns it spans, starting its
	/// content afresh with no space owed from the cell before it.
	fn table_cell(
		&mut self,
		table: &TableFrame<'_>,
		grid: &mut Grid,
		row: RowFrame<'_>,
		(child, cell): (usize, &Block),
	) {
		let Some(placed) = table
			.map
			.find(row.index, child)
			.map(|found| table.map.cells()[found])
		else {
			return;
		};
		let (left, right) = (
			table.edges[placed.column],
			table.edges[placed.column + placed.colspan],
		);
		let style = self
			.styles
			.resolve(cell, Some(row.block.kind()), Some(row.style));
		let frame = Frame {
			x: left + table.rule / 2.0,
			width: (right - left - table.rule).max(0.0),
			span: Some(row.span),
			lists: table.inner.lists,
		};
		let (rows_before, spans_before) = (self.flat.rows.len(), self.flat.spans.len());
		self.pending = 0;
		self.block(
			cell,
			Placement {
				style,
				parent_kind: Some(row.block.kind()),
				parent_style: Some(row.style),
				frame,
				marker: None,
			},
		);

		let index = grid.cells.len() as u32;

		if let Some(span) = self.flat.spans.get_mut(spans_before)
			&& span.block == cell.id()
		{
			span.grid = Some(GridPart::Cell {
				table: table.span,
				cell: index,
			});
		}

		let relative = |row: usize| (row - table.first_row) as u32;
		grid.cells.push(CellBox {
			rows: relative(rows_before)..relative(self.flat.rows.len()),
			bottom: self.pending,
			row: row.index as u32,
			rowspan: placed.rowspan as u32,
			left,
			right,
		});
	}

	/// Mark the table's rows as placed by its grid, solve it, and close the
	/// table's span.
	fn finish_table(&mut self, table: &TableFrame<'_>, mut grid: Grid, top: i64) {
		let (first, end) = (table.first_row, self.flat.rows.len());

		for row in &mut self.flat.rows[first..end] {
			row.table.get_or_insert(table.span);
		}

		grid.solve(&mut self.flat.rows[first..end]);

		let span = &mut self.flat.spans[table.span as usize];
		span.top = top;
		span.last_row = (end as u32).saturating_sub(1).max(first as u32);
		span.bottom = self.pending;
		span.grid = Some(GridPart::Table(Box::new(grid)));
	}
}

/// The edges of columns `widths` wide, starting at `left`.
fn column_edges(widths: &[f32], left: f32) -> Vec<f32> {
	std::iter::once(left)
		.chain(widths.iter().scan(left, |edge, width| {
			*edge += width;
			Some(*edge)
		}))
		.collect()
}

/// How far `x` lies outside `cell`'s left and right edges.
fn x_distance(cell: &CellBox, x: f32) -> f32 {
	(cell.left - x).max(x - cell.right).max(0.0)
}

impl DocumentLayout {
	/// Place the rows of table span `table` again after some changed height,
	/// and update the extents its first and last rows carry. Returns the top
	/// of its grid, below which rows may have moved.
	pub(super) fn resolve_grid(&mut self, table: u32) -> i64 {
		let Flat { rows, spans, .. } = &mut self.flat;
		let Some(span) = spans.get_mut(table as usize) else {
			return 0;
		};
		let (first, last) = (span.first_row as usize, span.last_row as usize);

		if let Some(GridPart::Table(grid)) = &mut span.grid {
			grid.solve(&mut rows[first..=last]);
		}

		self.heights.set(first, self.flat.extent(first));
		self.heights.set(last, self.flat.extent(last));
		self.flow_top(first)
	}

	/// The top of the grid of table span `table`, and the grid.
	fn grid_origin(&self, table: Option<u32>) -> Option<(i64, &Grid)> {
		let (first, grid) = self.flat.grid(table)?;
		Some((self.heights.prefix(first) + grid.lead, grid))
	}

	/// Where row `index` starts in the flow of blocks: its own top, or the
	/// top of the grid placing it.
	pub(crate) fn flow_top(&self, index: usize) -> i64 {
		let table = self.flat.rows.get(index).and_then(|row| row.table);

		self.grid_origin(table)
			.map_or_else(|| self.row_top(index), |(origin, _)| origin)
	}

	/// Where row `index` ends in the flow of blocks: its own bottom, or the
	/// bottom of the grid placing it.
	pub(super) fn flow_bottom(&self, index: usize) -> i64 {
		let Some(row) = self.flat.rows.get(index) else {
			return self.row_top(index);
		};

		self.grid_origin(row.table).map_or_else(
			|| self.row_top(index) + row.height,
			|(origin, grid)| origin + grid.height,
		)
	}

	/// The top and bottom of table rows `rows` of table span `table`, moved
	/// `inset` inside the lines around them.
	pub(super) fn grid_band(
		&self,
		table: u32,
		rows: std::ops::Range<usize>,
		inset: i64,
	) -> (i64, i64) {
		let Some((origin, grid)) = self.grid_origin(Some(table)) else {
			return (0, 0);
		};
		let (top, bottom) = grid.band(rows);

		(origin + top + inset, origin + bottom - inset)
	}

	/// The row nearest `(x, y)` among those placed by the grid that row
	/// `index` belongs to: in the cell under the point, or the nearest cell,
	/// the row whose box holds `y`, or the nearest. Rows outside a grid are
	/// returned as they are.
	pub(super) fn grid_hit(&self, index: usize, x: f32, y: f32) -> usize {
		let table = self.rows()[index].table;
		let Some((origin, grid)) = self.grid_origin(table) else {
			return index;
		};
		let first = self.flat.grid(table).map_or(index, |(first, _)| first);
		let y = to_units(y) - origin;
		let distance = |cell: &CellBox| {
			let (top, bottom) = grid.band(cell.spanned());
			x_distance(cell, x) + to_px((top - y).max(y - bottom).max(0))
		};
		let Some(cell) = grid
			.cells
			.iter()
			.min_by(|a, b| distance(a).total_cmp(&distance(b)))
		else {
			return index;
		};

		cell.range()
			.map(|offset| first + offset)
			.min_by_key(|&row| {
				let top = self.rows()[row].grid_y;
				let bottom = top + self.rows()[row].height;
				(top - y).max(y - bottom).max(0)
			})
			.unwrap_or(index)
	}

	/// The grid lines of table span `table` that fall inside `bounds`: the
	/// four edges of every cell, centred on the edge, so shared edges line up
	/// as CSS's collapsed borders do.
	pub(crate) fn grid_lines(&self, table: u32, bounds: Rect) -> Vec<Rect> {
		let Some((origin, grid)) = self.grid_origin(Some(table)) else {
			return Vec::new();
		};
		let width = to_px(grid.rule);
		let half = width / 2.0;

		if width <= 0.0 {
			return Vec::new();
		}

		let visible = |&(top, bottom): &(f32, f32)| {
			bottom + half >= bounds.y && top - half <= bounds.y + bounds.height
		};

		grid.cells
			.iter()
			.map(|cell| {
				let (top, bottom) = grid.band(cell.spanned());
				(cell, (to_px(origin + top), to_px(origin + bottom)))
			})
			.filter(|(_, band)| visible(band))
			.flat_map(|(cell, (top, bottom))| {
				let across = cell.right - cell.left + width;
				let down = bottom - top + width;
				[
					Rect::new(cell.left - half, top - half, across, width),
					Rect::new(cell.left - half, bottom - half, across, width),
					Rect::new(cell.left - half, top - half, width, down),
					Rect::new(cell.right - half, top - half, width, down),
				]
			})
			.collect()
	}

	/// The text row a vertical move from row `index` lands on when `index`
	/// is placed by a grid: the next text row of its cell, or the first (or
	/// last, moving up) text row of the cell above or below in the column of
	/// `goal_x`, or the nearest text row outside the table.
	pub(crate) fn grid_step(&self, index: usize, down: bool, goal_x: f32) -> GridStep {
		let rows = self.rows();
		let Some((first, grid)) = self.flat.grid(rows[index].table) else {
			return GridStep::Flow;
		};
		let Some(cell) = grid.cell_of((index - first) as u32) else {
			return GridStep::Flow;
		};
		let mut texts = self.text_rows(first, cell);
		let within = if down {
			texts.find(|&row| row > index)
		} else {
			texts.rev().find(|&row| row < index)
		};

		if within.is_some() {
			return GridStep::Row(within);
		}

		let bands = grid.lines.len() - 1;
		let entry = if down {
			(cell.spanned().end..bands)
				.find_map(|band| self.band_entry(first, grid, band, true, goal_x))
		} else {
			(0..cell.row as usize)
				.rev()
				.find_map(|band| self.band_entry(first, grid, band, false, goal_x))
		};

		GridStep::Row(entry.or_else(|| self.outside_grid(first, grid, down)))
	}

	/// The text rows of `cell`, as document row indices.
	fn text_rows<'s>(
		&'s self,
		first: usize,
		cell: &CellBox,
	) -> impl DoubleEndedIterator<Item = usize> + 's {
		cell.range()
			.map(move |offset| first + offset)
			.filter(|&row| self.rows()[row].kind == RowKind::Text)
	}

	/// The text row a vertical move enters table row `band` at: the first
	/// (or, moving up, last) of the cell spanning it nearest `goal_x`.
	fn band_entry(
		&self,
		first: usize,
		grid: &Grid,
		band: usize,
		down: bool,
		goal_x: f32,
	) -> Option<usize> {
		let cell = grid
			.cells
			.iter()
			.filter(|cell| cell.spanned().contains(&band))
			.min_by(|a, b| x_distance(a, goal_x).total_cmp(&x_distance(b, goal_x)))?;
		let mut texts = self.text_rows(first, cell);

		if down {
			texts.next()
		} else {
			texts.next_back()
		}
	}

	/// The nearest text row below (or above) the table whose rows start at
	/// `first`.
	fn outside_grid(&self, first: usize, grid: &Grid, down: bool) -> Option<usize> {
		let rows = self.rows();
		let is_text = |row: &usize| rows[*row].kind == RowKind::Text;

		if down {
			let last = first + grid.cells.last().map_or(0, |cell| cell.range().end);
			(last..rows.len()).find(is_text)
		} else {
			(0..first).rev().find(is_text)
		}
	}
}
