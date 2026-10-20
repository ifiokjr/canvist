//! Block extensions beyond the StarterKit: details, callouts, and math,
//! by command, typing, and keys, and through HTML and Markdown.

#![cfg(feature = "full")]

mod support;

use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::Env;
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

fn run(editor: &mut Editor, name: &str, args: &[(&'static str, Value)]) -> bool {
	let args: Attrs = args.iter().cloned().collect();
	editor
		.run_named(name, &args, &mut Env::headless(Duration::ZERO))
		.expect("commands run")
		.is_some()
}

fn query(editor: &Editor, name: &str) -> Value {
	editor.query(name, &Attrs::new()).expect("a known query")
}

fn doc(typist: &Typist) -> Json {
	typist.snapshot()["doc"].clone()
}

/// The empty paragraph the StarterKit keeps after a trailing container.
fn trailing() -> Json {
	json!({ "type": "paragraph" })
}

fn paragraph(text: &str) -> Json {
	json!({ "type": "paragraph", "content": [{ "type": "text", "text": text }] })
}

/// HTML through the clipboard codecs and back.
fn html_round_trip(html: &str) -> String {
	let typist = typist("");
	let schema = typist.editor.schema();
	clipboard::to_html(schema, &clipboard::from_html(schema, html))
}

/// Markdown through the clipboard codecs and back.
fn markdown_round_trip(markdown: &str) -> String {
	let typist = typist("");
	let schema = typist.editor.schema();
	clipboard::to_markdown(schema, &clipboard::from_markdown(schema, markdown))
}

#[test]
fn set_details_wraps_the_block_and_moves_into_the_summary() {
	let mut typist = typist("Hidden");

	assert!(run(&mut typist.editor, "setDetails", &[]));
	typist.type_script("Spoiler");

	assert_eq!(
		doc(&typist),
		json!([{
			"type": "details",
			"attrs": { "open": true },
			"content": [
				{ "type": "detailsSummary", "content": [{ "type": "text", "text": "Spoiler" }] },
				{ "type": "detailsContent", "content": [paragraph("Hidden")] },
			],
		}, trailing()])
	);
	assert_eq!(typist.snapshot()["selection"], "caret 0.0@7");
	assert_eq!(query(&typist.editor, "detailsOpen"), Value::Bool(true));
	support::assert_valid(typist.editor.state());
}

#[test]
fn closing_details_moves_the_caret_out_of_the_hidden_content() {
	let mut typist = typist("Hidden");
	run(&mut typist.editor, "setDetails", &[]);
	typist.type_script("Spoiler↓");

	assert_eq!(typist.snapshot()["selection"], "caret 0.1.0@6");
	assert!(run(&mut typist.editor, "toggleDetailsOpen", &[]));
	assert_eq!(doc(&typist)[0]["attrs"]["open"], false);
	assert_eq!(typist.snapshot()["selection"], "caret 0.0@7");
	assert_eq!(query(&typist.editor, "detailsOpen"), Value::Bool(false));
}

#[test]
fn enter_in_a_summary_opens_the_details_and_moves_into_the_content() {
	let mut typist = typist("Hidden");
	run(&mut typist.editor, "setDetails", &[]);
	typist.type_script("Spoiler");
	run(&mut typist.editor, "toggleDetailsOpen", &[]);
	typist.type_script("⏎");

	assert_eq!(doc(&typist)[0]["attrs"]["open"], true);
	assert_eq!(typist.snapshot()["selection"], "caret 0.1.0@0");
	support::assert_valid(typist.editor.state());
}

#[test]
fn unset_details_leaves_the_summary_and_content_as_blocks() {
	let mut typist = typist("Hidden");
	run(&mut typist.editor, "setDetails", &[]);
	typist.type_script("Spoiler");

	assert!(run(&mut typist.editor, "unsetDetails", &[]));
	assert_eq!(
		doc(&typist),
		json!([paragraph("Spoiler"), paragraph("Hidden"), trailing()])
	);
	assert_eq!(query(&typist.editor, "detailsOpen"), Value::Null);
	support::assert_valid(typist.editor.state());
}

#[test]
fn toggle_callout_wraps_and_lifts_with_its_type() {
	let mut typist = typist("Careful");

	assert!(run(
		&mut typist.editor,
		"toggleCallout",
		&[("type", Value::from("warning"))]
	));
	assert_eq!(
		doc(&typist),
		json!([
			{ "type": "callout", "attrs": { "type": "warning" }, "content": [paragraph("Careful")] },
			trailing(),
		])
	);
	assert_eq!(query(&typist.editor, "calloutType"), Value::from("warning"));

	assert!(run(
		&mut typist.editor,
		"setCalloutType",
		&[("type", Value::from("error"))]
	));
	assert_eq!(query(&typist.editor, "calloutType"), Value::from("error"));

	assert!(run(&mut typist.editor, "toggleCallout", &[]));
	assert_eq!(doc(&typist), json!([paragraph("Careful"), trailing()]));
	assert_eq!(query(&typist.editor, "calloutType"), Value::Null);
}

#[test]
fn an_unknown_callout_type_is_rejected() {
	let mut typist = typist("x");
	let args: Attrs = [("type", Value::from("purple"))].into_iter().collect();

	assert!(
		typist
			.editor
			.run_named("toggleCallout", &args, &mut Env::headless(Duration::ZERO))
			.is_err()
	);
}

#[test]
fn typing_triple_colons_makes_a_callout() {
	let typist = typist(":::warning Mind the gap");

	assert_eq!(
		doc(&typist),
		json!([
			{ "type": "callout", "attrs": { "type": "warning" }, "content": [paragraph("Mind the gap")] },
			trailing(),
		])
	);

	let plain = self::typist("::: Note");
	assert_eq!(doc(&plain)[0]["attrs"]["type"], "info");
}

#[test]
fn typing_double_dollars_inserts_inline_math() {
	let typist = typist("Area $$\\pi r^2$$ ok");

	assert_eq!(
		doc(&typist),
		json!([{
			"type": "paragraph",
			"content": [
				{ "type": "text", "text": "Area " },
				{ "type": "inlineMath", "attrs": { "latex": "\\pi r^2" } },
				{ "type": "text", "text": " ok" },
			],
		}])
	);
}

#[test]
fn typing_triple_dollars_on_a_line_inserts_block_math() {
	let typist = typist("$$$E = mc^2$$$");

	assert_eq!(
		doc(&typist)[0],
		json!({ "type": "blockMath", "attrs": { "latex": "E = mc^2" } })
	);
	support::assert_valid(typist.editor.state());
}

#[test]
fn math_commands_insert_formulas() {
	let mut typist = typist("x ");

	assert!(run(
		&mut typist.editor,
		"insertInlineMath",
		&[("latex", Value::from("a+b"))]
	));
	assert_eq!(
		doc(&typist)[0]["content"][1],
		json!({ "type": "inlineMath", "attrs": { "latex": "a+b" } })
	);

	assert!(run(
		&mut typist.editor,
		"insertBlockMath",
		&[("latex", Value::from("\\int f"))]
	));
	assert_eq!(
		doc(&typist)[1],
		json!({ "type": "blockMath", "attrs": { "latex": "\\int f" } })
	);
}

#[test]
fn blocks_round_trip_through_html_as_tiptap_writes_them() {
	let cases = [
		"<details open><summary>Title</summary><div \
		 data-type=\"detailsContent\"><p>Body</p></div></details>",
		"<details><summary>Title</summary><div \
		 data-type=\"detailsContent\"><p>Body</p></div></details>",
		"<div data-callout-type=\"warning\"><p>Careful</p></div>",
		"<p>Area <span data-type=\"inline-math\" data-latex=\"\\pi r^2\">$\\pi r^2$</span></p>",
		"<div data-type=\"block-math\" data-latex=\"E = mc^2\"></div>",
	];

	for html in cases {
		assert_eq!(html_round_trip(html), html);
	}
}

#[test]
fn blocks_round_trip_through_markdown() {
	let cases = [
		"> [!WARNING]\n> Careful",
		"> [!TIP]\n> Try this",
		"Area $\\pi r^2$",
		"$$\nE = mc^2\n$$",
		"<details open>\n<summary>Title</summary>\n\nBody\n\n</details>",
		"<details>\n<summary>Title</summary>\n\nBody\n\n</details>",
	];

	for markdown in cases {
		assert_eq!(markdown_round_trip(markdown), markdown);
	}
}

#[test]
fn quotes_without_a_known_alert_stay_quotes() {
	let typist = typist("");
	let schema = typist.editor.schema();
	let slice = clipboard::from_markdown(schema, "> [!NOPE]\n> x");

	assert!(clipboard::to_html(schema, &slice).starts_with("<blockquote>"));
	assert_eq!(markdown_round_trip("> Just a quote"), "> Just a quote");
}

#[test]
fn dollar_amounts_stay_text_and_literal_dollars_are_escaped() {
	assert_eq!(
		markdown_round_trip("It costs $5 and $6"),
		"It costs \\$5 and \\$6"
	);
	assert_eq!(markdown_round_trip("\\$x$"), "\\$x\\$");
}

#[test]
fn blocks_pasted_without_their_extensions_keep_their_content() {
	let plain = support::editor().schema().clone();
	let pasted = clipboard::from_html(
		&plain,
		"<div data-callout-type=\"info\"><p>a</p></div><details><summary>s</summary><div \
		 data-type=\"detailsContent\"><p>b</p></div></details>",
	);

	assert_eq!(
		clipboard::to_html(&plain, &pasted),
		"<p>a</p><p>s</p><p>b</p>"
	);
}
