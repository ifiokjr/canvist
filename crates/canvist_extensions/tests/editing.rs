//! Editing helpers beyond the StarterKit: text alignment, typography,
//! character counts and limits, and placeholders.

#![cfg(feature = "full")]

mod support;

use std::sync::Arc;
use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_core::Extension;
use canvist_extensions::CharacterCount;
use canvist_extensions::Placeholder;
use canvist_extensions::StarterKit;
use canvist_extensions::TextAlign;
use canvist_extensions::Typography;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::Value;
use serde_json::Value as Json;
use serde_json::json;
use support::Typist;

fn typist(script: &str) -> Typist {
	let mut typist = Typist::with(support::editor_with(canvist_extensions::extras()));
	typist.type_script(script);
	typist
}

/// A typist over the StarterKit plus only `extra`.
fn typist_with(extra: Vec<Arc<dyn Extension>>, script: &str) -> Typist {
	let mut typist = Typist::with(support::editor_with(extra));
	typist.type_script(script);
	typist
}

fn env() -> Env<'static> {
	Env::headless(Duration::ZERO)
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

/// The text of the first block.
fn first_text(typist: &Typist) -> String {
	let content = &typist.snapshot()["doc"][0]["content"];
	content
		.as_array()
		.into_iter()
		.flatten()
		.filter_map(|node| node["text"].as_str())
		.collect()
}

fn doc(typist: &Typist) -> Json {
	typist.snapshot()["doc"].clone()
}

#[test]
fn keys_align_paragraphs_and_headings() {
	let mut typist = typist("# Title⏎Body[Mod-Shift-e]");
	assert_eq!(query(&typist.editor, "textAlign"), Value::from("center"));

	typist.type_script("[Mod-a][Mod-Shift-r]");
	assert_eq!(
		doc(&typist),
		json!([
			{ "type": "heading", "attrs": { "level": 1, "textAlign": "right" }, "content": [{ "type": "text", "text": "Title" }] },
			{ "type": "paragraph", "attrs": { "textAlign": "right" }, "content": [{ "type": "text", "text": "Body" }] },
			{ "type": "paragraph", "attrs": { "textAlign": "right" } }
		]),
		"the trailing paragraph is selected too"
	);
}

#[test]
fn toggling_an_alignment_twice_clears_it() {
	let mut typist = typist("Body");
	let justify = [("alignment", Value::from("justify"))];

	assert!(run(&mut typist.editor, "toggleTextAlign", &justify));
	assert_eq!(query(&typist.editor, "textAlign"), Value::from("justify"));
	assert!(run(&mut typist.editor, "toggleTextAlign", &justify));
	assert_eq!(query(&typist.editor, "textAlign"), Value::Null);
	assert_eq!(
		doc(&typist),
		json!([{ "type": "paragraph", "content": [{ "type": "text", "text": "Body" }] }]),
		"clearing removes the attribute"
	);
	assert!(
		!run(&mut typist.editor, "unsetTextAlign", &[]),
		"nothing to clear"
	);
}

#[test]
fn mixed_alignments_answer_null_and_code_blocks_are_left_alone() {
	let mut typist = typist("one[Mod-Shift-e]⏎two[Mod-Shift-r]⏎``` code");

	typist.type_script("[Mod-a]");
	assert_eq!(query(&typist.editor, "textAlign"), Value::Null);

	run(
		&mut typist.editor,
		"setTextAlign",
		&[("alignment", Value::from("left"))],
	);
	let blocks = doc(&typist);
	assert_eq!(blocks[0]["attrs"]["textAlign"], "left");
	assert_eq!(blocks[1]["attrs"]["textAlign"], "left");
	assert_eq!(
		blocks[2]
			.get("attrs")
			.and_then(|attrs| attrs.get("textAlign")),
		None,
		"code blocks have no alignment"
	);
}

#[test]
fn alignment_round_trips_through_html_and_is_dropped_without_the_extension() {
	let typist = typist("Centred[Mod-Shift-e][Mod-a]");
	let slice = typist.editor.copy().expect("copy");
	let html = clipboard::to_html(typist.editor.schema(), &slice);

	assert_eq!(html, "<p style=\"text-align: center\">Centred</p>");

	let back = clipboard::from_html(typist.editor.schema(), &html);
	assert_eq!(clipboard::to_html(typist.editor.schema(), &back), html);

	let plain = support::editor().schema().clone();
	let pasted = clipboard::from_html(&plain, &html);
	assert_eq!(clipboard::to_html(&plain, &pasted), "<p>Centred</p>");
}

#[test]
fn alignment_metadata_names_only_types_the_schema_has() {
	let editor = Editor::builder()
		.extensions(
			StarterKit {
				heading: None,
				..StarterKit::default()
			}
			.extensions(),
		)
		.extension(TextAlign::default())
		.build()
		.expect("builds");
	let nodes: Vec<&str> = editor
		.metadata()
		.iter()
		.filter(|extension| extension.name == "textAlign")
		.flat_map(|extension| &extension.nodes)
		.map(|node| node.name)
		.collect();

	assert_eq!(nodes, ["paragraph"]);
}

/// Type `script` with typography on and return the first block's text.
fn typed(script: &str) -> String {
	first_text(&typist_with(vec![Arc::new(Typography::default())], script))
}

#[test]
fn typography_replaces_dashes_ellipses_arrows_and_symbols() {
	assert_eq!(typed("wait -- what..."), "wait — what…");
	assert_eq!(typed("a -> b <- c <-> d"), "a → b ← c ↔ d");
	assert_eq!(typed("(c) (tm) (sm) (r)"), "© ™ ℠ ®");
	assert_eq!(typed("a != b +/- c << d >>"), "a ≠ b ± c « d »");
}

#[test]
fn typography_curls_quotes_by_context() {
	assert_eq!(typed("\"Hello,\" she said"), "“Hello,” she said");
	assert_eq!(typed("it's 'fine'"), "it’s ‘fine’");
	assert_eq!(typed("(\"quoted\")"), "(“quoted”)");
}

#[test]
fn typography_fractions_and_numbers_fire_at_the_word_boundary() {
	assert_eq!(typed("1/2 cup"), "½ cup");
	assert_eq!(typed("3/4."), "¾.");
	assert_eq!(typed("12/4 stays"), "12/4 stays");
	assert_eq!(typed("2x3 and 2 * 3 "), "2×3 and 2 × 3 ");
	assert_eq!(typed("x^2 + y^3 "), "x² + y³ ");
}

#[test]
fn backspace_after_a_typography_rule_puts_back_what_was_typed() {
	assert_eq!(typed("a--⌫"), "a--");
	assert_eq!(typed("\"⌫x"), "\"x");
}

#[test]
fn typography_rules_can_be_turned_off_or_reworded() {
	let typography = Typography::default()
		.without(&["emDash"])
		.with_output("openDoubleQuote", "„");
	let typist = typist_with(vec![Arc::new(typography)], "a -- \"b");

	assert_eq!(first_text(&typist), "a -- „b");
}

#[test]
fn typography_never_runs_in_code_blocks() {
	assert_eq!(typed("``` a -- \"b\""), "a -- \"b\"");
}

#[test]
fn typography_keeps_marks_on_replaced_text() {
	let typist = typist_with(vec![Arc::new(Typography::default())], "[Mod-b]a--b");

	assert_eq!(
		doc(&typist)[0]["content"],
		json!([{ "type": "text", "text": "a—b", "marks": [{ "type": "bold" }] }])
	);
}

#[test]
fn character_count_counts_graphemes_and_words_across_blocks() {
	let typist = typist("👍🏽 é and\u{301}⏎two words");

	// 👍🏽, a space, é, a space, a, n, d́: seven; "two words": nine.
	assert_eq!(query(&typist.editor, "characters"), Value::Int(16));
	assert_eq!(query(&typist.editor, "words"), Value::Int(5));
	assert_eq!(query(&typist.editor, "characterLimit"), Value::Null);
}

#[test]
fn a_character_limit_refuses_typing_and_pasting_past_it() {
	let extra: Vec<Arc<dyn Extension>> = vec![Arc::new(CharacterCount { limit: Some(5) })];
	let mut typist = typist_with(extra, "abcdefg");

	assert_eq!(first_text(&typist), "abcde");
	assert_eq!(query(&typist.editor, "characterLimit"), Value::Int(5));

	typist.type_script("⌫⌫");
	typist.paste("xyz");
	assert_eq!(
		first_text(&typist),
		"abc",
		"a paste that overflows is refused whole"
	);
	typist.paste("xy");
	assert_eq!(first_text(&typist), "abcxy");

	let enter = canvist_core::command(canvist_core::commands::split_block);
	assert!(
		typist.editor.can(&enter, &mut env()).expect("can"),
		"splitting adds no characters"
	);
	let type_z = canvist_core::command(|state| canvist_core::commands::insert_text(state, "z"));
	assert!(!typist.editor.can(&type_z, &mut env()).expect("can"));
}

#[test]
fn the_character_limit_changes_at_run_time() {
	let mut typist = typist("abc");

	assert!(run(
		&mut typist.editor,
		"setCharacterLimit",
		&[("limit", Value::Int(3))]
	));
	typist.type_script("d");
	assert_eq!(first_text(&typist), "abc");

	assert!(run(
		&mut typist.editor,
		"setCharacterLimit",
		&[("limit", Value::Int(1))]
	));
	typist.type_script("⌫");
	assert_eq!(
		first_text(&typist),
		"ab",
		"shrinking past a lowered limit still works"
	);

	assert!(run(
		&mut typist.editor,
		"setCharacterLimit",
		&[("limit", Value::Null)]
	));
	typist.type_script("cd");
	assert_eq!(first_text(&typist), "abcd");
	assert_eq!(query(&typist.editor, "characterLimit"), Value::Null);
}

#[test]
fn placeholders_show_in_the_caret_s_empty_block() {
	let mut typist = typist("");
	assert_eq!(
		query(&typist.editor, "placeholder"),
		Value::from("Write something …")
	);

	typist.type_script("a");
	assert_eq!(query(&typist.editor, "placeholder"), Value::Null);

	typist.type_script("⏎");
	assert!(run(
		&mut typist.editor,
		"setPlaceholder",
		&[("placeholder", Value::from("Keep going"))]
	));
	assert_eq!(
		query(&typist.editor, "placeholder"),
		Value::from("Keep going")
	);
}

#[test]
fn placeholders_can_show_in_every_empty_block_including_nested_ones() {
	let mut builder = Editor::builder();
	StarterKit::add_all(&mut builder);
	let everywhere = builder.add(Placeholder {
		text: "Type here",
		show_only_current: false,
		include_children: true,
	});
	let mut typist = Typist::with(builder.build().expect("builds"));
	typist.type_script("⏎- ⏎");

	let blocks = everywhere.placeholders(typist.editor.state());
	assert_eq!(
		blocks.len(),
		3,
		"two top-level paragraphs and the list item"
	);
	assert!(blocks.iter().all(|(_, text)| &**text == "Type here"));

	let top_level = Placeholder {
		include_children: false,
		..Placeholder::default()
	};
	let mut builder = Editor::builder();
	StarterKit::add_all(&mut builder);
	let current = builder.add(top_level);
	let mut typist = Typist::with(builder.build().expect("builds"));
	typist.type_script("- ");
	assert!(
		current.placeholders(typist.editor.state()).is_empty(),
		"the caret is in a list item"
	);
}

#[test]
fn typography_handles_multibyte_text_before_the_caret() {
	assert_eq!(
		typed("\"Wait\" -- it's 1/2 done..."),
		"“Wait” — it’s ½ done…"
	);
}
