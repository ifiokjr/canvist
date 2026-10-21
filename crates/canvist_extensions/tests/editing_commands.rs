//! Editing helpers beyond the StarterKit, command by command: every
//! alignment, key, and argument error of text alignment, every typography
//! rule, character limits at their edges, and placeholders, with undo and
//! TipTap JSON, HTML, and Markdown round trips.

#![cfg(feature = "full")]

mod support;

use std::sync::Arc;
use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::EditorError;
use canvist_editor::Env;
use canvist_editor::Extension;
use canvist_editor::Platform;
use canvist_extensions::Alignment;
use canvist_extensions::CharacterCount;
use canvist_extensions::Placeholder;
use canvist_extensions::StarterKit;
use canvist_extensions::TextAlign;
use canvist_extensions::Typography;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::Document;
use canvist_model::Value;
use serde_json::Value as Json;
use serde_json::json;
use support::Typist;

/// An editor with the StarterKit plus `extra` on `platform`, optionally
/// loading `json`.
fn editor(extra: Vec<Arc<dyn Extension>>, platform: Platform, json: Option<&Json>) -> Editor {
	let builder = Editor::builder()
		.extensions(StarterKit::default().extensions())
		.extensions(extra)
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

fn typist(script: &str) -> Typist {
	typist_with(canvist_extensions::extras(), script)
}

fn typist_with(extra: Vec<Arc<dyn Extension>>, script: &str) -> Typist {
	let mut typist = Typist::with(editor(extra, Platform::Other, None));
	typist.type_script(script);
	typist
}

/// A typist over a document loaded from ProseMirror `content` blocks.
fn loaded(content: &Json) -> Typist {
	let json = json!({ "type": "doc", "content": content });
	Typist::with(editor(
		canvist_extensions::extras(),
		Platform::Other,
		Some(&json),
	))
}

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
}

fn args(args: &[(&'static str, Value)]) -> Attrs {
	args.iter().cloned().collect()
}

fn run(editor: &mut Editor, name: &str, given: &[(&'static str, Value)]) -> bool {
	editor
		.run_named(name, &args(given), &mut env())
		.expect("commands run")
		.is_some()
}

fn can(editor: &mut Editor, name: &str, given: &[(&'static str, Value)]) -> bool {
	let command = editor
		.command_named(name, &args(given))
		.expect("a known command");
	editor.can(&command, &mut env()).expect("commands check")
}

fn error(editor: &mut Editor, name: &str, given: &[(&'static str, Value)]) -> EditorError {
	editor
		.run_named(name, &args(given), &mut env())
		.expect_err("the command fails")
}

fn query(editor: &Editor, name: &str) -> Value {
	editor.query(name, &Attrs::new()).expect("a known query")
}

fn doc(typist: &Typist) -> Json {
	typist.snapshot()["doc"].clone()
}

/// Every block's text, one line each.
fn text(typist: &Typist) -> String {
	let schema = typist.editor.schema();
	let slice = whole(typist);
	clipboard::to_plain_text(schema, &slice)
}

/// The whole document as a slice, copied from a fresh editor so the
/// typist's selection is untouched.
fn whole(typist: &Typist) -> canvist_editor::Slice {
	let mut copy = loaded(&typist.editor.state().doc().to_json()["content"]);
	copy.type_script("[Mod-a]");
	copy.editor.copy().expect("copy")
}

fn align(value: &str) -> [(&'static str, Value); 1] {
	[("alignment", Value::from(value))]
}

#[test]
fn every_alignment_has_a_key_on_every_platform() {
	for platform in [Platform::Other, Platform::Apple] {
		let modifier = if platform == Platform::Apple {
			"Meta"
		} else {
			"Ctrl"
		};
		let mut typist = Typist::with(editor(canvist_extensions::extras(), platform, None));
		typist.type_script("Body");

		for (key, alignment) in [
			("l", "left"),
			("e", "center"),
			("r", "right"),
			("j", "justify"),
		] {
			typist.type_script(&format!("[{modifier}-Shift-{key}]"));
			assert_eq!(
				query(&typist.editor, "textAlign"),
				Value::from(alignment),
				"{modifier}-Shift-{key}"
			);
		}
	}
}

#[test]
fn set_unset_and_toggle_answer_can_and_change_only_what_differs() {
	let mut typist = typist("Body");
	let editor = &mut typist.editor;

	for alignment in ["left", "center", "right", "justify"] {
		assert!(
			can(editor, "setTextAlign", &align(alignment)),
			"{alignment}"
		);
		assert!(run(editor, "setTextAlign", &align(alignment)));
		assert!(
			!can(editor, "setTextAlign", &align(alignment)),
			"{alignment} is already set"
		);
		assert_eq!(query(editor, "textAlign"), Value::from(alignment));
	}

	assert!(can(editor, "unsetTextAlign", &[]));
	assert!(run(editor, "unsetTextAlign", &[]));
	assert!(!can(editor, "unsetTextAlign", &[]));
	assert!(run(editor, "toggleTextAlign", &align("center")));
	assert!(
		run(editor, "toggleTextAlign", &align("right")),
		"another alignment replaces it"
	);
	assert_eq!(query(editor, "textAlign"), Value::from("right"));
}

#[test]
fn toggling_a_mixed_selection_aligns_every_block() {
	let mut typist = typist("one[Mod-Shift-e]⏎two");
	typist.type_script("[Mod-a]");
	assert!(run(&mut typist.editor, "toggleTextAlign", &align("center")));

	let blocks = doc(&typist);
	assert!(
		blocks
			.as_array()
			.into_iter()
			.flatten()
			.all(|block| block["attrs"]["textAlign"] == "center"),
		"{blocks}"
	);
}

#[test]
fn alignment_undoes_and_redoes() {
	let mut typist = typist("Body⏸[Mod-Shift-r]");
	let aligned = typist.snapshot();

	typist.type_script("[Mod-z]");
	assert_eq!(query(&typist.editor, "textAlign"), Value::Null);
	assert_eq!(text(&typist), "Body", "only the alignment is undone");
	typist.type_script("[Mod-Shift-z]");
	assert_eq!(typist.snapshot(), aligned);
}

#[test]
fn alignment_arguments_are_checked() {
	let mut typist = typist("Body");
	let editor = &mut typist.editor;

	for name in ["setTextAlign", "toggleTextAlign"] {
		for given in [
			vec![],
			vec![("alignment", Value::from("middle"))],
			vec![("alignment", Value::from("Center"))],
			vec![("alignment", Value::Int(1))],
			vec![("alignment", Value::Null)],
			vec![
				("alignment", Value::from("left")),
				("extra", Value::Bool(true)),
			],
		] {
			assert!(
				matches!(error(editor, name, &given), EditorError::InvalidArgs { .. }),
				"{name} {given:?}"
			);
		}
	}

	assert!(matches!(
		error(editor, "unsetTextAlign", &align("left")),
		EditorError::InvalidArgs { .. }
	));
	assert_eq!(query(editor, "textAlign"), Value::Null, "nothing changed");
}

#[test]
fn only_the_configured_types_align() {
	let extra: Vec<Arc<dyn Extension>> = vec![Arc::new(TextAlign {
		types: vec!["paragraph"],
	})];
	let mut typist = typist_with(extra, "# Title");

	assert!(!can(&mut typist.editor, "setTextAlign", &align("center")));
	assert_eq!(query(&typist.editor, "textAlign"), Value::Null);

	typist.type_script("⏎Body[Mod-a]");
	assert!(run(&mut typist.editor, "setTextAlign", &align("center")));
	assert_eq!(
		query(&typist.editor, "textAlign"),
		Value::from("center"),
		"the query reads the blocks that can align"
	);
	assert_eq!(doc(&typist)[0]["attrs"], json!({ "level": 1 }));
}

#[test]
fn a_code_block_selection_cannot_align() {
	let mut typist = typist("``` code");

	assert!(!can(&mut typist.editor, "setTextAlign", &align("right")));
	assert!(!run(&mut typist.editor, "toggleTextAlign", &align("right")));
	assert_eq!(query(&typist.editor, "textAlign"), Value::Null);
}

#[test]
fn the_typed_handle_matches_the_named_commands() {
	let mut builder = Editor::builder();
	StarterKit::add_all(&mut builder);
	let handle = builder.add(TextAlign::default());
	let mut typist = Typist::with(builder.build().expect("builds"));
	typist.type_script("Body");
	let editor = &mut typist.editor;

	for alignment in [
		Alignment::Left,
		Alignment::Center,
		Alignment::Right,
		Alignment::Justify,
	] {
		editor
			.run(&handle.set(alignment), &mut env())
			.expect("aligns");
		assert_eq!(handle.alignment(editor.state()), Some(alignment));
		assert_eq!(query(editor, "textAlign"), Value::from(alignment.name()));
	}

	editor
		.run(&handle.toggle(Alignment::Justify), &mut env())
		.expect("toggles");
	assert_eq!(handle.alignment(editor.state()), None);
	editor
		.run(&handle.toggle(Alignment::Left), &mut env())
		.expect("toggles");
	editor.run(&handle.unset(), &mut env()).expect("unsets");
	assert_eq!(handle.alignment(editor.state()), None);
}

#[test]
fn tiptap_json_reads_writes_and_reads_back_alignment() {
	let typist = loaded(&json!([
		{ "type": "heading", "attrs": { "level": 2, "textAlign": "center" }, "content": [{ "type": "text", "text": "Title" }] },
		{ "type": "paragraph", "attrs": { "textAlign": "justify" }, "content": [{ "type": "text", "text": "Body" }] },
		{ "type": "paragraph", "attrs": { "textAlign": null }, "content": [{ "type": "text", "text": "Plain" }] },
		{ "type": "paragraph", "attrs": { "textAlign": "left" }, "content": [{ "type": "text", "text": "Left" }] }
	]));
	let written = doc(&typist);

	assert_eq!(
		written,
		json!([
			{ "type": "heading", "attrs": { "level": 2, "textAlign": "center" }, "content": [{ "type": "text", "text": "Title" }] },
			{ "type": "paragraph", "attrs": { "textAlign": "justify" }, "content": [{ "type": "text", "text": "Body" }] },
			{ "type": "paragraph", "attrs": { "textAlign": null }, "content": [{ "type": "text", "text": "Plain" }] },
			{ "type": "paragraph", "attrs": { "textAlign": "left" }, "content": [{ "type": "text", "text": "Left" }] }
		]),
		"TipTap's null alignment stays null; an explicit left is kept"
	);
	assert_eq!(doc(&loaded(&written)), written, "JSON reads back equal");

	let schema = typist.editor.schema();
	let slice = whole(&typist);
	let html = clipboard::to_html(schema, &slice);
	assert_eq!(
		html,
		"<h2 style=\"text-align: center\">Title</h2><p style=\"text-align: \
		 justify\">Body</p><p>Plain</p><p style=\"text-align: left\">Left</p>"
	);
	assert_eq!(
		clipboard::to_html(schema, &clipboard::from_html(schema, &html)),
		html
	);
	assert_eq!(
		clipboard::to_markdown(schema, &slice),
		"## Title\n\nBody\n\nPlain\n\nLeft",
		"Markdown has no alignment"
	);
}

#[test]
fn pasted_html_keeps_only_known_alignments() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let pasted = clipboard::from_html(
		schema,
		"<p style=\"text-align: sideways\">a</p><p style=\"TEXT-ALIGN: Right\">b</p><pre \
		 style=\"text-align: center\"><code>c</code></pre>",
	);

	assert_eq!(
		clipboard::to_html(schema, &pasted),
		"<p>a</p><p style=\"text-align: right\">b</p><pre><code>c</code></pre>"
	);
}

/// Enter inside the text keeps the block's alignment on both halves. (At
/// the end of a block, Enter starts a fresh default paragraph without one,
/// where TipTap would carry the alignment over.)
#[test]
fn splitting_an_aligned_paragraph_keeps_its_alignment() {
	let typist = typist("onetwo[Mod-Shift-e]←←←⏎");

	assert_eq!(
		doc(&typist),
		json!([
			{ "type": "paragraph", "attrs": { "textAlign": "center" }, "content": [{ "type": "text", "text": "one" }] },
			{ "type": "paragraph", "attrs": { "textAlign": "center" }, "content": [{ "type": "text", "text": "two" }] }
		])
	);
}

/// Type `script` with typography on and return the text.
fn typed(script: &str) -> String {
	text(&typist_with(vec![Arc::new(Typography::default())], script))
}

#[test]
fn every_typography_rule_fires() {
	for (script, expected) in [
		("a--", "a—"),
		("a...", "a…"),
		("\"a", "“a"),
		("a\"", "a”"),
		("'a", "‘a"),
		("a'", "a’"),
		("a<-", "a←"),
		("a<->", "a↔"),
		("a->", "a→"),
		("(c)", "©"),
		("(tm)", "™"),
		("(sm)", "℠"),
		("(r)", "®"),
		("1/2 ", "½ "),
		("+/-", "±"),
		("!=", "≠"),
		("<<", "«"),
		(">>", "»"),
		("2x3 ", "2×3 "),
		("2*3,", "2×3,"),
		("x^2 ", "x² "),
		("x^3.", "x³."),
		("1/4 ", "¼ "),
		("3/4)", "¾)"),
	] {
		assert_eq!(typed(script), expected, "{script}");
	}
}

#[test]
fn typography_leaves_lookalikes_alone() {
	for script in [
		"11/2 ", "a/2 ", "2x ", "x^4 ", "(cc)", "a - b", "a. .", "x2",
	] {
		assert_eq!(typed(script), script, "{script}");
	}
}

#[test]
fn backspace_puts_back_each_kind_of_rule() {
	assert_eq!(typed("1/2 ⌫"), "1/2 ", "a bounded rule keeps its boundary");
	assert_eq!(typed("(c)⌫"), "(c)");
	assert_eq!(typed("it'⌫"), "it'");
	assert_eq!(
		typed("it's⌫"),
		"it’",
		"once more is typed, Backspace deletes normally"
	);
	assert_eq!(typed("a->⌫b"), "a->b");
}

#[test]
fn undo_reverts_a_typography_rule_with_its_keystroke() {
	let mut typist = typist_with(vec![Arc::new(Typography::default())], "a--");
	assert_eq!(text(&typist), "a—");

	typist.type_script("[Mod-z]");
	assert_eq!(
		text(&typist),
		"a-",
		"the rule and the key that fired it are one step"
	);
	typist.unwind(false);
	assert_eq!(text(&typist), "");
}

#[test]
fn typography_never_runs_on_pasted_text() {
	let mut typist = typist_with(vec![Arc::new(Typography::default())], "");
	typist.paste("a -- \"b\" (c) 1/2 ");
	assert_eq!(text(&typist), "a -- \"b\" (c) 1/2 ");
}

/// Regression: inserting several characters at once, as an IME commit or
/// a host's `insert_text` does, put the rule's match inside the typed text,
/// and replacing it failed with a reversed range.
#[test]
fn typography_applies_to_strings_typed_at_once() {
	for (before, inserted, expected) in [
		("a", " ->", "a →"),
		("", "1/2 ", "½ "),
		("x", "--", "x—"),
		("a-", "->", "a-→"),
		("it", "'s", "it's"),
	] {
		let mut typist = typist_with(vec![Arc::new(Typography::default())], before);
		typist
			.editor
			.insert_text(inserted, &mut env())
			.expect("typing never fails");
		assert_eq!(text(&typist), expected, "{before:?} + {inserted:?}");
	}
}

#[test]
fn typography_rules_turn_off_and_reword_individually() {
	assert_eq!(Typography::default().rule_names().count(), 23);

	let every: Vec<&'static str> = Typography::default().rule_names().collect();
	let none = Typography::default().without(&every);
	assert_eq!(none.rule_names().count(), 0);
	let plain = text(&typist_with(vec![Arc::new(none)], "\"a\" -- 1/2 (c) x^2 "));
	assert_eq!(plain, "\"a\" -- 1/2 (c) x^2 ");

	let reworded = Typography::default()
		.with_output("oneHalf", "1⁄2")
		.with_output("rightArrow", "⇒");
	assert_eq!(
		text(&typist_with(vec![Arc::new(reworded)], "1/2 a->")),
		"1⁄2 a⇒"
	);
}

#[test]
fn typography_skips_inline_code() {
	assert_eq!(typed("[Mod-e]a -- b"), "a -- b");
}

fn limited(limit: u32, script: &str) -> Typist {
	typist_with(
		vec![Arc::new(CharacterCount { limit: Some(limit) })],
		script,
	)
}

#[test]
fn a_limit_of_zero_refuses_every_character() {
	let mut typist = limited(0, "abc");
	assert_eq!(text(&typist), "");
	typist.paste("x");
	assert_eq!(text(&typist), "");
	typist.type_script("⏎");
	assert_eq!(
		query(&typist.editor, "characters"),
		Value::Int(0),
		"empty blocks are allowed"
	);
}

#[test]
fn a_limit_counts_pasted_blocks_and_formats() {
	let mut typist = limited(6, "ab");
	let schema = typist.editor.schema().clone();

	let blocks = clipboard::from_markdown(&schema, "# cd\n\nefg");
	assert!(
		typist
			.editor
			.paste(blocks, &mut env())
			.expect("pastes")
			.is_none(),
		"two blocks of five characters overflow"
	);
	let fits = clipboard::from_html(&schema, "<p><strong>cd</strong></p><p>ef</p>");
	assert!(
		typist
			.editor
			.paste(fits, &mut env())
			.expect("pastes")
			.is_some()
	);
	assert_eq!(query(&typist.editor, "characters"), Value::Int(6));
}

#[test]
fn replacing_text_at_the_limit_is_allowed_when_it_does_not_grow() {
	let mut typist = limited(3, "abc[Shift-ArrowLeft]");
	typist.type_script("z");
	assert_eq!(text(&typist), "abz", "one replaced by one");

	typist.type_script("[Shift-ArrowLeft]");
	typist.paste("xy");
	assert_eq!(text(&typist), "abz", "one replaced by two overflows");
}

#[test]
fn limit_arguments_are_checked() {
	let mut typist = typist("");
	let editor = &mut typist.editor;

	for given in [
		vec![],
		vec![("limit", Value::Int(-1))],
		vec![("limit", Value::Int(i64::from(u32::MAX) + 1))],
		vec![("limit", Value::from("10"))],
		vec![("limit", Value::Float(2.5))],
	] {
		assert!(
			matches!(
				error(editor, "setCharacterLimit", &given),
				EditorError::InvalidArgs { .. }
			),
			"{given:?}"
		);
	}

	assert!(run(
		editor,
		"setCharacterLimit",
		&[("limit", Value::Int(i64::from(u32::MAX)))]
	));
	assert_eq!(
		query(editor, "characterLimit"),
		Value::Int(i64::from(u32::MAX))
	);
}

#[test]
fn a_null_limit_lifts_a_configured_one() {
	let mut builder = Editor::builder();
	StarterKit::add_all(&mut builder);
	let count = builder.add(CharacterCount { limit: Some(2) });
	let mut typist = Typist::with(builder.build().expect("builds"));
	typist.type_script("abc");
	assert_eq!(count.limit(typist.editor.state()), Some(2));
	assert_eq!(count.characters(typist.editor.state()), 2);

	typist
		.editor
		.run(&count.set_limit(None), &mut env())
		.expect("lifts");
	typist.type_script("cd");
	assert_eq!(count.limit(typist.editor.state()), None);
	assert_eq!(count.characters(typist.editor.state()), 4);
	assert_eq!(count.words(typist.editor.state()), 1);
}

#[test]
fn counts_leave_out_hard_breaks_and_split_words_at_them() {
	let typist = typist("one[Shift-Enter]two three");

	assert_eq!(query(&typist.editor, "characters"), Value::Int(12));
	assert_eq!(query(&typist.editor, "words"), Value::Int(3));
}

#[test]
fn the_limit_is_not_part_of_the_document_or_its_history() {
	let mut typist = typist("ab");
	let before = doc(&typist);
	assert!(run(
		&mut typist.editor,
		"setCharacterLimit",
		&[("limit", Value::Int(2))]
	));

	assert_eq!(doc(&typist), before);
	typist.type_script("[Mod-z]");
	assert_eq!(
		query(&typist.editor, "characterLimit"),
		Value::Int(2),
		"undo changes text, not settings"
	);
}

#[test]
fn placeholders_follow_the_caret_and_not_a_selection() {
	let mut typist = typist("a⏎");
	assert_eq!(
		query(&typist.editor, "placeholder"),
		Value::from("Write something …")
	);

	typist.type_script("[Shift-ArrowLeft]");
	assert_eq!(
		query(&typist.editor, "placeholder"),
		Value::Null,
		"a range shows none"
	);

	typist.type_script("[Mod-a]");
	assert_eq!(query(&typist.editor, "placeholder"), Value::Null);
}

#[test]
fn placeholder_text_can_be_set_to_anything_but_must_be_a_string() {
	let mut typist = typist("");
	let editor = &mut typist.editor;

	for given in [
		vec![],
		vec![("placeholder", Value::Null)],
		vec![("placeholder", Value::Int(3))],
		vec![("text", Value::from("x"))],
	] {
		assert!(
			matches!(
				error(editor, "setPlaceholder", &given),
				EditorError::InvalidArgs { .. }
			),
			"{given:?}"
		);
	}

	assert!(run(
		editor,
		"setPlaceholder",
		&[("placeholder", Value::from(""))]
	));
	assert_eq!(query(editor, "placeholder"), Value::from(""));
	assert!(run(
		editor,
		"setPlaceholder",
		&[("placeholder", Value::from("Écris 👋"))]
	));
	assert_eq!(query(editor, "placeholder"), Value::from("Écris 👋"));
}

#[test]
fn every_empty_top_level_block_shows_a_placeholder_when_configured() {
	let mut builder = Editor::builder();
	StarterKit::add_all(&mut builder);
	let handle = builder.add(Placeholder {
		text: "…",
		show_only_current: false,
		include_children: false,
	});
	let mut typist = Typist::with(builder.build().expect("builds"));
	typist.type_script("⏎a⏎⏎- ");

	let blocks = handle.placeholders(typist.editor.state());
	assert_eq!(
		blocks.len(),
		3,
		"the two empty paragraphs and the trailing one, not the list item: {blocks:?}"
	);

	typist
		.editor
		.run(&handle.set("Next"), &mut env())
		.expect("sets");
	assert!(
		handle
			.placeholders(typist.editor.state())
			.iter()
			.all(|(_, text)| &**text == "Next")
	);
}

/// The limit holds back local edits only: undo restores what the history
/// holds even past a limit set since, as a remote peer's edits do.
#[test]
fn undo_may_restore_text_past_a_limit_set_since() {
	let mut typist = typist("abcdef⏸[Mod-a]⌫");
	assert_eq!(text(&typist), "");
	assert!(run(
		&mut typist.editor,
		"setCharacterLimit",
		&[("limit", Value::Int(3))]
	));

	typist.type_script("[Mod-z]");
	assert_eq!(text(&typist), "abcdef");
	typist.type_script("→g");
	assert_eq!(text(&typist), "abcdef", "but typing past it is refused");
}
