//! Formatting beyond the StarterKit through the headless API: engines
//! named in any order build one schema, Yjs peers that format the same text
//! concurrently converge, and server-side HTML renders subscripts,
//! superscripts, and highlights with their semantic elements.

#![cfg(feature = "full")]

use canvist_headless::Engine;
use canvist_headless::Format;
use serde_json::Value as Json;
use serde_json::json;

/// The StarterKit plus every formatting extension, by name, as the
/// bindings register them.
fn engine() -> Engine {
	Engine::named(&[
		"bold",
		"italic",
		"history",
		"subscript",
		"superscript",
		"highlight",
		"textStyle",
		"color",
		"backgroundColor",
		"fontFamily",
		"fontSize",
		"lineHeight",
	])
	.expect("the extensions build")
}

/// Regression: marks nest and serialize in registration order, and engines
/// registered names in the order given, so Dart (which lists extensions
/// alphabetically) wrote `<mark><sub>` where TypeScript wrote `<sub><mark>`.
#[test]
fn names_in_any_order_build_the_same_schema() {
	let json = json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [
		{ "type": "text", "text": "2", "marks": [{ "type": "subscript" }, { "type": "highlight" }] }
	] }] });
	let docs: Vec<_> = [
		["subscript", "highlight", "bold"],
		["highlight", "bold", "subscript"],
	]
	.iter()
	.map(|names| {
		Engine::named(names)
			.and_then(|engine| engine.from_json(&json))
			.expect("loads")
	})
	.collect();

	assert_eq!(
		docs[0].export(Format::Html),
		"<p><sub><mark>2</mark></sub></p>"
	);
	assert_eq!(docs[1].export(Format::Html), docs[0].export(Format::Html));
	assert_eq!(docs[1].to_json(), docs[0].to_json());
	assert_eq!(docs[1].editor().metadata(), docs[0].editor().metadata());
}

#[cfg(feature = "collab")]
mod collab {
	use canvist_headless::Doc;
	use canvist_headless::HeadlessError;
	use canvist_headless::Position;

	use super::*;

	fn at(offset: u32) -> Position {
		Position {
			path: vec![0],
			offset,
		}
	}

	fn select(doc: &mut Doc, from: u32, to: u32) {
		doc.select_range(&at(from), &at(to)).expect("selects");
	}

	fn run(doc: &mut Doc, name: &str, args: &Json) {
		assert!(doc.run_named(name, args).expect("runs"), "{name} applies");
	}

	/// The first paragraph's inline content.
	fn content(doc: &Doc) -> Json {
		doc.to_json()["content"][0]["content"].clone()
	}

	/// A shared document as client 1 and a second peer joined from it as
	/// client 2.
	fn peers(markdown: &str) -> (Doc, Doc) {
		let engine = engine();
		let mut first = engine.parse(Format::Markdown, markdown).expect("parses");
		first.share(1).expect("shares");
		let second = engine
			.from_yjs(&first.encode_state().expect("encodes"), 2)
			.expect("joins");
		(first, second)
	}

	/// Deliver each peer's pending updates to the other, then require both
	/// to hold the same valid document.
	fn exchange(first: &mut Doc, second: &mut Doc) -> Result<(), HeadlessError> {
		let from_first = first.take_updates()?;
		let from_second = second.take_updates()?;

		for update in &from_first {
			second.apply_update(update)?;
		}

		for update in &from_second {
			first.apply_update(update)?;
		}

		assert_eq!(first.to_json(), second.to_json(), "the peers converge");
		assert!(first.validate()?.is_valid(), "the first peer is valid");
		assert!(second.validate()?.is_valid(), "the second peer is valid");
		Ok(())
	}

	#[test]
	fn overlapping_colour_and_highlight_both_survive() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("Hello world");
		select(&mut first, 0, 5);
		run(&mut first, "setColor", &json!({ "color": "#e8590c" }));
		select(&mut second, 3, 11);
		run(&mut second, "setHighlight", &json!({ "color": "#fef08a" }));

		exchange(&mut first, &mut second)?;
		let colour = json!({ "type": "textStyle", "attrs": { "color": "#e8590c" } });
		let highlight = json!({ "type": "highlight", "attrs": { "color": "#fef08a" } });
		assert_eq!(
			content(&first),
			json!([
				{ "type": "text", "text": "Hel", "marks": [colour] },
				{ "type": "text", "text": "lo", "marks": [colour, highlight] },
				{ "type": "text", "text": " world", "marks": [highlight] }
			])
		);
		Ok(())
	}

	/// Each peer sees a valid document, and they converge. Yjs merges the
	/// two marks as independent format attributes, so the character ends up
	/// with both even though the schema makes them exclude each other; the
	/// collaboration binding does not yet repair excluded marks.
	#[test]
	fn concurrent_scripts_on_one_character_converge() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("x2");
		select(&mut first, 1, 2);
		run(&mut first, "setSubscript", &Json::Null);
		select(&mut second, 1, 2);
		run(&mut second, "setSuperscript", &Json::Null);

		exchange(&mut first, &mut second)?;
		assert_eq!(first.export(Format::Text), "x2");
		assert_eq!(content(&first)[0], json!({ "type": "text", "text": "x" }));
		Ok(())
	}

	/// Peers converge on one `textStyle` mark. Every style attribute lives on
	/// that one mark, which Yjs stores as one format attribute, so concurrent
	/// edits to different attributes resolve last-writer-wins on the whole
	/// mark (as they do with y-prosemirror) rather than merging.
	#[test]
	fn concurrent_style_attributes_on_one_range_converge() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("styled");
		select(&mut first, 0, 6);
		run(&mut first, "setColor", &json!({ "color": "#958df1" }));
		select(&mut second, 0, 6);
		run(&mut second, "setFontSize", &json!({ "fontSize": "20px" }));

		exchange(&mut first, &mut second)?;
		let style = &content(&first)[0]["marks"][0];
		assert_eq!(style["type"], "textStyle", "{style}");
		assert_eq!(content(&first).as_array().map(Vec::len), Some(1), "one run");
		Ok(())
	}

	#[test]
	fn typing_into_a_highlight_while_it_is_removed_converges() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("a ==bright== idea");
		select(&mut first, 2, 8);
		run(&mut first, "unsetHighlight", &Json::Null);
		select(&mut second, 5, 5);
		assert!(second.insert_text("!!")?);

		exchange(&mut first, &mut second)?;
		assert_eq!(first.export(Format::Text), "a bri!!ght idea");
		Ok(())
	}

	#[test]
	fn undoing_a_style_reaches_the_other_peer() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("plain");
		select(&mut first, 0, 5);
		run(&mut first, "setLineHeight", &json!({ "lineHeight": "2" }));
		run(
			&mut first,
			"setBackgroundColor",
			&json!({ "backgroundColor": "#fafafa" }),
		);
		exchange(&mut first, &mut second)?;
		assert_eq!(
			content(&second)[0]["marks"][0]["attrs"],
			json!({ "backgroundColor": "#fafafa", "lineHeight": "2" })
		);

		assert!(first.undo()?);
		assert!(first.undo()?);
		exchange(&mut first, &mut second)?;
		assert_eq!(
			content(&second),
			json!([{ "type": "text", "text": "plain" }])
		);
		Ok(())
	}

	#[test]
	fn a_peer_rejoins_with_every_mark() -> Result<(), HeadlessError> {
		let engine = engine();
		let json = json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [
			{ "type": "text", "text": "H" },
			{ "type": "text", "text": "2", "marks": [{ "type": "subscript" }] },
			{ "type": "text", "text": "O", "marks": [{ "type": "superscript" }] },
			{ "type": "text", "text": "hot", "marks": [{ "type": "highlight", "attrs": { "color": "#ffc078" } }] },
			{ "type": "text", "text": "cool", "marks": [{ "type": "textStyle", "attrs": {
				"color": "#1c7ed6", "backgroundColor": "#fafafa", "fontFamily": "Georgia",
				"fontSize": "18px", "lineHeight": "1.5"
			} }] }
		] }] });
		let mut first = engine.from_json(&json)?;
		first.share(1)?;
		let second = engine.from_yjs(&first.encode_state()?, 2)?;

		assert_eq!(second.to_json(), first.to_json());
		assert_eq!(
			second.export(Format::Html),
			first.export(Format::Html),
			"HTML agrees too"
		);
		Ok(())
	}
}

#[cfg(feature = "ssr")]
mod ssr {
	use canvist_headless::Renderer;

	use super::*;

	fn render(content: &Json) -> String {
		let doc = engine()
			.from_json(
				&json!({ "type": "doc", "content": [{ "type": "paragraph", "content": content }] }),
			)
			.expect("loads");
		Renderer::new().html(&doc, 600.0).html
	}

	#[test]
	fn scripts_and_highlights_render_their_semantic_elements() {
		let html = render(&json!([
			{ "type": "text", "text": "H" },
			{ "type": "text", "text": "2", "marks": [{ "type": "subscript" }] },
			{ "type": "text", "text": "O x" },
			{ "type": "text", "text": "2", "marks": [{ "type": "superscript" }] },
			{ "type": "text", "text": " " },
			{ "type": "text", "text": "key", "marks": [{ "type": "highlight" }] },
			{ "type": "text", "text": "pink", "marks": [{ "type": "highlight", "attrs": { "color": "#fbcfe8" } }] }
		]));

		for element in [
			"<sub class=\"cv-m cv-m-subscript\">2</sub>",
			"<sup class=\"cv-m cv-m-superscript\">2</sup>",
			"<mark class=\"cv-m cv-m-highlight\">key</mark>",
			"<mark class=\"cv-m cv-m-highlight\" style=\"background-color:#fbcfe8;\">pink</mark>",
		] {
			assert!(html.contains(element), "{element} in {html}");
		}
	}

	#[test]
	fn text_styles_render_validated_inline_css() {
		let html = render(&json!([
			{ "type": "text", "text": "safe", "marks": [{ "type": "textStyle", "attrs": {
				"color": "#ff0000", "backgroundColor": "#00ff00", "fontFamily": "Georgia",
				"fontSize": "18px", "lineHeight": "1.5"
			} }] },
			{ "type": "text", "text": "hostile", "marks": [{ "type": "textStyle", "attrs": {
				"color": "red;background:url(x)", "fontFamily": "x;}</style><script>",
				"fontSize": "-4px", "lineHeight": "tall"
			} }] }
		]));

		assert!(
			html.contains(
				"style=\"color:#ff0000;background-color:#00ff00;font-family:Georgia;font-size:\
				 18px;line-height:1.5;\">safe"
			),
			"{html}"
		);
		assert!(
			html.contains("<span class=\"cv-m cv-m-textStyle\">hostile</span>"),
			"invalid values never reach the style attribute: {html}"
		);
		assert!(!html.contains("<script>"), "{html}");
	}
}
