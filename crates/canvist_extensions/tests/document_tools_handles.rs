//! Document tools through their typed handles: matches as byte ranges,
//! the history, kept marks, active headings, threads, and block moves.

#![cfg(feature = "full")]

mod support;

use canvist_extensions::clipboard;
use canvist_model::Point;
use canvist_model::Selection;
use serde_json::json;
use support::Typist;
use support::document_tools::apply;
use support::document_tools::block;
use support::document_tools::can;
use support::document_tools::env;
use support::document_tools::paragraphs;
use support::document_tools::select;
use support::document_tools::texts;
use support::document_tools::tools;
use support::document_tools::tools_on;

/// The whole document as HTML, leaving the selection as it was.
fn whole_html(typist: &mut Typist, schema: &canvist_model::Schema) -> String {
	let selection = typist.editor.state().selection();
	typist.type_script("[Mod-a]");
	let html = clipboard::to_html(schema, &typist.editor.copy().expect("a copy"));
	select(typist, selection);
	html
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
