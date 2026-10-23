//! Property tests for tables: random tables, merged cells and headers
//! included, round-trip exactly through TipTap JSON, HTML, and Markdown,
//! and random edits keep every table a full grid, undo to the exact
//! document, redo to the exact result, and stay stable through the formats.

#![cfg(feature = "full")]

mod support;

use std::sync::Arc;
use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_core::Extension;
use canvist_core::Slice;
use canvist_extensions::Table;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Selection;
use canvist_model::Step;
use canvist_model::TableMap;
use canvist_model::TreeContent;
use canvist_model::Value;
use proptest::prelude::*;
use serde_json::Value as Json;
use serde_json::json;
use support::Typist;

fn editor() -> Editor {
	let table: Vec<Arc<dyn Extension>> = vec![Arc::new(Table)];
	support::editor_with(table)
}

/// A table to generate: its size, which header row and column it has,
/// rectangles to merge, and words for its cells.
#[derive(Clone, Debug)]
struct Shape {
	rows: usize,
	columns: usize,
	header_row: bool,
	header_column: bool,
	merges: Vec<(usize, usize, usize, usize)>,
	words: Vec<String>,
}

fn shape() -> impl Strategy<Value = Shape> {
	(1..5usize, 1..5usize).prop_flat_map(|(rows, columns)| {
		let merge = (0..rows, 0..columns, 1..3usize, 1..3usize);
		(
			Just(rows),
			Just(columns),
			any::<bool>(),
			any::<bool>(),
			prop::collection::vec(merge, 0..3),
			prop::collection::vec("[a-z]{1,5}( [a-z]{1,5})?", rows * columns),
		)
			.prop_map(
				|(rows, columns, header_row, header_column, merges, words)| {
					Shape {
						rows,
						columns,
						header_row,
						header_column,
						merges,
						words,
					}
				},
			)
	})
}

/// The table `shape` describes as TipTap JSON: merges that fit and overlap
/// no earlier one become spanning cells, and the cells they cover go.
fn table_json(shape: &Shape) -> Json {
	// Each slot's owner: the slot that starts the cell covering it.
	let mut owner: Vec<Vec<(usize, usize)>> = (0..shape.rows)
		.map(|row| (0..shape.columns).map(|column| (row, column)).collect())
		.collect();
	let mut spans = vec![vec![(1, 1); shape.columns]; shape.rows];

	for &(top, left, height, width) in &shape.merges {
		let (bottom, right) = (top + height, left + width);
		let fits = bottom <= shape.rows && right <= shape.columns;
		let free = fits
			&& (top..bottom).all(|row| {
				(left..right).all(|column| {
					owner[row][column] == (row, column) && spans[row][column] == (1, 1)
				})
			});

		if !free || (height, width) == (1, 1) {
			continue;
		}

		for line in &mut owner[top..bottom] {
			line[left..right].fill((top, left));
		}

		spans[top][left] = (width, height);
	}

	let rows: Vec<Json> = (0..shape.rows)
		.map(|row| {
			let cells: Vec<Json> = (0..shape.columns)
				.filter(|&column| owner[row][column] == (row, column))
				.map(|column| {
					let header =
						(shape.header_row && row == 0) || (shape.header_column && column == 0);
					let (colspan, rowspan) = spans[row][column];
					json!({
						"type": if header { "tableHeader" } else { "tableCell" },
						"attrs": { "colspan": colspan, "rowspan": rowspan, "colwidth": null },
						"content": [{
							"type": "paragraph",
							"content": [{ "type": "text", "text": shape.words[row * shape.columns + column] }],
						}],
					})
				})
				.collect();
			// A row a cell above spans entirely holds no cells.
			if cells.is_empty() {
				json!({ "type": "tableRow" })
			} else {
				json!({ "type": "tableRow", "content": cells })
			}
		})
		.collect();

	json!({ "type": "table", "content": rows })
}

fn whole(doc: &Document) -> Slice {
	Slice::new(
		doc.children(None)
			.iter()
			.filter_map(|&id| doc.tree(id))
			.collect(),
	)
}

/// ProseMirror JSON for a slice the codecs read, without block ids.
fn slice_json(schema: &Arc<Schema>, slice: Slice) -> Json {
	fn fresh(doc: &mut Document, tree: BlockTree) -> BlockTree {
		let id = doc.allocate_id().expect("ids remain");
		let content = match tree.content {
			TreeContent::Blocks(children) => {
				TreeContent::Blocks(
					children
						.into_iter()
						.map(|child| fresh(doc, child))
						.collect(),
				)
			}
			content => content,
		};
		BlockTree {
			id,
			content,
			..tree
		}
	}

	let mut doc = Document::empty(schema.clone(), 1);
	let blocks: Vec<BlockTree> = slice
		.into_blocks()
		.into_iter()
		.map(|tree| fresh(&mut doc, tree))
		.collect();

	if !blocks.is_empty() {
		doc.apply_steps([Step::InsertBlocks {
			parent: None,
			index: 0,
			blocks,
		}])
		.expect("the codecs read valid blocks");
	}

	without_ids(doc.to_json())
}

fn without_ids(mut json: Json) -> Json {
	fn strip(node: &mut Json) {
		if let Some(attrs) = node.get_mut("attrs").and_then(Json::as_object_mut) {
			attrs.remove("id");

			if attrs.is_empty() {
				node.as_object_mut().map(|object| object.remove("attrs"));
			}
		}

		if let Some(children) = node.get_mut("content").and_then(Json::as_array_mut) {
			children.iter_mut().for_each(strip);
		}
	}

	strip(&mut json);
	json
}

/// `json` without cell attributes at their defaults: a span of one and no
/// column width.
fn without_defaults(mut json: Json) -> Json {
	fn strip(node: &mut Json) {
		if let Some(attrs) = node.get_mut("attrs").and_then(Json::as_object_mut) {
			attrs.retain(|key, value| {
				let one = value.as_i64() == Some(1);
				let default = match key.as_str() {
					"colspan" | "rowspan" => one,
					"colwidth" => value.is_null(),
					_ => false,
				};
				!default
			});

			if attrs.is_empty() {
				node.as_object_mut().map(|object| object.remove("attrs"));
			}
		}

		if let Some(children) = node.get_mut("content").and_then(Json::as_array_mut) {
			children.iter_mut().for_each(strip);
		}
	}

	strip(&mut json);
	json
}

/// The spans of every table in `doc`, row by row, as a `TableMap` reads
/// them.
fn maps(doc: &Document) -> Vec<TableMap> {
	let span = |cell: BlockId, key: &str| {
		doc.block(cell)
			.and_then(|block| block.attrs().get(key))
			.and_then(Value::as_int)
			.map_or(1, |span| usize::try_from(span).unwrap_or(1))
	};

	doc.iter()
		.filter(|block| &*doc.schema().block(block.kind()).name == "table")
		.map(|table| {
			TableMap::new(doc.children(Some(table.id())).iter().map(|&row| {
				doc.children(Some(row))
					.iter()
					.map(|&cell| (span(cell, "colspan"), span(cell, "rowspan")))
					.collect::<Vec<_>>()
			}))
		})
		.collect()
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(96))]

	#[test]
	fn tables_round_trip_through_every_format(shape in shape()) {
		let json = json!({ "type": "doc", "content": [table_json(&shape)] });
		let schema = editor().schema().clone();
		let doc = Document::from_json(schema.clone(), 1, &json).expect("valid TipTap JSON");

		prop_assert!(maps(&doc).iter().all(TableMap::is_full), "the generated table is a full grid");
		prop_assert_eq!(&without_ids(doc.to_json()), &json, "JSON");

		let html = clipboard::to_html(&schema, &whole(&doc));
		prop_assert_eq!(&slice_json(&schema, clipboard::from_html(&schema, &html)), &json, "HTML {}", html);

		let markdown = clipboard::to_markdown(&schema, &whole(&doc));
		prop_assert_eq!(&slice_json(&schema, clipboard::from_markdown(&schema, &markdown)), &json, "Markdown {:?}", markdown);

		// A pipe table exactly when GitHub's can hold it: a header row, no
		// other headers, and no merged cells.
		let first_row_headers = shape.header_row || (shape.header_column && shape.columns == 1);
		let other_headers = shape.header_column && shape.rows > 1;
		let merged = json.to_string().contains("span\":2");
		let pipe = first_row_headers && !other_headers && !merged;
		prop_assert_eq!(markdown.starts_with('|'), pipe, "{}", markdown);
	}
}

/// One edit: a table command, a merge from one cell to another, or keys,
/// each at cells picked by seed.
#[derive(Clone, Debug)]
enum Edit {
	Run(&'static str, (usize, usize)),
	Merge((usize, usize), (usize, usize)),
	Keys(&'static str, (usize, usize)),
}

fn edit() -> impl Strategy<Value = Edit> {
	let commands = prop::sample::select(vec![
		"addColumnBefore",
		"addColumnAfter",
		"deleteColumn",
		"addRowBefore",
		"addRowAfter",
		"deleteRow",
		"splitCell",
		"mergeOrSplit",
		"toggleHeaderRow",
		"toggleHeaderColumn",
		"toggleHeaderCell",
		"goToNextCell",
		"goToPreviousCell",
		"deleteTable",
	]);
	let keys = prop::sample::select(vec![
		"a⇥b",
		"⇤x",
		"⌫",
		"⌦",
		"y⏎z",
		"[Mod-a]⌫",
		"[Shift-ArrowDown]q",
	]);
	let cell = || (0..8usize, 0..8usize);

	prop_oneof![
		4 => (commands, cell()).prop_map(|(name, at)| Edit::Run(name, at)),
		2 => (cell(), cell()).prop_map(|(from, to)| Edit::Merge(from, to)),
		3 => (keys, cell()).prop_map(|(keys, at)| Edit::Keys(keys, at)),
	]
}

/// The text block of the cell at a seeded row and index of the first
/// table, if there is a table.
fn seeded(editor: &Editor, (row, cell): (usize, usize)) -> Option<BlockId> {
	let doc = editor.state().doc();
	let table = doc
		.iter()
		.find(|block| &*doc.schema().block(block.kind()).name == "table")?;
	let rows = doc.children(Some(table.id()));
	let row = rows[row.checked_rem(rows.len())?];
	let cells = doc.children(Some(row));
	let mut block = cells[cell.checked_rem(cells.len())?];

	while let Some(&first) = doc.children(Some(block)).first() {
		block = first;
	}

	Some(block)
}

fn select(typist: &mut Typist, anchor: BlockId, head: BlockId) {
	let selection = Selection::text(Point::new(anchor, 0), Point::new(head, 0));
	typist
		.editor
		.select(selection, &mut Env::headless(Duration::ZERO))
		.expect("a valid selection");
}

fn apply(typist: &mut Typist, edit: &Edit) {
	let mut env = Env::headless(Duration::ZERO);

	match *edit {
		Edit::Run(name, at) => {
			if let Some(block) = seeded(&typist.editor, at) {
				select(typist, block, block);
				typist
					.editor
					.run_named(name, &Attrs::new(), &mut env)
					.expect("table commands never fail");
			}
		}
		Edit::Merge(from, to) => {
			if let (Some(from), Some(to)) =
				(seeded(&typist.editor, from), seeded(&typist.editor, to))
			{
				select(typist, from, to);
				typist
					.editor
					.run_named("mergeCells", &Attrs::new(), &mut env)
					.expect("merging never fails");
			}
		}
		Edit::Keys(keys, at) => {
			if let Some(block) = seeded(&typist.editor, at) {
				select(typist, block, block);
				typist.type_script(keys);
			}
		}
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(96))]

	#[test]
	fn table_edits_redo_exactly_and_stay_stable_through_the_formats(
		rows in 1..4i64,
		cols in 1..4i64,
		header in any::<bool>(),
		edits in prop::collection::vec(edit(), 1..20),
	) {
		let mut typist = Typist::with(editor());
		let args: Attrs = [
			("rows", Value::Int(rows)),
			("cols", Value::Int(cols)),
			("withHeaderRow", Value::Bool(header)),
		]
		.into_iter()
		.collect();
		typist.editor.run_named("insertTable", &args, &mut Env::headless(Duration::ZERO)).expect("inserts");
		let start = typist.editor.state().doc().to_json();

		for edit in &edits {
			apply(&mut typist, edit);
			support::assert_valid(typist.editor.state());
			prop_assert!(maps(typist.editor.state().doc()).iter().all(TableMap::is_full), "every table stays a full grid");
		}

		let doc = typist.editor.state().doc();
		let schema = doc.schema().clone();
		// Commands leave a cell's default attributes unset where the codecs
		// write them out, so compare without them.
		let json = without_defaults(without_ids(doc.to_json()));
		let reloaded = Document::from_json(schema.clone(), 1, &doc.to_json()).expect("reloads");
		prop_assert_eq!(&without_defaults(without_ids(reloaded.to_json())), &json);

		let html = clipboard::to_html(&schema, &whole(doc));
		let read = without_defaults(slice_json(&schema, clipboard::from_html(&schema, &html)));
		prop_assert_eq!(&read, &json, "HTML {}", html);

		let markdown = clipboard::to_markdown(&schema, &whole(doc));
		let again = clipboard::to_markdown(&schema, &clipboard::from_markdown(&schema, &markdown));
		prop_assert_eq!(&clipboard::to_markdown(&schema, &clipboard::from_markdown(&schema, &again)), &again);

		let last = typist.editor.state().doc().to_json();
		let undone = typist.unwind(false);
		prop_assert!(undone > 0);
		// Undo stops at the inserted table, which joined the history first.
		let first = typist.editor.state().doc().to_json();
		prop_assert!(first == start || maps(typist.editor.state().doc()).is_empty());
		let redone = typist.unwind(true);
		prop_assert_eq!(redone, undone, "redo replays every step");
		prop_assert_eq!(typist.editor.state().doc().to_json(), last);
	}
}
