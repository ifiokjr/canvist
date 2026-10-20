//! Formatting beyond the StarterKit: subscript, superscript, highlights,
//! and text styles, typed, run as commands, and round-tripped through the
//! clipboard formats.

#![cfg(all(
	feature = "clipboard",
	feature = "highlight",
	feature = "subscript",
	feature = "superscript",
	feature = "text-style-kit"
))]

mod support;

use std::sync::Arc;
use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::Env;
use canvist_editor::Extension;
use canvist_extensions::BackgroundColor;
use canvist_extensions::Color;
use canvist_extensions::FontFamily;
use canvist_extensions::FontSize;
use canvist_extensions::Highlight;
use canvist_extensions::LineHeight;
use canvist_extensions::Subscript;
use canvist_extensions::Superscript;
use canvist_extensions::TextStyle;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::Value;
use serde_json::Value as Json;
use serde_json::json;
use support::Typist;

fn extras() -> Vec<Arc<dyn Extension>> {
	vec![
		Arc::new(Subscript),
		Arc::new(Superscript),
		Arc::new(Highlight::default()),
		Arc::new(TextStyle),
		Arc::new(Color),
		Arc::new(BackgroundColor),
		Arc::new(FontFamily),
		Arc::new(FontSize),
		Arc::new(LineHeight),
	]
}

fn typist(script: &str) -> Typist {
	let mut typist = Typist::with(support::editor_with(extras()));
	typist.type_script(script);
	typist
}

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

/// Run a named command with JSON-like arguments.
fn run(editor: &mut Editor, name: &str, args: &[(&'static str, Value)]) -> bool {
	let args: Attrs = args.iter().cloned().collect();
	let command = editor.command_named(name, &args).expect("a known command");
	editor
		.run(&command, &mut env())
		.expect("commands run")
		.is_some()
}

fn query(editor: &Editor, name: &str) -> Value {
	editor.query(name, &Attrs::new()).expect("a known query")
}

/// The first paragraph's inline content.
fn content(typist: &Typist) -> Json {
	typist.snapshot()["doc"][0]["content"].clone()
}

#[test]
fn scripts_toggle_with_their_keys_and_exclude_each_other() {
	let mut typist = typist("H[Mod-,]2[Mod-,]O x[Mod-.]n");

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "H" },
			{ "type": "text", "text": "2", "marks": [{ "type": "subscript" }] },
			{ "type": "text", "text": "O x" },
			{ "type": "text", "text": "n", "marks": [{ "type": "superscript" }] }
		])
	);

	typist.type_script("[Shift-ArrowLeft]");
	assert!(run(&mut typist.editor, "setSubscript", &[]));
	assert_eq!(query(&typist.editor, "isSubscript"), Value::Bool(true));
	assert_eq!(
		query(&typist.editor, "isSuperscript"),
		Value::Bool(false),
		"subscript replaced superscript"
	);
}

#[test]
fn double_equals_highlight_as_you_type() {
	let typist = typist("a ==bright== idea");

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "a " },
			{ "type": "text", "text": "bright", "marks": [{ "type": "highlight" }] },
			{ "type": "text", "text": " idea" }
		])
	);
}

#[test]
fn highlights_toggle_per_colour() {
	let mut typist = typist("word[Mod-a]");
	let editor = &mut typist.editor;
	let yellow = [("color", Value::from("#fef08a"))];
	let pink = [("color", Value::from("#fbcfe8"))];

	assert!(run(editor, "toggleHighlight", &yellow));
	assert_eq!(query(editor, "highlightColor"), Value::from("#fef08a"));

	assert!(
		run(editor, "toggleHighlight", &pink),
		"another colour recolours"
	);
	assert_eq!(query(editor, "highlightColor"), Value::from("#fbcfe8"));

	assert!(
		run(editor, "toggleHighlight", &pink),
		"the same colour removes"
	);
	assert_eq!(query(editor, "isHighlight"), Value::Bool(false));
}

#[test]
fn style_attributes_share_one_mark_and_clear_independently() {
	let mut typist = typist("styled[Mod-a]");
	let editor = &mut typist.editor;

	assert!(run(
		editor,
		"setColor",
		&[("color", Value::from("#958df1"))]
	));
	assert!(run(
		editor,
		"setFontSize",
		&[("fontSize", Value::from("18px"))]
	));
	assert_eq!(query(editor, "color"), Value::from("#958df1"));
	assert_eq!(query(editor, "fontSize"), Value::from("18px"));
	assert_eq!(query(editor, "fontFamily"), Value::Null);

	assert!(run(editor, "unsetColor", &[]));
	assert_eq!(query(editor, "color"), Value::Null);
	assert_eq!(query(editor, "fontSize"), Value::from("18px"), "kept");

	assert!(run(editor, "unsetFontSize", &[]));
	assert_eq!(
		content(&typist),
		json!([{ "type": "text", "text": "styled" }]),
		"a style with no attributes is removed"
	);
}

#[test]
fn style_queries_are_null_for_mixed_selections() {
	let mut typist = typist("ab[Shift-ArrowLeft]");
	assert!(run(
		&mut typist.editor,
		"setColor",
		&[("color", Value::from("red"))]
	));

	typist.type_script("[Mod-a]");
	assert_eq!(query(&typist.editor, "color"), Value::Null);
}

/// A document using every mark in this file, fully selected.
fn styled() -> Typist {
	let mut typist = typist("H[Mod-,]2[Mod-,]O, x[Mod-.]2[Mod-.] and ==marked== ink[Mod-a]");
	let editor = &mut typist.editor;
	editor
		.run(
			&editor
				.command_named(
					"setLineHeight",
					&[("lineHeight", Value::from("2"))].into_iter().collect(),
				)
				.expect("command"),
			&mut env(),
		)
		.expect("runs");
	typist
}

#[test]
fn html_round_trips_every_mark() {
	let typist = styled();
	let schema = typist.editor.schema();
	let slice = typist.editor.copy().expect("copy");
	let html = clipboard::to_html(schema, &slice);

	assert_eq!(
		html,
		"<p><span style=\"line-height: 2\">H</span><span style=\"line-height: \
		 2\"><sub>2</sub></span><span style=\"line-height: 2\">O, x</span><span \
		 style=\"line-height: 2\"><sup>2</sup></span><span style=\"line-height: 2\"> and \
		 </span><span style=\"line-height: 2\"><mark>marked</mark></span><span \
		 style=\"line-height: 2\"> ink</span></p>"
	);

	let back = clipboard::from_html(schema, &html);
	assert_eq!(clipboard::to_html(schema, &back), html);
}

#[test]
fn html_reads_tiptap_highlights_and_word_processor_styles() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let pasted = clipboard::from_html(
		schema,
		"<p><mark data-color=\"#ffc078\" style=\"background-color: #ffc078; color: \
		 inherit\">hot</mark> <span style=\"color: rgb(1, 2, 3); font-family: Georgia; \
		 background-color: transparent\">serif</span></p>",
	);

	assert_eq!(
		clipboard::to_html(schema, &pasted),
		"<p><mark data-color=\"#ffc078\" style=\"background-color: #ffc078; color: \
		 inherit\">hot</mark> <span style=\"color: rgb(1, 2, 3); font-family: \
		 Georgia\">serif</span></p>"
	);
}

#[test]
fn styled_spans_paste_plain_without_style_extensions() {
	let schema = support::editor().schema().clone();
	let pasted = clipboard::from_html(&schema, "<p><span style=\"color: red\">plain</span></p>");

	assert_eq!(clipboard::to_html(&schema, &pasted), "<p>plain</p>");
}

#[test]
fn markdown_round_trips_highlights_and_scripts() {
	let typist = styled();
	let schema = typist.editor.schema();
	let slice = typist.editor.copy().expect("copy");
	let markdown = clipboard::to_markdown(schema, &slice);

	assert_eq!(markdown, "H<sub>2</sub>O, x<sup>2</sup> and ==marked== ink");

	let back = clipboard::from_markdown(schema, &markdown);
	assert_eq!(clipboard::to_markdown(schema, &back), markdown);
}

#[test]
fn markdown_escapes_literal_highlight_and_tag_text() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let literal = clipboard::from_plain_text(schema, "a == b, ==c== and <sub>d</sub>");
	let markdown = clipboard::to_markdown(schema, &literal);

	assert_eq!(markdown, r"a \== b, \==c\== and \<sub>d\</sub>");
	assert_eq!(
		clipboard::to_plain_text(schema, &clipboard::from_markdown(schema, &markdown)),
		"a == b, ==c== and <sub>d</sub>"
	);
}
