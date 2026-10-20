//! Tables: TipTap's `table`, `tableRow`, `tableCell`, and `tableHeader`,
//! with merged cells, header rows and columns, and Tab between cells.

mod context;
mod edit;
mod navigate;

use canvist_editor::Binding;
use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::CommandSpec;
use canvist_editor::EditKind;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::NodeKind;
use canvist_editor::NodeMeta;
use canvist_editor::Platform;
use canvist_editor::QuerySpec;
use canvist_editor::Registration;
use canvist_editor::TypedExtension;
use canvist_editor::Update;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_editor::commands::Slice;
use canvist_editor::position;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockSpec;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::SchemaBuilder;
use canvist_model::Selection;
use canvist_model::Step;
use canvist_model::Value;

use self::context::CELL;
use self::context::HEADER;
use self::context::ROW;
use self::context::TABLE;
use self::context::TableContext;
use self::context::cell_attrs;
use self::context::current;
use self::context::empty_cell;
use self::context::is_named;
pub use self::edit::HeaderPart;
use crate::resolve;

/// The most rows or columns `insertTable` makes.
const MAX_SIZE: i64 = 1_000;

/// TipTap's `TableKit`: a `table` of `tableRow`s of `tableCell`s and
/// `tableHeader`s, each cell holding blocks, with `colspan`, `rowspan`, and
/// `colwidth` attributes.
///
/// Layout places the cells on a grid: each column as wide as a cell's
/// `colwidth` says or an even share of what is left, each row as tall as its
/// tallest cell. Commands add and delete rows and columns, merge and split
/// cells, and toggle header rows, columns, and cells; a selection from one
/// cell to another selects the rectangle between them. Tab moves to the
/// next cell, adding a row past the last, and Backspace and Delete never
/// join text across a cell's edge. HTML reads and writes `<table>`; Markdown
/// writes GitHub's pipe tables when the table fits them, and HTML when it
/// does not.
#[derive(Clone, Copy, Debug, Default)]
pub struct Table;

/// Typed access to [`Table`].
#[derive(Clone, Copy, Debug)]
pub struct TableHandle {
	_registration: Registration,
}

/// Where the selection head sits in its table, from [`TableHandle::cell`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TableCell {
	/// The cell's top row and leftmost column, from zero.
	pub row: usize,
	/// The cell's leftmost column.
	pub column: usize,
	/// How many rows and columns it spans.
	pub rowspan: usize,
	/// How many columns it spans.
	pub colspan: usize,
	/// Whether it is a header cell.
	pub header: bool,
	/// The size of its table.
	pub rows: usize,
	/// How many columns its table has.
	pub columns: usize,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl TableHandle {
	/// Insert a table of `rows` by `cols` empty cells at the selection, the
	/// first row of header cells when `header_row`, with the caret in the
	/// first cell.
	#[must_use]
	pub fn insert(self, rows: usize, cols: usize, header_row: bool) -> Command {
		insert_table(rows, cols, header_row)
	}

	/// Add a column left of the selected cells.
	#[must_use]
	pub fn add_column_before(self) -> Command {
		command(|state| edit::add_column(state, false))
	}

	/// Add a column right of the selected cells.
	#[must_use]
	pub fn add_column_after(self) -> Command {
		command(|state| edit::add_column(state, true))
	}

	/// Delete the selected columns.
	#[must_use]
	pub fn delete_column(self) -> Command {
		command(edit::delete_column)
	}

	/// Add a row above the selected cells.
	#[must_use]
	pub fn add_row_before(self) -> Command {
		command(|state| edit::add_row(state, false))
	}

	/// Add a row below the selected cells.
	#[must_use]
	pub fn add_row_after(self) -> Command {
		command(|state| edit::add_row(state, true))
	}

	/// Delete the selected rows.
	#[must_use]
	pub fn delete_row(self) -> Command {
		command(edit::delete_row)
	}

	/// Delete the table around the selection.
	#[must_use]
	pub fn delete_table(self) -> Command {
		command(edit::delete_table)
	}

	/// Merge the selected cells into one.
	#[must_use]
	pub fn merge_cells(self) -> Command {
		command(edit::merge_cells)
	}

	/// Split the merged cell holding the selection.
	#[must_use]
	pub fn split_cell(self) -> Command {
		command(edit::split_cell)
	}

	/// Merge the selected cells, or split the merged cell holding the caret.
	#[must_use]
	pub fn merge_or_split(self) -> Command {
		command(edit::merge_or_split)
	}

	/// Toggle header cells for the first row, the first column, or the
	/// selected cells.
	#[must_use]
	pub fn toggle_header(self, part: HeaderPart) -> Command {
		command(move |state| edit::toggle_header(state, part))
	}

	/// Set attribute `name` of the selected cells to `value`, or remove it
	/// for `None`.
	#[must_use]
	pub fn set_cell_attribute(self, name: &str, value: Option<&str>) -> Command {
		cell_attribute(name, value.map_or(Value::Null, Value::from))
	}

	/// Select the next cell's content.
	#[must_use]
	pub fn go_to_next_cell(self) -> Command {
		command(|state| navigate::go_to_cell(state, true))
	}

	/// Select the previous cell's content.
	#[must_use]
	pub fn go_to_previous_cell(self) -> Command {
		command(|state| navigate::go_to_cell(state, false))
	}

	/// Give every short row of every table empty cells up to its width.
	#[must_use]
	pub fn fix_tables(self) -> Command {
		command(fix_all)
	}

	/// Where the selection head sits in its table, or `None` outside one.
	#[must_use]
	pub fn cell(self, state: &EditorState) -> Option<TableCell> {
		cell_info(state)
	}
}

fn cell_attribute(name: &str, value: Value) -> Command {
	let name = name.to_owned();

	command(move |state| edit::set_cell_attribute(state, &name, &value))
}

fn cell_info(state: &EditorState) -> Option<TableCell> {
	let (context, index) = current(state)?;
	let cell = context.map.cells()[index];

	Some(TableCell {
		row: cell.row,
		column: cell.column,
		rowspan: cell.rowspan,
		colspan: cell.colspan,
		header: is_named(state.doc(), context.block(index), &[HEADER]),
		rows: context.map.height(),
		columns: context.map.width(),
	})
}

fn insert_table(rows: usize, cols: usize, header_row: bool) -> Command {
	command(move |state| {
		let kinds = (
			resolve::block(state, TABLE, TABLE)?,
			resolve::block(state, TABLE, ROW)?,
			resolve::block(state, TABLE, CELL)?,
			resolve::block(state, TABLE, HEADER)?,
		);
		let doc = state.doc();
		let Some(range) = position::text_range(doc, state.selection()) else {
			return Ok(None);
		};
		let parent = position::parent_of(doc, range.from.block)
			.and_then(|parent| doc.block(parent))
			.map(canvist_model::Block::kind);

		if !doc.schema().allows_child(parent, kinds.0) {
			return Ok(None);
		}

		let paragraph = doc.schema().default_text_block();
		// Placeholder ids: pasting gives every block a fresh one.
		let mut seq = 0;
		let mut next = || {
			seq += 1;
			BlockId::new(0, seq)
		};
		let table_rows = (0..rows)
			.map(|row| {
				let kind = if row == 0 && header_row {
					kinds.3
				} else {
					kinds.2
				};
				let cells = (0..cols)
					.map(|_| {
						let text = BlockTree::text(next(), paragraph, InlineText::new());
						BlockTree::container(next(), kind, vec![text])
					})
					.collect();
				BlockTree::container(next(), kinds.1, cells)
			})
			.collect();
		let table = BlockTree::container(next(), kinds.0, table_rows);

		state.atomic(|state| {
			let Some(pasted) = commands::replace_selection(state, Slice::new(vec![table]))? else {
				return Ok(None);
			};
			let Some(inserted) = inserted_table(&pasted, kinds.0) else {
				return Ok(Some(pasted));
			};
			let start = edit::first_text(state.doc(), inserted);
			let caret = state.change(EditKind::Selection, |tr| {
				tr.set_selection(Selection::caret(Point::new(start, 0)));
				Ok(true)
			})?;

			Ok(Some(
				caret.map_or(pasted.clone(), |caret| pasted.then(caret)),
			))
		})
	})
}

/// The table an update inserted.
fn inserted_table(update: &Update, table: canvist_model::BlockTypeId) -> Option<BlockId> {
	update.commit.steps.iter().find_map(|step| {
		match step {
			Step::InsertBlocks { blocks, .. } => {
				blocks
					.iter()
					.find(|tree| tree.kind == table)
					.map(|tree| tree.id)
			}
			_ => None,
		}
	})
}

/// The tables holding the blocks `update` touched.
fn touched_tables(doc: &Document, update: &Update) -> Vec<BlockId> {
	let mut tables = Vec::new();
	let parents = update.commit.steps.iter().filter_map(|step| {
		match step {
			Step::InsertBlocks { parent, .. } | Step::RemoveBlocks { parent, .. } => *parent,
			Step::MoveBlocks { to_parent, .. } => *to_parent,
			_ => None,
		}
	});

	for block in update.commit.touched.iter().copied().chain(parents) {
		let table = position::ancestors_or_self(doc, block).find(|&id| is_named(doc, id, &[TABLE]));

		if let Some(table) = table.filter(|table| !tables.contains(table)) {
			tables.push(table);
		}
	}

	tables
}

/// Keep `tables` well formed: every cell holds a block, and every row
/// covers every column, as TipTap's `fixTables` keeps them.
fn fix(state: &mut EditorState, tables: &[BlockId]) -> CommandResult {
	let doc = state.doc();
	let cell = resolve::block(state, TABLE, CELL)?;
	let paragraph = doc.schema().default_text_block();
	let mut empty_cells = Vec::new();
	let mut short_rows = Vec::new();

	for &table in tables {
		let context = TableContext::new(doc, table);
		empty_cells.extend(
			context
				.cells
				.iter()
				.flatten()
				.copied()
				.filter(|&id| doc.children(Some(id)).is_empty()),
		);

		for (row, missing) in context.map.missing().into_iter().enumerate() {
			if missing > 0
				&& let Some(&id) = context.rows.get(row)
			{
				short_rows.push((id, doc.children(Some(id)).len(), missing));
			}
		}
	}

	if empty_cells.is_empty() && short_rows.is_empty() {
		return Ok(None);
	}

	let selection = state.selection();
	state.change(EditKind::Structure, |tr| {
		for &id in &empty_cells {
			let text = tr.allocate_id()?;
			tr.insert_blocks(
				Some(id),
				0,
				vec![BlockTree::text(text, paragraph, InlineText::new())],
			)?;
		}

		for &(row, end, missing) in &short_rows {
			let cells = (0..missing)
				.map(|_| empty_cell(tr, cell, Attrs::new()))
				.collect::<Result<Vec<_>, _>>()?;
			tr.insert_blocks(Some(row), end, cells)?;
		}

		tr.set_selection(position::valid_selection(tr.doc(), selection));
		Ok(true)
	})
}

fn fix_all(state: &mut EditorState) -> CommandResult {
	let doc = state.doc();
	let tables: Vec<BlockId> = doc
		.iter()
		.filter(|block| &*doc.schema().block(block.kind()).name == TABLE)
		.map(canvist_model::Block::id)
		.collect();

	fix(state, &tables)
}

fn size_arg(args: &canvist_editor::Args<'_>, key: &str) -> Result<usize, EditorError> {
	let value = args.int(key)?;

	if !(1..=MAX_SIZE).contains(&value) {
		return Err(EditorError::InvalidArgs {
			name: "insertTable".to_owned(),
			reason: format!("`{key}` must be from 1 to {MAX_SIZE}"),
		});
	}

	Ok(value as usize)
}

/// The `tableCell` query's answer.
fn cell_value(state: &EditorState) -> Value {
	cell_info(state).map_or(Value::Null, |cell| {
		let attrs = Attrs::new()
			.with("row", cell.row as i64)
			.with("column", cell.column as i64)
			.with("rowspan", cell.rowspan as i64)
			.with("colspan", cell.colspan as i64)
			.with("header", cell.header)
			.with("rows", cell.rows as i64)
			.with("columns", cell.columns as i64);
		Value::from(attrs)
	})
}

fn cell_type() -> ValueType {
	let int = |name, doc| Field::required(name, ValueType::Int, doc);

	ValueType::nullable(ValueType::Record(vec![
		int("row", "The cell's top row, from zero."),
		int("column", "The cell's leftmost column, from zero."),
		int("rowspan", "How many rows it spans."),
		int("colspan", "How many columns it spans."),
		Field::required("header", ValueType::Bool, "Whether it is a header cell."),
		int("rows", "How many rows its table has."),
		int("columns", "How many columns its table has."),
	]))
}

fn simple(
	name: &'static str,
	doc: &'static str,
	run: fn(&mut EditorState) -> CommandResult,
) -> CommandSpec {
	CommandSpec::simple(name, doc, command(run))
}

impl Extension for Table {
	fn name(&self) -> &'static str {
		TABLE
	}

	/// Below the default, so a list inside a cell sinks on Tab and lists
	/// and code blocks handle their own Backspace first.
	fn priority(&self) -> i32 {
		99
	}

	fn schema(&self, schema: SchemaBuilder) -> SchemaBuilder {
		schema
			.block(BlockSpec::container(TABLE, &[ROW]))
			.block(BlockSpec::container(ROW, &[CELL, HEADER]).groups(&[]))
			.block(
				BlockSpec::container(CELL, &["block"])
					.attrs(cell_attrs())
					.groups(&[]),
			)
			.block(
				BlockSpec::container(HEADER, &["block"])
					.attrs(cell_attrs())
					.groups(&[]),
			)
	}

	fn nodes(&self) -> Vec<NodeMeta> {
		let cell = |name| {
			NodeMeta::new(NodeKind::Block, name)
				.attr(Field::optional(
					"colspan",
					ValueType::Int,
					1,
					"How many columns the cell spans.",
				))
				.attr(Field::optional(
					"rowspan",
					ValueType::Int,
					1,
					"How many rows the cell spans.",
				))
				.attr(Field::optional(
					"colwidth",
					ValueType::nullable(ValueType::List(Box::new(ValueType::Int))),
					Value::Null,
					"The width in pixels the cell gives each column it spans, or 0 for none.",
				))
		};

		vec![
			NodeMeta::new(NodeKind::Block, TABLE),
			NodeMeta::new(NodeKind::Block, ROW),
			cell(CELL),
			cell(HEADER),
		]
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let insert = CommandSpec::new(
			"insertTable",
			"Insert a table of empty cells at the selection, with the caret in the first cell.",
			|args| {
				Ok(insert_table(
					size_arg(args, "rows")?,
					size_arg(args, "cols")?,
					args.bool("withHeaderRow")?,
				))
			},
		)
		.arg(Field::optional(
			"rows",
			ValueType::Int,
			3,
			"How many rows, from 1 to 1000.",
		))
		.arg(Field::optional(
			"cols",
			ValueType::Int,
			3,
			"How many columns, from 1 to 1000.",
		))
		.arg(Field::optional(
			"withHeaderRow",
			ValueType::Bool,
			true,
			"Whether the first row is header cells.",
		));
		let attribute = CommandSpec::new(
			"setCellAttribute",
			"Set an attribute of the selected cells, or remove it when the value is null.",
			|args| {
				let value = args
					.optional_string("value")?
					.map_or(Value::Null, Value::from);
				Ok(cell_attribute(args.string("name")?, value))
			},
		)
		.arg(Field::required(
			"name",
			ValueType::String,
			"The attribute, such as `backgroundColor`.",
		))
		.arg(Field::required(
			"value",
			ValueType::nullable(ValueType::String),
			"Its value, or null to remove it.",
		));

		Ok(vec![
			insert,
			simple(
				"addColumnBefore",
				"Add a column left of the selected cells.",
				|state| edit::add_column(state, false),
			),
			simple(
				"addColumnAfter",
				"Add a column right of the selected cells.",
				|state| edit::add_column(state, true),
			),
			simple(
				"deleteColumn",
				"Delete the selected columns.",
				edit::delete_column,
			),
			simple(
				"addRowBefore",
				"Add a row above the selected cells.",
				|state| edit::add_row(state, false),
			),
			simple(
				"addRowAfter",
				"Add a row below the selected cells.",
				|state| edit::add_row(state, true),
			),
			simple("deleteRow", "Delete the selected rows.", edit::delete_row),
			simple(
				"deleteTable",
				"Delete the table around the selection.",
				edit::delete_table,
			),
			simple(
				"mergeCells",
				"Merge the selected cells into one.",
				edit::merge_cells,
			),
			simple(
				"splitCell",
				"Split the merged cell holding the selection.",
				edit::split_cell,
			),
			simple(
				"mergeOrSplit",
				"Merge the selected cells, or split the merged cell holding the caret.",
				edit::merge_or_split,
			),
			simple(
				"toggleHeaderRow",
				"Toggle header cells for the first row.",
				|state| edit::toggle_header(state, HeaderPart::Row),
			),
			simple(
				"toggleHeaderColumn",
				"Toggle header cells for the first column.",
				|state| edit::toggle_header(state, HeaderPart::Column),
			),
			simple(
				"toggleHeaderCell",
				"Toggle header cells for the selected cells.",
				|state| edit::toggle_header(state, HeaderPart::Cells),
			),
			attribute,
			simple("goToNextCell", "Select the next cell's content.", |state| {
				navigate::go_to_cell(state, true)
			}),
			simple(
				"goToPreviousCell",
				"Select the previous cell's content.",
				|state| navigate::go_to_cell(state, false),
			),
			simple(
				"fixTables",
				"Give every short row of every table empty cells up to its width.",
				fix_all,
			),
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![QuerySpec::new(
			"tableCell",
			"Where the selection head sits in its table, or null outside one.",
			cell_type(),
			|state, _| Ok(cell_value(state)),
		)])
	}

	fn keymap(&self, _schema: &Schema, _platform: Platform) -> Result<Vec<Binding>, EditorError> {
		let backward = || command(|state| navigate::guard_edge(state, false));
		let forward = || command(|state| navigate::guard_edge(state, true));

		Ok(vec![
			("Tab", command(navigate::tab)),
			(
				"Shift-Tab",
				command(|state| navigate::go_to_cell(state, false)),
			),
			("Backspace", backward()),
			("Shift-Backspace", backward()),
			("Alt-Backspace", backward()),
			("Ctrl-Backspace", backward()),
			("Mod-Backspace", backward()),
			("Delete", forward()),
			("Alt-Delete", forward()),
			("Ctrl-Delete", forward()),
			("Mod-Delete", forward()),
		])
	}

	fn after_update(&self, state: &mut EditorState, update: &Update) -> CommandResult {
		if !update.commit.structural {
			return Ok(None);
		}

		let tables = touched_tables(state.doc(), update);

		if tables.is_empty() {
			return Ok(None);
		}

		fix(state, &tables)
	}
}

impl TypedExtension for Table {
	type Handle = TableHandle;

	fn handle(&self, registration: Registration) -> TableHandle {
		TableHandle {
			_registration: registration,
		}
	}
}
