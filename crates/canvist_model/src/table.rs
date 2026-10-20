//! Table geometry: where each cell of a table sits on its grid of rows and
//! columns once column and row spans are counted.
//!
//! The model stores a table as rows of cells, the way HTML and ProseMirror
//! do; a cell spanning two rows is a child of its first row only. Layout and
//! the table commands both need the grid that implies, so it is computed
//! here once, from spans alone, without knowing any block type names.

/// One cell's place on a table's grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GridCell {
	/// The table row the cell belongs to, which is its top row.
	pub row: usize,
	/// The cell's index among that row's children.
	pub index: usize,
	/// The leftmost column it covers.
	pub column: usize,
	/// How many rows it covers, at least one.
	pub rowspan: usize,
	/// How many columns it covers, at least one.
	pub colspan: usize,
}

impl GridCell {
	/// The rows it covers.
	#[must_use]
	pub const fn rows(&self) -> std::ops::Range<usize> {
		self.row..self.row + self.rowspan
	}

	/// The columns it covers.
	#[must_use]
	pub const fn columns(&self) -> std::ops::Range<usize> {
		self.column..self.column + self.colspan
	}
}

/// A rectangle of grid slots: rows `top..bottom` and columns `left..right`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GridRect {
	/// The first row.
	pub top: usize,
	/// The first column.
	pub left: usize,
	/// One past the last row.
	pub bottom: usize,
	/// One past the last column.
	pub right: usize,
}

impl GridRect {
	/// The slots `cell` covers.
	#[must_use]
	pub const fn of(cell: &GridCell) -> Self {
		Self {
			top: cell.row,
			left: cell.column,
			bottom: cell.row + cell.rowspan,
			right: cell.column + cell.colspan,
		}
	}

	/// The smallest rectangle covering both.
	#[must_use]
	pub fn union(self, other: Self) -> Self {
		Self {
			top: self.top.min(other.top),
			left: self.left.min(other.left),
			bottom: self.bottom.max(other.bottom),
			right: self.right.max(other.right),
		}
	}

	/// Whether the two share a slot.
	#[must_use]
	pub const fn overlaps(&self, other: &Self) -> bool {
		self.top < other.bottom
			&& other.top < self.bottom
			&& self.left < other.right
			&& other.left < self.right
	}
}

/// The grid a table's rows of cells make.
///
/// Built from each row's cells as `(colspan, rowspan)` pairs, in document
/// order. Cells fill the first free slot of their row, after any slots that
/// cells from rows above span down into. Rows shorter than the widest leave
/// empty slots. A rowspan running past the last row is clamped, as browsers
/// do, and so is a colspan running into a cell spanning down from above, as
/// ProseMirror's `fixTables` does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TableMap {
	width: usize,
	height: usize,
	cells: Vec<GridCell>,
	/// For each row, the index into `cells` of the cell covering each slot.
	slots: Vec<Vec<Option<usize>>>,
}

impl TableMap {
	/// The map of a table whose rows hold cells with these spans.
	#[must_use]
	pub fn new<R, C>(rows: R) -> Self
	where
		R: IntoIterator<Item = C>,
		C: IntoIterator<Item = (usize, usize)>,
	{
		let rows: Vec<Vec<(usize, usize)>> = rows
			.into_iter()
			.map(|cells| cells.into_iter().collect())
			.collect();
		let height = rows.len();
		let mut slots: Vec<Vec<Option<usize>>> = vec![Vec::new(); height];
		let mut cells = Vec::new();

		for (row, spans) in rows.iter().enumerate() {
			let mut column = 0;

			for (index, &(colspan, rowspan)) in spans.iter().enumerate() {
				while slots[row].get(column).is_some_and(Option::is_some) {
					column += 1;
				}

				// A slot spanned down into from above stops the cell short.
				let free = slots[row]
					.get(column..)
					.unwrap_or_default()
					.iter()
					.take_while(|slot| slot.is_none())
					.count();
				let colspan = if column + free < slots[row].len() {
					colspan.clamp(1, free)
				} else {
					colspan.max(1)
				};
				let cell = GridCell {
					row,
					index,
					column,
					rowspan: rowspan.clamp(1, height - row),
					colspan,
				};

				for covered in cell.rows() {
					let line = &mut slots[covered];

					if line.len() < cell.column + cell.colspan {
						line.resize(cell.column + cell.colspan, None);
					}

					for slot in &mut line[cell.columns()] {
						*slot = Some(cells.len());
					}
				}

				column += cell.colspan;
				cells.push(cell);
			}
		}

		let width = slots.iter().map(Vec::len).max().unwrap_or(0);

		for line in &mut slots {
			line.resize(width, None);
		}

		Self {
			width,
			height,
			cells,
			slots,
		}
	}

	/// How many columns the widest row covers.
	#[must_use]
	pub const fn width(&self) -> usize {
		self.width
	}

	/// How many rows the table has.
	#[must_use]
	pub const fn height(&self) -> usize {
		self.height
	}

	/// Every cell, in document order.
	#[must_use]
	pub fn cells(&self) -> &[GridCell] {
		&self.cells
	}

	/// The index into [`Self::cells`] of the cell covering a slot, or `None`
	/// for an empty slot or one outside the grid.
	#[must_use]
	pub fn cell_at(&self, row: usize, column: usize) -> Option<usize> {
		self.slots.get(row)?.get(column).copied().flatten()
	}

	/// The index into [`Self::cells`] of the cell that is child `index` of
	/// row `row`.
	#[must_use]
	pub fn find(&self, row: usize, index: usize) -> Option<usize> {
		let start = self.cells.partition_point(|cell| cell.row < row);
		self.cells[start..]
			.iter()
			.position(|cell| cell.row == row && cell.index == index)
			.map(|offset| start + offset)
	}

	/// Where among row `row`'s children a new cell starting at `column`
	/// belongs: after every cell of that row starting to its left.
	#[must_use]
	pub fn insert_index(&self, row: usize, column: usize) -> usize {
		self.cells
			.iter()
			.filter(|cell| cell.row == row && cell.column < column)
			.count()
	}

	/// The cells overlapping `rect`, in document order.
	#[must_use]
	pub fn cells_in(&self, rect: GridRect) -> Vec<usize> {
		(0..self.cells.len())
			.filter(|&index| GridRect::of(&self.cells[index]).overlaps(&rect))
			.collect()
	}

	/// `rect` grown until no cell sticks out of it, as a selection of cells
	/// grows to whole merged cells.
	#[must_use]
	pub fn expand(&self, rect: GridRect) -> GridRect {
		let mut rect = rect;

		loop {
			let grown = self
				.cells_in(rect)
				.into_iter()
				.map(|index| GridRect::of(&self.cells[index]))
				.fold(rect, GridRect::union);

			if grown == rect {
				return rect;
			}

			rect = grown;
		}
	}

	/// Whether every row covers every column, with no empty slots.
	#[must_use]
	pub fn is_full(&self) -> bool {
		self.slots.iter().flatten().all(Option::is_some)
	}

	/// How many slots each row is missing at its end.
	#[must_use]
	pub fn missing(&self) -> Vec<usize> {
		self.slots
			.iter()
			.map(|line| line.iter().rev().take_while(|slot| slot.is_none()).count())
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn map(rows: &[&[(usize, usize)]]) -> TableMap {
		TableMap::new(rows.iter().map(|row| row.iter().copied()))
	}

	#[test]
	fn plain_cells_fill_a_grid() {
		let table = map(&[&[(1, 1), (1, 1)], &[(1, 1), (1, 1)]]);

		assert_eq!((table.width(), table.height()), (2, 2));
		assert_eq!(table.cell_at(1, 1), Some(3));
		assert!(table.is_full());
	}

	#[test]
	fn rowspans_push_later_cells_right() {
		// | a | b |
		// | a | c |
		let table = map(&[&[(1, 2), (1, 1)], &[(1, 1)]]);

		assert_eq!(table.cell_at(1, 0), Some(0));
		assert_eq!(table.cells()[2].column, 1);
		assert_eq!(table.insert_index(1, 1), 0);
		assert!(table.is_full());
	}

	#[test]
	fn short_rows_leave_empty_slots_and_long_rowspans_are_clamped() {
		let table = map(&[&[(2, 5)], &[]]);

		assert_eq!(table.cells()[0].rowspan, 2);
		assert!(table.is_full());

		let ragged = map(&[&[(1, 1), (1, 1), (1, 1)], &[(1, 1)]]);
		assert_eq!(ragged.missing(), vec![0, 2]);
		assert!(!ragged.is_full());
	}

	#[test]
	fn expand_grows_to_whole_merged_cells() {
		// | a a | b |
		// | c | d | e |
		let table = map(&[&[(2, 1), (1, 1)], &[(1, 1), (1, 1), (1, 1)]]);
		let rect = GridRect {
			top: 0,
			left: 1,
			bottom: 2,
			right: 2,
		};

		assert_eq!(
			table.expand(rect),
			GridRect {
				top: 0,
				left: 0,
				bottom: 2,
				right: 2
			}
		);
		assert_eq!(table.find(1, 2), Some(4));
	}
}
