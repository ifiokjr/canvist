//! Snapshot tests of layout trees, display lists, and server renderers, all
//! with the deterministic [`FixedMeasurer`](canvist_layout::FixedMeasurer).

#![allow(clippy::float_cmp, reason = "layout with the fixed measurer is exact")]

mod support;

use std::sync::Arc;

use canvist_layout::DisplayOptions;
use canvist_layout::HtmlOptions;
use canvist_layout::Theme;
use canvist_layout::to_html;
use canvist_layout::to_svg;
use canvist_model::Document;
use serde_json::json;
use support::describe;
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
fn starter_kit_display_list() {
	let doc = starter_kit();
	let layout = layout(&doc, STARTER_KIT_WIDTH);
	let list = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));

	insta::assert_snapshot!(describe(&list));
}

#[test]
fn starter_kit_svg() {
	let doc = starter_kit();
	let layout = layout(&doc, STARTER_KIT_WIDTH);
	let list = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));

	insta::assert_snapshot!(to_svg(&list));
}

#[test]
fn starter_kit_html() {
	let html = to_html(
		&starter_kit(),
		&Arc::new(Theme::starter_kit()),
		&HtmlOptions::new(STARTER_KIT_WIDTH),
	);

	insta::assert_snapshot!("starter_kit_html", html.html);
	insta::assert_snapshot!("starter_kit_css", html.css.replace('}', "}\n"));
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

/// The runs of a one-paragraph document, keyed by their text.
fn runs(content: &serde_json::Value) -> Vec<(String, f32, f32)> {
	let doc = doc(json!([{ "type": "paragraph", "content": content }]));
	let layout = layout(&doc, 600.0);
	let list = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));

	list.items
		.iter()
		.filter_map(|item| {
			match item {
				canvist_layout::DisplayItem::Text(run) => {
					let size = list.fonts[run.font.index()].size;
					Some((run.text().to_owned(), run.baseline, size))
				}
				_ => None,
			}
		})
		.collect()
}

#[test]
fn scripts_shrink_and_shift_the_baseline() {
	let runs = runs(&json!([
		{ "type": "text", "text": "H" },
		{ "type": "text", "text": "2", "marks": [{ "type": "subscript" }] },
		{ "type": "text", "text": "O" },
		{ "type": "text", "text": "n", "marks": [{ "type": "superscript" }] }
	]));
	let [(_, base, body), (_, low, low_size), _, (_, high, high_size)] = runs.as_slice() else {
		panic!("four runs: {runs:?}");
	};
	let line = layout(
		&doc(json!([{ "type": "paragraph", "content": [{ "type": "text", "text": "x" }] }])),
		600.0,
	);
	let scripted = layout(
		&doc(json!([{ "type": "paragraph", "content": [
		{ "type": "text", "text": "x" },
		{ "type": "text", "text": "n", "marks": [{ "type": "superscript" }] }
	] }])),
		600.0,
	);

	assert!(
		scripted.height() >= line.height(),
		"a raised inline box can grow its line"
	);

	assert_eq!(*low_size, body * 0.75);
	assert_eq!(*high_size, body * 0.75);
	assert_eq!(low - base, 0.25 * low_size, "subscripts drop a quarter em");
	assert_eq!(base - high, 0.5 * high_size, "superscripts rise half an em");
}

#[test]
fn text_style_attributes_set_colour_size_and_line_height() {
	let doc = doc(json!([{ "type": "paragraph", "content": [
		{ "type": "text", "text": "big", "marks": [{ "type": "textStyle", "attrs": {
			"color": "#ff0000", "fontSize": "32px", "lineHeight": "2", "backgroundColor": "#00ff00"
		} }] }
	] }]));
	let layout = layout(&doc, 600.0);
	let list = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));
	let html = to_html(
		&doc,
		&Arc::new(Theme::starter_kit()),
		&HtmlOptions::new(600.0),
	);

	assert_eq!(layout.height(), 64.0, "32 px at line height 2");
	assert!(list.items.iter().any(|item| {
		matches!(item, canvist_layout::DisplayItem::Text(run) if run.color.to_string() == "#ff0000")
	}));
	assert!(
		html.html.contains(
			"style=\"color:#ff0000;background-color:#00ff00;font-size:32px;line-height:2;\""
		),
		"{}",
		html.html
	);
}
