//! Ink and kinetic text through the headless API: two Yjs peers drawing
//! and animating concurrently converge, drawings and kinetic marks survive
//! the shared state, and server-rendered HTML names drawings and keeps
//! kinetic text still for readers who prefer reduced motion.

#![cfg(all(feature = "full", feature = "collab"))]

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::HeadlessError;
use canvist_headless::Position;
use serde_json::Value as Json;
use serde_json::json;

fn engine() -> Engine {
	Engine::named(&[
		"heading",
		"bold",
		"italic",
		"history",
		"trailingNode",
		"ink",
		"kinetic",
	])
	.expect("ink and kinetic register by name")
}

/// Two peers sharing `doc`: the first as client 1, the second joined from
/// its state as client 2.
fn peers(doc: &Json) -> Result<(Doc, Doc), HeadlessError> {
	let engine = engine();
	let mut first = engine.from_json(doc)?;
	first.share(1)?;
	let second = engine.from_yjs(&first.encode_state()?, 2)?;
	first.take_updates()?;

	Ok((first, second))
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

	Ok(())
}

fn assert_converged(first: &Doc, second: &Doc) -> Result<(), HeadlessError> {
	assert_eq!(first.to_json(), second.to_json(), "the peers converge");
	assert!(first.validate()?.is_valid());
	assert!(second.validate()?.is_valid());
	Ok(())
}

fn drawings(count: usize) -> Json {
	let mut content: Vec<Json> = (0..count).map(|_| json!({ "type": "ink" })).collect();
	content.push(
		json!({ "type": "paragraph", "content": [{ "type": "text", "text": "Say hello world" }] }),
	);
	json!({ "type": "doc", "content": content })
}

fn stroke(doc: &mut Doc, ink: usize, points: &[f64]) -> Result<bool, HeadlessError> {
	doc.run_named("addInkStroke", &json!({ "ink": ink, "points": points }))
}

fn strokes(doc: &Doc) -> Result<Vec<i64>, HeadlessError> {
	Ok(doc
		.query("inks", &Json::Null)?
		.as_array()
		.into_iter()
		.flatten()
		.filter_map(|ink| ink["strokes"].as_i64())
		.collect())
}

#[test]
fn strokes_on_different_drawings_from_two_peers_both_survive() -> Result<(), HeadlessError> {
	let (mut first, mut second) = peers(&drawings(2))?;

	assert!(stroke(&mut first, 0, &[1.0, 2.0, 3.0, 4.0])?);
	assert!(stroke(&mut second, 1, &[5.0, 6.0])?);
	exchange(&mut first, &mut second)?;

	assert_converged(&first, &second)?;
	assert_eq!(strokes(&first)?, [1, 1]);

	// Taking turns on one drawing keeps every stroke.
	assert!(stroke(&mut first, 1, &[7.0, 8.0])?);
	exchange(&mut first, &mut second)?;
	assert!(stroke(&mut second, 1, &[9.0, 10.0])?);
	exchange(&mut first, &mut second)?;
	assert_converged(&first, &second)?;
	assert_eq!(strokes(&second)?, [1, 3]);

	assert!(second.undo()?);
	exchange(&mut first, &mut second)?;
	assert_converged(&first, &second)?;
	assert_eq!(strokes(&first)?, [1, 2], "undo removes only its own stroke");
	Ok(())
}

/// A drawing's strokes are one attribute, so two peers adding a stroke to
/// the same drawing at once converge on one peer's list: the other stroke
/// is lost. This pins convergence, and that the drawing keeps a stroke.
#[test]
fn concurrent_strokes_on_one_drawing_converge() -> Result<(), HeadlessError> {
	let (mut first, mut second) = peers(&drawings(1))?;

	assert!(stroke(&mut first, 0, &[1.0, 2.0])?);
	assert!(stroke(&mut second, 0, &[3.0, 4.0])?);
	exchange(&mut first, &mut second)?;

	assert_converged(&first, &second)?;
	assert!(strokes(&first)?[0] >= 1);
	Ok(())
}

#[test]
fn kinetic_marks_from_two_peers_converge() -> Result<(), HeadlessError> {
	let (mut first, mut second) = peers(&drawings(0))?;

	first.select_range(&Position::new([0], 0), &Position::new([0], 3))?;
	assert!(first.run_named("setKinetic", &json!({ "effect": "shake" }))?);
	second.select_range(&Position::new([0], 10), &Position::new([0], 15))?;
	assert!(second.run_named("setKinetic", &json!({ "effect": "rainbow" }))?);
	exchange(&mut first, &mut second)?;

	assert_converged(&first, &second)?;
	assert_eq!(
		first.export(Format::Html),
		"<p><span data-kinetic=\"shake\">Say</span> hello <span \
		 data-kinetic=\"rainbow\">world</span></p>"
	);

	second.select_range(&Position::new([0], 0), &Position::new([0], 15))?;
	assert!(second.run_named("unsetKinetic", &Json::Null)?);
	exchange(&mut first, &mut second)?;
	assert_converged(&first, &second)?;
	assert_eq!(first.export(Format::Html), "<p>Say hello world</p>");
	Ok(())
}

#[test]
fn drawings_and_kinetic_marks_survive_the_shared_state() -> Result<(), HeadlessError> {
	let (mut first, _) = peers(&drawings(1))?;
	first.run_named(
		"addInkStroke",
		&json!({ "ink": 0, "points": [1.25, 2.5, 999.5, 0.5], "color": "#123abc", "width": 2.2 }),
	)?;
	first.select_range(&Position::new([1], 4), &Position::new([1], 9))?;
	first.run_named("setKinetic", &json!({ "effect": "pulse" }))?;

	let joined = engine().from_yjs(&first.encode_state()?, 3)?;
	assert_eq!(joined.to_json(), first.to_json());
	assert_eq!(
		joined.to_json()["content"][0]["attrs"]["strokes"],
		json!([{ "points": [1.3, 2.5, 999.5, 0.5], "color": "#123abc", "width": 2.2 }])
	);
	Ok(())
}

#[cfg(feature = "ssr")]
#[test]
fn server_html_names_drawings_and_stills_kinetic_text_for_reduced_motion()
-> Result<(), HeadlessError> {
	let mut doc = engine().from_json(&drawings(1))?;
	stroke(&mut doc, 0, &[10.0, 10.0, 200.0, 50.0])?;
	doc.select_range(&Position::new([1], 4), &Position::new([1], 9))?;
	doc.run_named("setKinetic", &json!({ "effect": "wave" }))?;

	let page = canvist_headless::Renderer::new().html(&doc, 500.0);
	assert!(
		page.html.contains("role=\"img\" aria-label=\"Drawing\""),
		"a drawing is a named image: {}",
		page.html
	);
	assert!(
		page.html.contains("ease-in-out infinite;\">hello</span>"),
		"{}",
		page.html
	);
	assert!(
		page.css.contains(
			"@media (prefers-reduced-motion:reduce){.cv-doc \
			 [style*=\"cv-k-\"]{animation:none!important}}"
		),
		"{}",
		page.css
	);

	let svg = canvist_headless::Renderer::new().svg(&doc, 500.0)?;
	assert!(
		svg.contains("stroke-linecap=\"round\""),
		"the SVG draws the stroke"
	);
	assert!(
		svg.contains(">hello</text>"),
		"without a time, kinetic text is one still run of text in SVG"
	);
	Ok(())
}
