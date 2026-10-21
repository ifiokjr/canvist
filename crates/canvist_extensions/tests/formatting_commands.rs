//! Formatting beyond the StarterKit, command by command: every command and
//! query of subscript, superscript, highlight, and the text style kit, with
//! their keys, input and paste rules, undo, argument errors, and TipTap
//! JSON, HTML, and Markdown round trips.

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
use canvist_editor::EditorError;
use canvist_editor::Env;
use canvist_editor::Extension;
use canvist_editor::Platform;
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
use canvist_model::Document;
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

/// The StarterKit plus [`extras`] on `platform`, optionally loading `json`.
fn editor_on(platform: Platform, json: Option<&Json>) -> Editor {
	let builder = Editor::builder()
		.extensions(canvist_extensions::StarterKit::default().extensions())
		.extensions(extras())
		.platform(platform)
		.client(1);

	match json {
		Some(json) => {
			builder
				.build_with(|schema, client| Ok(Document::from_json(schema, client, json)?))
				.expect("the document loads")
		}
		None => builder.build().expect("the extensions build"),
	}
}

/// A typist for a document loaded from ProseMirror JSON.
fn loaded(json: &Json) -> Typist {
	Typist::with(editor_on(Platform::Other, Some(json)))
}

/// One paragraph holding `content`, as ProseMirror JSON.
fn paragraph(content: &Json) -> Json {
	json!({ "type": "doc", "content": [{ "type": "paragraph", "content": content }] })
}

fn can(editor: &mut Editor, name: &str, args: &[(&'static str, Value)]) -> bool {
	let args: Attrs = args.iter().cloned().collect();
	let command = editor.command_named(name, &args).expect("a known command");
	editor.can(&command, &mut env()).expect("commands check")
}

fn named_error(editor: &mut Editor, name: &str, args: &[(&'static str, Value)]) -> EditorError {
	let args: Attrs = args.iter().cloned().collect();
	let result = editor
		.command_named(name, &args)
		.and_then(|command| editor.run(&command, &mut env()));
	result.expect_err("the command fails")
}

/// The document's ProseMirror JSON with block ids removed.
fn doc_json(editor: &Editor) -> Json {
	support::snapshot(editor.state())["doc"].clone()
}

/// The whole document as HTML and Markdown, copied from a fresh editor so
/// the caller's selection is untouched.
fn exports(editor: &Editor) -> (String, String) {
	let mut probe = loaded(&editor.state().doc().to_json());
	probe.type_script("[Mod-a]");
	let slice = probe.editor.copy().expect("copy");
	let schema = probe.editor.schema();
	(
		clipboard::to_html(schema, &slice),
		clipboard::to_markdown(schema, &slice),
	)
}

#[test]
fn script_commands_set_unset_and_answer_can() {
	let mut typist = typist("x2[Shift-ArrowLeft]");
	let editor = &mut typist.editor;

	assert!(can(editor, "setSubscript", &[]));
	run(editor, "unsetSubscript", &[]);
	assert_eq!(
		content(&typist),
		json!([{ "type": "text", "text": "x2" }]),
		"unsetting plain text changes nothing"
	);
	let editor = &mut typist.editor;
	assert!(run(editor, "setSubscript", &[]));
	assert_eq!(query(editor, "isSubscript"), Value::Bool(true));
	assert!(can(editor, "unsetSubscript", &[]));
	assert!(run(editor, "unsetSubscript", &[]));
	assert_eq!(query(editor, "isSubscript"), Value::Bool(false));

	assert!(run(editor, "toggleSuperscript", &[]));
	assert_eq!(query(editor, "isSuperscript"), Value::Bool(true));
	assert!(run(editor, "toggleSubscript", &[]), "the other script");
	assert_eq!(query(editor, "isSuperscript"), Value::Bool(false));
	assert!(run(editor, "toggleSubscript", &[]));
	assert_eq!(query(editor, "isSubscript"), Value::Bool(false));
	assert_eq!(content(&typist), json!([{ "type": "text", "text": "x2" }]));
}

#[test]
fn script_keys_use_cmd_on_apple_platforms() {
	let mut typist = Typist::with(editor_on(Platform::Apple, None));
	typist.type_script("x[Meta-.]2[Meta-.] y[Meta-,]1");

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "x" },
			{ "type": "text", "text": "2", "marks": [{ "type": "superscript" }] },
			{ "type": "text", "text": " y" },
			{ "type": "text", "text": "1", "marks": [{ "type": "subscript" }] }
		])
	);
}

#[test]
fn script_marks_undo_and_redo() {
	let mut typist = typist("x2[Shift-ArrowLeft][Mod-.]");
	let marked = typist.snapshot();

	typist.type_script("[Mod-z]");
	assert_eq!(content(&typist), json!([{ "type": "text", "text": "x2" }]));
	typist.type_script("[Mod-Shift-z]");
	assert_eq!(typist.snapshot(), marked);
}

#[test]
fn highlight_key_toggles_the_theme_colour() {
	let mut typist = typist("word[Mod-a][Mod-Shift-h]");

	assert_eq!(
		content(&typist),
		json!([{ "type": "text", "text": "word", "marks": [{ "type": "highlight" }] }])
	);
	assert_eq!(query(&typist.editor, "highlightColor"), Value::Null);

	typist.type_script("[Mod-Shift-h]");
	assert_eq!(
		content(&typist),
		json!([{ "type": "text", "text": "word" }])
	);
}

#[test]
fn backspace_undoes_the_highlight_input_rule() {
	let typist = typist("a ==b==⌫");

	assert_eq!(
		content(&typist),
		json!([{ "type": "text", "text": "a ==b==" }]),
		"the rule is undone, its text restored"
	);
}

#[test]
fn highlight_input_rule_needs_text_between_the_delimiters() {
	for script in ["a ==== b", "a == b ==", "a==b=="] {
		let typist = typist(script);
		let text = script.to_owned();
		assert_eq!(
			content(&typist),
			json!([{ "type": "text", "text": text }]),
			"{script}"
		);
	}
}

#[test]
fn pasted_text_highlights_by_paste_rule() {
	let mut typist = typist("");
	typist.paste("see ==this== now");

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "see " },
			{ "type": "text", "text": "this", "marks": [{ "type": "highlight" }] },
			{ "type": "text", "text": " now" }
		])
	);
}

#[test]
fn highlight_at_a_caret_marks_the_next_typed_text() {
	let mut typist = typist("a ");
	assert!(run(
		&mut typist.editor,
		"setHighlight",
		&[("color", Value::from("#fef08a"))]
	));
	assert_eq!(query(&typist.editor, "isHighlight"), Value::Bool(true));
	assert_eq!(
		query(&typist.editor, "highlightColor"),
		Value::from("#fef08a")
	);
	typist.type_script("b");

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "a " },
			{ "type": "text", "text": "b", "marks": [{ "type": "highlight", "attrs": { "color": "#fef08a" } }] }
		])
	);
}

#[test]
fn a_highlight_without_a_colour_is_the_typed_highlight() {
	let typed = typist("==word==");
	let mut commanded = typist("word[Mod-a]");
	assert!(run(
		&mut commanded.editor,
		"setHighlight",
		&[("color", Value::Null)]
	));

	assert_eq!(content(&commanded), content(&typed));
	assert!(
		run(&mut commanded.editor, "toggleHighlight", &[]),
		"no colour toggles a colourless highlight off"
	);
	assert_eq!(query(&commanded.editor, "isHighlight"), Value::Bool(false));
}

#[test]
fn unset_highlight_removes_every_colour_and_answers_can() {
	let mut typist = typist("ab[Shift-ArrowLeft]");
	let editor = &mut typist.editor;
	assert!(run(
		editor,
		"setHighlight",
		&[("color", Value::from("red"))]
	));
	typist.type_script("[Mod-a]");
	let editor = &mut typist.editor;
	assert!(run(
		editor,
		"toggleHighlight",
		&[("color", Value::from("blue"))]
	));
	assert_eq!(query(editor, "highlightColor"), Value::from("blue"));
	typist.type_script("[Shift-ArrowLeft]");
	assert!(run(
		&mut typist.editor,
		"setHighlight",
		&[("color", Value::from("red"))]
	));

	typist.type_script("[Mod-a]");
	let editor = &mut typist.editor;
	assert_eq!(
		query(editor, "highlightColor"),
		Value::Null,
		"two colours share none"
	);
	assert_eq!(query(editor, "isHighlight"), Value::Bool(true));
	assert!(can(editor, "unsetHighlight", &[]));
	assert!(run(editor, "unsetHighlight", &[]));
	assert_eq!(content(&typist), json!([{ "type": "text", "text": "ab" }]));
}

/// The StarterKit plus a highlight with one colour.
fn single_colour() -> Typist {
	let editor = Editor::builder()
		.extensions(canvist_extensions::StarterKit::default().extensions())
		.extension(Highlight { multicolor: false })
		.client(1)
		.build()
		.expect("builds");
	Typist::with(editor)
}

#[test]
fn single_colour_highlights_take_no_colour() {
	let mut typist = single_colour();
	typist.type_script("word[Mod-a]");
	let editor = &mut typist.editor;

	assert!(matches!(
		named_error(editor, "setHighlight", &[("color", Value::from("red"))]),
		EditorError::InvalidArgs { .. }
	));
	assert!(matches!(
		editor.query("highlightColor", &Attrs::new()),
		Err(EditorError::UnknownName(_))
	));
	assert!(run(editor, "setHighlight", &[]));
	assert_eq!(
		content(&typist),
		json!([{ "type": "text", "text": "word", "marks": [{ "type": "highlight" }] }])
	);

	let schema = typist.editor.schema();
	let pasted = clipboard::from_html(schema, "<p><mark data-color=\"#ffc078\">hot</mark></p>");
	assert_eq!(
		clipboard::to_html(schema, &pasted),
		"<p><mark>hot</mark></p>",
		"a pasted colour is dropped"
	);
}

#[test]
fn style_commands_reject_bad_arguments_with_invalid_args() {
	let mut typist = typist("styled[Mod-a]");
	let editor = &mut typist.editor;

	for (command, attr) in [
		("setColor", "color"),
		("setBackgroundColor", "backgroundColor"),
		("setFontFamily", "fontFamily"),
		("setFontSize", "fontSize"),
		("setLineHeight", "lineHeight"),
	] {
		for args in [
			vec![],
			vec![(attr, Value::Int(18))],
			vec![(attr, Value::Null)],
			vec![(attr, Value::from("x")), ("extra", Value::from("y"))],
		] {
			assert!(
				matches!(
					named_error(editor, command, &args),
					EditorError::InvalidArgs { .. }
				),
				"{command} {args:?}"
			);
		}
	}

	assert!(matches!(
		named_error(editor, "setHighlight", &[("color", Value::Bool(true))]),
		EditorError::InvalidArgs { .. }
	));
	assert!(matches!(
		named_error(editor, "unsetColor", &[("color", Value::from("red"))]),
		EditorError::InvalidArgs { .. }
	));
	assert_eq!(
		content(&typist),
		json!([{ "type": "text", "text": "styled" }]),
		"nothing changed"
	);
}

#[test]
fn every_style_command_needs_the_text_style_mark() {
	let extensions: Vec<(Arc<dyn Extension>, &str, &str)> = vec![
		(Arc::new(Color), "setColor", "color"),
		(
			Arc::new(BackgroundColor),
			"setBackgroundColor",
			"backgroundColor",
		),
		(Arc::new(FontFamily), "setFontFamily", "fontFamily"),
		(Arc::new(FontSize), "setFontSize", "fontSize"),
		(Arc::new(LineHeight), "setLineHeight", "lineHeight"),
	];

	for (extension, command, attr) in extensions {
		let mut editor = Editor::builder()
			.extensions(canvist_extensions::StarterKit::default().extensions())
			.extensions([extension])
			.build()
			.expect("builds");

		assert!(
			matches!(
				named_error(&mut editor, command, &[(attr, Value::from("1"))]),
				EditorError::MissingType {
					name: "textStyle",
					..
				}
			),
			"{command}"
		);
		assert_eq!(
			editor.query(attr, &Attrs::new()).expect("queries answer"),
			Value::Null
		);
	}
}

#[test]
fn style_commands_undo_and_redo_one_attribute_at_a_time() {
	let mut typist = typist("styled[Mod-a]");
	let plain = typist.snapshot();
	assert!(run(
		&mut typist.editor,
		"setColor",
		&[("color", Value::from("#958df1"))]
	));
	let coloured = typist.snapshot();
	assert!(run(
		&mut typist.editor,
		"setFontFamily",
		&[("fontFamily", Value::from("Georgia"))]
	));
	let styled = typist.snapshot();

	typist.type_script("[Mod-z]");
	assert_eq!(typist.snapshot(), coloured);
	assert_eq!(query(&typist.editor, "fontFamily"), Value::Null);
	typist.type_script("[Mod-z]");
	assert_eq!(typist.snapshot(), plain);
	typist.type_script("[Mod-Shift-z][Mod-Shift-z]");
	assert_eq!(typist.snapshot(), styled);
}

#[test]
fn a_style_set_at_a_caret_styles_the_next_typed_text() {
	let mut typist = typist("a");
	assert!(run(
		&mut typist.editor,
		"setFontSize",
		&[("fontSize", Value::from("24px"))]
	));
	assert_eq!(query(&typist.editor, "fontSize"), Value::from("24px"));
	assert!(run(
		&mut typist.editor,
		"setBackgroundColor",
		&[("backgroundColor", Value::from("#000"))]
	));
	typist.type_script("b");
	assert!(run(&mut typist.editor, "unsetFontSize", &[]));
	assert!(run(&mut typist.editor, "unsetBackgroundColor", &[]));
	typist.type_script("c");

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "a" },
			{ "type": "text", "text": "b", "marks": [{ "type": "textStyle", "attrs": {
				"backgroundColor": "#000", "fontSize": "24px"
			} }] },
			{ "type": "text", "text": "c" }
		])
	);
}

#[test]
fn remove_empty_text_style_drops_styles_with_no_attributes() {
	let mut typist = loaded(&paragraph(&json!([
		{ "type": "text", "text": "empty", "marks": [{ "type": "textStyle", "attrs": { "color": null } }] },
		{ "type": "text", "text": " kept", "marks": [{ "type": "textStyle", "attrs": { "color": "red" } }] }
	])));
	typist.type_script("[Mod-a]");
	assert!(run(&mut typist.editor, "removeEmptyTextStyle", &[]));

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "empty" },
			{ "type": "text", "text": " kept", "marks": [{ "type": "textStyle", "attrs": { "color": "red" } }] }
		])
	);
}

/// TipTap's JSON for every mark in this file: highlights with and without
/// colours (TipTap writes `color: null`), and a style with every attribute.
fn tiptap_marks() -> Json {
	paragraph(&json!([
		{ "type": "text", "text": "H" },
		{ "type": "text", "text": "2", "marks": [{ "type": "subscript" }] },
		{ "type": "text", "text": "O x" },
		{ "type": "text", "text": "2", "marks": [{ "type": "superscript" }] },
		{ "type": "text", "text": " " },
		{ "type": "text", "text": "plain", "marks": [{ "type": "highlight", "attrs": { "color": null } }] },
		{ "type": "text", "text": " " },
		{ "type": "text", "text": "pink", "marks": [{ "type": "highlight", "attrs": { "color": "#fbcfe8" } }] },
		{ "type": "text", "text": " " },
		{ "type": "text", "text": "styled", "marks": [
			{ "type": "bold" },
			{ "type": "textStyle", "attrs": {
				"color": "#958df1",
				"backgroundColor": "#fafafa",
				"fontFamily": "Inter, sans-serif",
				"fontSize": "18px",
				"lineHeight": "1.5"
			} }
		] }
	]))
}

#[test]
fn tiptap_json_reads_writes_and_reads_back_every_mark() {
	let typist = loaded(&tiptap_marks());
	let written = doc_json(&typist.editor);
	let back = loaded(&json!({ "type": "doc", "content": written.clone() }));

	assert_eq!(doc_json(&back.editor), written, "JSON reads back equal");
	assert_eq!(
		query(&back.editor, "isSubscript"),
		Value::Bool(false),
		"queries answer on a loaded document"
	);

	let (html, markdown) = exports(&typist.editor);
	assert_eq!(
		html,
		"<p>H<sub>2</sub>O x<sup>2</sup> <mark>plain</mark> <mark data-color=\"#fbcfe8\" \
		 style=\"background-color: #fbcfe8; color: inherit\">pink</mark> <span style=\"color: \
		 #958df1; background-color: #fafafa; font-family: Inter, sans-serif; font-size: 18px; \
		 line-height: 1.5\"><strong>styled</strong></span></p>"
	);
	assert_eq!(
		markdown, "H<sub>2</sub>O x<sup>2</sup> ==plain pink== **styled**",
		"Markdown keeps what it can spell; a lone space between two highlights joins them, as it \
		 does between two bold runs"
	);

	let schema = typist.editor.schema();
	let from_html = clipboard::from_html(schema, &html);
	assert_eq!(
		clipboard::to_html(schema, &from_html),
		html,
		"HTML reads back equal"
	);
}

#[test]
fn styles_outside_the_selection_survive_a_partial_unset() {
	let mut typist = typist("abc[Mod-a]");
	assert!(run(
		&mut typist.editor,
		"setColor",
		&[("color", Value::from("red"))]
	));
	typist.type_script("[ArrowRight][Shift-ArrowLeft]");
	assert!(run(&mut typist.editor, "unsetColor", &[]));

	assert_eq!(
		content(&typist),
		json!([
			{ "type": "text", "text": "ab", "marks": [{ "type": "textStyle", "attrs": { "color": "red" } }] },
			{ "type": "text", "text": "c" }
		])
	);
}

#[test]
fn markdown_nests_highlights_and_scripts_with_other_marks() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let slice = clipboard::from_markdown(schema, "**==bold glow==** and *x<sup>2</sup>*");
	let html = clipboard::to_html(schema, &slice);

	assert_eq!(
		html,
		"<p><strong><mark>bold glow</mark></strong> and <em>x</em><em><sup>2</sup></em></p>"
	);
	assert_eq!(
		clipboard::to_markdown(schema, &clipboard::from_html(schema, &html)),
		"**==bold glow==** and *x<sup>2</sup>*"
	);
}

#[test]
fn unknown_marks_in_json_fail_with_a_schema_error() {
	let error = Document::from_json(
		support::editor().schema().clone(),
		1,
		&paragraph(&json!([{ "type": "text", "text": "x", "marks": [{ "type": "highlight" }] }])),
	);

	assert!(
		matches!(
			error,
			Err(canvist_model::JsonError::UnknownType { kind: "mark", ref name }) if name == "highlight"
		),
		"the StarterKit has no highlight: {error:?}"
	);
}

/// Regression: Markdown has no highlight colours, so adjacent highlights in
/// different colours were written as `==a====b==`, which reads back as one
/// highlight and so never round-tripped.
#[test]
fn adjacent_highlight_colours_write_one_markdown_span() {
	let typist = loaded(&paragraph(&json!([
		{ "type": "text", "text": "a", "marks": [{ "type": "highlight" }] },
		{ "type": "text", "text": " b", "marks": [{ "type": "highlight", "attrs": { "color": "#fef08a" } }] },
		{ "type": "text", "text": "c" }
	])));
	let (_, markdown) = exports(&typist.editor);
	let schema = typist.editor.schema();

	assert_eq!(markdown, "==a b==c");
	assert_eq!(
		clipboard::to_markdown(schema, &clipboard::from_markdown(schema, &markdown)),
		markdown
	);
}
