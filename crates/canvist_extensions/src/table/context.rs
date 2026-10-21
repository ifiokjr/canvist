//! Reading the table around the selection: its rows, its cells, and the
//! grid they make.

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
use canvist_model::Selection;
use canvist_model::TableMap;
use canvist_model::Value;

pub(crate) const TABLE: &str = "table";
pub(crate) const ROW: &str = "tableRow";
pub(crate) const CELL: &str = "tableCell";
pub(crate) const HEADER: &str = "tableHeader";

/// The attributes of a cell, with TipTap's defaults.
pub(crate) fn cell_attrs() -> Attrs {
	Attrs::new()
		.with("colspan", 1)
		.with("rowspan", 1)
		.with("colwidth", Value::Null)
}

/// A table and the grid its cells make, read from a document.
#[derive(Clone, Debug)]
pub(crate) struct TableContext {
	pub(crate) table: BlockId,
	pub(crate) rows: Vec<BlockId>,
	/// Each row's cells, in order.
	pub(crate) cells: Vec<Vec<BlockId>>,
	pub(crate) map: TableMap,
}

impl TableContext {
	/// Read table `table` from `doc`.
	pub(crate) fn new(doc: &Document, table: BlockId) -> Self {
		let rows = doc.children(Some(table)).to_vec();
		let cells: Vec<Vec<BlockId>> = rows
			.iter()
			.map(|&row| doc.children(Some(row)).to_vec())
			.collect();
		let map = TableMap::new(cells.iter().map(|row| {
			row.iter()
				.map(|&cell| (span(doc, cell, "colspan"), span(doc, cell, "rowspan")))
		}));

		Self {
			table,
			rows,
			cells,
			map,
		}
	}

	/// The block of map cell `index`.
	pub(crate) fn block(&self, index: usize) -> BlockId {
		let cell = self.map.cells()[index];
		self.cells[cell.row][cell.index]
	}

	/// The map index of cell block `cell`.
	pub(crate) fn index_of(&self, cell: BlockId) -> Option<usize> {
		self.cells.iter().enumerate().find_map(|(row, cells)| {
			let index = cells.iter().position(|&id| id == cell)?;
			self.map.find(row, index)
		})
	}

	/// The blocks of the cells overlapping `rect`, in document order.
	pub(crate) fn blocks_in(&self, rect: GridRect) -> Vec<BlockId> {
		self.map
			.cells_in(rect)
			.into_iter()
			.map(|index| self.block(index))
			.collect()
	}
}

/// A cell's `colspan` or `rowspan`, at least one.
pub(crate) fn span(doc: &Document, cell: BlockId, key: &str) -> usize {
	doc.block(cell)
		.and_then(|block| block.attrs().get(key))
		.and_then(Value::as_int)
		.and_then(|value| usize::try_from(value).ok())
		.unwrap_or(1)
		.max(1)
}

/// Whether block `id` has one of the type names `names`.
pub(crate) fn is_named(doc: &Document, id: BlockId, names: &[&str]) -> bool {
	doc.block(id)
		.is_some_and(|block| names.contains(&&*doc.schema().block(block.kind()).name))
}

/// The cell holding `block`, or `block` itself if it is one, and the table
/// that cell belongs to.
pub(crate) fn cell_around(doc: &Document, block: BlockId) -> Option<(BlockId, BlockId)> {
	position::ancestors_or_self(doc, block).find_map(|id| {
		if !is_named(doc, id, &[CELL, HEADER]) {
			return None;
		}

		let row = position::parent_of(doc, id)?;
		let table = position::parent_of(doc, row)?;
		is_named(doc, table, &[TABLE]).then_some((id, table))
	})
}

/// The two blocks the selection runs between.
fn ends(selection: Selection) -> (BlockId, BlockId) {
	match selection {
		Selection::Text { anchor, head } => (anchor.block, head.block),
		Selection::Blocks { anchor, head } => (anchor, head),
	}
}

/// The table around the selection and the rectangle of cells it covers:
/// from the anchor's cell to the head's when both are in the table, grown
/// to whole merged cells.
pub(crate) fn selected(state: &EditorState) -> Option<(TableContext, GridRect)> {
	let doc = state.doc();
	let (anchor, head) = ends(state.selection());
	let (head_cell, table) = cell_around(doc, head)?;
	let anchor_cell = cell_around(doc, anchor)
		.filter(|(_, anchor_table)| *anchor_table == table)
		.map_or(head_cell, |(cell, _)| cell);
	let context = TableContext::new(doc, table);
	let rect_of = |cell| {
		context
			.index_of(cell)
			.map(|index| GridRect::of(&context.map.cells()[index]))
	};
	let rect = rect_of(anchor_cell)?.union(rect_of(head_cell)?);
	let rect = context.map.expand(rect);

	Some((context, rect))
}

/// The table around the selection and the map index of the cell holding
/// the selection head.
pub(crate) fn current(state: &EditorState) -> Option<(TableContext, usize)> {
	let doc = state.doc();
	let (_, head) = ends(state.selection());
	let (cell, table) = cell_around(doc, head)?;
	let context = TableContext::new(doc, table);
	let index = context.index_of(cell)?;

	Some((context, index))
}

/// A new cell of type `kind` holding one empty paragraph.
pub(crate) fn empty_cell(
	tr: &mut EditorTransaction<'_>,
	kind: BlockTypeId,
	attrs: Attrs,
) -> Result<BlockTree, EditorError> {
	let paragraph = tr.doc().schema().default_text_block();
	let (cell, text) = (tr.allocate_id()?, tr.allocate_id()?);

	Ok(BlockTree::container(
		cell,
		kind,
		vec![BlockTree::text(text, paragraph, InlineText::new())],
	)
	.with_attrs(attrs))
}

/// `attrs` with `key` set to `span`, or without it when `span` is one, the
/// default.
pub(crate) fn with_span(mut attrs: Attrs, key: &str, span: usize) -> Attrs {
	if span <= 1 {
		attrs.remove(key);
	} else {
		attrs.set(key, span as i64);
	}

	attrs
}

/// A cell's `colwidth` entries, one per column it spans, with `0` for a
/// column it gives no width.
pub(crate) fn colwidth(doc: &Document, cell: BlockId) -> Vec<i64> {
	let colspan = span(doc, cell, "colspan");
	let widths = doc
		.block(cell)
		.and_then(|block| block.attrs().get("colwidth"))
		.and_then(Value::as_array)
		.unwrap_or_default();

	(0..colspan)
		.map(|column| widths.get(column).and_then(Value::as_int).unwrap_or(0))
		.collect()
}

/// `attrs` with `colwidth` set to `widths`, or without it when none is set.
pub(crate) fn with_colwidth(mut attrs: Attrs, widths: &[i64]) -> Attrs {
	if widths.iter().all(|&width| width <= 0) {
		attrs.remove("colwidth");
	} else {
		let values: Vec<Value> = widths.iter().map(|&width| Value::Int(width)).collect();
		attrs.set("colwidth", values);
	}

	attrs
}
