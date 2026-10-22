//! Block extensions beyond the StarterKit: details, callouts, and math,
//! by command, typing, and keys, and through HTML and Markdown.

#![cfg(feature = "full")]

mod support;

use std::sync::Arc;
use std::time::Duration;

use canvist_core::Editor;
use canvist_core::EditorError;
use canvist_core::Env;
use canvist_core::Extension;
use canvist_extensions::Callout;
use canvist_extensions::Details;
use canvist_extensions::Mathematics;
use canvist_extensions::clipboard;
use canvist_model::Attrs;
use canvist_model::Value;
use serde_json::Value as Json;
use serde_json::json;
use support::Typist;

/// A StarterKit editor with the three block extensions, typed into.
fn typist(script: &str) -> Typist {
	let blocks: Vec<Arc<dyn Extension>> =
		vec![Arc::new(Details), Arc::new(Callout), Arc::new(Mathematics)];
	let mut typist = Typist::with(support::editor_with(blocks));
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

/// Named command arguments.
type Args = Vec<(&'static str, Value)>;

/// The whole document as Markdown.
fn markdown_of(typist: &Typist) -> String {
	let doc = typist.editor.state().doc();
	let blocks = doc
		.children(None)
		.iter()
		.filter_map(|&id| doc.tree(id))
		.collect();
	clipboard::to_markdown(doc.schema(), &canvist_core::Slice::new(blocks))
}

/// Run `name` an hour in, after anything typed, so it never joins the
/// typing's undo group.
fn run_later(editor: &mut Editor, name: &str, args: &[(&'static str, Value)]) -> bool {
	let args: Attrs = args.iter().cloned().collect();
	editor
		.run_named(name, &args, &mut Env::headless(Duration::from_secs(3600)))
		.expect("commands run")
		.is_some()
}

fn can(editor: &mut Editor, name: &str, args: &[(&'static str, Value)]) -> bool {
	let args: Attrs = args.iter().cloned().collect();
	let command = editor.command_named(name, &args).expect("a known command");
	editor
		.can(&command, &mut Env::headless(Duration::ZERO))
		.expect("checking never fails")
}

fn run_error(editor: &mut Editor, name: &str, args: &[(&'static str, Value)]) -> EditorError {
	let args: Attrs = args.iter().cloned().collect();
	let before = editor.state().doc().to_json();
	let error = editor
		.run_named(name, &args, &mut Env::headless(Duration::ZERO))
		.expect_err("the command is refused");
	assert_eq!(editor.state().doc().to_json(), before, "nothing changes");
	error
}

/// A details holding `summary` and paragraphs of `content`.
fn details(open: bool, summary: &str, content: &[&str]) -> Json {
	let summary = if summary.is_empty() {
		json!({ "type": "detailsSummary" })
	} else {
		json!({ "type": "detailsSummary", "content": [{ "type": "text", "text": summary }] })
	};
	json!({
		"type": "details",
		"attrs": { "open": open },
		"content": [
			summary,
			{ "type": "detailsContent", "content": content.iter().map(|text| paragraph(text)).collect::<Vec<_>>() },
		],
	})
}

/// Every block extension's command, whether it applies outside and inside
/// its block.
#[test]
fn can_reports_whether_each_command_applies_without_changing_anything() {
	let mut typist = typist("Plain");
	let editor = &mut typist.editor;
	let before = editor.state().doc().to_json();
	let latex = [("latex", Value::from("x"))];
	let info = [("type", Value::from("info"))];

	assert!(can(editor, "setDetails", &[]));
	assert!(!can(editor, "unsetDetails", &[]));
	assert!(!can(editor, "toggleDetailsOpen", &[]));
	assert!(can(editor, "toggleCallout", &[]));
	assert!(can(editor, "toggleCallout", &info));
	assert!(!can(editor, "setCalloutType", &info));
	assert!(can(editor, "insertInlineMath", &latex));
	assert!(can(editor, "insertBlockMath", &latex));
	assert_eq!(editor.state().doc().to_json(), before);

	assert!(run(editor, "setDetails", &[]));
	assert!(can(editor, "unsetDetails", &[]));
	assert!(can(editor, "toggleDetailsOpen", &[]));

	// A summary holds text, never a callout or a block formula.
	assert!(!can(editor, "toggleCallout", &[]));
	assert!(!run(editor, "toggleCallout", &[]));
	assert!(can(editor, "insertInlineMath", &latex));

	typist.type_script("↓");
	let editor = &mut typist.editor;
	assert!(run(editor, "toggleCallout", &[]));
	// Setting the type it already has changes nothing.
	assert!(!can(editor, "setCalloutType", &info));
	assert!(can(
		editor,
		"setCalloutType",
		&[("type", Value::from("warning"))]
	));
	assert_eq!(query(editor, "calloutType"), Value::from("info"));
	assert_eq!(query(editor, "detailsOpen"), Value::Bool(true));
	support::assert_valid(editor.state());
}

#[test]
fn every_callout_type_wraps_and_reads_back() {
	for kind in ["info", "warning", "error", "success", "blank"] {
		let mut typist = typist("Note");
		assert!(run(
			&mut typist.editor,
			"toggleCallout",
			&[("type", Value::from(kind))]
		));
		assert_eq!(query(&typist.editor, "calloutType"), Value::from(kind));
		assert_eq!(doc(&typist)[0]["attrs"]["type"], kind);
	}
}

#[test]
fn set_details_and_toggle_callout_wrap_every_selected_block() {
	let mut typist = typist("a⏎b[Mod-a]");

	assert!(run(&mut typist.editor, "setDetails", &[]));
	assert_eq!(
		doc(&typist)[0]["content"][1]["content"]
			.as_array()
			.map(Vec::len),
		Some(2)
	);
	assert_eq!(typist.snapshot()["selection"], "caret 0.0@0");

	let mut typist = self::typist("a⏎b[Mod-a]");
	assert!(run(
		&mut typist.editor,
		"toggleCallout",
		&[("type", Value::from("success"))]
	));
	assert_eq!(markdown_of(&typist), "> [!TIP]\n> a\n>\n> b");
	support::assert_valid(typist.editor.state());
}

#[test]
fn details_nest_inside_lists_and_callouts() {
	let mut typist = typist("- item");
	assert!(run(&mut typist.editor, "setDetails", &[]));
	assert_eq!(
		doc(&typist)[0]["content"][0]["content"][0]["type"],
		"details"
	);
	support::assert_valid(typist.editor.state());

	let mut typist = self::typist(":::warning x");
	assert!(run(&mut typist.editor, "setDetails", &[]));
	assert_eq!(doc(&typist)[0]["content"][0]["type"], "details");
	assert_eq!(query(&typist.editor, "calloutType"), Value::from("warning"));
	assert_eq!(query(&typist.editor, "detailsOpen"), Value::Bool(true));
	support::assert_valid(typist.editor.state());
}

#[test]
fn unsetting_details_with_an_empty_summary_keeps_only_the_content() {
	let mut typist = typist("Hidden");
	run(&mut typist.editor, "setDetails", &[]);

	assert!(run(&mut typist.editor, "unsetDetails", &[]));
	assert_eq!(doc(&typist), json!([paragraph("Hidden"), trailing()]));
	assert_eq!(typist.snapshot()["selection"], "caret 0@0");
}

#[test]
fn closing_details_from_the_summary_keeps_the_caret_there() {
	let mut typist = typist("Hidden");
	run(&mut typist.editor, "setDetails", &[]);
	typist.type_script("Sum←");

	assert!(run(&mut typist.editor, "toggleDetailsOpen", &[]));
	assert_eq!(typist.snapshot()["selection"], "caret 0.0@2");
	assert!(run(&mut typist.editor, "toggleDetailsOpen", &[]));
	assert_eq!(query(&typist.editor, "detailsOpen"), Value::Bool(true));
}

#[test]
fn enter_anywhere_in_a_summary_moves_into_the_content_without_splitting_it() {
	let mut typist = typist("Hidden");
	run(&mut typist.editor, "setDetails", &[]);
	typist.type_script("Sum←←⏎");

	assert_eq!(doc(&typist)[0], details(true, "Sum", &["Hidden"]));
	assert_eq!(typist.snapshot()["selection"], "caret 0.1.0@0");
}

#[test]
fn enter_in_a_summary_without_content_adds_a_paragraph() {
	let mut typist = Typist::with(support::editor_with(canvist_extensions::extras()));
	let editor = &mut typist.editor;
	let schema = editor.schema().clone();
	let pasted = clipboard::from_html(
		&schema,
		"<details><summary>S</summary><div data-type=\"detailsContent\"><p>x</p></div></details>",
	);
	editor
		.paste(pasted, &mut Env::headless(Duration::ZERO))
		.expect("paste");
	typist.type_script("[Mod-Home]");
	run(&mut typist.editor, "toggleDetailsOpen", &[]);
	typist.type_script("[End]⏎");

	assert_eq!(doc(&typist)[0]["attrs"]["open"], true);
	assert_eq!(typist.snapshot()["selection"], "caret 0.1.0@0");
	support::assert_valid(typist.editor.state());
}

#[test]
fn backspace_at_the_start_of_a_summary_unwraps_the_details() {
	let mut typist = typist("Before⏎Body");
	run(&mut typist.editor, "setDetails", &[]);
	typist.type_script("Sum[Home]⌫");

	assert_eq!(
		doc(&typist),
		json!([
			paragraph("Before"),
			paragraph("Sum"),
			paragraph("Body"),
			trailing()
		])
	);
	assert_eq!(typist.snapshot()["selection"], "caret 1@0");
	support::assert_valid(typist.editor.state());
}

#[test]
fn backspace_at_the_start_of_single_block_content_moves_to_the_summary() {
	let mut typist = typist("Body");
	run(&mut typist.editor, "setDetails", &[]);
	typist.type_script("Sum↓[Home]⌫");

	assert_eq!(doc(&typist)[0], details(true, "Sum", &["Body"]));
	assert_eq!(typist.snapshot()["selection"], "caret 0.0@3");

	// With more blocks, the first joins the summary and the rest stay.
	let mut typist = self::typist("a⏎b[Mod-a]");
	run(&mut typist.editor, "setDetails", &[]);
	typist.type_script("S↓[Home]⌫");
	assert_eq!(doc(&typist)[0], details(true, "Sa", &["b"]));
	support::assert_valid(typist.editor.state());
}

#[test]
fn delete_never_joins_a_summary_or_its_content_away() {
	let mut typist = typist("Body");
	run(&mut typist.editor, "setDetails", &[]);
	typist.type_script("Sum⌦");

	assert_eq!(doc(&typist)[0], details(true, "Sum", &["Body"]));
	assert_eq!(typist.snapshot()["selection"], "caret 0.1.0@0");

	let mut typist = self::typist("Before⏎Body");
	run(&mut typist.editor, "setDetails", &[]);
	typist.type_script("Sum↑[End]⌦");
	assert_eq!(typist.snapshot()["selection"], "blocks 1 → 1");
	typist.type_script("⌦");
	assert_eq!(doc(&typist), json!([paragraph("Before"), trailing()]));
	support::assert_valid(typist.editor.state());
}

#[test]
fn enter_on_an_empty_line_leaves_a_callout_and_backspace_lifts_out_of_it() {
	let typist = typist(":::note Hi⏎⏎After");

	assert_eq!(doc(&typist)[0]["content"], json!([paragraph("Hi")]));
	assert_eq!(doc(&typist)[1]["content"][0]["text"], "After");

	let typist = self::typist(":::note Hi[Home]⌫");
	assert_eq!(doc(&typist), json!([paragraph("Hi"), trailing()]));
}

#[test]
fn backspace_undoes_each_input_rule() {
	for (script, text) in [
		(":::warning ", ":::warning "),
		("a $$x$$", "a $$x$$"),
		("$$$x$$$", "$$$x$$$"),
	] {
		let mut typist = typist(script);
		assert_ne!(doc(&typist)[0], paragraph(text), "{script} applies a rule");
		typist.type_script("⌫");
		assert_eq!(doc(&typist), json!([paragraph(text)]), "{script}");
	}
}

#[test]
fn callout_input_rules_read_the_type_case_insensitively_and_only_at_the_start() {
	assert_eq!(doc(&typist(":::Error x"))[0]["attrs"]["type"], "error");
	assert_eq!(doc(&typist(":::bogus x"))[0]["attrs"]["type"], "info");
	assert_eq!(
		doc(&typist("a :::warning x")),
		json!([paragraph("a :::warning x")])
	);
}

#[test]
fn math_input_rules_need_a_formula_between_the_dollars() {
	assert_eq!(doc(&typist("$$$$")), json!([paragraph("$$$$")]));
	assert_eq!(doc(&typist("costs $5")), json!([paragraph("costs $5")]));
	assert_eq!(
		doc(&typist("x $$a$$ and $$b$$"))[0]["content"],
		json!([
			{ "type": "text", "text": "x " },
			{ "type": "inlineMath", "attrs": { "latex": "a" } },
			{ "type": "text", "text": " and " },
			{ "type": "inlineMath", "attrs": { "latex": "b" } },
		])
	);
}

#[test]
fn inline_math_is_one_character_to_backspace() {
	let mut typist = typist("ab←");
	run(
		&mut typist.editor,
		"insertInlineMath",
		&[("latex", Value::from("y"))],
	);
	typist.type_script("⌫");

	assert_eq!(doc(&typist), json!([paragraph("ab")]));
}

#[test]
fn block_math_mid_paragraph_splits_it() {
	let mut typist = typist("ab←");
	run(
		&mut typist.editor,
		"insertBlockMath",
		&[("latex", Value::from("y"))],
	);

	assert_eq!(
		doc(&typist)[1],
		json!({ "type": "blockMath", "attrs": { "latex": "y" } })
	);
	support::assert_valid(typist.editor.state());
}

#[test]
fn each_command_undoes_to_the_exact_document_and_redoes() {
	let cases: [(&str, &str, Args); 7] = [
		("Hidden", "setDetails", vec![]),
		(
			"Careful",
			"toggleCallout",
			vec![("type", Value::from("warning"))],
		),
		(
			":::warning x",
			"setCalloutType",
			vec![("type", Value::from("error"))],
		),
		(":::warning x", "toggleCallout", vec![]),
		(
			"x ",
			"insertInlineMath",
			vec![("latex", Value::from("a+b"))],
		),
		(
			"x",
			"insertBlockMath",
			vec![("latex", Value::from("\\int f"))],
		),
		("x[Mod-a]", "toggleCallout", vec![]),
	];

	for (script, name, args) in cases {
		let mut typist = typist(script);
		let before = typist.snapshot();
		assert!(run_later(&mut typist.editor, name, &args), "{name} applies");
		let after = typist.snapshot();

		typist.type_script("⏸[Mod-z]");
		assert_eq!(typist.snapshot()["doc"], before["doc"], "undo {name}");
		typist.type_script("[Mod-Shift-z]");
		assert_eq!(typist.snapshot()["doc"], after["doc"], "redo {name}");
	}

	let mut typist = typist("Hidden");
	run_later(&mut typist.editor, "setDetails", &[]);
	let before = typist.snapshot();
	for name in ["toggleDetailsOpen", "unsetDetails"] {
		assert!(run_later(&mut typist.editor, name, &[]));
		typist.type_script("⏸[Mod-z]");
		assert_eq!(typist.snapshot()["doc"], before["doc"], "undo {name}");
	}
}

#[test]
fn invalid_arguments_are_refused_with_invalid_args() {
	let mut typist = typist("x");
	let editor = &mut typist.editor;
	let cases: [(&str, Args); 7] = [
		("toggleCallout", vec![("type", Value::from("purple"))]),
		("toggleCallout", vec![("type", Value::from(3_i64))]),
		("setCalloutType", vec![]),
		("insertInlineMath", vec![]),
		("insertBlockMath", vec![("latex", Value::from(true))]),
		("setDetails", vec![("open", Value::from(true))]),
		(
			"insertInlineMath",
			vec![("latex", Value::from("x")), ("display", Value::from(true))],
		),
	];

	for (name, args) in cases {
		assert!(
			matches!(
				run_error(editor, name, &args),
				EditorError::InvalidArgs { .. }
			),
			"{name} {args:?}"
		);
	}

	let unknown = Attrs::new().with("x", 1_i64);
	assert!(matches!(
		editor.query("calloutType", &unknown),
		Err(EditorError::InvalidArgs { .. })
	));
}

#[test]
fn an_editor_without_the_extensions_has_none_of_their_names() {
	let mut editor = support::editor();

	for name in [
		"setDetails",
		"toggleCallout",
		"insertInlineMath",
		"insertBlockMath",
	] {
		assert!(matches!(
			run_error(&mut editor, name, &[]),
			EditorError::UnknownName(_)
		));
	}

	assert!(matches!(
		editor.query("detailsOpen", &Attrs::new()),
		Err(EditorError::UnknownName(_))
	));
}

#[test]
fn an_unknown_callout_type_pastes_as_info() {
	let typist = typist("");
	let schema = typist.editor.schema().clone();
	let pasted = clipboard::from_html(&schema, "<div data-callout-type=\"purple\"><p>x</p></div>");
	assert_eq!(
		clipboard::to_html(&schema, &pasted),
		"<div data-callout-type=\"info\"><p>x</p></div>"
	);

	assert_eq!(markdown_round_trip_html(&schema, &pasted), "> [!NOTE]\n> x");
}

fn markdown_round_trip_html(schema: &canvist_model::Schema, slice: &canvist_core::Slice) -> String {
	clipboard::to_markdown(schema, slice)
}

#[test]
fn tiptap_json_reads_and_writes_back_unchanged() {
	let typist = typist("");
	let schema = typist.editor.schema().clone();
	let json = json!({ "type": "doc", "content": [
		details(false, "Title", &["Body", "More"]),
		{ "type": "callout", "attrs": { "type": "success" }, "content": [
			paragraph("Tip"),
			details(true, "", &["Nested"]),
		] },
		{ "type": "paragraph", "content": [
			{ "type": "text", "text": "Area " },
			{ "type": "inlineMath", "attrs": { "latex": "\\pi r^2" } },
		] },
		{ "type": "blockMath", "attrs": { "latex": "E = mc^2" } },
	] });
	let loaded = canvist_model::Document::from_json(schema, 1, &json).expect("TipTap JSON loads");
	let mut written = loaded.to_json();
	strip_ids(&mut written);

	assert_eq!(written, json);
}

#[test]
fn tiptap_json_without_attrs_takes_the_defaults() {
	let typist = typist("");
	let schema = typist.editor.schema().clone();
	let json = json!({ "type": "doc", "content": [
		{ "type": "details", "content": [
			{ "type": "detailsSummary", "content": [{ "type": "text", "text": "S" }] },
			{ "type": "detailsContent", "content": [paragraph("B")] },
		] },
		{ "type": "blockMath" },
	] });
	let loaded = canvist_model::Document::from_json(schema.clone(), 1, &json).expect("loads");
	let slice = canvist_core::Slice::new(
		loaded
			.children(None)
			.iter()
			.filter_map(|&id| loaded.tree(id))
			.collect(),
	);

	assert_eq!(
		clipboard::to_html(&schema, &slice),
		"<details><summary>S</summary><div \
		 data-type=\"detailsContent\"><p>B</p></div></details><div data-type=\"block-math\"></div>"
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
fn html_details_take_tiptaps_shape_whatever_they_hold() {
	let cases = [
		(
			"<details><p>no summary</p></details>",
			"<details><summary></summary><div data-type=\"detailsContent\"><p>no \
			 summary</p></div></details>",
		),
		(
			"<details><summary>s</summary><p>a</p><p>b</p></details>",
			"<details><summary>s</summary><div \
			 data-type=\"detailsContent\"><p>a</p><p>b</p></div></details>",
		),
		(
			"<details open=\"\"><summary>A &amp; B</summary>loose</details>",
			"<details open><summary>A &amp; B</summary><div \
			 data-type=\"detailsContent\"><p>loose</p></div></details>",
		),
		(
			"<details><summary>only</summary></details>",
			"<details><summary>only</summary><div \
			 data-type=\"detailsContent\"><p></p></div></details>",
		),
	];

	for (html, expected) in cases {
		assert_eq!(html_round_trip(html), expected, "{html}");
	}

	let markdown = {
		let typist = typist("");
		let schema = typist.editor.schema();
		clipboard::to_markdown(schema, &clipboard::from_html(schema, cases[0].0))
	};
	assert_eq!(
		markdown,
		"<details>\n<summary></summary>\n\nno summary\n\n</details>"
	);
}

#[test]
fn math_sources_survive_html_escaping() {
	let html = "<p><span data-type=\"inline-math\" \
	            data-latex=\"a&quot;&lt;b&amp;c\">$a&quot;&lt;b&amp;c$</span></p><div \
	            data-type=\"block-math\" data-latex=\"x &lt; &quot;y&quot;\"></div>";

	assert_eq!(html_round_trip(html), html);
	assert_eq!(
		markdown_round_trip("$$\nx < \"y\"\n$$"),
		"$$\nx < \"y\"\n$$"
	);
}

#[test]
fn markdown_reads_every_github_alert_and_writes_it_back() {
	for (alert, kind) in [
		("NOTE", "info"),
		("TIP", "success"),
		("IMPORTANT", "blank"),
		("WARNING", "warning"),
		("CAUTION", "error"),
	] {
		let markdown = format!("> [!{alert}]\n> Text");
		let typist = typist("");
		let schema = typist.editor.schema();
		let slice = clipboard::from_markdown(schema, &markdown);

		assert_eq!(
			clipboard::to_html(schema, &slice),
			format!("<div data-callout-type=\"{kind}\"><p>Text</p></div>")
		);
		assert_eq!(clipboard::to_markdown(schema, &slice), markdown);
	}

	assert_eq!(
		markdown_round_trip("> [!note]\n> lower"),
		"> [!NOTE]\n> lower"
	);
	assert_eq!(
		markdown_round_trip("> [!CAUTION]\n> c\n>\n> d"),
		"> [!CAUTION]\n> c\n>\n> d"
	);
}

#[test]
fn markdown_nests_details_and_callouts_both_ways() {
	for markdown in [
		"<details open>\n<summary>S</summary>\n\n> [!NOTE]\n> x\n\n</details>",
		"> [!NOTE]\n> <details>\n> <summary>s</summary>\n>\n> Body\n>\n> </details>",
		"<details>\n<summary>Outer</summary>\n\n<details \
		 open>\n<summary>Inner</summary>\n\nx\n\n</details>\n\n</details>",
	] {
		assert_eq!(markdown_round_trip(markdown), markdown);
	}
}

#[test]
fn markdown_summaries_are_html_text() {
	for markdown in [
		"<details>\n<summary>A &amp; B &lt;c&gt;</summary>\n\nx\n\n</details>",
		"<details>\n<summary>a*b*c</summary>\n\nx\n\n</details>",
	] {
		assert_eq!(markdown_round_trip(markdown), markdown);
	}

	let typist = typist("");
	let schema = typist.editor.schema();
	let slice = clipboard::from_markdown(
		schema,
		"<details>\n<summary>A &amp; B</summary>\n\nx\n\n</details>",
	);
	assert!(clipboard::to_html(schema, &slice).contains("<summary>A &amp; B</summary>"));
}

#[test]
fn markdown_block_math_spans_lines_and_closes_at_the_end() {
	assert_eq!(markdown_round_trip("$$\na\nb\n$$"), "$$\na\nb\n$$");
	assert_eq!(
		markdown_round_trip("$$\nunterminated"),
		"$$\nunterminated\n$$"
	);
}

#[test]
fn pasting_blocks_keeps_the_document_valid() {
	let mut typist = typist("Hello");
	let schema = typist.editor.schema().clone();
	let pasted = clipboard::from_markdown(
		&schema,
		"> [!WARNING]\n> \
		 Careful\n\n<details>\n<summary>S</summary>\n\nBody\n\n</details>\n\n$$\nx\n$$",
	);
	typist
		.editor
		.paste(pasted, &mut Env::headless(Duration::ZERO))
		.expect("paste");

	support::assert_valid(typist.editor.state());
	let markdown = clipboard::to_markdown(&schema, &{
		typist.type_script("[Mod-a]");
		typist.editor.copy().expect("copy")
	});
	assert!(markdown.contains("> [!WARNING]\n> Careful"), "{markdown}");
	assert!(markdown.contains("<summary>S</summary>"), "{markdown}");
	assert!(markdown.contains("$$\nx\n$$"), "{markdown}");
}
