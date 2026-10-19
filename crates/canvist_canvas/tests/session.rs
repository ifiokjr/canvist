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
