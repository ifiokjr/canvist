//! Property tests for `TableMap` and `GridRect`, the grid table commands
//! and layout read cells from: rectangles, expanding a selection to whole
//! merged cells, insert positions, and the short rows that `fixTables`
//! pads.

use canvist_model::GridRect;
use canvist_model::TableMap;
use proptest::prelude::*;

/// A row whose last cell sits left of a cell spanning down from above
/// misses the slot between them, not only slots at its end.
#[test]
fn a_hole_before_a_spanning_cell_is_missing() {
	let table = map(&[vec![(1, 1), (1, 1), (1, 2)], vec![(1, 1)]]);

	assert_eq!(table.missing(), vec![0, 1]);
	assert!(!table.is_full());
}

/// Rows of `(colspan, rowspan)` cells, some ragged, some spanning past the
/// table's edge.
fn spans() -> impl Strategy<Value = Vec<Vec<(usize, usize)>>> {
	prop::collection::vec(prop::collection::vec((1..4usize, 1..4usize), 0..5), 1..6)
}

fn map(rows: &[Vec<(usize, usize)>]) -> TableMap {
	TableMap::new(rows.iter().map(|row| row.iter().copied()))
}

/// A rectangle inside a `width` by `height` grid, from seeds.
fn rect_in(width: usize, height: usize, seeds: (usize, usize, usize, usize)) -> GridRect {
	let (a, b, c, d) = seeds;
	let (top, bottom) = (a % height, b % height);
	let (left, right) = (c % width, d % width);

	GridRect {
		top: top.min(bottom),
		left: left.min(right),
		bottom: top.max(bottom) + 1,
		right: left.max(right) + 1,
	}
}

/// Every `(start, end)` with `start <= from` and `to <= end <= limit`.
fn spans_of(from: usize, to: usize, limit: usize) -> impl Iterator<Item = (usize, usize)> {
	(0..=from).flat_map(move |start| (to..=limit).map(move |end| (start, end)))
}

fn contains(outer: GridRect, inner: GridRect) -> bool {
	outer.top <= inner.top
		&& outer.left <= inner.left
		&& inner.bottom <= outer.bottom
		&& inner.right <= outer.right
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(128))]

	#[test]
	fn cells_stay_inside_the_grid_and_rowspans_are_clamped(rows in spans()) {
		let map = map(&rows);

		prop_assert_eq!(map.height(), rows.len());

		for cell in map.cells() {
			prop_assert!(cell.rowspan >= 1 && cell.colspan >= 1);
			prop_assert!(cell.row + cell.rowspan <= map.height(), "{:?}", cell);
			prop_assert!(cell.column + cell.colspan <= map.width(), "{:?}", cell);
			prop_assert_eq!(GridRect::of(cell).bottom - GridRect::of(cell).top, cell.rowspan);
		}

		let widest = map.cells().iter().map(|cell| cell.column + cell.colspan).max().unwrap_or(0);
		prop_assert_eq!(map.width(), widest, "the grid is as wide as its widest row");
	}

	#[test]
	fn expanding_a_selection_gives_the_smallest_rectangle_of_whole_cells(
		rows in spans(),
		seeds in (any::<usize>(), any::<usize>(), any::<usize>(), any::<usize>()),
	) {
		let map = map(&rows);
		prop_assume!(map.width() > 0);
		let rect = rect_in(map.width(), map.height(), seeds);
		let expanded = map.expand(rect);

		prop_assert!(contains(expanded, rect), "{:?} holds {:?}", expanded, rect);
		prop_assert_eq!(map.expand(expanded), expanded, "expanding is idempotent");

		for index in map.cells_in(expanded) {
			let cell = GridRect::of(&map.cells()[index]);
			prop_assert!(contains(expanded, cell), "{:?} is cut by {:?}", cell, expanded);
		}

		// Every rectangle that holds the selection and cuts no cell holds the
		// expansion too, so no smaller one would do.
		for (top, bottom) in spans_of(rect.top, rect.bottom, map.height()) {
			for (left, right) in spans_of(rect.left, rect.right, map.width()) {
				let candidate = GridRect { top, left, bottom, right };
				let whole = map
					.cells_in(candidate)
					.into_iter()
					.all(|index| contains(candidate, GridRect::of(&map.cells()[index])));

				if whole {
					prop_assert!(contains(candidate, expanded), "{:?} is smaller than {:?}", candidate, expanded);
				}
			}
		}
	}

	#[test]
	fn cells_in_a_rectangle_are_exactly_those_overlapping_it(
		rows in spans(),
		seeds in (any::<usize>(), any::<usize>(), any::<usize>(), any::<usize>()),
	) {
		let map = map(&rows);
		prop_assume!(map.width() > 0);
		let rect = rect_in(map.width(), map.height(), seeds);
		let found = map.cells_in(rect);
		let slots: std::collections::BTreeSet<usize> = (rect.top..rect.bottom)
			.flat_map(|row| (rect.left..rect.right).map(move |column| (row, column)))
			.filter_map(|(row, column)| map.cell_at(row, column))
			.collect();

		prop_assert_eq!(found.iter().copied().collect::<std::collections::BTreeSet<_>>(), slots);
		prop_assert!(found.windows(2).all(|pair| pair[0] < pair[1]), "in document order");
	}

	#[test]
	fn insert_indexes_count_the_row_cells_left_of_a_column(rows in spans(), column in 0..8usize) {
		let map = map(&rows);

		for (row, cells) in rows.iter().enumerate() {
			let index = map.insert_index(row, column);
			prop_assert!(index <= cells.len());

			for cell in map.cells().iter().filter(|cell| cell.row == row) {
				prop_assert_eq!(cell.index < index, cell.column < column, "{:?}", cell);
			}
		}
	}

	#[test]
	fn appending_the_missing_cells_fills_every_row(rows in spans()) {
		let map = map(&rows);
		let missing = map.missing();

		prop_assert_eq!(missing.len(), map.height());
		prop_assert_eq!(map.is_full(), missing.iter().all(|&count| count == 0));

		for (row, &count) in missing.iter().enumerate() {
			let empty = (0..map.width()).filter(|&column| map.cell_at(row, column).is_none()).count();
			prop_assert_eq!(count, empty, "row {}", row);
		}

		// What fixTables does: append that many plain cells to each row.
		let padded: Vec<Vec<(usize, usize)>> = rows
			.iter()
			.zip(&missing)
			.map(|(cells, &count)| cells.iter().copied().chain(std::iter::repeat_n((1, 1), count)).collect())
			.collect();
		let padded = self::map(&padded);
		prop_assert!(padded.is_full(), "{:?} → {:?}", rows, padded.missing());
		prop_assert_eq!((padded.width(), padded.height()), (map.width(), map.height()));
	}

	#[test]
	fn a_full_grid_of_plain_cells_is_full_and_rectangular(height in 1..6usize, width in 1..6usize) {
		let rows = vec![vec![(1, 1); width]; height];
		let map = map(&rows);

		prop_assert!(map.is_full());
		prop_assert_eq!((map.width(), map.height()), (width, height));
		prop_assert_eq!(map.cells().len(), width * height);
	}

	#[test]
	fn union_is_the_smallest_rectangle_holding_both(
		a in (0..6usize, 0..6usize, 1..4usize, 1..4usize),
		b in (0..6usize, 0..6usize, 1..4usize, 1..4usize),
	) {
		let rect = |(top, left, rows, columns): (usize, usize, usize, usize)| GridRect {
			top,
			left,
			bottom: top + rows,
			right: left + columns,
		};
		let (a, b) = (rect(a), rect(b));
		let union = a.union(b);

		prop_assert_eq!(union, b.union(a));
		prop_assert!(contains(union, a) && contains(union, b));
		prop_assert_eq!(union.top, a.top.min(b.top));
		prop_assert_eq!(union.right, a.right.max(b.right));
		prop_assert_eq!(a.overlaps(&b), b.overlaps(&a));
		prop_assert!(a.overlaps(&union));
	}
}
