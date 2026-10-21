//! Tables: inserting, Tab between cells, rows and columns, merged cells,
//! headers, keys that stop at a cell's edge, and HTML and Markdown.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::EditorError;
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

/// Named command arguments.
type Args = Vec<(&'static str, Value)>;

/// Every table command, by name.
const COMMANDS: [&str; 16] = [
	"addColumnBefore",
	"addColumnAfter",
	"deleteColumn",
	"addRowBefore",
	"addRowAfter",
	"deleteRow",
	"deleteTable",
	"mergeCells",
	"splitCell",
	"mergeOrSplit",
	"toggleHeaderRow",
	"toggleHeaderColumn",
	"toggleHeaderCell",
	"goToNextCell",
	"goToPreviousCell",
	"fixTables",
];

fn can(editor: &mut Editor, name: &str, args: &[(&'static str, Value)]) -> bool {
	let args: Attrs = args.iter().cloned().collect();
	let command = editor.command_named(name, &args).expect("a known command");
	editor
		.can(&command, &mut env())
		.expect("checking never fails")
}

fn run_error(editor: &mut Editor, name: &str, args: &[(&'static str, Value)]) -> EditorError {
	let args: Attrs = args.iter().cloned().collect();
	let before = editor.state().doc().to_json();
	let error = editor
		.run_named(name, &args, &mut env())
		.expect_err("the command is refused");
	assert_eq!(editor.state().doc().to_json(), before, "nothing changes");
	error
}

fn table_cell(typist: &Typist) -> Value {
	typist
		.editor
		.query("tableCell", &Attrs::new())
		.expect("a query")
}

/// The whole document as a slice, as the formats write it.
fn whole(typist: &Typist) -> canvist_editor::Slice {
	let doc = typist.editor.state().doc();
	canvist_editor::Slice::new(
		doc.children(None)
			.iter()
			.filter_map(|&id| doc.tree(id))
			.collect(),
	)
}

#[test]
fn outside_a_table_only_insert_table_applies() {
	let mut typist = typist("Plain");
	let before = typist.snapshot();
	let attribute = [("name", Value::from("x")), ("value", Value::from("y"))];

	for name in COMMANDS {
		assert!(!can(&mut typist.editor, name, &[]), "{name}");
		assert!(!run(&mut typist.editor, name, &[]), "{name}");
	}

	assert!(!can(&mut typist.editor, "setCellAttribute", &attribute));
	assert!(!run(&mut typist.editor, "setCellAttribute", &attribute));
	assert_eq!(table_cell(&typist), Value::Null);
	assert_eq!(typist.snapshot(), before);
	assert!(can(&mut typist.editor, "insertTable", &[]));
}

#[test]
fn inside_a_table_can_reports_each_command_without_changing_anything() {
	let mut typist = with_table(2, 2);
	let before = typist.snapshot();
	let expected = [
		("addColumnBefore", true),
		("addColumnAfter", true),
		("deleteColumn", true),
		("addRowBefore", true),
		("addRowAfter", true),
		("deleteRow", true),
		("deleteTable", true),
		("mergeCells", false),
		("splitCell", false),
		("mergeOrSplit", false),
		("toggleHeaderRow", true),
		("toggleHeaderColumn", true),
		("toggleHeaderCell", true),
		("goToNextCell", true),
		("goToPreviousCell", false),
		("fixTables", false),
	];

	for (name, applies) in expected {
		assert_eq!(can(&mut typist.editor, name, &[]), applies, "{name}");
	}

	assert_eq!(typist.snapshot(), before);
	select_cells(&mut typist, (0, 0), (1, 1));
	assert!(can(&mut typist.editor, "mergeCells", &[]));
	assert!(can(&mut typist.editor, "mergeOrSplit", &[]));
}

#[test]
fn insert_table_takes_typed_arguments_and_defaults() {
	let mut typist = typist("");
	assert!(run(&mut typist.editor, "insertTable", &[]));
	assert_eq!(
		cells(&typist),
		grid(&[&["#", "#", "#"], &["", "", ""], &["", "", ""]])
	);

	let mut typist = self::typist("");
	assert!(run(
		&mut typist.editor,
		"insertTable",
		&[
			("rows", Value::Int(1)),
			("cols", Value::Int(4)),
			("withHeaderRow", Value::Bool(false))
		]
	));
	assert_eq!(cells(&typist), grid(&[&["", "", "", ""]]));
	assert_eq!(typist.snapshot()["selection"], "caret 0.0.0.0@0");

	let mut typist = self::typist("");
	assert!(run(
		&mut typist.editor,
		"insertTable",
		&[("rows", Value::Int(1000)), ("cols", Value::Int(1))]
	));
	assert_eq!(cells(&typist).len(), 1000);
}

#[test]
fn invalid_arguments_are_refused_with_invalid_args() {
	let mut typist = with_table(1, 1);
	let cases: [(&str, Args); 10] = [
		("insertTable", vec![("rows", Value::Int(0))]),
		("insertTable", vec![("cols", Value::Int(1001))]),
		("insertTable", vec![("rows", Value::Int(-1))]),
		("insertTable", vec![("rows", Value::from("2"))]),
		("insertTable", vec![("withHeaderRow", Value::Int(1))]),
		("insertTable", vec![("depth", Value::Int(1))]),
		("setCellAttribute", vec![("name", Value::from("x"))]),
		("setCellAttribute", vec![("value", Value::from("x"))]),
		(
			"setCellAttribute",
			vec![("name", Value::from("x")), ("value", Value::Int(1))],
		),
		("addRowAfter", vec![("count", Value::Int(2))]),
	];

	for (name, args) in cases {
		assert!(
			matches!(
				run_error(&mut typist.editor, name, &args),
				EditorError::InvalidArgs { .. }
			),
			"{name} {args:?}"
		);
	}

	assert!(matches!(
		typist
			.editor
			.query("tableCell", &Attrs::new().with("row", 1)),
		Err(EditorError::InvalidArgs { .. })
	));
}

#[test]
fn an_editor_without_the_extension_has_none_of_its_names() {
	let mut editor = support::editor();

	for name in COMMANDS.iter().chain(&["insertTable"]) {
		assert!(
			matches!(
				run_error(&mut editor, name, &[]),
				EditorError::UnknownName(_)
			),
			"{name}"
		);
	}

	assert!(matches!(
		editor.query("tableCell", &Attrs::new()),
		Err(EditorError::UnknownName(_))
	));
}

#[test]
fn tables_go_where_blocks_go_and_nowhere_else() {
	// A table in a cell is allowed, and layout stacks it.
	let mut typist = with_table(1, 1);
	assert!(run(
		&mut typist.editor,
		"insertTable",
		&[("rows", Value::Int(1)), ("cols", Value::Int(1))]
	));
	assert_eq!(typist.snapshot()["selection"], "caret 0.0.0.0.0.0.0@0");
	support::assert_valid(typist.editor.state());

	// A details summary holds only text.
	let mut typist = self::typist("x");
	assert!(run(&mut typist.editor, "setDetails", &[]));
	assert!(!can(&mut typist.editor, "insertTable", &[]));
}

#[test]
fn go_to_next_and_previous_cell_select_whole_cells_and_stop_at_the_ends() {
	let mut typist = with_table(1, 2);
	typist.type_script("ab⏎c");

	assert!(run(&mut typist.editor, "goToNextCell", &[]));
	assert_eq!(typist.snapshot()["selection"], "caret 0.0.1.0@0");
	assert!(!run(&mut typist.editor, "goToNextCell", &[]));
	assert!(run(&mut typist.editor, "goToPreviousCell", &[]));
	assert_eq!(
		typist.snapshot()["selection"],
		"text 0.0.0.0@0 → 0.0.0.1@1",
		"the previous cell's paragraphs are all selected"
	);
	assert!(!run(&mut typist.editor, "goToPreviousCell", &[]));

	// Shift-Tab in the first cell does nothing at all.
	caret_in(&mut typist, 0, 0);
	let before = typist.snapshot();
	typist.type_script("⇤");
	assert_eq!(typist.snapshot(), before);
}

#[test]
fn tab_in_a_cell_list_sinks_the_item_before_moving_on() {
	let mut typist = with_table(1, 2);
	typist.type_script("- one⏎two⇥");

	assert_eq!(
		typist.snapshot()["doc"][0]["content"][0]["content"][0]["content"][0]["content"][0]
			["content"][1]["type"],
		"bulletList",
		"the second item nests in the first"
	);
	assert_eq!(
		table_cell(&typist)
			.as_map()
			.and_then(|cell| cell.get("column"))
			.cloned(),
		Some(Value::Int(0))
	);
}

#[test]
fn enter_in_a_cell_adds_a_paragraph_in_that_cell() {
	let mut typist = with_table(1, 2);
	typist.type_script("a⏎b");

	assert_eq!(cells(&typist), grid(&[&["#a/b", "#"]]));
}

#[test]
fn every_word_and_line_delete_stops_at_a_cells_edge() {
	for chord in [
		"[Mod-Backspace]",
		"[Ctrl-Backspace]",
		"[Alt-Backspace]",
		"[Shift-Backspace]",
	] {
		let mut typist = with_table(1, 2);
		typist.type_script("a b⇥");
		typist.type_script(chord);
		assert_eq!(cells(&typist), grid(&[&["#a b", "#"]]), "{chord}");
	}

	for chord in ["[Mod-Delete]", "[Ctrl-Delete]", "[Alt-Delete]"] {
		let mut typist = with_table(1, 2);
		typist.type_script("a⇥b");
		caret_in(&mut typist, 0, 0);
		typist.type_script(chord);
		assert_eq!(cells(&typist), grid(&[&["#a", "#b"]]), "{chord}");
	}
}

#[test]
fn merging_a_selection_that_cuts_a_merged_cell_grows_to_a_rectangle() {
	let mut typist = with_table(3, 3);
	typist.type_script("a⇥b⇥c⇥d⇥e⇥f⇥g⇥h⇥i");
	select_cells(&mut typist, (1, 1), (2, 1));
	assert!(run(&mut typist.editor, "mergeCells", &[]));
	assert_eq!(
		cells(&typist),
		grid(&[&["#a", "#b", "#c"], &["d", "e/h|1x2", "f"], &["g", "i"]])
	);

	// From d to the merged cell's lower half: the rectangle grows to cover
	// the whole merged cell, so g joins too.
	select_cells(&mut typist, (1, 0), (1, 1));
	assert!(run(&mut typist.editor, "mergeCells", &[]));
	assert_eq!(
		cells(&typist),
		grid(&[&["#a", "#b", "#c"], &["d/e/h/g|2x2", "f"], &["i"]])
	);
	support::assert_valid(typist.editor.state());
}

#[test]
fn merging_one_cell_does_nothing_and_merge_or_split_toggles() {
	let mut typist = with_table(2, 2);
	select_cells(&mut typist, (1, 0), (1, 0));
	assert!(!run(&mut typist.editor, "mergeCells", &[]));

	select_cells(&mut typist, (1, 0), (1, 1));
	assert!(run(&mut typist.editor, "mergeOrSplit", &[]));
	assert_eq!(cells(&typist), grid(&[&["#", "#"], &["|2x1"]]));
	assert!(run(&mut typist.editor, "mergeOrSplit", &[]));
	assert_eq!(cells(&typist), grid(&[&["#", "#"], &["", ""]]));
}

#[test]
fn merging_keeps_column_widths_and_splitting_hands_them_back() {
	let mut typist = typist("");
	let schema = typist.editor.schema().clone();
	let pasted = clipboard::from_html(
		&schema,
		"<table><tr><td colwidth=\"120\">a</td><td colwidth=\"80\">b</td></tr></table>",
	);
	typist.editor.paste(pasted, &mut env()).expect("pastes");
	select_cells(&mut typist, (0, 0), (0, 1));

	assert!(run(&mut typist.editor, "mergeCells", &[]));
	let html = clipboard::to_html(&schema, &whole(&typist));
	assert!(
		html.contains("<td colspan=\"2\" colwidth=\"120,80\">"),
		"{html}"
	);

	assert!(run(&mut typist.editor, "splitCell", &[]));
	let html = clipboard::to_html(&schema, &whole(&typist));
	assert!(
		html.contains("<td colwidth=\"120\"><p>a</p><p>b</p></td><td colwidth=\"80\">"),
		"{html}"
	);
}

#[test]
fn deleting_every_row_or_column_deletes_the_table() {
	for name in ["deleteRow", "deleteColumn"] {
		let mut typist = typist("Before");
		typist.type_script("⏎");
		run(
			&mut typist.editor,
			"insertTable",
			&[("rows", Value::Int(2)), ("cols", Value::Int(2))],
		);
		select_cells(&mut typist, (0, 0), (1, 1));

		assert!(run(&mut typist.editor, name, &[]), "{name}");
		assert_eq!(cells(&typist), Vec::<Vec<String>>::new(), "{name}");
		assert_eq!(typist.snapshot()["doc"][0]["content"][0]["text"], "Before");
		support::assert_valid(typist.editor.state());
	}
}

#[test]
fn delete_table_between_blocks_keeps_them_and_adds_nothing() {
	let mut typist = typist("Before");
	typist.type_script("⏎");
	run(
		&mut typist.editor,
		"insertTable",
		&[("rows", Value::Int(1)), ("cols", Value::Int(1))],
	);

	assert!(run(&mut typist.editor, "deleteTable", &[]));
	let doc = typist.snapshot()["doc"].clone();
	assert_eq!(doc.as_array().map(Vec::len), Some(2), "{doc}");
	assert_eq!(typist.snapshot()["selection"], "caret 1@0");
}

#[test]
fn toggling_a_header_twice_restores_it() {
	let mut typist = with_table(2, 2);
	let before = cells(&typist);

	for name in ["toggleHeaderRow", "toggleHeaderColumn", "toggleHeaderCell"] {
		assert!(run(&mut typist.editor, name, &[]));
		assert!(run(&mut typist.editor, name, &[]));
		assert_eq!(cells(&typist), before, "{name}");
	}

	select_cells(&mut typist, (1, 0), (1, 1));
	assert!(run(&mut typist.editor, "toggleHeaderCell", &[]));
	assert_eq!(cells(&typist), grid(&[&["#", "#"], &["#", "#"]]));
	assert_eq!(
		table_cell(&typist)
			.as_map()
			.and_then(|cell| cell.get("header"))
			.cloned(),
		Some(Value::Bool(true))
	);
}

#[test]
fn each_command_undoes_to_the_exact_document_and_redoes() {
	let mut cases: Vec<(&str, Args)> = COMMANDS
		.iter()
		.filter(|name| !matches!(**name, "goToNextCell" | "goToPreviousCell" | "fixTables"))
		.map(|&name| (name, Vec::new()))
		.collect();
	cases.push((
		"setCellAttribute",
		vec![
			("name", Value::from("align")),
			("value", Value::from("left")),
		],
	));
	cases.push((
		"insertTable",
		vec![("rows", Value::Int(1)), ("cols", Value::Int(1))],
	));

	for (name, args) in cases {
		let mut typist = with_table(3, 3);
		typist.type_script("a⇥b⇥c⇥d⇥e⇥f⏸");
		select_cells(&mut typist, (1, 0), (2, 1));
		if name == "splitCell" {
			run(&mut typist.editor, "mergeCells", &[]);
			typist.type_script("⏸");
		}
		let before = typist.snapshot()["doc"].clone();

		let args: Attrs = args.iter().cloned().collect();
		let applied = typist
			.editor
			.run_named(name, &args, &mut Env::headless(Duration::from_secs(3600)))
			.expect("commands run");
		assert!(applied.is_some(), "{name} applies");
		let after = typist.snapshot()["doc"].clone();
		assert_ne!(after, before, "{name} changes the document");

		typist.type_script("[Mod-z]");
		assert_eq!(typist.snapshot()["doc"], before, "undo {name}");
		typist.type_script("[Mod-Shift-z]");
		assert_eq!(typist.snapshot()["doc"], after, "redo {name}");
	}
}

#[test]
fn tiptap_json_tables_read_and_write_back_unchanged() {
	let typist = typist("");
	let schema = typist.editor.schema().clone();
	let cell = |kind: &str, attrs: Json, text: &str| {
		serde_json::json!({
			"type": kind,
			"attrs": attrs,
			"content": [{ "type": "paragraph", "content": [{ "type": "text", "text": text }] }],
		})
	};
	let plain = || serde_json::json!({ "colspan": 1, "rowspan": 1, "colwidth": null });
	let json = serde_json::json!({ "type": "doc", "content": [{ "type": "table", "content": [
		{ "type": "tableRow", "content": [
			cell("tableHeader", serde_json::json!({ "colspan": 2, "rowspan": 1, "colwidth": [100, 0] }), "Wide"),
			cell("tableHeader", plain(), "C"),
		] },
		{ "type": "tableRow", "content": [
			cell("tableCell", serde_json::json!({ "colspan": 1, "rowspan": 2, "colwidth": null }), "Tall"),
			cell("tableCell", plain(), "b"),
			cell("tableCell", plain(), "c"),
		] },
		{ "type": "tableRow", "content": [
			cell("tableCell", plain(), "e"),
			cell("tableCell", plain(), "f"),
		] },
	] }] });
	let loaded =
		canvist_model::Document::from_json(schema.clone(), 1, &json).expect("TipTap JSON loads");
	let mut written = loaded.to_json();
	strip_ids(&mut written);
	assert_eq!(written, json);

	let slice = canvist_editor::Slice::new(
		loaded
			.children(None)
			.iter()
			.filter_map(|&id| loaded.tree(id))
			.collect(),
	);
	assert_eq!(
		clipboard::to_html(&schema, &slice),
		"<table><tbody><tr><th colspan=\"2\" \
		 colwidth=\"100,0\"><p>Wide</p></th><th><p>C</p></th></tr><tr><td \
		 rowspan=\"2\"><p>Tall</p></td><td><p>b</p></td><td><p>c</p></td></tr><tr><td><p>e</p></\
		 td><td><p>f</p></td></tr></tbody></table>"
	);
}

fn strip_ids(node: &mut Json) {
	if let Some(attrs) = node.get_mut("attrs").and_then(Json::as_object_mut) {
		attrs.remove("id");

		if attrs.is_empty() {
			node.as_object_mut().map(|object| object.remove("attrs"));
		}
	}

	if let Some(children) = node.get_mut("content").and_then(Json::as_array_mut) {
		children.iter_mut().for_each(strip_ids);
	}
}

#[test]
fn html_tables_normalise_to_tiptaps_markup() {
	let cases = [
		(
			"<table><thead><tr><th>a</th></tr></thead><tbody><tr><td>b</td></tr></tbody></table>",
			"<table><tbody><tr><th><p>a</p></th></tr><tr><td><p>b</p></td></tr></tbody></table>",
		),
		(
			"<table><caption>Cap</caption><tr><td>a</td></tr></table>",
			"<table><tbody><tr><td><p>a</p></td></tr></tbody></table>",
		),
		(
			"<table><tr><td colspan=\"x\" rowspan=\"-1\" colwidth=\"abc\">a</td></tr></table>",
			"<table><tbody><tr><td><p>a</p></td></tr></tbody></table>",
		),
		(
			"<table><tr><td><p>a</p><ul><li>b</li></ul></td></tr></table>",
			"<table><tbody><tr><td><p>a</p><ul><li><p>b</p></li></ul></td></tr></tbody></table>",
		),
		(
			"<table><tr><td>a &amp; <strong>b</strong></td></tr></table>",
			"<table><tbody><tr><td><p>a &amp; <strong>b</strong></p></td></tr></tbody></table>",
		),
	];

	for (html, expected) in cases {
		assert_eq!(html_round_trip(html), expected, "{html}");
		assert_eq!(html_round_trip(expected), expected, "{expected} is stable");
	}
}

#[test]
fn markdown_pipe_tables_keep_alignment_escapes_and_marks() {
	for markdown in [
		"| L | C | R |\n| :--- | :---: | ---: |\n| a | b | c |",
		"| **Bold** | `code` | [link](u) |\n| --- | --- | --- |\n| a \\| b | ~~s~~ | *i* |",
		"| Only |\n| --- |",
	] {
		assert_eq!(markdown_round_trip(markdown), markdown);
	}

	// GitHub drops cells past the header's and pads short rows.
	assert_eq!(
		markdown_round_trip("| a | b |\n| --- | --- |\n| c | d | e |\n| f |"),
		"| a | b |\n| --- | --- |\n| c | d |\n| f |  |"
	);
	// A pipe inside a code span still splits the cell, as in GitHub.
	assert_eq!(
		markdown_round_trip("| a | b |\n| --- | --- |\n| `x|y` |"),
		"| a | b |\n| --- | --- |\n| \\`x | y\\` |"
	);
}

#[test]
fn markdown_writes_html_for_tables_a_pipe_table_cannot_hold() {
	for html in [
		"<table><tbody><tr><td><p>no</p></td><td><p>header</p></td></tr></tbody></table>",
		"<table><tbody><tr><th><p>h</p></th><td><p>a</p></td></tr></tbody></table>",
		"<table><tbody><tr><th><p>a</p></th></tr><tr><td><p>b</p><p>c</p></td></tr></tbody></\
		 table>",
		"<table><tbody><tr><th rowspan=\"2\"><p>a</p></th></tr><tr></tr></tbody></table>",
	] {
		let typist = typist("");
		let schema = typist.editor.schema();
		let markdown = clipboard::to_markdown(schema, &clipboard::from_html(schema, html));

		assert!(markdown.starts_with("<table>"), "{html} → {markdown}");
		assert_eq!(markdown_round_trip(&markdown), markdown, "{markdown}");
	}
}

#[test]
fn typing_over_a_selection_into_the_next_row_keeps_the_grid_full() {
	let mut typist = with_table(3, 3);
	select_cells(&mut typist, (0, 2), (2, 2));
	assert!(run(&mut typist.editor, "mergeCells", &[]));
	assert_eq!(
		cells(&typist),
		grid(&[&["#", "#", "#|1x3"], &["", ""], &["", ""]])
	);

	// From the tall cell down into the next row: deleting the selection
	// joins away the cell it ends in, and the table refills the hole left
	// of the tall cell.
	let anchor = cell_text(&typist, 0, 2);
	let head = cell_text(&typist, 1, 0);
	typist
		.editor
		.select(
			Selection::text(Point::new(anchor, 0), Point::new(head, 0)),
			&mut env(),
		)
		.expect("a valid selection");
	typist.type_script("q");

	assert_eq!(
		cells(&typist).iter().map(Vec::len).collect::<Vec<_>>(),
		[3, 2, 2]
	);
	let typist_ref = &typist;
	let doc = typist_ref.editor.state().doc();
	let table = doc.children(None)[0];
	let map = canvist_model::TableMap::new(doc.children(Some(table)).iter().map(|&row| {
		doc.children(Some(row)).iter().map(|&cell| {
			let span = |key: &str| {
				doc.block(cell)
					.and_then(|block| block.attrs().get(key))
					.and_then(Value::as_int)
					.map_or(1, |span| span as usize)
			};
			(span("colspan"), span("rowspan"))
		})
	}));
	assert!(map.is_full(), "{:?}", cells(&typist));
	support::assert_valid(typist.editor.state());
}
