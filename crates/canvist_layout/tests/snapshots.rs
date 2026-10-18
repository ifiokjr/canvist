//! Snapshot tests of layout trees, all with the deterministic [`FixedMeasurer`](canvist_layout::FixedMeasurer).

#![allow(clippy::float_cmp, reason = "layout with the fixed measurer is exact")]

mod support;

use canvist_model::Document;
use serde_json::json;
use support::doc;
use support::layout;
use support::paragraphs;

/// Narrow enough that the starter kit's long paragraphs and quote wrap.
const STARTER_KIT_WIDTH: f32 = 320.0;
/// The starter kit image's intrinsic height, in pixels.
const IMAGE_HEIGHT: u32 = 80;

/// Every StarterKit block, mark, and embed.
fn starter_kit() -> Document {
	doc(json!([
		{ "type": "heading", "attrs": { "level": 2 }, "content": [{ "type": "text", "text": "Release notes" }] },
		{ "type": "paragraph", "content": [
			{ "type": "text", "text": "Plain, " },
			{ "type": "text", "text": "bold", "marks": [{ "type": "bold" }] },
			{ "type": "text", "text": ", " },
			{ "type": "text", "text": "code", "marks": [{ "type": "code" }] },
			{ "type": "text", "text": " and " },
			{ "type": "text", "text": "a link", "marks": [{ "type": "link", "attrs": { "href": "https://canvist.dev" } }] },
			{ "type": "text", "text": " with " },
			{ "type": "mention", "attrs": { "id": "7", "label": "ada" } },
			{ "type": "hardBreak" },
			{ "type": "text", "text": "after the break" }
		] },
		{ "type": "bulletList", "content": [
			{ "type": "listItem", "content": [
				{ "type": "paragraph", "content": [{ "type": "text", "text": "first" }] },
				{ "type": "bulletList", "content": [
					{ "type": "listItem", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "nested" }] }] }
				] }
			] },
			{ "type": "listItem", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "second" }] }] }
		] },
		{ "type": "orderedList", "attrs": { "start": 3 }, "content": [
			{ "type": "listItem", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "three" }] }] },
			{ "type": "listItem", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "four" }] }] }
		] },
		{ "type": "taskList", "content": [
			{ "type": "taskItem", "attrs": { "checked": true }, "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "done" }] }] },
			{ "type": "taskItem", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "todo" }] }] }
		] },
		{ "type": "blockquote", "content": [
			{ "type": "paragraph", "content": [{ "type": "text", "text": "Quoted text that is long enough to wrap." }] }
		] },
		{ "type": "codeBlock", "content": [{ "type": "text", "text": "fn main() {\n\tprintln!(\"hi\");\n}" }] },
		{ "type": "horizontalRule" },
		{ "type": "image", "attrs": { "src": "cat.png", "height": IMAGE_HEIGHT } },
		{ "type": "paragraph" }
	]))
}

#[test]
fn paragraphs_wrap_greedily() {
	let doc = paragraphs(&[
		"The quick brown fox jumps over the lazy dog.",
		"",
		"Supercalifragilisticexpialidocious words split.",
	]);

	insta::assert_snapshot!(layout(&doc, 200.0).dump());
}

#[test]
fn starter_kit_layout_tree() {
	insta::assert_snapshot!(layout(&starter_kit(), STARTER_KIT_WIDTH).dump());
}

#[test]
fn right_to_left_paragraph() {
	let doc = paragraphs(&["שלום עולם, זהו משפט ארוך בעברית"]);

	insta::assert_snapshot!(layout(&doc, 160.0).dump());
}

#[test]
fn mixed_direction_paragraph() {
	let doc = paragraphs(&[
		"The word مرحبا means hello, and 123 stays LTR.",
		"قال: \"hello world\" ثم 2024 انتهى.",
	]);

	insta::assert_snapshot!(layout(&doc, 220.0).dump());
}

#[test]
fn emoji_and_combining_marks() {
	let doc =
		paragraphs(&["Family 👨\u{200D}👩\u{200D}👧 and cafe\u{301} with flags 🇳🇬🇬🇧 and skin 👍🏽."]);

	insta::assert_snapshot!(layout(&doc, 150.0).dump());
}

#[test]
fn cjk_breaks_between_ideographs() {
	let doc = paragraphs(&["日本語の文章は、単語の間に空白がありません。中文也一样。"]);

	insta::assert_snapshot!(layout(&doc, 120.0).dump());
}

#[test]
fn tabs_advance_to_stops() {
	let doc = paragraphs(&["a\tbb\tccc\td", "\tindented"]);

	insta::assert_snapshot!(layout(&doc, 600.0).dump());
}
