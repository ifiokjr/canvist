//! Changing a table's shape: rows, columns, merged cells, headers, and
//! cell attributes. Each command reads the grid once and changes the
//! table with the fewest steps it can, so collaborators editing other cells
//! keep their work.

use canvist_editor::CommandResult;
use canvist_editor::EditKind;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::EditorTransaction;
use canvist_editor::position;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::BlockTypeId;
use canvist_model::Document;
use canvist_model::GridRect;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Selection;
use canvist_model::StepError;
use canvist_model::Value;

use super::context::CELL;
use super::context::HEADER;
use super::context::ROW;
use super::context::TABLE;
use super::context::TableContext;
use super::context::colwidth;
use super::context::current;
use super::context::empty_cell;
use super::context::is_named;
use super::context::selected;
use super::context::span;
use super::context::with_colwidth;
use super::context::with_span;
use crate::resolve;

/// The cell and header cell types.
#[derive(Clone, Copy)]
pub(crate) struct CellKinds {
	cell: BlockTypeId,
	header: BlockTypeId,
}

impl CellKinds {
	pub(crate) fn of(state: &EditorState) -> Result<Self, EditorError> {
		Ok(Self {
			cell: resolve::block(state, TABLE, CELL)?,
			header: resolve::block(state, TABLE, HEADER)?,
		})
	}

	const fn pick(self, header: bool) -> BlockTypeId {
		if header { self.header } else { self.cell }
	}
}

/// Whether the cells on both sides of a new row or column are headers:
/// `neighbours` are the map indices of those that exist.
fn between_headers(doc: &Document, context: &TableContext, neighbours: [Option<usize>; 2]) -> bool {
	let mut found = neighbours.into_iter().flatten().peekable();

	found.peek().is_some() && found.all(|index| is_named(doc, context.block(index), &[HEADER]))
}

/// Set `cell`'s attributes to `change` applied to its current ones.
fn update_cell(
	tr: &mut EditorTransaction<'_>,
	cell: BlockId,
	change: impl FnOnce(&Document, Attrs) -> Attrs,
) -> Result<(), EditorError> {
	let Some(block) = tr.doc().block(cell) else {
		return Ok(());
	};
	let (kind, attrs) = (block.kind(), block.attrs().clone());
	let attrs = change(tr.doc(), attrs);
	tr.set_block(cell, kind, attrs)?;
	Ok(())
}

/// Add a column before or after the selected cells. A cell spanning the new
/// column's place grows wider instead of being split.
pub(crate) fn add_column(state: &mut EditorState, after: bool) -> CommandResult {
	let Some((context, rect)) = selected(state) else {
		return Ok(None);
	};
	let kinds = CellKinds::of(state)?;
	let column = if after { rect.right } else { rect.left };

	state.change(EditKind::Structure, |tr| {
		insert_column(tr, &context, column, kinds)?;
		Ok(true)
	})
}

fn insert_column(
	tr: &mut EditorTransaction<'_>,
	context: &TableContext,
	column: usize,
	kinds: CellKinds,
) -> Result<(), EditorError> {
	let map = &context.map;
	let mut widened = Vec::new();

	for row in 0..map.height() {
		let straddling = map
			.cell_at(row, column)
			.filter(|&index| map.cells()[index].column < column);

		if let Some(index) = straddling {
			if !widened.contains(&index) {
				widened.push(index);
				let offset = column - map.cells()[index].column;
				update_cell(tr, context.block(index), |doc, attrs| {
					let block = context.block(index);
					let mut widths = colwidth(doc, block);
					widths.insert(offset, 0);
					with_colwidth(
						with_span(attrs, "colspan", span(doc, block, "colspan") + 1),
						&widths,
					)
				})?;
			}

			continue;
		}

		let neighbours = [
			column
				.checked_sub(1)
				.and_then(|left| map.cell_at(row, left)),
			map.cell_at(row, column),
		];
		let kind = kinds.pick(between_headers(tr.doc(), context, neighbours));
		let cell = empty_cell(tr, kind, Attrs::new())?;
		tr.insert_blocks(
			Some(context.rows[row]),
			map.insert_index(row, column),
			vec![cell],
		)?;
	}

	Ok(())
}

/// Delete the selected columns, or the whole table when every column is
/// selected. Cells spanning a deleted column grow narrower.
pub(crate) fn delete_column(state: &mut EditorState) -> CommandResult {
	let Some((context, rect)) = selected(state) else {
		return Ok(None);
	};

	if rect.left == 0 && rect.right >= context.map.width() {
		return delete_table(state);
	}

	state.change(EditKind::Structure, |tr| {
		for column in (rect.left..rect.right).rev() {
			remove_column(tr, context.table, column)?;
		}

		caret_near(tr, context.table, (rect.top, rect.left));
		Ok(true)
	})
}

fn remove_column(
	tr: &mut EditorTransaction<'_>,
	table: BlockId,
	column: usize,
) -> Result<(), EditorError> {
	let context = TableContext::new(tr.doc(), table);

	// From the last cell back, so removing one never moves those before it.
	for (index, cell) in context.map.cells().iter().enumerate().rev() {
		if !cell.columns().contains(&column) {
			continue;
		}

		let block = context.block(index);

		if cell.colspan == 1 {
			tr.remove_blocks(Some(context.rows[cell.row]), cell.index, 1)?;
			continue;
		}

		let offset = column - cell.column;
		update_cell(tr, block, |doc, attrs| {
			let mut widths = colwidth(doc, block);
			widths.remove(offset);
			with_colwidth(with_span(attrs, "colspan", cell.colspan - 1), &widths)
		})?;
	}

	Ok(())
}

/// Add a row above or below the selected cells. A cell spanning the new
/// row's place grows taller instead of being split.
pub(crate) fn add_row(state: &mut EditorState, after: bool) -> CommandResult {
	let Some((context, rect)) = selected(state) else {
		return Ok(None);
	};
	let kinds = CellKinds::of(state)?;
	let row_kind = resolve::block(state, TABLE, ROW)?;
	let row = if after { rect.bottom } else { rect.top };

	state.change(EditKind::Structure, |tr| {
		let cells = new_row_cells(tr, &context, row, kinds)?;
		let id = tr.allocate_id()?;
		tr.insert_blocks(
			Some(context.table),
			row,
			vec![BlockTree::container(id, row_kind, cells)],
		)?;
		Ok(true)
	})
}

/// The cells of a new row at `row`, growing the cells that span across it.
fn new_row_cells(
	tr: &mut EditorTransaction<'_>,
	context: &TableContext,
	row: usize,
	kinds: CellKinds,
) -> Result<Vec<BlockTree>, EditorError> {
	let map = &context.map;
	let mut cells = Vec::new();
	let mut column = 0;

	while column < map.width() {
		let straddling = map
			.cell_at(row, column)
			.filter(|&index| map.cells()[index].row < row);

		if let Some(index) = straddling {
			let cell = map.cells()[index];
			update_cell(tr, context.block(index), |_, attrs| {
				with_span(attrs, "rowspan", cell.rowspan + 1)
			})?;
			column = cell.column + cell.colspan;
			continue;
		}

		let neighbours = [
			row.checked_sub(1)
				.and_then(|above| map.cell_at(above, column)),
			map.cell_at(row, column),
		];
		let kind = kinds.pick(between_headers(tr.doc(), context, neighbours));
		cells.push(empty_cell(tr, kind, Attrs::new())?);
		column += 1;
	}

	Ok(cells)
}

/// Delete the selected rows, or the whole table when every row is
/// selected. A cell spanning a deleted row grows shorter, moving down a row
/// when it started in the deleted one.
pub(crate) fn delete_row(state: &mut EditorState) -> CommandResult {
	let Some((context, rect)) = selected(state) else {
		return Ok(None);
	};

	if rect.top == 0 && rect.bottom >= context.map.height() {
		return delete_table(state);
	}

	state.change(EditKind::Structure, |tr| {
		for row in (rect.top..rect.bottom).rev() {
			remove_row(tr, context.table, row)?;
		}

		caret_near(tr, context.table, (rect.top, rect.left));
		Ok(true)
	})
}

fn remove_row(
	tr: &mut EditorTransaction<'_>,
	table: BlockId,
	row: usize,
) -> Result<(), EditorError> {
	let context = TableContext::new(tr.doc(), table);
	let map = &context.map;
	let mut moved = 0;

	for (index, cell) in map.cells().iter().enumerate() {
		if !cell.rows().contains(&row) || cell.rowspan == 1 {
			continue;
		}

		let block = context.block(index);
		update_cell(tr, block, |_, attrs| {
			with_span(attrs, "rowspan", cell.rowspan - 1)
		})?;

		// A spanning cell starting in the deleted row carries on in the next.
		if cell.row == row {
			let to = map.insert_index(row + 1, cell.column) + moved;
			tr.move_blocks(
				(Some(context.rows[row]), cell.index - moved, 1),
				(Some(context.rows[row + 1]), to),
			)?;
			moved += 1;
		}
	}

	tr.remove_blocks(Some(table), row, 1)?;
	Ok(())
}

/// Put the caret at the start of the cell at `slot` of `table`, or the
/// nearest one when the table no longer reaches it, as the cell that took
/// a deleted one's place.
fn caret_near(tr: &mut EditorTransaction<'_>, table: BlockId, (row, column): (usize, usize)) {
	let context = TableContext::new(tr.doc(), table);
	let map = &context.map;
	let row = row.min(map.height().saturating_sub(1));
	let column = column.min(map.width().saturating_sub(1));
	let selection = match map.cell_at(row, column) {
		Some(index) => Selection::caret(Point::new(first_text(tr.doc(), context.block(index)), 0)),
		None => position::valid_selection(tr.doc(), tr.selection_before()),
	};

	tr.set_selection(selection);
}

/// Delete the table around the selection, leaving the caret where it was.
pub(crate) fn delete_table(state: &mut EditorState) -> CommandResult {
	let Some((context, _)) = current(state) else {
		return Ok(None);
	};
	let doc = state.doc();
	let parent = position::parent_of(doc, context.table);
	let Some(index) = doc.index_in_parent(context.table) else {
		return Ok(None);
	};
	let alone = doc.children(parent).len() == 1;
	let paragraph = doc.schema().default_text_block();

	state.change(EditKind::Structure, |tr| {
		tr.remove_blocks(parent, index, 1)?;

		if alone {
			let id = tr.allocate_id()?;
			tr.insert_blocks(
				parent,
				index,
				vec![BlockTree::text(id, paragraph, InlineText::new())],
			)?;
		}

		let near = tr
			.doc()
			.children(parent)
			.get(index.min(tr.doc().children(parent).len().saturating_sub(1)))
			.copied();
		let caret = near.map_or_else(
			|| position::start_selection(tr.doc()),
			|block| Selection::caret(Point::new(block, 0)),
		);
		tr.set_selection(position::valid_selection(tr.doc(), caret));
		Ok(true)
	})
}

/// Merge the selected cells into the top-left one, which spans their
/// rectangle and holds their content in order. Does nothing for one cell,
/// or when the rectangle's corner is an empty slot.
pub(crate) fn merge_cells(state: &mut EditorState) -> CommandResult {
	let Some((context, rect)) = selected(state) else {
		return Ok(None);
	};
	let cells = context.blocks_in(rect);
	let Some(target) = context
		.map
		.cell_at(rect.top, rect.left)
		.map(|index| context.block(index))
	else {
		return Ok(None);
	};

	if cells.len() < 2 {
		return Ok(None);
	}

	let widths = merged_widths(state.doc(), &context, rect);

	state.change(EditKind::Structure, |tr| {
		let empty_target = is_empty_cell(tr.doc(), target);
		let mut moved = false;

		for &cell in cells.iter().filter(|&&cell| cell != target) {
			if !is_empty_cell(tr.doc(), cell) {
				let count = tr.doc().children(Some(cell)).len();
				let end = tr.doc().children(Some(target)).len();
				tr.move_blocks((Some(cell), 0, count), (Some(target), end))?;
				moved = true;
			}

			let (row, index) = location(tr.doc(), cell)?;
			tr.remove_blocks(row, index, 1)?;
		}

		if empty_target && moved {
			tr.remove_blocks(Some(target), 0, 1)?;
		}

		update_cell(tr, target, |_, attrs| {
			let attrs = with_span(attrs, "colspan", rect.right - rect.left);
			with_colwidth(with_span(attrs, "rowspan", rect.bottom - rect.top), &widths)
		})?;
		let start = first_text(tr.doc(), target);
		tr.set_selection(Selection::caret(Point::new(start, 0)));
		Ok(true)
	})
}

/// The width each column of `rect` was given, from the first cell in it to
/// give one.
fn merged_widths(doc: &Document, context: &TableContext, rect: GridRect) -> Vec<i64> {
	(rect.left..rect.right)
		.map(|column| {
			(rect.top..rect.bottom)
				.filter_map(|row| context.map.cell_at(row, column))
				.map(|index| {
					let cell = context.map.cells()[index];
					colwidth(doc, context.block(index))[column - cell.column]
				})
				.find(|&width| width > 0)
				.unwrap_or(0)
		})
		.collect()
}

/// Split the merged cell holding the selection back into one cell per
/// slot. The new cells are empty, of the same type, and keep the widths.
pub(crate) fn split_cell(state: &mut EditorState) -> CommandResult {
	let Some((context, index)) = current(state) else {
		return Ok(None);
	};
	let cell = context.map.cells()[index];

	if cell.colspan == 1 && cell.rowspan == 1 {
		return Ok(None);
	}

	let block = context.block(index);
	let Some(kind) = state.doc().block(block).map(canvist_model::Block::kind) else {
		return Ok(None);
	};
	let widths = colwidth(state.doc(), block);

	state.change(EditKind::Structure, |tr| {
		update_cell(tr, block, |_, attrs| {
			let attrs = with_span(with_span(attrs, "colspan", 1), "rowspan", 1);
			with_colwidth(attrs, &widths[..1])
		})?;

		for row in cell.rows() {
			let (at, first) = if row == cell.row {
				(cell.index + 1, cell.column + 1)
			} else {
				(context.map.insert_index(row, cell.column), cell.column)
			};
			let trees = (first..cell.column + cell.colspan)
				.map(|column| {
					let attrs = with_colwidth(
						Attrs::new(),
						&widths[column - cell.column..=column - cell.column],
					);
					empty_cell(tr, kind, attrs)
				})
				.collect::<Result<Vec<_>, _>>()?;
			tr.insert_blocks(Some(context.rows[row]), at, trees)?;
		}

		Ok(true)
	})
}

/// Merge the selected cells, or split the merged cell holding the caret.
pub(crate) fn merge_or_split(state: &mut EditorState) -> CommandResult {
	match merge_cells(state)? {
		Some(update) => Ok(Some(update)),
		None => split_cell(state),
	}
}

/// Which cells [`TableHandle::toggle_header`](super::TableHandle::toggle_header)
/// switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeaderPart {
	/// The first row.
	Row,
	/// The first column.
	Column,
	/// The selected cells.
	Cells,
}

/// Turn the first row, the first column, or the selected cells into header
/// cells, or back into plain cells when they all are headers already.
///
/// As in TipTap, the corner cell belongs to a header row or column that is
/// already on: toggling the other one leaves it a header, so toggling twice
/// changes nothing.
pub(crate) fn toggle_header(state: &mut EditorState, part: HeaderPart) -> CommandResult {
	let Some((context, rect)) = selected(state) else {
		return Ok(None);
	};
	let kinds = CellKinds::of(state)?;
	let doc = state.doc();
	let cells = context.map.cells();
	let all_headers = |first: fn(&canvist_model::GridCell) -> bool| {
		cells
			.iter()
			.enumerate()
			.filter(|(_, cell)| first(cell))
			.all(|(index, _)| is_named(doc, context.block(index), &[HEADER]))
	};
	let header_row = all_headers(|cell| cell.row == 0);
	let header_column = all_headers(|cell| cell.column == 0);
	let targets: Vec<BlockId> = cells
		.iter()
		.enumerate()
		.filter(|(index, cell)| {
			match part {
				HeaderPart::Row => cell.row == 0 && !(header_column && cell.column == 0),
				HeaderPart::Column => cell.column == 0 && !(header_row && cell.row == 0),
				HeaderPart::Cells => context.blocks_in(rect).contains(&context.block(*index)),
			}
		})
		.map(|(index, _)| context.block(index))
		.collect();

	if targets.is_empty() {
		return Ok(None);
	}

	let to_header = !targets.iter().all(|&cell| is_named(doc, cell, &[HEADER]));
	let kind = kinds.pick(to_header);

	state.change(EditKind::Structure, |tr| {
		for &cell in &targets {
			let Some(block) = tr.doc().block(cell) else {
				continue;
			};

			if block.kind() != kind {
				let attrs = block.attrs().clone();
				tr.set_block(cell, kind, attrs)?;
			}
		}

		Ok(true)
	})
}

/// Set attribute `name` of every selected cell to `value`, or remove it
/// when `value` is null.
pub(crate) fn set_cell_attribute(
	state: &mut EditorState,
	name: &str,
	value: &Value,
) -> CommandResult {
	let Some((context, rect)) = selected(state) else {
		return Ok(None);
	};
	let cells = context.blocks_in(rect);

	state.change(EditKind::Structure, |tr| {
		for &cell in &cells {
			update_cell(tr, cell, |_, mut attrs| {
				if *value == Value::Null {
					attrs.remove(name);
				} else {
					attrs.set(name, value.clone());
				}

				attrs
			})?;
		}

		Ok(true)
	})
}

/// Whether `cell` holds nothing but one empty text block.
pub(crate) fn is_empty_cell(doc: &Document, cell: BlockId) -> bool {
	match doc.children(Some(cell)) {
		[only] => {
			doc.block(*only)
				.and_then(canvist_model::Block::inline)
				.is_some_and(InlineText::is_empty)
		}
		children => children.is_empty(),
	}
}

/// The parent and index of `block`.
fn location(doc: &Document, block: BlockId) -> Result<(Option<BlockId>, usize), EditorError> {
	let index = doc
		.index_in_parent(block)
		.ok_or(StepError::UnknownBlock(block))?;
	Ok((position::parent_of(doc, block), index))
}

/// The first text block inside `block`, or `block` itself.
pub(crate) fn first_text(doc: &Document, block: BlockId) -> BlockId {
	let mut current = block;

	while let Some(&first) = doc.children(Some(current)).first() {
		current = first;
	}

	current
}
