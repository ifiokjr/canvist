//! DOM interaction utilities.
//!
//! Helpers for working with the browser DOM — event listeners, accessibility
//! shadow DOM, and canonical editor input event mapping.

#![allow(dead_code)]

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
		"position: absolute; left: -9999px; top: -9999px; width: 1px; height: 1px; overflow: \
		 hidden;",
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

	pub fn push_key_down(&mut self, key: &str, modifiers: Modifiers, repeat: bool) {
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

	pub fn push_composition(&mut self, phase: CompositionPhase, text: impl Into<String>) {
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
	use super::*;

	#[test]
	fn map_dom_key_normalizes_named_and_character_keys() {
		assert_eq!(map_dom_key("Enter"), EditorKey::Enter);
		assert_eq!(map_dom_key("ArrowLeft"), EditorKey::ArrowLeft);
		assert_eq!(map_dom_key("a"), EditorKey::Character("a".to_string()));
		assert_eq!(map_dom_key("ß"), EditorKey::Character("ß".to_string()));
		assert_eq!(
			map_dom_key("Unidentified"),
			EditorKey::Unknown("Unidentified".to_string())
		);
	}

	#[test]
	fn web_event_source_preserves_fifo_order_and_shape() {
		let mut source = WebEventSource::new();
		let modifiers = Modifiers {
			shift: true,
			control: false,
			alt: true,
			meta: false,
		};

		source.push_focus();
		source.push_key_down("Enter", modifiers, true);
		source.push_text_input("x");
		source.push_key_up("Enter", modifiers);
		source.push_paste("clip");
		source.push_blur();

		assert_eq!(source.poll_event(), Some(EditorEvent::Focus));
		assert_eq!(
			source.poll_event(),
			Some(EditorEvent::KeyDown {
				key: EditorKey::Enter,
				modifiers,
				repeat: true,
			})
		);
		assert_eq!(
			source.poll_event(),
			Some(EditorEvent::TextInsert {
				text: "x".to_string(),
			})
		);
		assert_eq!(
			source.poll_event(),
			Some(EditorEvent::KeyUp {
				key: EditorKey::Enter,
				modifiers,
			})
		);
		assert_eq!(
			source.poll_event(),
			Some(EditorEvent::ClipboardPaste {
				text: "clip".to_string(),
			})
		);
		assert_eq!(source.poll_event(), Some(EditorEvent::Blur));
		assert_eq!(source.poll_event(), None);
	}

	#[test]
	fn web_event_source_golden_ime_sequence() {
		let mut source = WebEventSource::new();

		source.push_composition(CompositionPhase::Start, "");
		source.push_composition(CompositionPhase::Update, "に");
		source.push_composition(CompositionPhase::Update, "日本");
		source.push_composition(CompositionPhase::End, "日本");
		source.push_text_input("日本");

		let expected = vec![
			EditorEvent::Composition {
				phase: CompositionPhase::Start,
				text: "".to_string(),
			},
			EditorEvent::Composition {
				phase: CompositionPhase::Update,
				text: "に".to_string(),
			},
			EditorEvent::Composition {
				phase: CompositionPhase::Update,
				text: "日本".to_string(),
			},
			EditorEvent::Composition {
				phase: CompositionPhase::End,
				text: "日本".to_string(),
			},
			EditorEvent::TextInsert {
				text: "日本".to_string(),
			},
		];

		let mut actual = Vec::new();
		while let Some(event) = source.poll_event() {
			actual.push(event);
		}

		assert_eq!(actual, expected);
	}

	#[test]
	fn web_event_source_golden_pointer_sequence() {
		let mut source = WebEventSource::new();
		let modifiers = Modifiers {
			shift: false,
			control: true,
			alt: false,
			meta: false,
		};

		source.push_pointer(PointerPhase::Down, 7, 10.0, 20.0, 0.5, modifiers);
		source.push_pointer(PointerPhase::Move, 7, 15.5, 25.25, 0.75, modifiers);
		source.push_pointer(PointerPhase::Up, 7, 16.0, 26.0, 0.0, modifiers);

		let expected = vec![
			EditorEvent::Pointer(PointerEvent {
				phase: PointerPhase::Down,
				pointer_id: 7,
				x: 10.0,
				y: 20.0,
				pressure: 0.5,
				modifiers,
			}),
			EditorEvent::Pointer(PointerEvent {
				phase: PointerPhase::Move,
				pointer_id: 7,
				x: 15.5,
				y: 25.25,
				pressure: 0.75,
				modifiers,
			}),
			EditorEvent::Pointer(PointerEvent {
				phase: PointerPhase::Up,
				pointer_id: 7,
				x: 16.0,
				y: 26.0,
				pressure: 0.0,
				modifiers,
			}),
		];

		let mut actual = Vec::new();
		while let Some(event) = source.poll_event() {
			actual.push(event);
		}

		assert_eq!(actual, expected);
	}
}
