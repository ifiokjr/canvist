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

use canvist_core::Color;
use canvist_core::Document;
use canvist_core::EditorEvent;
use canvist_core::EditorRuntime;
use canvist_core::EventSource;
use canvist_core::Modifiers;
use canvist_core::Position;
use canvist_core::Selection;
use canvist_core::Style;
use canvist_core::Transaction;
use canvist_core::layout::LayoutConfig;
use canvist_core::layout::TextFragment;
use canvist_core::layout::TextMeasure;
use canvist_core::layout::hit_test_point;
use canvist_core::layout::layout_paragraph;
use canvist_core::operation::Operation;
use canvist_render::Canvas;
use canvist_render::Rect;
use canvist_render::Viewport;
use wasm_bindgen::prelude::*;

use crate::canvas_renderer::Canvas2dRenderer;

/// Shared layout configuration constants used by both `render()` and
/// `hit_test()`. Extracted here so the values are defined once and stay
/// in sync.
struct LayoutConstants {
	/// Horizontal padding from canvas edge to content area (pixels).
	padding_x: f32,
	/// Vertical padding from canvas edge to content area (pixels).
	padding_y: f32,
	/// Default text style applied when no explicit styling is present.
	default_style: Style,
	/// Layout engine configuration derived from the above (embeds `max_width`
	/// computed from the canvas width and padding).
	layout_config: LayoutConfig,
}

impl LayoutConstants {
	/// Create layout constants for a canvas of the given pixel dimensions.
	fn new(canvas_width: f32) -> Self {
		let padding_x: f32 = 20.0;
		let padding_y: f32 = 20.0;
		let content_width = (canvas_width - padding_x * 2.0).max(100.0);
		let default_style = Style::new().font_size(16.0).font_family("sans-serif");
		let layout_config = LayoutConfig {
			max_width: content_width,
			default_style: default_style.clone(),
		};

		Self {
			padding_x,
			padding_y,
			default_style,
			layout_config,
		}
	}
}

/// Extra vertical spacing added between consecutive paragraphs (pixels).
const PARAGRAPH_SPACING: f32 = 8.0;

/// Computed layout for a single paragraph within a multi-paragraph document.
struct ParagraphLayoutInfo {
	/// The paragraph layout (line breaks, widths, heights).
	layout: canvist_core::layout::ParagraphLayout,
	/// Global character offset where this paragraph starts in the document.
	global_char_start: usize,
	/// Character count of this paragraph (excluding the `\n` separator).
	char_count: usize,
	/// Vertical offset of this paragraph's first line within the content area.
	y_offset: f32,
	/// Styled runs belonging to this paragraph, with offsets relative to the
	/// paragraph (not the document). Each entry is
	/// `(text, style, paragraph-local offset, char count)`.
	local_runs: Vec<(String, Style, usize, usize)>,
	/// The paragraph's plain text (no `\n`).
	text: String,
}

/// Split the document into per-paragraph data and lay out each one.
///
/// Returns a vec of [`ParagraphLayoutInfo`] structs, one per paragraph. The
/// `\n` characters that separate paragraphs in `plain_text` are consumed as
/// boundaries but are not part of any paragraph's text or layout.
fn layout_paragraphs(
	plain_text: &str,
	styled_runs: &[(String, Style, usize, usize)],
	layout_config: &LayoutConfig,
	measurer: &dyn TextMeasure,
	default_style: &Style,
) -> Vec<ParagraphLayoutInfo> {
	// Split the plain text on '\n' to determine paragraph boundaries.
	let para_texts: Vec<&str> = plain_text.split('\n').collect();

	let mut result = Vec::with_capacity(para_texts.len());
	let mut global_offset = 0usize; // running character offset through the doc
	let mut y_offset = 0.0f32;

	for (para_idx, para_text) in para_texts.iter().enumerate() {
		let para_char_count = para_text.chars().count();
		let para_start = global_offset;
		let para_end = para_start + para_char_count;

		// Collect styled runs that overlap this paragraph and remap their
		// offsets to be paragraph-local.
		let local_runs: Vec<(String, Style, usize, usize)> = styled_runs
			.iter()
			.filter_map(|(text, style, run_offset, run_len)| {
				let run_start = *run_offset;
				let run_end = run_start + *run_len;

				// Does this run overlap [para_start, para_end)?
				if run_start >= para_end || run_end <= para_start {
					return None;
				}

				// Compute overlap.
				let overlap_start = run_start.max(para_start);
				let overlap_end = run_end.min(para_end);
				if overlap_start >= overlap_end {
					return None;
				}

				// Slice the run text to the overlap range.
				let local_run_start = overlap_start - run_start;
				let local_run_end = overlap_end - run_start;
				let run_chars: Vec<char> = text.chars().collect();
				let sliced_text: String = run_chars
					[local_run_start..local_run_end.min(run_chars.len())]
					.iter()
					.collect();
				let sliced_len = sliced_text.chars().count();

				// Paragraph-local offset.
				let local_offset = overlap_start - para_start;

				Some((sliced_text, style.clone(), local_offset, sliced_len))
			})
			.collect();

		// Build fragments for this paragraph.
		let fragments: Vec<TextFragment<'_>> = if local_runs.is_empty() {
			vec![TextFragment {
				text: para_text,
				style: default_style,
			}]
		} else {
			local_runs
				.iter()
				.map(|(text, style, _off, _len)| {
					TextFragment {
						text: text.as_str(),
						style,
					}
				})
				.collect()
		};

		let layout = layout_paragraph(&fragments, layout_config, measurer);

		result.push(ParagraphLayoutInfo {
			layout,
			global_char_start: para_start,
			char_count: para_char_count,
			y_offset,
			local_runs,
			text: para_text.to_string(),
		});

		// Advance y_offset for next paragraph.
		let para_height = result.last().unwrap().layout.total_height;
		y_offset += para_height;
		// Add paragraph spacing (except after the last paragraph, but it
		// doesn't matter — it just adds trailing space).
		if para_idx + 1 < para_texts.len() {
			y_offset += PARAGRAPH_SPACING;
		}

		// Advance global offset past this paragraph's text + the '\n'.
		global_offset = para_end;
		if para_idx + 1 < para_texts.len() {
			global_offset += 1; // skip the '\n' separator
		}
	}

	result
}

/// Find which paragraph and layout line a global character offset falls on.
///
/// Returns `(paragraph_index, line_index_within_paragraph)`.
fn find_para_and_line_for_offset(
	paragraphs: &[ParagraphLayoutInfo],
	offset: usize,
) -> (usize, usize) {
	for (pi, para) in paragraphs.iter().enumerate() {
		let para_end = para.global_char_start + para.char_count;
		// The caret can be at the very end of the last paragraph.
		let is_last = pi == paragraphs.len() - 1;

		if offset < para_end || (is_last && offset <= para_end) {
			let local_offset = offset.saturating_sub(para.global_char_start);
			for (li, line) in para.layout.lines.iter().enumerate() {
				let is_last_line = li == para.layout.lines.len() - 1;
				if local_offset < line.end_offset
					|| (is_last_line && local_offset <= line.end_offset)
				{
					return (pi, li);
				}
			}
			// Fallback: last line of this paragraph.
			return (pi, para.layout.lines.len().saturating_sub(1));
		}
	}
	// Fallback: last line of last paragraph.
	let last_para = paragraphs.len().saturating_sub(1);
	let last_line = paragraphs
		.last()
		.map(|p| p.layout.lines.len().saturating_sub(1))
		.unwrap_or(0);
	(last_para, last_line)
}

/// The main editor handle exposed to JavaScript.
///
/// Wraps a [`Document`] and a Canvas2D rendering backend. Create one per
/// `<canvas>` element.
#[wasm_bindgen]
pub struct CanvistEditor {
	runtime: EditorRuntime,
	canvas_id: String,
	event_source: dom::WebEventSource,
	/// Whether the caret is currently visible (used for blink toggling from JS).
	caret_visible: bool,
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
			caret_visible: true,
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
		let _ = self
			.runtime
			.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
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

	/// Undo the most recent transaction.
	///
	/// Applies inverse operations to restore the document to its previous state.
	/// Returns `true` if an undo was performed, `false` if the undo stack was empty.
	#[wasm_bindgen]
	pub fn undo(&mut self) -> bool {
		self.runtime.undo()
	}

	/// Redo the most recently undone transaction.
	///
	/// Re-applies the forward operations that were undone. Returns `true` if a
	/// redo was performed, `false` if the redo stack was empty.
	#[wasm_bindgen]
	pub fn redo(&mut self) -> bool {
		self.runtime.redo()
	}

	/// Whether there are entries on the undo stack.
	#[wasm_bindgen]
	pub fn can_undo(&self) -> bool {
		self.runtime.can_undo()
	}

	/// Whether there are entries on the redo stack.
	#[wasm_bindgen]
	pub fn can_redo(&self) -> bool {
		self.runtime.can_redo()
	}

	/// Return the currently selected text (empty string if selection is collapsed).
	#[wasm_bindgen]
	pub fn get_selected_text(&self) -> String {
		let selection = self.runtime.selection();
		if selection.is_collapsed() {
			return String::new();
		}
		let start = selection.start().offset();
		let end = selection.end().offset();
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let s = start.min(chars.len());
		let e = end.min(chars.len());
		if s >= e {
			return String::new();
		}
		chars[s..e].iter().collect()
	}

	/// Perform a clipboard cut: delete the current selection.
	///
	/// The caller is expected to have already read `get_selected_text()` and
	/// written it to the system clipboard before calling this method.
	#[wasm_bindgen]
	pub fn clipboard_cut(&mut self) {
		let selection = self.runtime.selection();
		if selection.is_collapsed() {
			return;
		}
		let start = selection.start().offset();
		let end = selection.end().offset();
		self.delete_range(start, end);
	}

	/// Paste text at the current cursor position (replacing any selection).
	#[wasm_bindgen]
	pub fn clipboard_paste(&mut self, text: &str) {
		let _ = self.runtime.handle_event(EditorEvent::ClipboardPaste {
			text: text.to_string(),
		});
	}

	/// Select the entire document.
	#[wasm_bindgen]
	pub fn select_all(&mut self) {
		let len = self.runtime.document().char_count();
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(0), Position::new(len)),
		});
	}

	/// Select the word at the given character offset.
	#[wasm_bindgen]
	pub fn select_word_at(&mut self, offset: usize) {
		let (start, end) = self.runtime.document().word_at(offset);
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(end)),
		});
	}

	/// Find the previous word boundary from a character offset.
	#[wasm_bindgen]
	pub fn word_boundary_left(&self, offset: usize) -> usize {
		self.runtime.document().word_boundary_left(offset)
	}

	/// Find the next word boundary from a character offset.
	#[wasm_bindgen]
	pub fn word_boundary_right(&self, offset: usize) -> usize {
		self.runtime.document().word_boundary_right(offset)
	}

	/// Force-break the current undo coalescing chain.
	///
	/// Normally, rapid single-character inserts are merged into a single undo
	/// group so that `undo()` reverses a whole burst of typing at once. Call
	/// this method to ensure that the *next* insert starts a fresh undo
	/// group, even if it would otherwise be coalesced with the previous one.
	///
	/// Typical use-cases:
	/// - Before programmatic (non-user) edits, so they form their own undo
	///   entry.
	/// - After a focus change or explicit "save-point".
	#[wasm_bindgen]
	pub fn break_undo_coalescing(&mut self) {
		self.runtime.break_undo_coalescing();
	}

	/// Set the undo-coalescing timeout in milliseconds.
	///
	/// Single-character inserts that arrive within this interval (and satisfy
	/// position/boundary checks) are merged into a single undo entry.
	/// Increasing this value makes undo steps coarser; decreasing it makes
	/// them finer.
	///
	/// The default is 500 ms.
	///
	/// # Arguments
	///
	/// - `ms` — timeout in milliseconds (as `f64` because JS numbers are
	///   doubles; the value is truncated to `u64` internally).
	#[wasm_bindgen]
	pub fn set_coalesce_timeout(&mut self, ms: f64) {
		self.runtime.set_coalesce_timeout_ms(ms as u64);
	}

	/// Return the current undo-coalescing timeout in milliseconds.
	#[wasm_bindgen]
	pub fn coalesce_timeout(&self) -> f64 {
		self.runtime.coalesce_timeout_ms() as f64
	}

	/// Set whether the caret (text cursor) is visible.
	///
	/// Called by the JS blink controller on a 530 ms interval to toggle the
	/// caret on and off. When `visible` is `false`, `render()` skips drawing
	/// the caret line, producing the classic blinking effect.
	#[wasm_bindgen]
	pub fn set_caret_visible(&mut self, visible: bool) {
		self.caret_visible = visible;
	}

	/// Set the current wall-clock time (milliseconds since epoch) for the
	/// undo coalescing timer.
	///
	/// Call this with `Date.now()` before every user action so the runtime
	/// can measure real time gaps between keystrokes. Without this, the
	/// runtime falls back to its monotonic counter which doesn't reflect
	/// actual typing speed.
	#[wasm_bindgen]
	pub fn set_now_ms(&mut self, ms: f64) {
		self.runtime.set_now_ms(ms as u64);
	}

	/// Replay a JSON-encoded operation list into current runtime.
	#[wasm_bindgen]
	pub fn replay_operations_json(&mut self, operations_json: &str) -> Result<(), JsValue> {
		let tx: Transaction = serde_json::from_str(operations_json)
			.map_err(|e| JsValue::from_str(&format!("failed to parse transaction: {e}")))?;
		self.runtime.apply_transaction(tx);
		Ok(())
	}
}

// Private helper methods — not exported to JavaScript.
impl CanvistEditor {
	/// Obtain the `<canvas>` element and its 2D rendering context.
	fn canvas_and_context(
		&self,
	) -> Result<
		(
			web_sys::HtmlCanvasElement,
			web_sys::CanvasRenderingContext2d,
		),
		JsValue,
	> {
		let window = web_sys::window().ok_or_else(|| JsValue::from_str("no global window"))?;
		let html_doc = window
			.document()
			.ok_or_else(|| JsValue::from_str("no document"))?;
		let canvas_el = html_doc
			.get_element_by_id(&self.canvas_id)
			.ok_or_else(|| JsValue::from_str("canvas not found"))?;
		let canvas: web_sys::HtmlCanvasElement = canvas_el
			.dyn_into()
			.map_err(|_| JsValue::from_str("element is not a canvas"))?;
		let ctx = canvas
			.get_context("2d")?
			.ok_or_else(|| JsValue::from_str("failed to get 2d context"))?
			.dyn_into::<web_sys::CanvasRenderingContext2d>()?;
		Ok((canvas, ctx))
	}
}

#[wasm_bindgen]
impl CanvistEditor {
	/// Hit-test a screen-space point to determine the character offset at that
	/// position.
	///
	/// Converts screen coordinates to document coordinates (accounting for
	/// scroll and zoom via `Viewport`), then performs layout and walks the
	/// resulting lines/characters to find the closest inter-character boundary.
	///
	/// Returns a character offset suitable for setting the cursor position.
	///
	/// # Arguments
	///
	/// - `screen_x` — X coordinate in canvas/screen pixels
	/// - `screen_y` — Y coordinate in canvas/screen pixels
	/// Hit-test a screen-space point to determine the character offset at that
	/// position.
	///
	/// Converts screen coordinates to document coordinates (accounting for
	/// scroll and zoom via `Viewport`), then walks the multi-paragraph layout
	/// to find the closest inter-character boundary. Each paragraph is laid out
	/// independently, so the hit-test accounts for paragraph spacing.
	///
	/// Returns a character offset suitable for setting the cursor position.
	///
	/// # Arguments
	///
	/// - `screen_x` — X coordinate in canvas/screen pixels
	/// - `screen_y` — Y coordinate in canvas/screen pixels
	#[wasm_bindgen]
	pub fn hit_test(&self, screen_x: f64, screen_y: f64) -> Result<usize, JsValue> {
		let (canvas, ctx) = self.canvas_and_context()?;

		let width = canvas.width() as f32;
		let height = canvas.height() as f32;

		let renderer = Canvas2dRenderer::new(ctx, width, height);
		let lc = LayoutConstants::new(width);

		// Convert screen coords to document coords via viewport.
		let viewport = Viewport::new(width, height);
		let (doc_x, doc_y) = viewport.screen_to_document(screen_x as f32, screen_y as f32);

		// Coordinates relative to the content area origin.
		let content_x = doc_x - lc.padding_x;
		let content_y = doc_y - lc.padding_y;

		let doc = self.runtime.document();
		let styled_runs = doc.styled_runs();
		let plain_text = doc.plain_text();

		let paragraphs = layout_paragraphs(
			&plain_text,
			&styled_runs,
			&lc.layout_config,
			&renderer,
			&lc.default_style,
		);

		if paragraphs.is_empty() {
			return Ok(0);
		}

		// Find which paragraph the y coordinate falls into.
		let mut target_para_idx = paragraphs.len() - 1;
		for (i, para) in paragraphs.iter().enumerate() {
			let para_bottom = para.y_offset + para.layout.total_height;
			// Add half the paragraph spacing as the boundary between paragraphs.
			let boundary = if i + 1 < paragraphs.len() {
				para_bottom + PARAGRAPH_SPACING * 0.5
			} else {
				f32::MAX
			};
			if content_y < boundary {
				target_para_idx = i;
				break;
			}
		}

		let para = &paragraphs[target_para_idx];

		// Build fragments for hit-testing within this paragraph.
		let fragments: Vec<TextFragment<'_>> = if para.local_runs.is_empty() {
			vec![TextFragment {
				text: &para.text,
				style: &lc.default_style,
			}]
		} else {
			para.local_runs
				.iter()
				.map(|(text, style, _off, _len)| {
					TextFragment {
						text: text.as_str(),
						style,
					}
				})
				.collect()
		};

		// Adjust y coordinate to be relative to the paragraph origin.
		let para_local_y = content_y - para.y_offset;
		let local_offset =
			hit_test_point(content_x, para_local_y, &para.layout, &fragments, &renderer);

		// Convert paragraph-local offset to global document offset.
		let global_offset = para.global_char_start + local_offset;

		let max_offset = doc.char_count();
		Ok(global_offset.min(max_offset))
	}

	/// Request a re-render of the document to the canvas.
	///
	/// Performs multi-paragraph, multi-line text rendering with styled runs,
	/// selection highlights, and a blinking caret. Each paragraph in the
	/// document tree is laid out independently with configurable paragraph
	/// spacing between them.
	#[wasm_bindgen]
	pub fn render(&self) -> Result<(), JsValue> {
		let (canvas, ctx) = self.canvas_and_context()?;

		let width = canvas.width() as f32;
		let height = canvas.height() as f32;

		let mut renderer = Canvas2dRenderer::new(ctx, width, height);

		// Clear canvas to white.
		renderer.clear(Color::WHITE);

		let lc = LayoutConstants::new(width);
		let doc = self.runtime.document();
		let selection = self.runtime.selection();
		let styled_runs = doc.styled_runs();
		let plain_text = doc.plain_text();

		// Compute per-paragraph layouts.
		let paragraphs = layout_paragraphs(
			&plain_text,
			&styled_runs,
			&lc.layout_config,
			&renderer,
			&lc.default_style,
		);

		// Helper: compute the x-offset (pixels) from the start of a layout
		// line to a given paragraph-local character offset, using styled
		// runs for accurate measurement.
		let x_offset_in_para_line = |renderer: &Canvas2dRenderer,
		                             para: &ParagraphLayoutInfo,
		                             line_start: usize,
		                             target: usize|
		 -> f32 {
			if target <= line_start {
				return 0.0;
			}
			let para_chars: Vec<char> = para.text.chars().collect();
			if para.local_runs.is_empty() {
				// No styled runs — use default style.
				if line_start >= para_chars.len() {
					return 0.0;
				}
				let to = target.min(para_chars.len());
				let text: String = para_chars[line_start..to].iter().collect();
				return renderer.measure_text(&text, &lc.default_style);
			}
			let mut x = 0.0f32;
			for (run_text, run_style, run_offset, _run_len) in &para.local_runs {
				let run_start = *run_offset;
				let run_end = run_start + run_text.chars().count();

				let slice_start = line_start.max(run_start);
				let slice_end = target.min(run_end);

				if slice_start >= slice_end {
					continue;
				}

				let merged = lc.default_style.merge(run_style);
				if slice_start < para_chars.len() {
					let to_clamped = slice_end.min(para_chars.len());
					let text: String = para_chars[slice_start..to_clamped].iter().collect();
					x += renderer.measure_text(&text, &merged);
				}
			}
			x
		};

		// ── Selection highlights ─────────────────────────────────────────
		if !selection.is_collapsed() {
			let sel_start = selection.start().offset();
			let sel_end = selection.end().offset();
			let selection_color = Color::new(66, 133, 244, 80);

			for para in &paragraphs {
				let para_global_end = para.global_char_start + para.char_count;

				for line in &para.layout.lines {
					// Convert line offsets to global.
					let line_global_start = para.global_char_start + line.start_offset;
					let line_global_end = para.global_char_start + line.end_offset;

					let line_sel_start = sel_start.max(line_global_start);
					let line_sel_end = sel_end.min(line_global_end);

					if line_sel_start >= line_sel_end {
						continue;
					}

					// Convert back to paragraph-local for measurement.
					let local_sel_start = line_sel_start - para.global_char_start;
					let local_sel_end = line_sel_end - para.global_char_start;

					let x_start = lc.padding_x
						+ x_offset_in_para_line(
							&renderer,
							para,
							line.start_offset,
							local_sel_start,
						);
					let x_end = lc.padding_x
						+ x_offset_in_para_line(&renderer, para, line.start_offset, local_sel_end);

					let y = lc.padding_y + para.y_offset + line.y;
					renderer.fill_rect(
						Rect::new(x_start, y, x_end - x_start, line.height),
						selection_color,
					);
				}

				// If the selection spans across the '\n' after this paragraph,
				// draw a small highlight to indicate the newline is selected.
				if sel_start <= para_global_end
					&& sel_end > para_global_end
					&& para_global_end < doc.char_count()
				{
					if let Some(last_line) = para.layout.lines.last() {
						let x = lc.padding_x + last_line.width;
						let y = lc.padding_y + para.y_offset + last_line.y;
						renderer.fill_rect(Rect::new(x, y, 4.0, last_line.height), selection_color);
					}
				}
			}
		}

		// ── Render styled text runs per paragraph / per line ─────────────
		renderer.ctx().set_text_baseline("top");

		for para in &paragraphs {
			let para_chars: Vec<char> = para.text.chars().collect();

			for line in &para.layout.lines {
				let line_start = line.start_offset;
				let line_end = line.end_offset;

				if line_start >= line_end {
					continue;
				}

				let line_y = lc.padding_y + para.y_offset + line.y;

				if para.local_runs.is_empty() {
					let line_text: String = para_chars[line_start..line_end.min(para_chars.len())]
						.iter()
						.collect();
					renderer.draw_text(lc.padding_x, line_y, &line_text, &lc.default_style);
				} else {
					let mut x = lc.padding_x;

					for (run_text, run_style, run_offset, _run_len) in &para.local_runs {
						let run_start = *run_offset;
						let run_end = run_start + run_text.chars().count();

						if run_end <= line_start || run_start >= line_end {
							continue;
						}

						let slice_start = line_start.max(run_start);
						let slice_end = line_end.min(run_end);

						if slice_start >= slice_end {
							continue;
						}

						let local_start = slice_start - run_start;
						let local_end = slice_end - run_start;
						let run_chars: Vec<char> = run_text.chars().collect();
						let slice_text: String = run_chars
							[local_start..local_end.min(run_chars.len())]
							.iter()
							.collect();

						let merged_style = lc.default_style.merge(run_style);
						let resolved = merged_style.resolve();
						let slice_width = renderer.measure_text(&slice_text, &merged_style);

						if resolved.background != Color::TRANSPARENT {
							renderer.fill_rect(
								Rect::new(x, line_y, slice_width, line.height),
								resolved.background,
							);
						}

						renderer.draw_text(x, line_y, &slice_text, &merged_style);

						if resolved.underline {
							let uy = line_y + line.height - 2.0;
							renderer.draw_line(x, uy, x + slice_width, uy, resolved.color);
						}

						if resolved.strikethrough {
							let sy = line_y + line.height * 0.5;
							renderer.draw_line(x, sy, x + slice_width, sy, resolved.color);
						}

						x += slice_width;
					}
				}
			}
		}

		// ── Caret ────────────────────────────────────────────────────────
		// Only draw the caret when the JS blink controller says it should be
		// visible. This lets the JS side toggle `set_caret_visible()` on a
		// 530 ms timer to produce the classic blinking effect.
		if self.caret_visible {
			let caret_offset = selection.end().offset();
			let caret_color = Color::BLACK;

			let (caret_para_idx, caret_line_idx) =
				find_para_and_line_for_offset(&paragraphs, caret_offset);

			if let Some(para) = paragraphs.get(caret_para_idx) {
				if let Some(caret_line) = para.layout.lines.get(caret_line_idx) {
					let local_caret = caret_offset.saturating_sub(para.global_char_start);
					let caret_x = lc.padding_x
						+ x_offset_in_para_line(
							&renderer,
							para,
							caret_line.start_offset,
							local_caret,
						);
					let caret_y = lc.padding_y + para.y_offset + caret_line.y;
					let caret_height = caret_line.height;

					renderer.draw_line(
						caret_x,
						caret_y,
						caret_x,
						caret_y + caret_height,
						caret_color,
					);
				}
			}
		}

		Ok(())
	}
}
