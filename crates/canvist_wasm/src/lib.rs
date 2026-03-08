//! # canvist_wasm
//!
//! WebAssembly + Canvas2D rendering backend for the canvist canvas editor.
//!
//! This crate is the browser entry point. It exposes a [`CanvistEditor`] class
//! to JavaScript via `wasm-bindgen` that manages a `<canvas>` element and
//! renders the document using the Canvas 2D API.
//!
//! # Usage from JavaScript
//!
//! ```js
//! import init, { CanvistEditor } from './canvist_wasm.js';
//!
//! await init();
//! const editor = CanvistEditor.create('my-canvas-id');
//! editor.insert_text('Hello from canvist!');
//! ```

mod canvas_renderer;
mod dom;

use canvist_core::Document;
use canvist_core::EditorEvent;
use canvist_core::EditorKey;
use canvist_core::EditorRuntime;
use canvist_core::EventSource;
use canvist_core::Modifiers;
use canvist_core::Position;
use canvist_core::Selection;
use canvist_core::Style;
use canvist_core::Transaction;
use canvist_core::operation::Operation;
use wasm_bindgen::prelude::*;

/// The main editor handle exposed to JavaScript.
///
/// Wraps a [`Document`] and a Canvas2D rendering backend. Create one per
/// `<canvas>` element.
#[wasm_bindgen]
pub struct CanvistEditor {
	runtime: EditorRuntime,
	canvas_id: String,
	event_source: dom::WebEventSource,
}

#[wasm_bindgen]
impl CanvistEditor {
	/// Create a new editor attached to the canvas element with the given ID.
	///
	/// # Errors
	///
	/// Returns an error if the canvas element is not found.
	#[wasm_bindgen]
	pub fn create(canvas_id: &str) -> Result<CanvistEditor, JsValue> {
		let window = web_sys::window().ok_or_else(|| JsValue::from_str("no global window"))?;
		let document = window
			.document()
			.ok_or_else(|| JsValue::from_str("no document"))?;

		let _canvas = document
			.get_element_by_id(canvas_id)
			.ok_or_else(|| JsValue::from_str(&format!("canvas '{canvas_id}' not found")))?;

		Ok(Self {
			runtime: EditorRuntime::new(
				Document::new(),
				Selection::collapsed(Position::new(0)),
				"wasm",
			),
			canvas_id: canvas_id.to_string(),
			event_source: dom::WebEventSource::new(),
		})
	}

	/// Insert text at the current cursor position (start of document).
	#[wasm_bindgen]
	pub fn insert_text(&mut self, text: &str) {
		self.event_source.push_text_input(text);
		self.process_events();
	}

	/// Insert text at a specific character offset.
	#[wasm_bindgen]
	pub fn insert_text_at(&mut self, offset: usize, text: &str) {
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(offset)),
		});
		let _ = self.runtime.handle_event(EditorEvent::TextInsert {
			text: text.to_string(),
		});
	}

	/// Delete a range of characters from `start` to `end`.
	#[wasm_bindgen]
	pub fn delete_range(&mut self, start: usize, end: usize) {
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(end)),
		});
		let key = if start == end {
			EditorKey::Backspace
		} else {
			EditorKey::Delete
		};
		let _ = self.runtime.handle_event(EditorEvent::KeyDown {
			key,
			modifiers: Modifiers::default(),
			repeat: false,
		});
	}

	/// Return the full plain-text content of the document.
	#[wasm_bindgen]
	pub fn plain_text(&self) -> String {
		self.runtime.document().plain_text()
	}

	/// Return the character count.
	#[wasm_bindgen]
	pub fn char_count(&self) -> usize {
		self.runtime.document().char_count()
	}

	/// Set the document title.
	#[wasm_bindgen]
	pub fn set_title(&mut self, title: &str) {
		self.runtime.document_mut().set_title(title);
	}

	/// Return the canvas element ID this editor is attached to.
	#[wasm_bindgen]
	pub fn canvas_id(&self) -> String {
		self.canvas_id.clone()
	}

	/// Export the document as a JSON string.
	#[wasm_bindgen]
	pub fn to_json(&self) -> Result<String, JsValue> {
		self.runtime
			.document()
			.to_json()
			.map_err(|e| JsValue::from_str(&e.to_string()))
	}

	/// Queue canonical text input and process it into operations.
	#[wasm_bindgen]
	pub fn queue_text_input(&mut self, text: &str) {
		self.event_source.push_text_input(text);
		self.process_events();
	}

	/// Queue a key down event and process resulting operations.
	#[wasm_bindgen]
	pub fn queue_key_down(&mut self, key: &str) {
		self.queue_key_down_with_modifiers(key, false, false, false, false, false);
	}

	/// Queue key down with explicit modifier + repeat state.
	#[wasm_bindgen]
	pub fn queue_key_down_with_modifiers(
		&mut self,
		key: &str,
		shift: bool,
		control: bool,
		alt: bool,
		meta: bool,
		repeat: bool,
	) {
		self.event_source.push_key_down(
			key,
			Modifiers {
				shift,
				control,
				alt,
				meta,
			},
			repeat,
		);
		self.process_events();
	}

	/// Process all pending canonical events via the editor runtime.
	#[wasm_bindgen]
	pub fn process_events(&mut self) {
		while let Some(event) = self.event_source.poll_event() {
			let _ = self.runtime.handle_event(event);
		}
	}

	/// Get selection start offset.
	#[wasm_bindgen]
	pub fn selection_start(&self) -> usize {
		self.runtime.selection().start().offset()
	}

	/// Get selection end offset.
	#[wasm_bindgen]
	pub fn selection_end(&self) -> usize {
		self.runtime.selection().end().offset()
	}

	/// Set selection range.
	#[wasm_bindgen]
	pub fn set_selection(&mut self, start: usize, end: usize) {
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(end)),
		});
	}

	/// Move cursor to an absolute position; extend toggles range selection.
	#[wasm_bindgen]
	pub fn move_cursor_to(&mut self, position: usize, extend: bool) {
		let _ = self.runtime.handle_event(EditorEvent::CursorMove {
			position: Position::new(position),
			extend,
		});
	}

	/// Move cursor one character left.
	#[wasm_bindgen]
	pub fn move_cursor_left(&mut self, extend: bool) {
		let caret = self.runtime.selection().end().offset();
		self.move_cursor_to(caret.saturating_sub(1), extend);
	}

	/// Move cursor one character right.
	#[wasm_bindgen]
	pub fn move_cursor_right(&mut self, extend: bool) {
		let caret = self.runtime.selection().end().offset();
		let max = self.runtime.document().char_count();
		self.move_cursor_to(caret.saturating_add(1).min(max), extend);
	}

	/// Apply style to the given character range.
	#[wasm_bindgen]
	pub fn apply_style_range(
		&mut self,
		start: usize,
		end: usize,
		bold: bool,
		italic: bool,
		underline: bool,
		font_size: Option<f32>,
		font_family: Option<String>,
		color_rgba: Option<Vec<u8>>,
	) {
		let mut style = Style::new();
		if bold {
			style = style.bold();
		}
		if italic {
			style = style.italic();
		}
		if underline {
			style = style.underline();
		}
		if let Some(size) = font_size {
			style = style.font_size(size);
		}
		if let Some(family) = font_family {
			style = style.font_family(family);
		}
		if let Some(rgba) = color_rgba {
			if rgba.len() == 4 {
				style = style.color(rgba[0], rgba[1], rgba[2], rgba[3]);
			}
		}

		self.runtime.apply_operation(Operation::format(
			Selection::range(Position::new(start), Position::new(end)),
			style,
		));
	}

	/// Undo by replaying operation log minus the last operation.
	#[wasm_bindgen]
	pub fn undo_last_operation(&mut self) -> Result<(), JsValue> {
		let entries = self.runtime.operation_log().entries().to_vec();
		if entries.is_empty() {
			return Ok(());
		}
		let mut document = Document::new();
		for entry in &entries[..entries.len() - 1] {
			entry.operation.clone().apply(&mut document);
		}
		self.runtime = EditorRuntime::new(document, Selection::collapsed(Position::new(0)), "wasm");
		Ok(())
	}

	/// Replay a JSON-encoded operation list into current runtime.
	#[wasm_bindgen]
	pub fn replay_operations_json(&mut self, operations_json: &str) -> Result<(), JsValue> {
		let tx: Transaction = serde_json::from_str(operations_json)
			.map_err(|e| JsValue::from_str(&format!("failed to parse transaction: {e}")))?;
		self.runtime.apply_transaction(tx);
		Ok(())
	}

	/// Request a re-render of the document to the canvas.
	///
	/// This reads the document state and draws it using the Canvas 2D API.
	#[wasm_bindgen]
	pub fn render(&self) -> Result<(), JsValue> {
		let window = web_sys::window().ok_or_else(|| JsValue::from_str("no global window"))?;
		let document = window
			.document()
			.ok_or_else(|| JsValue::from_str("no document"))?;
		let canvas = document
			.get_element_by_id(&self.canvas_id)
			.ok_or_else(|| JsValue::from_str("canvas not found"))?;
		let canvas: web_sys::HtmlCanvasElement = canvas
			.dyn_into()
			.map_err(|_| JsValue::from_str("element is not a canvas"))?;
		let ctx = canvas
			.get_context("2d")?
			.ok_or_else(|| JsValue::from_str("failed to get 2d context"))?
			.dyn_into::<web_sys::CanvasRenderingContext2d>()?;

		// Clear.
		let width = f64::from(canvas.width());
		let height = f64::from(canvas.height());
		ctx.clear_rect(0.0, 0.0, width, height);

		// Fill white background.
		ctx.set_fill_style_str("white");
		ctx.fill_rect(0.0, 0.0, width, height);

		// Draw the plain text as a simple proof-of-concept.
		let text = self.runtime.document().plain_text();
		let default_style = Style::new().font_size(16.0).font_family("sans-serif");
		let resolved = default_style.resolve();

		ctx.set_fill_style_str(&resolved.color.to_css());
		ctx.set_font(&format!(
			"{}px {}",
			resolved.font_size, resolved.font_family
		));
		ctx.fill_text(&text, 20.0, 40.0)
			.map_err(|e| JsValue::from_str(&format!("fill_text failed: {e:?}")))?;

		Ok(())
	}
}
