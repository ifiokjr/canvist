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

/// The five document tools' typed handles.
struct Tools {
	find: canvist_extensions::FindAndReplaceHandle,
	toc: canvist_extensions::TableOfContentsHandle,
	comments: canvist_extensions::CommentsHandle,
	drag: canvist_extensions::DragHandleHandle,
	clear: canvist_extensions::ClearFormattingHandle,
}

/// A StarterKit editor on `platform` with the document tools, holding
/// `doc` (ProseMirror JSON) or one empty paragraph.
fn tools_on(platform: canvist_editor::Platform, doc: Option<&Json>) -> (Typist, Tools) {
	let mut builder = Editor::builder();
	canvist_extensions::StarterKit::add_all(&mut builder);
	let tools = Tools {
		find: builder.add(canvist_extensions::FindAndReplace),
		toc: builder.add(canvist_extensions::TableOfContents),
		comments: builder.add(canvist_extensions::Comments),
		drag: builder.add(canvist_extensions::DragHandle),
		clear: builder.add(canvist_extensions::ClearFormatting),
	};
	let editor = builder
		.platform(platform)
		.client(1)
		.build_with(|schema, client| {
			Ok(match doc {
				Some(json) => canvist_model::Document::from_json(schema, client, json)?,
				None => canvist_model::Document::new(schema, client),
			})
		})
		.expect("the document tools build");

	(Typist::with(editor), tools)
}

fn tools(doc: Option<&Json>) -> (Typist, Tools) {
	tools_on(canvist_editor::Platform::Other, doc)
}

/// A document of one paragraph per entry of `texts`.
fn paragraphs(texts: &[&str]) -> Json {
	let content: Vec<Json> = texts
		.iter()
		.map(|text| {
			if text.is_empty() {
				json!({ "type": "paragraph" })
			} else {
				json!({ "type": "paragraph", "content": [{ "type": "text", "text": text }] })
			}
		})
		.collect();
	json!({ "type": "doc", "content": content })
}

/// The whole document as HTML, leaving the selection as it was.
fn whole_html(typist: &mut Typist, schema: &canvist_model::Schema) -> String {
	let selection = typist.editor.state().selection();
	typist.type_script("[Mod-a]");
	let html = clipboard::to_html(schema, &typist.editor.copy().expect("a copy"));
	select(typist, selection);
	html
}

fn apply(typist: &mut Typist, command: &canvist_editor::Command) -> bool {
	typist
		.editor
		.run(command, &mut env())
		.expect("commands run")
		.is_some()
}

fn can(typist: &mut Typist, command: &canvist_editor::Command) -> bool {
	typist
		.editor
		.can(command, &mut env())
		.expect("dry runs run")
}

fn select(typist: &mut Typist, selection: Selection) {
	typist
		.editor
		.select(selection, &mut env())
		.expect("a valid selection");
}

/// The id of top-level block `index`.
fn block(typist: &Typist, index: usize) -> canvist_model::BlockId {
	typist.editor.state().doc().children(None)[index]
}

#[test]
fn the_find_handle_lists_matches_as_byte_ranges_and_tracks_the_current_one() {
	let doc = paragraphs(&["Ça va, ça va", "ÇA"]);
	let (mut typist, tools) = tools(Some(&doc));
	let first = block(&typist, 0);
	let second = block(&typist, 1);

	assert_eq!(tools.find.matches(typist.editor.state()), []);
	assert!(!can(&mut typist, &tools.find.next()), "no term, no match");
	assert!(apply(&mut typist, &tools.find.set_term("ça")));

	let found = tools.find.matches(typist.editor.state());
	assert_eq!(
		found
			.iter()
			.map(|found| (found.block, found.range.clone()))
			.collect::<Vec<_>>(),
		[(first, 0..3), (first, 8..11), (second, 0..3)]
	);
	assert_eq!(tools.find.current(typist.editor.state()), None);

	select(&mut typist, Selection::caret(Point::new(first, 5)));
	assert!(can(&mut typist, &tools.find.next()));
	assert_eq!(
		typist.editor.state().selection(),
		Selection::caret(Point::new(first, 5)),
		"a dry run changes nothing"
	);
	assert!(apply(&mut typist, &tools.find.next()));
	assert_eq!(tools.find.current(typist.editor.state()), Some(1));
	assert_eq!(typist.editor.state().selection(), found[1].selection());
	assert!(apply(&mut typist, &tools.find.previous()));
	assert_eq!(tools.find.current(typist.editor.state()), Some(0));

	assert!(apply(&mut typist, &tools.find.set_case_sensitive(true)));
	assert_eq!(tools.find.matches(typist.editor.state()).len(), 1);
	assert!(apply(&mut typist, &tools.find.replace_all("oui")));
	assert_eq!(texts(&typist), ["Ça va, oui va", "ÇA"]);
	assert!(!can(&mut typist, &tools.find.replace_all("oui")));
}

#[test]
fn matches_stay_inside_text_and_never_overlap() {
	let (mut typist, tools) = tools(None);
	typist.type_script("a⏎b a[Shift-Enter]b aaa");
	assert!(apply(&mut typist, &tools.find.set_term("ab")));
	assert!(
		tools.find.matches(typist.editor.state()).is_empty(),
		"neither a block boundary nor a hard break is part of a match"
	);

	assert!(apply(&mut typist, &tools.find.set_term("aa")));
	assert_eq!(tools.find.matches(typist.editor.state()).len(), 1);
}

#[test]
fn the_search_term_lives_outside_the_history() {
	let (mut typist, tools) = tools(Some(&paragraphs(&["fish"])));

	assert!(apply(&mut typist, &tools.find.set_term("fish")));
	assert!(!typist.editor.can_undo(), "searching is not an edit");
	assert!(apply(&mut typist, &tools.find.replace_all("cat")));
	assert_eq!(typist.unwind(false), 1, "replacing every match is one step");
	assert_eq!(texts(&typist), ["fish"]);
	assert_eq!(
		tools.find.matches(typist.editor.state()).len(),
		1,
		"undo keeps the term"
	);
}

/// Regression: a replacement took the marks before the match rather than
/// the match's own, so a bold, linked, or commented word lost them.
#[test]
fn replacing_keeps_the_marks_of_the_replaced_text() {
	let doc = json!({
		"type": "doc",
		"content": [{
			"type": "paragraph",
			"content": [
				{ "type": "text", "text": "a " },
				{ "type": "text", "text": "fish", "marks": [{ "type": "bold" }] },
				{ "type": "text", "text": " and " },
				{
					"type": "text",
					"text": "fish",
					"marks": [
						{ "type": "link", "attrs": { "href": "https://canvist.dev" } },
						{ "type": "comment", "attrs": { "threads": ["t"] } }
					]
				}
			]
		}]
	});
	let (mut typist, tools) = tools(Some(&doc));
	let schema = typist.editor.schema().clone();

	assert!(apply(&mut typist, &tools.find.set_term("fish")));
	assert!(apply(&mut typist, &tools.find.replace_all("cat")));
	let html = whole_html(&mut typist, &schema);
	assert_eq!(
		html,
		"<p>a <strong>cat</strong> and <a href=\"https://canvist.dev\"><span \
		 data-type=\"comment\" data-threads=\"t\">cat</span></a></p>"
	);

	typist.unwind(false);
	assert!(apply(&mut typist, &tools.find.next()));
	assert!(apply(&mut typist, &tools.find.replace("cat")));
	assert!(apply(&mut typist, &tools.find.replace("dog")));
	assert_eq!(
		tools.comments.threads(typist.editor.state()),
		[("t".to_owned(), "dog".to_owned())]
	);
}

/// Regression: a caret in a heading inside a quote marked the heading
/// before the quote as active.
#[test]
fn the_active_heading_is_the_one_the_selection_is_in() {
	let (mut tools_typist, tools) = tools(None);
	tools_typist.type_script("# Before⏎> ## Inside⏎⏎⏎after");

	let quote = block(&tools_typist, 1);
	let inner = tools_typist.editor.state().doc().children(Some(quote))[0];
	select(&mut tools_typist, Selection::caret(Point::new(inner, 2)));
	let active: Vec<(String, bool)> = tools
		.toc
		.entries(tools_typist.editor.state())
		.into_iter()
		.map(|entry| (entry.text, entry.active))
		.collect();
	assert_eq!(
		active,
		[("Before".to_owned(), false), ("Inside".to_owned(), true)]
	);

	let after = block(&tools_typist, 2);
	select(&mut tools_typist, Selection::caret(Point::new(after, 0)));
	assert!(tools.toc.entries(tools_typist.editor.state())[1].active);
	assert!(apply(&mut tools_typist, &tools.toc.go_to(0)));
	assert!(tools.toc.entries(tools_typist.editor.state())[0].active);
	assert!(!apply(&mut tools_typist, &tools.toc.go_to(2)));
}

/// Regression: adding a thread the selection already had reported a change.
#[test]
fn adding_a_thread_twice_changes_nothing() {
	let (mut typist, tools) = tools(Some(&paragraphs(&["one", "two"])));
	typist.type_script("[Mod-a]");

	assert!(apply(&mut typist, &tools.comments.set("t")));
	assert!(!can(&mut typist, &tools.comments.set("t")));
	assert!(!apply(&mut typist, &tools.comments.set("t")));
	assert!(apply(&mut typist, &tools.comments.set("u")));
	assert_eq!(
		tools.comments.threads(typist.editor.state()),
		[
			("t".to_owned(), "onetwo".to_owned()),
			("u".to_owned(), "onetwo".to_owned()),
		]
	);
	assert_eq!(typist.unwind(false), 2, "only real changes are undo steps");
	assert_eq!(tools.comments.threads(typist.editor.state()), []);
	assert_eq!(typist.unwind(true), 2);
	assert!(apply(&mut typist, &tools.comments.unset("t")));
	assert!(apply(&mut typist, &tools.comments.unset("u")));
	assert_eq!(
		typist.snapshot()["doc"],
		json!([
			{ "type": "paragraph", "content": [{ "type": "text", "text": "one" }] },
			{ "type": "paragraph", "content": [{ "type": "text", "text": "two" }] },
		]),
		"removing the last thread removes the mark"
	);
}

#[test]
fn json_with_an_unknown_mark_name_is_refused() {
	let (typist, _) = tools(None);
	let schema = typist.editor.schema().clone();
	let unknown = json!({
		"type": "doc",
		"content": [{
			"type": "paragraph",
			"content": [{ "type": "text", "text": "x", "marks": [{ "type": "comments" }] }]
		}]
	});

	assert!(matches!(
		canvist_model::Document::from_json(schema, 1, &unknown),
		Err(canvist_model::JsonError::UnknownType { kind: "mark", .. })
	));
}

/// Regression: clearing plain paragraphs reported a change, because the
/// caret moved block to block.
#[test]
fn clearing_plain_paragraphs_changes_nothing() {
	let (mut typist, tools) = tools(Some(&paragraphs(&["one", "two"])));
	typist.type_script("[Mod-a]");
	let before = typist.snapshot();

	assert!(!can(&mut typist, &tools.clear.clear_nodes()));
	assert!(!apply(&mut typist, &tools.clear.clear_nodes()));
	assert!(!apply(&mut typist, &tools.clear.unset_all_marks()));
	assert!(!apply(&mut typist, &tools.clear.clear()));
	assert_eq!(typist.snapshot(), before, "the selection stays too");
	assert!(!typist.editor.can_undo());
}

#[test]
fn clear_formatting_follows_the_platform_modifier() {
	let doc = json!({
		"type": "doc",
		"content": [{ "type": "heading", "attrs": { "level": 2 }, "content": [
			{ "type": "text", "text": "Title", "marks": [{ "type": "italic" }] }
		] }]
	});
	let (mut apple, _) = tools_on(canvist_editor::Platform::Apple, Some(&doc));
	apple.type_script("[Mod-a][Ctrl-\\]");
	assert_eq!(
		apple.snapshot()["doc"][0]["type"],
		"heading",
		"Ctrl is not Mod on Apple"
	);
	apple.type_script("[Meta-\\]");
	assert_eq!(
		apple.snapshot()["doc"][0],
		json!({ "type": "paragraph", "content": [{ "type": "text", "text": "Title" }] })
	);
	apple
		.editor
		.undo(&mut env())
		.expect("undo applies")
		.expect("there is a step");
	assert_eq!(
		apple.snapshot()["doc"][0]["content"][0]["marks"],
		json!([{ "type": "italic" }]),
		"marks and blocks clear as one step"
	);
	assert_eq!(apple.snapshot()["doc"][0]["type"], "heading");

	let (mut other, _) = tools_on(
		canvist_editor::Platform::Apple,
		Some(&paragraphs(&["a", "b"])),
	);
	other.type_script("[Alt-ArrowDown]");
	assert_eq!(texts(&other), ["b", "a"], "Alt is Option on Apple");
}

#[test]
fn the_drag_handle_moves_a_block_selection_and_stays_in_its_container() {
	let (mut typist, tools) = tools(None);
	typist.type_script("- a⏎b⏎c");
	let list = block(&typist, 0);
	let items = typist.editor.state().doc().children(Some(list)).to_vec();
	assert_eq!(items.len(), 3);

	select(
		&mut typist,
		Selection::Blocks {
			anchor: items[1],
			head: items[2],
		},
	);
	assert!(apply(&mut typist, &tools.drag.move_to(0)));
	assert_eq!(
		typist.editor.state().doc().children(Some(list)),
		[items[1], items[2], items[0]],
		"the selected items move together, inside their list"
	);
	assert_eq!(
		typist.editor.state().selection(),
		Selection::Blocks {
			anchor: items[1],
			head: items[2]
		},
		"the selection moves with them"
	);
	assert!(!can(&mut typist, &tools.drag.move_to(1)), "already there");
	assert!(apply(&mut typist, &tools.drag.down()));
	assert_eq!(
		typist.editor.state().doc().children(Some(list)),
		[items[0], items[1], items[2]]
	);
	assert!(
		can(&mut typist, &tools.drag.down()),
		"at the list's end, the list itself moves past the trailing paragraph"
	);
	assert!(apply(&mut typist, &tools.drag.up()));

	for _ in 0..3 {
		typist
			.editor
			.undo(&mut env())
			.expect("undo applies")
			.expect("each move is a step");
	}
	assert_eq!(
		typist.editor.state().doc().children(Some(list)),
		items.as_slice()
	);
}

#[test]
fn every_document_tool_declares_its_attributes() {
	let (typist, _) = tools(None);
	support::assert_metadata_matches_schema(&typist.editor);
}

/// Regression: a block selection, as a drag handle click or a selected
/// horizontal rule makes, moved nothing.
#[test]
fn a_selected_horizontal_rule_moves_with_alt_and_the_arrows() {
	let doc = json!({
		"type": "doc",
		"content": [
			{ "type": "paragraph", "content": [{ "type": "text", "text": "a" }] },
			{ "type": "horizontalRule" },
			{ "type": "paragraph", "content": [{ "type": "text", "text": "b" }] },
		]
	});
	let (mut typist, tools) = tools(Some(&doc));
	let rule = block(&typist, 1);
	select(
		&mut typist,
		Selection::Blocks {
			anchor: rule,
			head: rule,
		},
	);

	assert!(can(&mut typist, &tools.drag.up()));
	typist.type_script("[Alt-ArrowUp]");
	assert_eq!(block(&typist, 0), rule);
	assert_eq!(
		typist.editor.state().selection(),
		Selection::Blocks {
			anchor: rule,
			head: rule
		}
	);
	typist.type_script("[Alt-ArrowDown][Alt-ArrowDown]");
	assert_eq!(block(&typist, 2), rule);
	assert!(apply(&mut typist, &tools.drag.move_to(0)));
	assert_eq!(block(&typist, 0), rule);
}

mod properties {
	use proptest::prelude::*;

	use super::*;

	/// Words from a small alphabet, so terms often match, with case and
	/// width variations.
	fn word() -> impl Strategy<Value = String> {
		prop::collection::vec(
			prop::sample::select(vec!["a", "b", "A", "é", "É", "日", "😀", " "]),
			0..8,
		)
		.prop_map(|parts| parts.concat())
	}

	fn term() -> impl Strategy<Value = String> {
		prop::collection::vec(
			prop::sample::select(vec!["a", "b", "A", "é", "日", "😀"]),
			1..3,
		)
		.prop_map(|parts| parts.concat())
	}

	/// What a block's text becomes when every match of `term` is replaced,
	/// left to right without overlap.
	fn replaced(text: &str, term: &str, replacement: &str, exact: bool) -> String {
		if exact {
			return text.replace(term, replacement);
		}

		// Every character in the alphabet lowercases to one character of the
		// same width, so the lowercase text's byte offsets are the original's.
		let lower = text.to_lowercase();
		let wanted = term.to_lowercase();
		let mut out = String::new();
		let mut from = 0;

		for (start, found) in lower.match_indices(wanted.as_str()) {
			out.push_str(&text[from..start]);
			out.push_str(replacement);
			from = start + found.len();
		}

		out.push_str(&text[from..]);
		out
	}

	/// One drag-handle action on top-level paragraph `block`.
	#[derive(Clone, Debug)]
	enum Move {
		Up,
		Down,
		To(usize),
	}

	fn moves() -> impl Strategy<Value = Vec<(usize, Move)>> {
		prop::collection::vec(
			(
				0..6_usize,
				prop_oneof![
					Just(Move::Up),
					Just(Move::Down),
					(0..8_usize).prop_map(Move::To),
				],
			),
			1..12,
		)
	}

	/// A block of the kinds clearing formatting touches, with marked text.
	fn rich_block() -> impl Strategy<Value = Json> {
		let text = (
			"[a-z]{1,4}",
			prop::sample::subsequence(vec!["bold", "italic", "code", "strike", "underline"], 0..3),
		)
			.prop_map(|(text, marks)| {
				let marks: Vec<Json> = marks
					.into_iter()
					.map(|mark| json!({ "type": mark }))
					.collect();
				if marks.is_empty() {
					json!({ "type": "text", "text": text })
				} else {
					json!({ "type": "text", "text": text, "marks": marks })
				}
			});
		let inline = prop::collection::vec(text, 1..3);

		inline.prop_flat_map(|content| {
			let paragraph = json!({ "type": "paragraph", "content": content });
			prop_oneof![
				Just(paragraph.clone()),
				(1..4_i64).prop_map({
					let content = content.clone();
					move |level| json!({ "type": "heading", "attrs": { "level": level }, "content": content })
				}),
				Just(json!({ "type": "blockquote", "content": [paragraph.clone()] })),
				Just(json!({
					"type": "bulletList",
					"content": [{ "type": "listItem", "content": [paragraph.clone()] }]
				})),
				Just(json!({
					"type": "taskList",
					"content": [{ "type": "taskItem", "attrs": { "checked": true }, "content": [paragraph] }]
				})),
			]
		})
	}

	/// The text of every text block, in document order.
	fn text_blocks(typist: &Typist) -> Vec<String> {
		typist
			.editor
			.state()
			.doc()
			.text_blocks()
			.map(|block| {
				block
					.inline()
					.map(canvist_model::InlineText::as_str)
					.unwrap_or_default()
					.to_owned()
			})
			.collect()
	}

	proptest! {
		#![proptest_config(ProptestConfig::with_cases(96))]

		#[test]
		fn replace_all_replaces_exactly_the_matches_and_undoes_in_one_step(
			blocks in prop::collection::vec(word(), 1..4),
			term in term(),
			replacement in prop::sample::select(vec!["", "x", "😀😀", "aA"]),
			exact in any::<bool>(),
		) {
			let texts: Vec<&str> = blocks.iter().map(String::as_str).collect();
			let (mut typist, tools) = tools(Some(&paragraphs(&texts)));
			let before = typist.snapshot();
			let expected: Vec<String> = blocks
				.iter()
				.map(|text| replaced(text, &term, replacement, exact))
				.collect();

			prop_assert!(apply(&mut typist, &tools.find.set_term(&term)));
			prop_assert!(apply(&mut typist, &tools.find.set_case_sensitive(exact)));
			let found = tools.find.matches(typist.editor.state()).len();
			let applied = apply(&mut typist, &tools.find.replace_all(replacement));

			prop_assert_eq!(applied, found > 0);
			prop_assert_eq!(text_blocks(&typist), expected);
			support::assert_valid(typist.editor.state());

			if applied {
				typist.editor.undo(&mut env()).expect("undo applies");
				prop_assert_eq!(typist.snapshot(), before.clone());
				prop_assert!(!typist.editor.can_undo(), "replacing all was one step");
				typist.editor.redo(&mut env()).expect("redo applies");
				prop_assert_eq!(
					text_blocks(&typist),
					blocks
						.iter()
						.map(|text| replaced(text, &term, replacement, exact))
						.collect::<Vec<_>>()
				);
			}
		}

		#[test]
		fn block_moves_follow_a_list_model_and_undo_exactly(script in moves()) {
			let names = ["a", "b", "c", "d", "e", "f"];
			let (mut typist, tools) = tools(Some(&paragraphs(&names)));
			let before = typist.snapshot();
			let mut model: Vec<&str> = names.to_vec();
			let mut steps = 0;

			for (at, action) in script {
				let target = block(&typist, at);
				select(&mut typist, Selection::caret(Point::new(target, 0)));
				let index = model
					.iter()
					.position(|&name| name == texts(&typist)[at])
					.expect("the model has every block");
				let (command, destination) = match action {
					Move::Up => (tools.drag.up(), index.checked_sub(1)),
					Move::Down => (tools.drag.down(), (index + 1 < model.len()).then_some(index + 1)),
					Move::To(to) => {
						let to = to.min(model.len());
						let destination = if to == index || to == index + 1 {
							None
						} else if to > index {
							Some(to - 1)
						} else {
							Some(to)
						};
						(tools.drag.move_to(to), destination)
					}
				};

				prop_assert_eq!(apply(&mut typist, &command), destination.is_some());

				if let Some(destination) = destination {
					let moved = model.remove(index);
					model.insert(destination, moved);
					steps += 1;
				}

				prop_assert_eq!(texts(&typist), model.iter().map(|&name| name.to_owned()).collect::<Vec<_>>());
				prop_assert_eq!(
					typist.editor.state().selection(),
					Selection::caret(Point::new(target, 0)),
					"the caret moves with its block"
				);
			}

			for _ in 0..steps {
				typist.editor.undo(&mut env()).expect("undo applies");
			}

			prop_assert_eq!(texts(&typist), names.map(str::to_owned).to_vec());
			prop_assert_eq!(
				typist.snapshot()["doc"].clone(),
				before["doc"].clone(),
				"undo restores the order exactly"
			);
		}

		#[test]
		fn clear_formatting_keeps_the_text_and_is_idempotent(
			blocks in prop::collection::vec(rich_block(), 1..5),
		) {
			let doc = json!({ "type": "doc", "content": blocks });
			let (mut typist, tools) = tools(Some(&doc));
			let text = text_blocks(&typist);
			typist.type_script("[Mod-a]");

			apply(&mut typist, &tools.clear.clear());
			let cleared = typist.snapshot();
			support::assert_valid(typist.editor.state());
			// The trailing node may add an empty paragraph; every generated
			// block has text.
			let mut after = text_blocks(&typist);
			after.retain(|text| !text.is_empty());
			prop_assert_eq!(after, text, "the text survives in order");

			for child in cleared["doc"].as_array().into_iter().flatten() {
				prop_assert_eq!(&child["type"], "paragraph");
				prop_assert!(child.get("attrs").is_none());

				for run in child["content"].as_array().into_iter().flatten() {
					prop_assert!(run.get("marks").is_none(), "no marks left: {run}");
				}
			}

			typist.type_script("[Mod-a]");
			prop_assert!(!can(&mut typist, &tools.clear.clear_nodes()));
			prop_assert!(!can(&mut typist, &tools.clear.unset_all_marks()));
			prop_assert!(!apply(&mut typist, &tools.clear.clear()));
			prop_assert_eq!(typist.snapshot()["doc"].clone(), cleared["doc"].clone());
		}
	}
}

/// The model keeps attribute values as they come, so a `threads` value
/// that is not a list must not break the queries or later comments.
#[test]
fn a_malformed_threads_value_reads_as_no_threads_and_is_repaired_by_a_comment() {
	let wrong = json!({
		"type": "doc",
		"content": [{
			"type": "paragraph",
			"content": [{
				"type": "text",
				"text": "x",
				"marks": [{ "type": "comment", "attrs": { "threads": "a" } }]
			}]
		}]
	});
	let (mut typist, tools) = tools(Some(&wrong));

	assert_eq!(tools.comments.threads(typist.editor.state()), []);
	typist.type_script("[Mod-a]");
	assert_eq!(
		tools.comments.active(typist.editor.state()),
		Vec::<String>::new()
	);
	assert!(apply(&mut typist, &tools.comments.set("b")));
	assert_eq!(
		typist.snapshot()["doc"][0]["content"][0]["marks"],
		json!([{ "type": "comment", "attrs": { "threads": ["b"] } }])
	);
}
