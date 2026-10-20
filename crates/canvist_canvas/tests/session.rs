//! Editing a text frame with the real canvist text editor.

#![cfg(feature = "editor")]

mod support;

use std::time::Duration;

use canvist_canvas::Canvas;
use canvist_canvas::DesignKit;
use canvist_canvas::Modifiers;
use canvist_canvas::OverlayItem;
use canvist_canvas::TextSession;
use canvist_editor::Editor;
use canvist_editor::EditorBuilder;
use canvist_scene::Scene;

/// A StarterKit editor builder: what both the scene's text schema and the
/// session's editor come from.
fn starter_kit() -> EditorBuilder {
	let mut builder = Editor::builder();
	canvist_extensions::StarterKit::add_all(&mut builder);
	builder
}

fn canvas() -> Canvas {
	let text = starter_kit().schema().expect("starter kit schema");
	let mut builder = Canvas::builder().text_schema(text);
	DesignKit::register(&mut builder);
	let scene = Scene::new(builder.schema().expect("schema"), 1).expect("scene");
	builder.build(scene, support::texts()).expect("canvas")
}

fn plain_text(canvas: &Canvas, node: canvist_scene::NodeId) -> String {
	canvas
		.state()
		.scene()
		.text(node)
		.map(canvist_model::Document::plain_text)
		.unwrap_or_default()
}

#[test]
fn typing_in_a_frame_uses_the_text_editor() -> Result<(), Box<dyn std::error::Error>> {
	let mut canvas = canvas();
	let page = support::page(&canvas);
	let (node, _) = canvas.state_mut().edit(false, |transaction| {
		transaction.insert(page, None, "text", canvist_scene::Props::default())
	})?;
	let state = canvas.state_mut();
	let mut session = TextSession::open(state, node, starter_kit())?;
	let now = Duration::ZERO;

	session.type_text(state, "Hello", now)?;
	session.press(state, "Enter", Modifiers::default(), now)?;
	// Typed a key at a time, as a keyboard does, so input rules run.
	for key in "## Big".chars() {
		session.type_text(state, &key.to_string(), now)?;
	}

	assert!(state.scene().is_text_lent(node));
	assert!(
		session
			.overlay(state)
			.iter()
			.any(|item| matches!(item, OverlayItem::Caret { .. }))
	);

	// Text frames grow with their text while editing.
	let height = state
		.index()
		.get(node)
		.map(|geometry| geometry.size.y)
		.unwrap_or_default();
	assert!(height > 30.0, "two lines tall, got {height}");

	session.press(state, "Backspace", Modifiers::default(), now)?;
	session.close(state)?;
	// The StarterKit's trailing node keeps an empty paragraph after a final
	// heading.
	assert_eq!(plain_text(&canvas, node), "Hello\nBi\n");

	// The heading input rule ran: the second block is a heading.
	let json = canvas
		.state()
		.scene()
		.text(node)
		.map(canvist_model::Document::to_json)
		.unwrap_or_default();
	assert_eq!(json["content"][1]["type"], "heading");
	Ok(())
}

/// Where the clicked frame sits, in world units: the default camera shows
/// it at the same screen position.
const FRAME_ORIGIN: kurbo::Vec2 = kurbo::Vec2::new(100.0, 100.0);
/// The clicked frame's width.
const FRAME_WIDTH: f64 = 300.0;

/// A text frame at [`FRAME_ORIGIN`] holding `text`.
fn frame_with(
	canvas: &mut Canvas,
	text: &str,
) -> Result<canvist_scene::NodeId, Box<dyn std::error::Error>> {
	let page = support::page(canvas);
	let props = canvist_scene::Props {
		transform: kurbo::Affine::translate(FRAME_ORIGIN),
		size: kurbo::Vec2::new(FRAME_WIDTH, 0.0),
		..canvist_scene::Props::default()
	};
	let (node, _) = canvas.state_mut().edit(false, |transaction| {
		let node = transaction.insert(page, None, "text", props)?;
		transaction.text(node, |inner| {
			let block = inner
				.doc()
				.first_text_block()
				.expect("a new frame has a paragraph");
			inner.insert_text(
				canvist_model::Point::new(block, 0),
				text,
				canvist_model::MarkSet::empty(),
			)
		})?;
		Ok(node)
	})?;
	Ok(node)
}

#[test]
fn clicks_place_the_caret_and_shift_clicks_select() -> Result<(), Box<dyn std::error::Error>> {
	let mut canvas = canvas();
	let node = frame_with(&mut canvas, "Hello world")?;
	let state = canvas.state_mut();
	let mut session = TextSession::open(state, node, starter_kit())?;
	let start = kurbo::Point::new(FRAME_ORIGIN.x + 1.0, FRAME_ORIGIN.y + 5.0);
	let end = kurbo::Point::new(FRAME_ORIGIN.x + FRAME_WIDTH - 1.0, FRAME_ORIGIN.y + 5.0);

	assert!(session.contains(state, start));
	assert!(!session.contains(
		state,
		kurbo::Point::new(FRAME_ORIGIN.x - 10.0, FRAME_ORIGIN.y)
	));

	session.click(state, start, false, Duration::ZERO)?;
	let caret = session
		.editor()
		.state()
		.selection()
		.as_caret()
		.map(|point| point.offset);
	assert_eq!(caret, Some(0));

	session.click(state, end, true, Duration::ZERO)?;
	let selection = session.editor().state().selection();
	let canvist_model::Selection::Text { anchor, head } = selection else {
		panic!("expected a text selection, got {selection:?}");
	};
	assert_eq!(anchor.offset, 0);
	assert_eq!(head.offset, u32::try_from("Hello world".len())?);

	session.close(state)?;
	Ok(())
}

#[test]
fn a_failed_open_keeps_the_frames_text() -> Result<(), Box<dyn std::error::Error>> {
	let mut canvas = canvas();
	let node = frame_with(&mut canvas, "Keep me")?;

	// No extensions, so no paragraph type: the editor cannot hold the frame.
	let opened = TextSession::open(canvas.state_mut(), node, Editor::builder());

	assert!(opened.is_err());
	assert!(!canvas.state().scene().is_text_lent(node));
	assert_eq!(plain_text(&canvas, node), "Keep me");
	Ok(())
}
