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
use canvist_core::Position;
use canvist_core::Style;
use wasm_bindgen::prelude::*;

/// The main editor handle exposed to JavaScript.
///
/// Wraps a [`Document`] and a Canvas2D rendering backend. Create one per
/// `<canvas>` element.
#[wasm_bindgen]
pub struct CanvistEditor {
	document: Document,
	canvas_id: String,
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
		let window =
			web_sys::window().ok_or_else(|| JsValue::from_str("no global window"))?;
		let document = window
			.document()
			.ok_or_else(|| JsValue::from_str("no document"))?;

		let _canvas = document
			.get_element_by_id(canvas_id)
			.ok_or_else(|| JsValue::from_str(&format!("canvas '{canvas_id}' not found")))?;

		Ok(Self {
			document: Document::new(),
			canvas_id: canvas_id.to_string(),
		})
	}

	/// Insert text at the current cursor position (start of document).
	#[wasm_bindgen]
	pub fn insert_text(&mut self, text: &str) {
		let position = Position::zero();
		self.document.insert_text(position, text);
	}

	/// Insert text at a specific character offset.
	#[wasm_bindgen]
	pub fn insert_text_at(&mut self, offset: usize, text: &str) {
		let position = Position::new(offset);
		self.document.insert_text(position, text);
	}

	/// Delete a range of characters from `start` to `end`.
	#[wasm_bindgen]
	pub fn delete_range(&mut self, start: usize, end: usize) {
		let selection = canvist_core::Selection::range(
			Position::new(start),
			Position::new(end),
		);
		self.document.delete(&selection);
	}

	/// Return the full plain-text content of the document.
	#[wasm_bindgen]
	pub fn plain_text(&self) -> String {
		self.document.plain_text()
	}

	/// Return the character count.
	#[wasm_bindgen]
	pub fn char_count(&self) -> usize {
		self.document.char_count()
	}

	/// Set the document title.
	#[wasm_bindgen]
	pub fn set_title(&mut self, title: &str) {
		self.document.set_title(title);
	}

	/// Return the canvas element ID this editor is attached to.
	#[wasm_bindgen]
	pub fn canvas_id(&self) -> String {
		self.canvas_id.clone()
	}

	/// Export the document as a JSON string.
	#[wasm_bindgen]
	pub fn to_json(&self) -> Result<String, JsValue> {
		self.document
			.to_json()
			.map_err(|e| JsValue::from_str(&e.to_string()))
	}

	/// Request a re-render of the document to the canvas.
	///
	/// This reads the document state and draws it using the Canvas 2D API.
	#[wasm_bindgen]
	pub fn render(&self) -> Result<(), JsValue> {
		let window =
			web_sys::window().ok_or_else(|| JsValue::from_str("no global window"))?;
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
		let text = self.document.plain_text();
		let default_style = Style::new().font_size(16.0).font_family("sans-serif");
		let resolved = default_style.resolve();

		ctx.set_fill_style_str(&resolved.color.to_css());
		ctx.set_font(&format!("{}px {}", resolved.font_size, resolved.font_family));
		ctx.fill_text(&text, 20.0, 40.0)
			.map_err(|e| JsValue::from_str(&format!("fill_text failed: {e:?}")))?;

		Ok(())
	}
}
