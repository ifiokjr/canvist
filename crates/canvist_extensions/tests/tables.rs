//! Tables: inserting, Tab between cells, rows and columns, merged cells,
//! headers, keys that stop at a cell's edge, and HTML and Markdown.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::Env;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::Point;
use canvist_model::Selection;
use canvist_model::Value;
use serde_json::Value as Json;
use support::Typist;

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

fn typist(script: &str) -> Typist {
	let mut typist = Typist::with(support::editor_with(canvist_extensions::extras()));
	typist.type_script(script);
	typist
}

fn run(editor: &mut Editor, name: &str, args: &[(&'static str, Value)]) -> bool {
	let args: Attrs = args.iter().cloned().collect();
	editor
		.run_named(name, &args, &mut env())
		.expect("commands run")
		.is_some()
}

/// A typist with a fresh table of `rows` by `cols`, the first row headers,
/// and the caret in the first cell.
fn with_table(rows: i64, cols: i64) -> Typist {
	let mut typist = typist("");
	assert!(run(
		&mut typist.editor,
		"insertTable",
		&[
			("rows", Value::Int(rows)),
			("cols", Value::Int(cols)),
			("withHeaderRow", Value::Bool(true)),
		]
	));
	typist
}

/// The first table's cells, row by row: each cell's text (paragraphs
/// joined by `/`), prefixed `#` for a header and suffixed `|CxR` when it
/// spans more than one slot.
fn cells(typist: &Typist) -> Vec<Vec<String>> {
	let doc = typist.snapshot()["doc"].clone();
	let table = doc
		.as_array()
		.and_then(|blocks| blocks.iter().find(|block| block["type"] == "table"))
		.cloned()
		.unwrap_or(Json::Null);

	table["content"]
		.as_array()
		.into_iter()
		.flatten()
		.map(|row| {
			row["content"]
				.as_array()
				.into_iter()
				.flatten()
				.map(describe_cell)
				.collect()
		})
		.collect()
}

fn describe_cell(cell: &Json) -> String {
	let text: Vec<String> = cell["content"]
		.as_array()
		.into_iter()
		.flatten()
		.map(|paragraph| {
			paragraph["content"]
				.as_array()
				.into_iter()
				.flatten()
				.filter_map(|run| run["text"].as_str())
				.collect()
		})
		.collect();
	let header = if cell["type"] == "tableHeader" {
		"#"
	} else {
		""
	};
	let span = |key: &str| cell["attrs"][key].as_i64().unwrap_or(1);
	let spans = if span("colspan") > 1 || span("rowspan") > 1 {
		format!("|{}x{}", span("colspan"), span("rowspan"))
	} else {
		String::new()
	};

	format!("{header}{}{spans}", text.join("/"))
}

/// The text block of cell `index` of table row `row`.
fn cell_text(typist: &Typist, row: usize, index: usize) -> BlockId {
	let doc = typist.editor.state().doc();
	let table = doc
		.children(None)
		.iter()
		.copied()
		.find(|&id| {
			doc.block(id)
				.is_some_and(|block| &*doc.schema().block(block.kind()).name == "table")
		})
		.expect("a table");
	let row = doc.children(Some(table))[row];
	let cell = doc.children(Some(row))[index];
	doc.children(Some(cell))[0]
}

fn caret_in(typist: &mut Typist, row: usize, index: usize) {
	let block = cell_text(typist, row, index);
	let end = canvist_editor::position::end_of(typist.editor.state().doc(), block);
	typist
		.editor
		.select(Selection::caret(end), &mut env())
		.expect("a valid selection");
}

fn select_cells(typist: &mut Typist, from: (usize, usize), to: (usize, usize)) {
	let anchor = cell_text(typist, from.0, from.1);
	let head = cell_text(typist, to.0, to.1);
	typist
		.editor
		.select(
			Selection::text(Point::new(anchor, 0), Point::new(head, 0)),
			&mut env(),
		)
		.expect("a valid selection");
}

fn grid(rows: &[&[&str]]) -> Vec<Vec<String>> {
	rows.iter()
		.map(|row| row.iter().map(|cell| (*cell).to_owned()).collect())
		.collect()
}

#[test]
fn insert_table_makes_a_header_row_and_puts_the_caret_in_the_first_cell() {
	let mut typist = with_table(2, 3);
	typist.type_script("Name");

	assert_eq!(cells(&typist), grid(&[&["#Name", "#", "#"], &["", "", ""]]));
	support::assert_valid(typist.editor.state());
}

#[test]
fn insert_table_rejects_impossible_sizes() {
	let mut typist = typist("");
	let args: Attrs = [("rows", Value::Int(0))].into_iter().collect();

	assert!(
		typist
			.editor
			.run_named("insertTable", &args, &mut env())
			.is_err()
	);
}

#[test]
fn tab_moves_through_cells_and_adds_a_row_past_the_last() {
	let mut typist = with_table(2, 2);
	typist.type_script("a⇥b⇥c⇥d⇥e");

	assert_eq!(
		cells(&typist),
		grid(&[&["#a", "#b"], &["c", "d"], &["e", ""]])
	);

	typist.type_script("⇤");
	assert_eq!(
		typist.snapshot()["selection"],
		"text 0.1.1.0@0 → 0.1.1.0@1",
		"Shift-Tab selects the previous cell's content"
	);
}

#[test]
fn columns_are_added_beside_and_deleted_from_the_selection() {
	let mut typist = with_table(2, 2);
	typist.type_script("a⇥b⇥c⇥d");
	caret_in(&mut typist, 0, 0);

	assert!(run(&mut typist.editor, "addColumnAfter", &[]));
	assert_eq!(
		cells(&typist),
		grid(&[&["#a", "#", "#b"], &["c", "", "d"]]),
		"a column between two header cells is a header"
	);

	assert!(run(&mut typist.editor, "addColumnBefore", &[]));
	assert_eq!(cells(&typist)[1], vec!["", "c", "", "d"]);

	caret_in(&mut typist, 1, 1);
	assert!(run(&mut typist.editor, "deleteColumn", &[]));
	assert_eq!(cells(&typist), grid(&[&["#", "#", "#b"], &["", "", "d"]]));
	support::assert_valid(typist.editor.state());
}

#[test]
fn rows_are_added_above_and_below_and_deleted() {
	let mut typist = with_table(2, 2);
	typist.type_script("a⇥b⇥c⇥d");

	assert!(run(&mut typist.editor, "addRowBefore", &[]));
	assert!(run(&mut typist.editor, "addRowAfter", &[]));
	assert_eq!(
		cells(&typist),
		grid(&[&["#a", "#b"], &["", ""], &["c", "d"], &["", ""]])
	);

	caret_in(&mut typist, 2, 1);
	assert!(run(&mut typist.editor, "deleteRow", &[]));
	assert_eq!(cells(&typist), grid(&[&["#a", "#b"], &["", ""], &["", ""]]));
}

#[test]
fn merging_cells_spans_their_rectangle_and_keeps_their_content() {
	let mut typist = with_table(3, 3);
	typist.type_script("a⇥b⇥c⇥d⇥e⇥f⇥g⇥h⇥i");
	select_cells(&mut typist, (1, 0), (2, 1));

	assert!(run(&mut typist.editor, "mergeCells", &[]));
	assert_eq!(
		cells(&typist),
		grid(&[&["#a", "#b", "#c"], &["d/e/g/h|2x2", "f"], &["i"]])
	);
	assert_eq!(
		typist
			.editor
			.query("tableCell", &Attrs::new())
			.expect("a query"),
		Value::from(
			Attrs::new()
				.with("row", 1)
				.with("column", 0)
				.with("rowspan", 2)
				.with("colspan", 2)
				.with("header", false)
				.with("rows", 3)
				.with("columns", 3)
		)
	);

	assert!(run(&mut typist.editor, "splitCell", &[]));
	assert_eq!(
		cells(&typist),
		grid(&[&["#a", "#b", "#c"], &["d/e/g/h", "", "f"], &["", "", "i"]])
	);
	support::assert_valid(typist.editor.state());
}

#[test]
fn a_column_or_row_across_a_merged_cell_grows_it() {
	let mut typist = with_table(2, 2);
	select_cells(&mut typist, (0, 0), (0, 1));
	assert!(run(&mut typist.editor, "mergeCells", &[]));
	caret_in(&mut typist, 1, 0);

	assert!(run(&mut typist.editor, "addColumnAfter", &[]));
	assert_eq!(cells(&typist), grid(&[&["#|3x1"], &["", "", ""]]));

	select_cells(&mut typist, (1, 0), (1, 0));
	assert!(run(&mut typist.editor, "deleteColumn", &[]));
	assert_eq!(cells(&typist), grid(&[&["#|2x1"], &["", ""]]));
}

#[test]
fn deleting_a_row_moves_a_cell_spanning_below_it_down() {
	let mut typist = with_table(3, 2);
	typist.type_script("a⇥b⇥c⇥d⇥e⇥f");
	select_cells(&mut typist, (1, 1), (2, 1));
	assert!(run(&mut typist.editor, "mergeCells", &[]));
	caret_in(&mut typist, 1, 0);

	assert!(run(&mut typist.editor, "deleteRow", &[]));
	assert_eq!(cells(&typist), grid(&[&["#a", "#b"], &["e", "d/f"]]));
	support::assert_valid(typist.editor.state());
}

#[test]
fn header_toggles_switch_the_first_row_column_or_selected_cells() {
	let mut typist = with_table(2, 2);

	assert!(run(&mut typist.editor, "toggleHeaderRow", &[]));
	assert_eq!(cells(&typist), grid(&[&["", ""], &["", ""]]));

	assert!(run(&mut typist.editor, "toggleHeaderColumn", &[]));
	assert_eq!(cells(&typist), grid(&[&["#", ""], &["#", ""]]));

	caret_in(&mut typist, 1, 1);
	assert!(run(&mut typist.editor, "toggleHeaderCell", &[]));
	assert_eq!(cells(&typist), grid(&[&["#", ""], &["#", "#"]]));
}

#[test]
fn set_cell_attribute_sets_and_removes_an_attribute() {
	let mut typist = with_table(1, 2);
	select_cells(&mut typist, (0, 0), (0, 1));

	assert!(run(
		&mut typist.editor,
		"setCellAttribute",
		&[
			("name", Value::from("backgroundColor")),
			("value", Value::from("#fef08a"))
		]
	));
	let doc = typist.snapshot()["doc"].clone();
	assert_eq!(
		doc[0]["content"][0]["content"][1]["attrs"]["backgroundColor"],
		"#fef08a"
	);

	assert!(run(
		&mut typist.editor,
		"setCellAttribute",
		&[
			("name", Value::from("backgroundColor")),
			("value", Value::Null)
		]
	));
	let doc = typist.snapshot()["doc"].clone();
	assert_eq!(
		doc[0]["content"][0]["content"][1]["attrs"]["backgroundColor"],
		Json::Null
	);
}

#[test]
fn backspace_and_delete_stop_at_a_cells_edge() {
	let mut typist = with_table(1, 2);
	typist.type_script("a⇥b");
	let before = cells(&typist);
	caret_in(&mut typist, 0, 1);
	typist.type_script("⌫⌫⌫");

	assert_eq!(cells(&typist), grid(&[&["#a", "#"]]));

	caret_in(&mut typist, 0, 0);
	typist.type_script("⌦⌦");
	assert_eq!(cells(&typist), grid(&[&["#a", "#"]]));
	assert_ne!(before, cells(&typist));
}

#[test]
fn deleting_a_selection_across_cells_clears_them() {
	let mut typist = with_table(2, 2);
	typist.type_script("a⇥b⇥c⇥d");
	select_cells(&mut typist, (0, 1), (1, 1));
	typist.type_script("⌫");

	assert_eq!(cells(&typist), grid(&[&["#a", "#"], &["c", ""]]));
	support::assert_valid(typist.editor.state());
}

#[test]
fn delete_table_leaves_a_paragraph() {
	let mut typist = with_table(2, 2);

	assert!(run(&mut typist.editor, "deleteTable", &[]));
	assert!(cells(&typist).is_empty());
	assert_eq!(typist.snapshot()["doc"][0]["type"], "paragraph");
	assert_eq!(
		typist
			.editor
			.query("tableCell", &Attrs::new())
			.expect("a query"),
		Value::Null
	);
}

#[test]
fn a_ragged_table_is_padded_to_full_rows() {
	let mut typist = typist("");
	let schema = typist.editor.schema().clone();
	let slice = clipboard::from_html(
		&schema,
		"<table><tr><td>a</td><td>b</td><td>c</td></tr><tr><td>d</td></tr></table>",
	);
	typist.editor.paste(slice, &mut env()).expect("pastes");

	assert_eq!(cells(&typist), grid(&[&["a", "b", "c"], &["d", "", ""]]));
}

/// HTML through the clipboard codecs and back.
fn html_round_trip(html: &str) -> String {
	let typist = typist("");
	let schema = typist.editor.schema();
	clipboard::to_html(schema, &clipboard::from_html(schema, html))
}

fn markdown_round_trip(markdown: &str) -> String {
	let typist = typist("");
	let schema = typist.editor.schema();
	clipboard::to_markdown(schema, &clipboard::from_markdown(schema, markdown))
}

#[test]
fn tables_round_trip_through_html_as_tiptap_writes_them() {
	let html = "<table><tbody><tr><th colspan=\"2\" \
	            colwidth=\"100,0\"><p>a</p></th></tr><tr><td><p>b</p></td><td \
	            rowspan=\"2\"><p>c</p></td></tr><tr><td><p>d</p></td></tr></tbody></table>";

	assert_eq!(html_round_trip(html), html);
	assert_eq!(
		html_round_trip("<table><tr><td>bare text</td><td></td></tr></table>"),
		"<table><tbody><tr><td><p>bare text</p></td><td><p></p></td></tr></tbody></table>"
	);
}

#[test]
fn simple_tables_round_trip_through_markdown_as_pipe_tables() {
	let markdown = "| Name | Score |\n| :--- | ---: |\n| Ada | 10 \\| 9 |\n| Grace | 8 |";

	assert_eq!(markdown_round_trip(markdown), markdown);
	assert_eq!(
		markdown_round_trip("a | b\n--|--\nc"),
		"| a | b |\n| --- | --- |\n| c |  |"
	);
}

#[test]
fn tables_a_pipe_table_cannot_hold_are_written_as_html() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let merged = clipboard::from_html(
		schema,
		"<table><tr><th colspan=\"2\">a</th></tr><tr><td>b</td><td>c</td></tr></table>",
	);
	let markdown = clipboard::to_markdown(schema, &merged);

	assert!(markdown.starts_with("<table>"), "{markdown}");
	assert_eq!(markdown_round_trip(&markdown), markdown);
}

#[test]
fn tables_pasted_without_the_extension_keep_their_text() {
	let plain = support::editor().schema().clone();
	let pasted = clipboard::from_html(&plain, "<table><tr><td>a</td><td>b</td></tr></table>");

	assert_eq!(clipboard::to_html(&plain, &pasted), "<p>a</p><p>b</p>");
}

/// One random table edit, its cells picked by seed.
#[derive(Clone, Debug)]
enum TableEdit {
	Command(&'static str, (usize, usize)),
	Merge((usize, usize), (usize, usize)),
	Keys(&'static str, (usize, usize)),
}

const TABLE_COMMANDS: [&str; 11] = [
	"addColumnBefore",
	"addColumnAfter",
	"deleteColumn",
	"addRowBefore",
	"addRowAfter",
	"deleteRow",
	"splitCell",
	"toggleHeaderRow",
	"toggleHeaderColumn",
	"toggleHeaderCell",
	"fixTables",
];

fn table_edit() -> impl proptest::strategy::Strategy<Value = TableEdit> {
	use proptest::prelude::*;

	let cell = || (0..6usize, 0..6usize);
	prop_oneof![
		4 => (proptest::sample::select(TABLE_COMMANDS.to_vec()), cell())
			.prop_map(|(name, at)| TableEdit::Command(name, at)),
		2 => (cell(), cell()).prop_map(|(from, to)| TableEdit::Merge(from, to)),
		2 => (proptest::sample::select(vec!["x⇥y", "⌫⌫", "⌦", "z⏎w", "⇤⇤"]), cell())
			.prop_map(|(keys, at)| TableEdit::Keys(keys, at)),
	]
}

/// The cell at a seeded position: a real row and cell of the table.
fn seeded(typist: &Typist, (row, cell): (usize, usize)) -> Option<(usize, usize)> {
	let doc = typist.editor.state().doc();
	let table = doc.children(None).iter().copied().find(|&id| {
		doc.block(id)
			.is_some_and(|block| &*doc.schema().block(block.kind()).name == "table")
	})?;
	let rows = doc.children(Some(table));
	let row = row.checked_rem(rows.len())?;
	let cell = cell.checked_rem(doc.children(Some(rows[row])).len())?;

	Some((row, cell))
}

/// Every table in the document is a full grid of cells holding blocks.
fn assert_tables_well_formed(typist: &Typist) -> Result<(), proptest::test_runner::TestCaseError> {
	let doc = typist.editor.state().doc();
	support::assert_valid(typist.editor.state());

	for block in doc.iter() {
		if &*doc.schema().block(block.kind()).name != "table" {
			continue;
		}

		proptest::prop_assert!(
			!doc.children(Some(block.id())).is_empty(),
			"a table has rows"
		);

		let rows: Vec<Vec<(usize, usize)>> = doc
			.children(Some(block.id()))
			.iter()
			.map(|&row| {
				doc.children(Some(row))
					.iter()
					.map(|&cell| {
						let span = |key: &str| {
							doc.block(cell)
								.and_then(|cell| cell.attrs().get(key))
								.and_then(Value::as_int)
								.map_or(1, |span| span as usize)
						};
						proptest::prop_assert!(
							!doc.children(Some(cell)).is_empty(),
							"a cell holds a block"
						);
						Ok((span("colspan"), span("rowspan")))
					})
					.collect::<Result<_, proptest::test_runner::TestCaseError>>()
			})
			.collect::<Result<_, _>>()?;
		let map = canvist_model::TableMap::new(rows.iter().map(|row| row.iter().copied()));
		proptest::prop_assert!(map.is_full(), "rows cover every column: {:?}", rows);
	}

	Ok(())
}

fn apply_table_edit(typist: &mut Typist, edit: &TableEdit) {
	match *edit {
		TableEdit::Command(name, at) => {
			if let Some((row, cell)) = seeded(typist, at) {
				caret_in(typist, row, cell);
				let _ = typist
					.editor
					.run_named(name, &Attrs::new(), &mut env())
					.expect("table commands never fail");
			}
		}
		TableEdit::Merge(from, to) => {
			if let (Some(from), Some(to)) = (seeded(typist, from), seeded(typist, to)) {
				select_cells(typist, from, to);
				let _ = typist
					.editor
					.run_named("mergeCells", &Attrs::new(), &mut env())
					.expect("merging never fails");
			}
		}
		TableEdit::Keys(keys, at) => {
			if let Some((row, cell)) = seeded(typist, at) {
				caret_in(typist, row, cell);
				typist.type_script(keys);
			}
		}
	}
}

proptest::proptest! {
	#![proptest_config(proptest::prelude::ProptestConfig::with_cases(128))]

	/// Any sequence of table edits leaves every table a full grid, and
	/// undoing them all restores the document.
	#[test]
	fn table_edits_keep_tables_well_formed(
		rows in 1..4i64,
		cols in 1..4i64,
		edits in proptest::collection::vec(table_edit(), 1..24),
	) {
		let before = typist("").snapshot()["doc"].clone();
		let mut typist = with_table(rows, cols);

		for edit in &edits {
			apply_table_edit(&mut typist, edit);
			assert_tables_well_formed(&typist)?;
		}

		typist.unwind(false);
		proptest::prop_assert_eq!(typist.snapshot()["doc"].clone(), before);
	}
}
