//! Editing helpers beyond the StarterKit through the headless API: Yjs
//! peers that align, type with typography, and count characters converge,
//! and server-side HTML carries alignment but never placeholder text.

#![cfg(all(feature = "full", any(feature = "collab", feature = "ssr")))]

use canvist_headless::Engine;
use serde_json::Value as Json;
use serde_json::json;

fn engine() -> Engine {
	Engine::named(&[
		"heading",
		"bold",
		"code",
		"history",
		"textAlign",
		"typography",
		"characterCount",
		"placeholder",
	])
	.expect("the extensions build")
}

#[cfg(feature = "collab")]
mod collab {
	use canvist_headless::Doc;
	use canvist_headless::Format;
	use canvist_headless::HeadlessError;
	use canvist_headless::Position;

	use super::*;

	fn at(block: u32, offset: u32) -> Position {
		Position {
			path: vec![block],
			offset,
		}
	}

	fn caret(doc: &mut Doc, block: u32, offset: u32) {
		doc.select_range(&at(block, offset), &at(block, offset))
			.expect("selects");
	}

	fn align(doc: &mut Doc, alignment: &str) -> bool {
		doc.run_named("setTextAlign", &json!({ "alignment": alignment }))
			.expect("aligns")
	}

	fn peers(markdown: &str) -> (Doc, Doc) {
		let engine = engine();
		let mut first = engine.parse(Format::Markdown, markdown).expect("parses");
		first.share(1).expect("shares");
		let second = engine
			.from_yjs(&first.encode_state().expect("encodes"), 2)
			.expect("joins");
		(first, second)
	}

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
		assert!(first.validate()?.is_valid());
		assert!(second.validate()?.is_valid());
		Ok(())
	}

	fn alignments(doc: &Doc) -> Vec<Json> {
		doc.to_json()["content"]
			.as_array()
			.into_iter()
			.flatten()
			.map(|block| block["attrs"]["textAlign"].clone())
			.collect()
	}

	#[test]
	fn peers_aligning_different_blocks_keep_both() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("# Title\n\nBody");
		caret(&mut first, 0, 0);
		assert!(align(&mut first, "center"));
		caret(&mut second, 1, 0);
		assert!(align(&mut second, "justify"));

		exchange(&mut first, &mut second)?;
		assert_eq!(alignments(&first), [json!("center"), json!("justify")]);
		Ok(())
	}

	#[test]
	fn peers_aligning_one_block_differently_converge() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("Body");
		caret(&mut first, 0, 0);
		assert!(align(&mut first, "left"));
		caret(&mut second, 0, 0);
		assert!(align(&mut second, "right"));

		exchange(&mut first, &mut second)?;
		let [alignment] = alignments(&first).try_into().expect("one block");
		assert!(
			alignment == "left" || alignment == "right",
			"one peer's alignment wins: {alignment}"
		);
		Ok(())
	}

	#[test]
	fn aligning_while_the_other_peer_types_keeps_both() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("Body");
		caret(&mut first, 0, 0);
		assert!(align(&mut first, "center"));
		caret(&mut second, 0, 4);
		assert!(second.insert_text(" text")?);

		exchange(&mut first, &mut second)?;
		assert_eq!(
			first.export(Format::Html),
			"<p style=\"text-align: center\">Body text</p>"
		);
		Ok(())
	}

	#[test]
	fn typography_edits_reach_the_other_peer() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("Wait");
		caret(&mut first, 0, 4);
		assert!(first.type_text(" -- \"what\"...")?);
		caret(&mut second, 0, 0);
		assert!(second.insert_text("> ")?);

		exchange(&mut first, &mut second)?;
		assert_eq!(second.export(Format::Text), "> Wait — “what”…");
		Ok(())
	}

	#[test]
	fn a_character_limit_holds_back_local_edits_not_remote_ones() -> Result<(), HeadlessError> {
		let (mut first, mut second) = peers("abc");
		assert!(first.run_named("setCharacterLimit", &json!({ "limit": 3 }))?);
		caret(&mut first, 0, 3);
		first.type_text("d")?;
		assert_eq!(
			first.export(Format::Text),
			"abc",
			"the local edit is refused"
		);

		caret(&mut second, 0, 3);
		assert!(second.insert_text("def")?);
		exchange(&mut first, &mut second)?;
		assert_eq!(
			first.export(Format::Text),
			"abcdef",
			"a remote edit applies, so peers converge"
		);
		assert_eq!(first.query("characters", &Json::Null)?, json!(6));
		assert_eq!(
			second.query("characterLimit", &Json::Null)?,
			Json::Null,
			"the limit is each peer's own"
		);
		Ok(())
	}

	#[test]
	fn a_rejoining_peer_keeps_every_alignment() -> Result<(), HeadlessError> {
		let engine = engine();
		let mut first = engine.from_json(&json!({ "type": "doc", "content": [
			{ "type": "heading", "attrs": { "level": 1, "textAlign": "right" }, "content": [{ "type": "text", "text": "T" }] },
			{ "type": "paragraph", "attrs": { "textAlign": "justify" }, "content": [{ "type": "text", "text": "B" }] }
		] }))?;
		first.share(1)?;
		let second = engine.from_yjs(&first.encode_state()?, 2)?;

		assert_eq!(second.to_json(), first.to_json());
		assert_eq!(alignments(&second), [json!("right"), json!("justify")]);
		Ok(())
	}
}

#[cfg(feature = "ssr")]
mod ssr {
	use canvist_headless::Renderer;

	use super::*;

	fn render(content: &Json) -> String {
		let doc = engine()
			.from_json(&json!({ "type": "doc", "content": content }))
			.expect("loads");
		Renderer::new().html(&doc, 600.0).html
	}

	#[test]
	fn aligned_blocks_render_validated_text_align() {
		let html = render(&json!([
			{ "type": "heading", "attrs": { "level": 2, "textAlign": "center" }, "content": [{ "type": "text", "text": "Title" }] },
			{ "type": "paragraph", "attrs": { "textAlign": "justify" }, "content": [{ "type": "text", "text": "Body" }] },
			{ "type": "paragraph", "attrs": { "textAlign": "center;color:red" }, "content": [{ "type": "text", "text": "Hostile" }] }
		]));

		assert!(html.contains("<h2 "), "headings stay headings: {html}");
		assert!(html.contains("text-align:center;"), "{html}");
		assert!(html.contains("text-align:justify;"), "{html}");
		assert!(
			!html.contains("color:red"),
			"invalid values never reach style: {html}"
		);
	}

	#[test]
	fn placeholder_text_is_never_document_content() {
		let html = render(&json!([{ "type": "paragraph" }]));

		assert!(
			!html.contains("Write something"),
			"the accessibility mirror reads SSR HTML, which holds only content: {html}"
		);
	}
}
