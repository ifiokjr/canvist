//! Document tools: find and replace, the table of contents, comments,
//! moving blocks, and clearing formatting.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::Env;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::Point;
use canvist_model::Selection;
use canvist_model::Value;
use serde_json::Value as Json;
use serde_json::json;
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

fn query(editor: &Editor, name: &str) -> Value {
	editor.query(name, &Attrs::new()).expect("a known query")
}

/// The plain text of every top-level block.
fn texts(typist: &Typist) -> Vec<String> {
	let doc = typist.editor.state().doc();
	doc.children(None)
		.iter()
		.map(|&id| {
			canvist_editor::position::selected_text(
				doc,
				Selection::text(
					Point::new(first_text(doc, id), 0),
					canvist_editor::position::end_of(doc, last_text(doc, id)),
				),
			)
			.iter()
			.filter_map(|(block, range)| {
				doc.inline(*block)
					.map(|text| text.as_str()[range.start as usize..range.end as usize].to_owned())
			})
			.collect::<Vec<_>>()
			.join("/")
		})
		.collect()
}

fn first_text(doc: &canvist_model::Document, id: canvist_model::BlockId) -> canvist_model::BlockId {
	let mut current = id;
	while let Some(&child) = doc.children(Some(current)).first() {
		current = child;
	}
	current
}

fn last_text(doc: &canvist_model::Document, id: canvist_model::BlockId) -> canvist_model::BlockId {
	let mut current = id;
	while let Some(&child) = doc.children(Some(current)).last() {
		current = child;
	}
	current
}

fn selected(typist: &Typist) -> String {
	let doc = typist.editor.state().doc();
	canvist_editor::position::selected_text(doc, typist.editor.state().selection())
		.iter()
		.filter_map(|(block, range)| {
			doc.inline(*block)
				.map(|text| text.as_str()[range.start as usize..range.end as usize].to_owned())
		})
		.collect()
}

fn search(typist: &mut Typist, term: &str) {
	assert!(run(
		&mut typist.editor,
		"setSearchTerm",
		&[("term", Value::from(term))]
	));
}

#[test]
fn search_counts_matches_and_steps_through_them_wrapping() {
	let mut typist = typist("The cat⏎A Cat and a cat");
	search(&mut typist, "cat");

	let results = query(&typist.editor, "searchResults");
	assert_eq!(
		results.as_map().and_then(|map| map.get("count")),
		Some(&Value::Int(3))
	);

	typist.type_script("[Mod-Home]");
	assert!(run(&mut typist.editor, "nextSearchResult", &[]));
	assert_eq!(selected(&typist), "cat");
	assert!(run(&mut typist.editor, "nextSearchResult", &[]));
	assert_eq!(selected(&typist), "Cat");
	assert!(run(&mut typist.editor, "previousSearchResult", &[]));
	assert!(run(&mut typist.editor, "previousSearchResult", &[]));
	assert_eq!(
		query(&typist.editor, "searchResults")
			.as_map()
			.and_then(|map| map.get("current").cloned()),
		Some(Value::Int(2)),
		"stepping back from the first wraps to the last"
	);

	assert!(run(
		&mut typist.editor,
		"setCaseSensitive",
		&[("caseSensitive", Value::Bool(true))]
	));
	let results = query(&typist.editor, "searchResults");
	assert_eq!(
		results.as_map().and_then(|map| map.get("count")),
		Some(&Value::Int(2))
	);

	search(&mut typist, "");
	assert_eq!(query(&typist.editor, "searchResults"), Value::Null);
}

#[test]
fn replace_swaps_the_selected_match_and_replace_all_swaps_every_one() {
	let mut typist = typist("one fish two fish⏎red fish");
	search(&mut typist, "fish");
	typist.type_script("[Mod-Home]");

	assert!(run(
		&mut typist.editor,
		"replace",
		&[("replacement", Value::from("cat"))]
	));
	assert_eq!(
		selected(&typist),
		"fish",
		"the first replace only selects a match"
	);
	assert!(run(
		&mut typist.editor,
		"replace",
		&[("replacement", Value::from("cat"))]
	));
	assert_eq!(texts(&typist), ["one cat two fish", "red fish"]);

	assert!(run(
		&mut typist.editor,
		"replaceAll",
		&[("replacement", Value::from("🐟"))]
	));
	assert_eq!(texts(&typist), ["one cat two 🐟", "red 🐟"]);
	assert!(!run(
		&mut typist.editor,
		"replaceAll",
		&[("replacement", Value::from("x"))]
	));

	typist.unwind(false);
	assert_eq!(texts(&typist), [""]);
}

#[test]
fn the_table_of_contents_numbers_headings_by_outline() {
	let mut typist = typist("# Intro⏎Text⏎### Detail⏎## Part⏎# Next");

	let toc = query(&typist.editor, "tableOfContents");
	let entries: Vec<(i64, String, String, bool)> = toc
		.as_array()
		.unwrap_or_default()
		.iter()
		.filter_map(Value::as_map)
		.map(|entry| {
			(
				entry
					.get("level")
					.and_then(Value::as_int)
					.unwrap_or_default(),
				entry
					.get("text")
					.and_then(Value::as_str)
					.unwrap_or_default()
					.to_owned(),
				entry
					.get("number")
					.and_then(Value::as_str)
					.unwrap_or_default()
					.to_owned(),
				entry
					.get("active")
					.and_then(Value::as_bool)
					.unwrap_or_default(),
			)
		})
		.collect();

	assert_eq!(
		entries,
		[
			(1, "Intro".to_owned(), "1".to_owned(), false),
			(3, "Detail".to_owned(), "1.1".to_owned(), false),
			(2, "Part".to_owned(), "1.2".to_owned(), false),
			(1, "Next".to_owned(), "2".to_owned(), true),
		]
	);

	assert!(run(
		&mut typist.editor,
		"goToHeading",
		&[("index", Value::Int(1))]
	));
	assert_eq!(typist.snapshot()["selection"], "caret 2@0");
	assert!(!run(
		&mut typist.editor,
		"goToHeading",
		&[("index", Value::Int(9))]
	));
}

#[test]
fn comments_overlap_and_unset_removes_one_thread_everywhere() {
	let mut typist = typist(
		"Shared words here[Shift-ArrowLeft][Shift-ArrowLeft][Shift-ArrowLeft][Shift-ArrowLeft]",
	);
	assert!(run(
		&mut typist.editor,
		"setComment",
		&[("threadId", Value::from("a"))]
	));
	typist.type_script("[Mod-a]");
	assert!(run(
		&mut typist.editor,
		"setComment",
		&[("threadId", Value::from("b"))]
	));

	let doc = typist.snapshot()["doc"].clone();
	let runs: Vec<(String, Json)> = doc[0]["content"]
		.as_array()
		.unwrap_or(&Vec::new())
		.iter()
		.map(|run| {
			(
				run["text"].as_str().unwrap_or_default().to_owned(),
				run["marks"][0]["attrs"]["threads"].clone(),
			)
		})
		.collect();
	assert_eq!(
		runs,
		[
			("Shared words ".to_owned(), json!(["b"])),
			("here".to_owned(), json!(["a", "b"])),
		]
	);

	let block = typist.editor.state().doc().children(None)[0];
	let end = canvist_editor::position::end_of(typist.editor.state().doc(), block);
	typist
		.editor
		.select(Selection::caret(end), &mut env())
		.expect("a caret");
	assert_eq!(
		query(&typist.editor, "activeComments"),
		Value::from(vec![Value::from("a"), Value::from("b")])
	);

	assert!(run(
		&mut typist.editor,
		"unsetComment",
		&[("threadId", Value::from("a"))]
	));
	assert_eq!(
		query(&typist.editor, "activeComments"),
		Value::from(vec![Value::from("b")])
	);
	assert!(!run(
		&mut typist.editor,
		"unsetComment",
		&[("threadId", Value::from("a"))]
	));
	assert!(
		!run(
			&mut typist.editor,
			"setComment",
			&[("threadId", Value::from("c"))]
		),
		"a caret comments nothing"
	);
}

#[test]
fn comments_round_trip_through_html() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let html = "<p>Some <span data-type=\"comment\" data-threads=\"a b\">noted</span> text</p>";

	assert_eq!(
		clipboard::to_html(schema, &clipboard::from_html(schema, html)),
		html
	);
	assert_eq!(
		clipboard::to_markdown(schema, &clipboard::from_html(schema, html)),
		"Some noted text"
	);
}

#[test]
fn blocks_move_up_and_down_and_climb_out_of_list_items() {
	let mut typist = typist("one⏎two⏎three");

	typist.type_script("[Alt-ArrowUp]");
	assert_eq!(texts(&typist), ["one", "three", "two"]);
	assert!(run(&mut typist.editor, "moveBlockDown", &[]));
	assert_eq!(texts(&typist), ["one", "two", "three"]);
	assert!(!run(&mut typist.editor, "moveBlockDown", &[]));

	assert!(run(
		&mut typist.editor,
		"moveBlocksTo",
		&[("index", Value::Int(0))]
	));
	assert_eq!(texts(&typist), ["three", "one", "two"]);

	let mut list = self::typist("- a⏎b");
	assert!(run(&mut list.editor, "moveBlockUp", &[]));
	assert_eq!(
		list.snapshot()["doc"][0]["content"][0]["content"][0]["content"][0]["text"],
		"b"
	);
}

#[test]
fn clear_formatting_removes_marks_and_unwraps_blocks() {
	let mut typist = typist("> ## **bold** heading");
	typist.type_script("[Mod-a][Mod-\\]");

	assert_eq!(
		typist.snapshot()["doc"][0],
		json!({ "type": "paragraph", "content": [{ "type": "text", "text": "bold heading" }] })
	);

	let mut list = self::typist("- *one*⏎two");
	list.type_script("[Mod-a]");
	assert!(run(&mut list.editor, "clearNodes", &[]));
	assert_eq!(list.snapshot()["doc"][0]["type"], "paragraph");
	assert!(run(&mut list.editor, "unsetAllMarks", &[]));
	assert!(
		!run(&mut list.editor, "unsetAllMarks", &[]),
		"nothing left to clear"
	);
}
