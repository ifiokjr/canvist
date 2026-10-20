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

#[cfg(feature = "theme-extensions")]
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

// The extension theme rules are behind `theme-extensions`.
#[cfg(feature = "theme-extensions")]
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

/// The left edge and width of every text run on each line of a one-block
/// document whose paragraph has `align`.
fn aligned(align: &str, text: &str, width: f32) -> Vec<Vec<(f32, f32)>> {
	let doc = doc(json!([{
		"type": "paragraph",
		"attrs": { "textAlign": align },
		"content": [{ "type": "text", "text": text }]
	}]));
	let layout = layout(&doc, width);
	let list = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));
	let mut lines: Vec<Vec<(f32, f32)>> = Vec::new();
	let mut baseline = None;

	for item in &list.items {
		let canvist_layout::DisplayItem::Text(run) = item else {
			continue;
		};

		if baseline != Some(run.baseline) {
			baseline = Some(run.baseline);
			lines.push(Vec::new());
		}

		if let Some(line) = lines.last_mut() {
			line.push((run.x, run.width));
		}
	}

	lines
}

#[test]
fn text_aligns_left_centre_and_right() {
	// "abc" is three 9.6 px advances: 28.8 px in a 100 px line.
	let left = aligned("left", "abc", 100.0);
	let center = aligned("center", "abc", 100.0);
	let right = aligned("right", "abc", 100.0);

	assert_eq!(left[0][0].0, 0.0);
	assert_eq!(center[0][0].0, (100.0 - 28.8) / 2.0);
	assert_eq!(right[0][0].0, 100.0 - 28.8);
}

#[test]
fn justified_lines_widen_their_spaces_except_the_last_line() {
	// Fits "aaa bbb " but not "aaa bbb ccc", so the text wraps after "bbb".
	let lines = aligned("justify", "aaa bbb ccc", 100.0);
	let [first, last] = lines.as_slice() else {
		panic!("two lines: {lines:?}");
	};
	let (bbb_x, bbb_width) = first[first.len() - 1];

	assert_eq!(first[0].0, 0.0);
	assert!(
		(bbb_x + 3.0 * 9.6 - 100.0).abs() < 0.01,
		"the wrapped line reaches the right edge: bbb at {bbb_x}, {bbb_width} wide"
	);
	assert_eq!(last[0].0, 0.0, "the last line aligns to the start");
}

#[test]
fn server_html_carries_text_alignment() {
	let doc = doc(json!([{
		"type": "paragraph",
		"attrs": { "textAlign": "center" },
		"content": [{ "type": "text", "text": "Title" }]
	}, {
		"type": "paragraph",
		"attrs": { "textAlign": "sideways" },
		"content": [{ "type": "text", "text": "Ignored" }]
	}]));
	let html = to_html(
		&doc,
		&Arc::new(Theme::starter_kit()),
		&HtmlOptions::new(320.0),
	)
	.html;

	assert!(html.contains("style=\"text-align:center;\""), "{html}");
	assert!(!html.contains("sideways"), "{html}");
}

#[test]
fn right_to_left_text_aligns_to_the_physical_edge_it_names() {
	let start = aligned("start", "שלום", 100.0);
	let left = aligned("left", "שלום", 100.0);
	let (x, width) = start[0][0];

	assert!(
		(x + width - 100.0).abs() < 0.01,
		"right-to-left starts on the right"
	);
	assert_eq!(left[0][0].0, 0.0, "`left` means the left edge");
}

#[test]
fn alignment_survives_typing_and_changing_it() {
	let mut doc = doc(json!([{
		"type": "paragraph",
		"attrs": { "textAlign": "center" },
		"content": [{ "type": "text", "text": "a centred line that wraps around" }]
	}]));
	let mut measurer = canvist_layout::FixedMeasurer::new();
	let mut layout = canvist_layout::DocumentLayout::new(
		&doc,
		Arc::new(Theme::starter_kit()),
		canvist_layout::LayoutOptions::new(120.0),
	);
	layout.layout_all(&doc, &mut measurer).expect("measured");
	let block = doc.children(None)[0];
	let paragraph = doc.schema().default_text_block();

	let mut typing = doc.transaction();
	typing
		.insert_text(
			canvist_model::Point::new(block, 1),
			"!",
			canvist_model::MarkSet::empty(),
		)
		.expect("valid point");
	let commit = typing.commit();
	layout.apply(&doc, &commit);
	layout
		.ensure(&doc, &mut measurer, 0.0, 1000.0)
		.expect("measured");
	assert_eq!(
		layout.dump(),
		support::layout(&doc, 120.0).dump(),
		"after typing"
	);

	let mut realign = doc.transaction();
	realign
		.set_block(
			block,
			paragraph,
			canvist_model::Attrs::new().with("textAlign", "right"),
		)
		.expect("valid block");
	let commit = realign.commit();
	layout.apply(&doc, &commit);
	layout
		.ensure(&doc, &mut measurer, 0.0, 1000.0)
		.expect("measured");
	assert_eq!(
		layout.dump(),
		support::layout(&doc, 120.0).dump(),
		"after realigning"
	);
}

#[test]
fn placeholders_draw_faded_in_empty_blocks_only() {
	let doc = doc(json!([{ "type": "paragraph" }, support::paragraph("text")]));
	let layout = layout(&doc, 320.0);
	let blocks = doc.children(None);
	let options = DisplayOptions::viewport(0.0, layout.height())
		.placeholder(blocks[0], "Write something …")
		.placeholder(blocks[1], "Hidden");
	let list = layout.display_list(&options);
	let runs: Vec<&canvist_layout::TextRun> = list
		.items
		.iter()
		.filter_map(|item| {
			match item {
				canvist_layout::DisplayItem::Text(run) => Some(run),
				_ => None,
			}
		})
		.collect();

	assert_eq!(runs.len(), 2, "the placeholder and the second block's text");
	let placeholder = runs
		.iter()
		.find(|run| run.text() == "Write something …")
		.expect("drawn");
	let ink = Theme::starter_kit().base().color;

	assert_eq!(placeholder.block, blocks[0]);
	assert_eq!(placeholder.x, 0.0);
	assert_eq!(placeholder.color.a, (f32::from(ink.a) * 0.4).round() as u8);
	assert!(!runs.iter().any(|run| run.text() == "Hidden"));
}

// The extension theme rules are behind `theme-extensions`.
#[cfg(feature = "theme-extensions")]
#[test]
fn emoji_draw_their_character_or_their_shortcode() {
	let schema = canvist_model::Schema::builder()
		.block(canvist_model::BlockSpec::text("paragraph"))
		.embed(canvist_model::EmbedSpec::new("emoji"))
		.build()
		.expect("valid schema");
	let doc = Document::from_json(
		Arc::new(schema),
		1,
		&json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [
			{ "type": "emoji", "attrs": { "name": "smile", "emoji": "😄" } },
			{ "type": "emoji", "attrs": { "name": "wave" } }
		] }] }),
	)
	.expect("valid JSON");
	let layout = layout(&doc, 320.0);
	let list = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));
	let texts: Vec<&str> = list
		.items
		.iter()
		.filter_map(|item| {
			match item {
				canvist_layout::DisplayItem::Text(run) => Some(run.text()),
				_ => None,
			}
		})
		.collect();

	assert_eq!(texts, ["😄", ":wave:"]);
}

/// A document with a details block, `open` or closed.
fn details(open: bool) -> Document {
	let schema = canvist_model::Schema::builder()
		.block(canvist_model::BlockSpec::text("paragraph"))
		.block(
			canvist_model::BlockSpec::container("details", &["detailsSummary", "detailsContent"])
				.attrs(canvist_model::Attrs::new().with("open", false)),
		)
		.block(canvist_model::BlockSpec::text("detailsSummary").groups(&[]))
		.block(canvist_model::BlockSpec::container("detailsContent", &["block"]).groups(&[]))
		.build()
		.expect("valid schema");

	Document::from_json(
		Arc::new(schema),
		1,
		&json!({ "type": "doc", "content": [{
			"type": "details",
			"attrs": { "open": open },
			"content": [
				{ "type": "detailsSummary", "content": [{ "type": "text", "text": "Summary" }] },
				{ "type": "detailsContent", "content": [support::paragraph("Hidden until opened")] }
			]
		}] }),
	)
	.expect("valid JSON")
}

fn texts(doc: &Document) -> Vec<String> {
	let layout = layout(doc, 320.0);
	let list = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));

	list.items
		.iter()
		.filter_map(|item| {
			match item {
				canvist_layout::DisplayItem::Text(run) => Some(run.text().to_owned()),
				_ => None,
			}
		})
		.collect()
}

#[test]
fn closed_details_show_only_their_summary() {
	assert_eq!(texts(&details(false)).concat(), "▸Summary");
	assert_eq!(
		texts(&details(true)).concat(),
		"▾SummaryHidden until opened"
	);

	let theme = Arc::new(Theme::starter_kit());
	let closed = to_html(&details(false), &theme, &HtmlOptions::new(320.0));
	assert!(!closed.html.contains("Hidden"), "{}", closed.html);
	assert!(closed.html.contains("cv-mk-d0"), "{}", closed.html);
	assert!(closed.css.contains(".cv-mk-d1::before{content:\"▾\"}"));
}

#[test]
fn opening_details_relays_out_like_a_fresh_layout() {
	let mut doc = details(false);
	let mut measurer = canvist_layout::FixedMeasurer::new();
	let mut layout = canvist_layout::DocumentLayout::new(
		&doc,
		Arc::new(Theme::starter_kit()),
		canvist_layout::LayoutOptions::new(320.0),
	);
	layout.layout_all(&doc, &mut measurer).expect("measured");
	let block = doc.children(None)[0];
	let kind = doc.block(block).expect("details").kind();

	let mut open = doc.transaction();
	open.set_block(block, kind, canvist_model::Attrs::new().with("open", true))
		.expect("valid block");
	let commit = open.commit();
	layout.apply(&doc, &commit);
	layout
		.ensure(&doc, &mut measurer, 0.0, 1000.0)
		.expect("measured");

	assert_eq!(layout.dump(), support::layout(&doc, 320.0).dump());
	assert!(
		layout.dump().contains("\"Hidden \""),
		"the content is laid out"
	);
}
