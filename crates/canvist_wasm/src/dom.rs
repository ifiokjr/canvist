//! DOM interaction utilities.
//!
//! Helpers for working with the browser DOM — event listeners, accessibility
//! shadow DOM, and canonical editor input event mapping.

use std::collections::VecDeque;

use canvist_core::CompositionPhase;
use canvist_core::EditorEvent;
use canvist_core::EditorKey;
use canvist_core::EventSource;
use canvist_core::Modifiers;
use canvist_core::PointerEvent;
use canvist_core::PointerPhase;
use wasm_bindgen::prelude::*;
use web_sys::Document;
use web_sys::Element;

/// Create a hidden accessibility container element.
///
/// This creates an off-screen `<div>` that mirrors the document structure
/// as plain HTML elements, allowing screen readers to interpret the content.
/// This is the same technique used by Google Docs.
///
/// # Errors
///
/// Returns an error if DOM manipulation fails.
#[allow(dead_code)]
pub fn create_accessibility_container(document: &Document) -> Result<Element, JsValue> {
	let container = document.create_element("div")?;
	container.set_attribute("role", "textbox")?;
	container.set_attribute("aria-multiline", "true")?;
	container.set_attribute("aria-label", "Document editor")?;

	// Position off-screen but still accessible to screen readers.
	container.set_attribute(
		"style",
		"position: absolute; left: -9999px; top: -9999px; width: 1px; height: 1px; overflow: hidden;",
	)?;

	Ok(container)
}

/// Web implementation of [`EventSource`].
///
/// `WebEventSource` receives DOM and hidden-input callbacks and maps them into
/// canonical [`EditorEvent`] values consumed by the core editor pipeline.
#[derive(Debug, Default)]
pub struct WebEventSource {
	queue: VecDeque<EditorEvent>,
}

impl WebEventSource {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	pub fn push_text_input(&mut self, text: impl Into<String>) {
		self.queue
			.push_back(EditorEvent::TextInsert { text: text.into() });
	}

	pub fn push_key_down(
		&mut self,
		key: &str,
		modifiers: Modifiers,
		repeat: bool,
	) {
		self.queue.push_back(EditorEvent::KeyDown {
			key: map_dom_key(key),
			modifiers,
			repeat,
		});
	}

	pub fn push_key_up(&mut self, key: &str, modifiers: Modifiers) {
		self.queue.push_back(EditorEvent::KeyUp {
			key: map_dom_key(key),
			modifiers,
		});
	}

	pub fn push_pointer(
		&mut self,
		phase: PointerPhase,
		pointer_id: i64,
		x: f64,
		y: f64,
		pressure: f32,
		modifiers: Modifiers,
	) {
		self.queue.push_back(EditorEvent::Pointer(PointerEvent {
			phase,
			pointer_id,
			x,
			y,
			pressure,
			modifiers,
		}));
	}

	pub fn push_composition(
		&mut self,
		phase: CompositionPhase,
		text: impl Into<String>,
	) {
		self.queue.push_back(EditorEvent::Composition {
			phase,
			text: text.into(),
		});
	}

	pub fn push_paste(&mut self, text: impl Into<String>) {
		self.queue
			.push_back(EditorEvent::ClipboardPaste { text: text.into() });
	}

	pub fn push_focus(&mut self) {
		self.queue.push_back(EditorEvent::Focus);
	}

	pub fn push_blur(&mut self) {
		self.queue.push_back(EditorEvent::Blur);
	}
}

impl EventSource for WebEventSource {
	fn poll_event(&mut self) -> Option<EditorEvent> {
		self.queue.pop_front()
	}
}

/// Native/mobile adapter example that feeds the same canonical event stream.
#[derive(Debug, Default)]
pub struct NativeEventSource {
	queue: VecDeque<EditorEvent>,
}

impl NativeEventSource {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	pub fn push_mobile_text_delta(&mut self, inserted: impl Into<String>) {
		self.queue.push_back(EditorEvent::TextInsert {
			text: inserted.into(),
		});
	}

	pub fn push_mobile_backspace(&mut self) {
		self.queue
			.push_back(EditorEvent::TextDeleteBackward { count: 1 });
	}

	pub fn push_mobile_composition_update(&mut self, text: impl Into<String>) {
		self.queue.push_back(EditorEvent::Composition {
			phase: CompositionPhase::Update,
			text: text.into(),
		});
	}
}

impl EventSource for NativeEventSource {
	fn poll_event(&mut self) -> Option<EditorEvent> {
		self.queue.pop_front()
	}
}

/// Maps a browser DOM KeyboardEvent key value into the canonical key enum.
#[must_use]
pub fn map_dom_key(key: &str) -> EditorKey {
	match key {
		"Enter" => EditorKey::Enter,
		"Tab" => EditorKey::Tab,
		"Backspace" => EditorKey::Backspace,
		"Delete" => EditorKey::Delete,
		"Escape" => EditorKey::Escape,
		"ArrowLeft" => EditorKey::ArrowLeft,
		"ArrowRight" => EditorKey::ArrowRight,
		"ArrowUp" => EditorKey::ArrowUp,
		"ArrowDown" => EditorKey::ArrowDown,
		"Home" => EditorKey::Home,
		"End" => EditorKey::End,
		"PageUp" => EditorKey::PageUp,
		"PageDown" => EditorKey::PageDown,
		_ if key.chars().count() == 1 => EditorKey::Character(key.to_string()),
		_ => EditorKey::Unknown(key.to_string()),
	}
}

/// Log a message to the browser console.
#[allow(dead_code)]
pub fn console_log(msg: &str) {
	web_sys::console::log_1(&JsValue::from_str(msg));
}
