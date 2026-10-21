//! Canvas-native drawing: ink strokes that layout draws itself, and kinetic
//! text that moves glyph by glyph when the display list is given a time.

#![cfg(all(feature = "ink", feature = "kinetic", feature = "theme-extensions"))]
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

/// A paragraph whose word `hello` moves with `effect`.
fn moving(effect: &str) -> canvist_model::Document {
	doc(json!([{
		"type": "paragraph",
		"content": [
			{ "type": "text", "text": "Say " },
			{ "type": "text", "text": "hello", "marks": [{ "type": "kinetic", "attrs": { "effect": effect } }] },
		],
	}]))
}

fn glyph_runs(
	list: &canvist_layout::DisplayList,
) -> Vec<(String, f32, f32, canvist_layout::Color)> {
	list.items
		.iter()
		.filter_map(|item| {
			match item {
				DisplayItem::Text(run) => {
					Some((run.text().to_owned(), run.x, run.baseline, run.color))
				}
				_ => None,
			}
		})
		.collect()
}

#[test]
fn every_effect_keeps_the_text_and_changes_over_time() {
	for effect in ["wave", "shake", "pulse", "rainbow"] {
		let doc = moving(effect);
		let layout = layout(&doc, 400.0);
		let still = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));
		let at = |time: f32| {
			glyph_runs(
				&layout.display_list(&DisplayOptions::viewport(0.0, layout.height()).time(time)),
			)
		};
		let first = at(0.13);
		let text: String = first.iter().map(|run| run.0.as_str()).collect();

		assert!(still.animated, "{effect}");
		assert_eq!(
			text, "Say hello",
			"{effect}: every glyph is still text, in order"
		);
		assert_ne!(first, at(0.61), "{effect} moves or changes colour");
		assert_eq!(
			glyph_runs(&still)
				.iter()
				.map(|run| run.0.as_str())
				.collect::<String>(),
			"Say hello",
			"{effect}: without a time, for reduced motion, the text stays as it is"
		);
	}
}

#[test]
fn an_unknown_effect_is_still_and_not_animated() {
	let doc = moving("explode");
	let layout = layout(&doc, 400.0);
	let list = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()).time(0.5));

	assert!(!list.animated);
	assert_eq!(runs(&list).len(), 2, "the text is laid out as plain runs");

	let html = to_html(
		&doc,
		&Arc::new(Theme::starter_kit()),
		&HtmlOptions::new(400.0),
	);
	assert!(!html.html.contains("animation:"), "{}", html.html);
}

#[test]
fn server_html_keeps_kinetic_text_readable_and_still_for_reduced_motion() {
	for effect in ["wave", "shake", "pulse", "rainbow"] {
		let html = to_html(
			&moving(effect),
			&Arc::new(Theme::starter_kit()),
			&HtmlOptions::new(400.0),
		);

		assert!(
			html.html.contains(&format!("animation:cv-k-{effect} ")),
			"{}",
			html.html
		);
		assert!(
			html.html.contains(">hello</span>"),
			"the word stays one text node: {}",
			html.html
		);
		assert!(html.css.contains(&format!("@keyframes cv-k-{effect}{{")));
	}

	let css = to_html(
		&moving("wave"),
		&Arc::new(Theme::starter_kit()),
		&HtmlOptions::new(400.0),
	)
	.css;
	let reduced = css
		.split("@media (prefers-reduced-motion:reduce)")
		.nth(1)
		.expect("a reduced-motion rule");
	assert!(
		reduced.starts_with("{.cv-doc [style*=\"cv-k-\"]{animation:none!important}}"),
		"every animated run stops: {reduced}"
	);
}

/// Regression: a drawing in server HTML had no accessible name; its SVG
/// is hidden from assistive technology, so the figure was empty to it.
#[test]
fn server_html_names_drawings_as_images() {
	let html = to_html(
		&doc(json!([drawing(), { "type": "ink" }])),
		&Arc::new(Theme::starter_kit()),
		&HtmlOptions::new(500.0),
	);

	assert_eq!(
		html.html
			.matches("role=\"img\" aria-label=\"Drawing\"")
			.count(),
		2,
		"{}",
		html.html
	);
	assert!(html.html.contains("<svg class=\"cv-ink\""));
	assert!(
		html.html.contains("aria-hidden=\"true\""),
		"the strokes themselves are decoration"
	);
}

#[test]
fn malformed_strokes_are_skipped_and_empty_drawings_draw_nothing() {
	let doc = doc(json!([
		{ "type": "ink", "attrs": { "strokes": [] } },
		{
			"type": "ink",
			"attrs": {
				"height": 100,
				"strokes": [
					{ "points": [1, 2, "x"] },
					{ "points": [] },
					"not a stroke",
					{ "points": [10, 10, 20, 20], "color": "nope", "width": -3 },
				],
			},
		},
	]));
	let layout = layout(&doc, 1000.0);
	let list = layout.display_list(&DisplayOptions::viewport(0.0, layout.height()));
	let second = doc.children(None)[1];
	let rect = layout.block_rect(second).expect("a drawing");
	let drawn: Vec<(Vec<f32>, f32)> = strokes(&list);

	assert_eq!(rect.height, 100.0);
	assert_eq!(
		drawn.len(),
		2,
		"the two strokes with whole points: {drawn:?}"
	);
	assert_eq!(drawn[0].0, [rect.x + 1.0, rect.y + 2.0]);
	assert!(drawn[1].1 > 0.0, "a bad width falls back to the theme's");
	assert!(to_svg(&list).contains("<path"));
}
