//! Canvas-native drawing: ink strokes that layout draws itself, and kinetic
//! text that moves glyph by glyph when the display list is given a time.

#![allow(clippy::float_cmp, reason = "layout with the fixed measurer is exact")]

mod support;

use std::sync::Arc;

use canvist_layout::DisplayItem;
use canvist_layout::DisplayOptions;
use canvist_layout::HtmlOptions;
use canvist_layout::Theme;
use canvist_layout::to_html;
use canvist_layout::to_svg;
use serde_json::json;
use support::doc;
use support::layout;
use support::paragraph;

/// A drawing 400 units tall with one stroke from the top-left corner to the
/// middle of the right edge.
fn drawing() -> serde_json::Value {
	json!({
		"type": "ink",
		"attrs": {
			"height": 400,
			"strokes": [{ "points": [0, 0, 1000, 200], "color": "#ff0000", "width": 10 }],
		},
	})
}

fn strokes(list: &canvist_layout::DisplayList) -> Vec<(Vec<f32>, f32)> {
	list.items
		.iter()
		.filter_map(|item| {
			match item {
				DisplayItem::Stroke { points, width, .. } => Some((points.to_vec(), *width)),
				_ => None,
			}
		})
		.collect()
}

#[test]
fn ink_scales_with_its_column() {
	let doc = doc(json!([drawing()]));

	for width in [500.0, 250.0] {
		let layout = layout(&doc, width);
		let block = doc.children(None)[0];
		let rect = layout.block_rect(block).expect("a drawing");
		let list = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));
		let scale = width / canvist_layout::INK_WIDTH;

		assert_eq!(
			rect.height,
			400.0 * scale,
			"the drawing keeps its 5:2 shape"
		);
		assert_eq!(
			strokes(&list),
			[(
				vec![rect.x, rect.y, rect.x + width, rect.y + 200.0 * scale],
				10.0 * scale
			)]
		);
	}
}

#[test]
fn ink_draws_in_svg_and_html() {
	let doc = doc(json!([drawing()]));
	let layout = layout(&doc, 500.0);
	let list = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));

	assert!(
		to_svg(&list).contains(
			"<path d=\"M0 0L500 100\" fill=\"none\" stroke=\"#ff0000\" stroke-width=\"5\""
		)
	);

	let html = to_html(
		&doc,
		&Arc::new(Theme::starter_kit()),
		&HtmlOptions::new(500.0),
	);
	assert!(
		html.html
			.contains("<svg class=\"cv-ink\" viewBox=\"0 0 1000 400\""),
		"{}",
		html.html
	);
	assert!(html.html.contains("height:200px;"), "{}", html.html);
	assert!(html.css.contains(".cv-ink{"));
}

/// A paragraph whose second word waves.
fn kinetic() -> canvist_model::Document {
	doc(json!([{
		"type": "paragraph",
		"content": [
			{ "type": "text", "text": "Say " },
			{ "type": "text", "text": "hello", "marks": [{ "type": "kinetic", "attrs": { "effect": "wave" } }] },
		],
	}]))
}

fn runs(list: &canvist_layout::DisplayList) -> Vec<(String, f32)> {
	list.items
		.iter()
		.filter_map(|item| {
			match item {
				DisplayItem::Text(run) => Some((run.text().to_owned(), run.baseline)),
				_ => None,
			}
		})
		.collect()
}

#[test]
fn kinetic_text_is_still_without_a_time_and_moves_glyph_by_glyph_with_one() {
	let doc = kinetic();
	let layout = layout(&doc, 400.0);
	let still = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));

	assert!(still.animated, "the host learns it could animate");
	assert_eq!(runs(&still).len(), 2, "one run per style, as usual");

	let moving = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()).time(0.3));
	let glyphs = runs(&moving);
	let texts: Vec<&str> = glyphs.iter().map(|(text, _)| text.as_str()).collect();

	assert_eq!(texts, ["Say ", "h", "e", "l", "l", "o"]);
	assert!(
		glyphs[1].1 != glyphs[2].1,
		"neighbouring glyphs are at different heights in a wave"
	);

	let later = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()).time(0.6));
	assert_ne!(runs(&later), glyphs, "the next frame differs");
	assert_eq!(
		later.damage(&moving).len(),
		2,
		"only the kinetic block repaints, in its old and new places"
	);
}

#[test]
fn still_documents_are_not_animated() {
	let doc = doc(json!([paragraph("Nothing moves")]));
	let layout = layout(&doc, 400.0);

	assert!(
		!layout
			.display_list(&DisplayOptions::viewport(0.0, layout.height()).time(1.0))
			.animated
	);
}

#[test]
fn server_html_animates_kinetic_text_with_css_unless_motion_is_reduced() {
	let html = to_html(
		&kinetic(),
		&Arc::new(Theme::starter_kit()),
		&HtmlOptions::new(400.0),
	);

	assert!(html.html.contains("animation:cv-k-wave"), "{}", html.html);
	assert!(html.css.contains("@keyframes cv-k-wave"));
	assert!(html.css.contains("prefers-reduced-motion:reduce"));
}
