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
use canvist_core::layout::LayoutLine;
use canvist_core::layout::TextFragment;
use canvist_core::layout::TextMeasure;
use canvist_core::layout::hit_test_point;
use canvist_core::layout::layout_paragraph;
use canvist_core::operation::Operation;
use canvist_render::Canvas;
use canvist_render::Rect;
use canvist_render::Viewport;
use wasm_bindgen::JsCast;
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
		let default_style = Style::new()
			.font_size(16.0)
			.font_family("Inter, system-ui, -apple-system, sans-serif");
		let layout_config = LayoutConfig {
			max_width: content_width,
			default_style: default_style.clone(),
			text_align: canvist_core::style::TextAlign::Left,
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

/// Build [`TextFragment`] slices from local runs for layout helpers.
fn build_fragments<'a>(
	text: &'a str,
	local_runs: &'a [(String, Style, usize, usize)],
	default_style: &'a Style,
) -> Vec<TextFragment<'a>> {
	if local_runs.is_empty() {
		return vec![TextFragment {
			text,
			style: default_style,
		}];
	}
	// Build fragments that align with the run structure.
	// We need owned merged styles, so leak them into a Vec we keep alive.
	// Since this is transient per-call, the small allocation is fine.
	local_runs
		.iter()
		.map(|(run_text, _run_style, _offset, _len)| {
			TextFragment {
				text: run_text.as_str(),
				style: default_style, // simplified: use default for position math
			}
		})
		.collect()
}

/// Find the style for a character offset within a list of fragments.
fn find_style_at<'a>(fragments: &[TextFragment<'a>], offset: usize) -> Option<&'a Style> {
	let mut pos = 0;
	for frag in fragments {
		let len = frag.text.chars().count();
		if offset < pos + len {
			return Some(frag.style);
		}
		pos += len;
	}
	fragments.last().map(|f| f.style)
}

/// Proxy for `canvist_core::layout::hit_x_on_line` (which is private).
/// Finds the character offset closest to `target_x` on a single line.
fn hit_x_on_line_ext(
	line: &LayoutLine,
	target_x: f32,
	fragments: &[TextFragment<'_>],
	measurer: &dyn TextMeasure,
) -> usize {
	let full_text: String = fragments.iter().map(|f| f.text).collect();
	let chars: Vec<char> = full_text.chars().collect();
	let default_style = Style::new();
	let mut x = 0.0f32;
	for i in line.start_offset..line.end_offset.min(chars.len()) {
		let style = find_style_at(fragments, i).unwrap_or(&default_style);
		let w = measurer.measure_char(chars[i], style);
		if x + w * 0.5 > target_x {
			return i;
		}
		x += w;
	}
	line.end_offset
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
	/// Cached canvas width in logical pixels.
	width: f32,
	/// Cached canvas height in logical pixels.
	height: f32,
	/// Vertical scroll offset in logical pixels.
	scroll_y: f32,
	/// Whether the editor currently has focus.
	focused: bool,
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

		let canvas_el = document
			.get_element_by_id(canvas_id)
			.ok_or_else(|| JsValue::from_str(&format!("canvas '{canvas_id}' not found")))?;
		let canvas: web_sys::HtmlCanvasElement = canvas_el
			.dyn_into()
			.map_err(|_| JsValue::from_str("element is not a canvas"))?;

		Ok(Self {
			runtime: EditorRuntime::new(
				Document::new(),
				Selection::collapsed(Position::new(0)),
				"wasm",
			),
			canvas_id: canvas_id.to_string(),
			event_source: dom::WebEventSource::new(),
			caret_visible: true,
			width: canvas.width() as f32,
			height: canvas.height() as f32,
			scroll_y: 0.0,
			focused: true,
		})
	}

	/// Set the logical (CSS) dimensions of the editor canvas.
	///
	/// Call this after changing the canvas's CSS size so layout wrapping
	/// and hit-testing use the correct dimensions (not the DPR-scaled pixel
	/// dimensions).
	#[wasm_bindgen]
	pub fn set_size(&mut self, width: f32, height: f32) {
		self.width = width;
		self.height = height;
	}

	// ── Scroll ───────────────────────────────────────────────────────

	/// Get the current vertical scroll offset.
	#[wasm_bindgen]
	pub fn scroll_y(&self) -> f32 {
		self.scroll_y
	}

	/// Set the vertical scroll offset (clamped to valid range).
	#[wasm_bindgen]
	pub fn set_scroll_y(&mut self, y: f32) {
		let max = self.max_scroll_y();
		self.scroll_y = y.clamp(0.0, max);
	}

	/// Scroll by a delta (positive = down, negative = up).
	#[wasm_bindgen]
	pub fn scroll_by(&mut self, delta_y: f32) {
		self.set_scroll_y(self.scroll_y + delta_y);
	}

	/// Compute the total content height in logical pixels.
	///
	/// Uses the paragraph layout engine to determine the full document
	/// height including padding and paragraph spacing.
	#[wasm_bindgen]
	pub fn content_height(&self) -> Result<f32, JsValue> {
		let (_, ctx) = self.canvas_and_context()?;
		let renderer = Canvas2dRenderer::new(ctx, self.width, self.height);
		let lc = LayoutConstants::new(self.width);
		let plain_text = self.runtime.document().plain_text();
		let styled_runs = self.runtime.document().styled_runs();
		let paragraphs = layout_paragraphs(
			&plain_text,
			&styled_runs,
			&lc.layout_config,
			&renderer,
			&lc.default_style,
		);
		let last_y = paragraphs
			.last()
			.map(|p| p.y_offset + p.layout.total_height)
			.unwrap_or(0.0);
		Ok(last_y + lc.padding_y * 2.0)
	}

	/// Compute the Y position of the caret in content coordinates.
	///
	/// Returns `(y, height)` for the caret line. Useful for scroll-into-view.
	#[wasm_bindgen]
	pub fn caret_y(&self) -> Result<Vec<f32>, JsValue> {
		let (_, ctx) = self.canvas_and_context()?;
		let renderer = Canvas2dRenderer::new(ctx, self.width, self.height);
		let lc = LayoutConstants::new(self.width);
		let plain_text = self.runtime.document().plain_text();
		let styled_runs = self.runtime.document().styled_runs();
		let paragraphs = layout_paragraphs(
			&plain_text,
			&styled_runs,
			&lc.layout_config,
			&renderer,
			&lc.default_style,
		);
		let caret_offset = self.runtime.selection().end().offset();
		let (para_idx, line_idx) = find_para_and_line_for_offset(&paragraphs, caret_offset);
		if let Some(para) = paragraphs.get(para_idx) {
			if let Some(line) = para.layout.lines.get(line_idx) {
				let y = lc.padding_y + para.y_offset + line.y;
				return Ok(vec![y, line.height]);
			}
		}
		Ok(vec![lc.padding_y, 24.0])
	}

	// ── Focus ────────────────────────────────────────────────────────

	/// Set whether the editor has focus.
	///
	/// When unfocused, the caret is drawn as a gray line and selection
	/// uses a lighter highlight color.
	#[wasm_bindgen]
	pub fn set_focused(&mut self, focused: bool) {
		self.focused = focused;
	}

	/// Get the current focus state.
	#[wasm_bindgen]
	pub fn focused(&self) -> bool {
		self.focused
	}

	// ── Statistics ───────────────────────────────────────────────────

	/// Count the number of words (whitespace-separated tokens).
	#[wasm_bindgen]
	pub fn word_count(&self) -> usize {
		self.runtime.document().word_count()
	}

	/// Count the number of visual lines using the paragraph layout engine.
	#[wasm_bindgen]
	pub fn line_count(&self) -> Result<usize, JsValue> {
		let (_, ctx) = self.canvas_and_context()?;
		let renderer = Canvas2dRenderer::new(ctx, self.width, self.height);
		let lc = LayoutConstants::new(self.width);
		let plain_text = self.runtime.document().plain_text();
		let styled_runs = self.runtime.document().styled_runs();
		let paragraphs = layout_paragraphs(
			&plain_text,
			&styled_runs,
			&lc.layout_config,
			&renderer,
			&lc.default_style,
		);
		Ok(paragraphs.iter().map(|p| p.layout.lines.len()).sum())
	}

	/// Return the 1-based visual line number the caret is on.
	#[wasm_bindgen]
	pub fn cursor_line(&self) -> Result<usize, JsValue> {
		let (_, ctx) = self.canvas_and_context()?;
		let renderer = Canvas2dRenderer::new(ctx, self.width, self.height);
		let lc = LayoutConstants::new(self.width);
		let plain_text = self.runtime.document().plain_text();
		let styled_runs = self.runtime.document().styled_runs();
		let paragraphs = layout_paragraphs(
			&plain_text,
			&styled_runs,
			&lc.layout_config,
			&renderer,
			&lc.default_style,
		);
		let caret = self.runtime.selection().end().offset();
		let (para_idx, line_idx) = find_para_and_line_for_offset(&paragraphs, caret);
		// Count all lines in previous paragraphs + line_idx in this one.
		let mut total = 0;
		for (i, para) in paragraphs.iter().enumerate() {
			if i < para_idx {
				total += para.layout.lines.len();
			} else {
				total += line_idx + 1;
				break;
			}
		}
		Ok(total)
	}

	/// Return the 1-based column (character position within the visual line).
	#[wasm_bindgen]
	pub fn cursor_column(&self) -> Result<usize, JsValue> {
		let (_, ctx) = self.canvas_and_context()?;
		let renderer = Canvas2dRenderer::new(ctx, self.width, self.height);
		let lc = LayoutConstants::new(self.width);
		let plain_text = self.runtime.document().plain_text();
		let styled_runs = self.runtime.document().styled_runs();
		let paragraphs = layout_paragraphs(
			&plain_text,
			&styled_runs,
			&lc.layout_config,
			&renderer,
			&lc.default_style,
		);
		let caret = self.runtime.selection().end().offset();
		let (para_idx, line_idx) = find_para_and_line_for_offset(&paragraphs, caret);
		if let Some(para) = paragraphs.get(para_idx) {
			if let Some(line) = para.layout.lines.get(line_idx) {
				let local_caret = caret.saturating_sub(para.global_char_start);
				return Ok(local_caret - line.start_offset + 1);
			}
		}
		Ok(1)
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

	/// Export the document as HTML.
	///
	/// Each paragraph becomes a `<p>` element. Styled text gets inline
	/// elements (`<strong>`, `<em>`, `<u>`, `<s>`, `<span>`).
	#[wasm_bindgen]
	pub fn to_html(&self) -> String {
		self.runtime.document().to_html()
	}

	/// Export the document as Markdown.
	///
	/// Bold → `**text**`, italic → `*text*`, strikethrough → `~~text~~`.
	#[wasm_bindgen]
	pub fn to_markdown(&self) -> String {
		self.runtime.document().to_markdown()
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

	/// Toggle bold on the current selection.
	///
	/// If all characters in the selection are already bold, removes bold.
	/// Otherwise, applies bold. Preserves the current selection.
	#[wasm_bindgen]
	pub fn toggle_bold(&mut self) {
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return;
		}
		let start = sel.start().offset();
		let end = sel.end().offset();
		let all_bold = self.runtime.document().is_bold_in_range(start, end);
		let style = if all_bold {
			Style::new().font_weight(canvist_core::FontWeight::Normal)
		} else {
			Style::new().bold()
		};
		self.runtime.apply_operation(Operation::format(
			Selection::range(Position::new(start), Position::new(end)),
			style,
		));
		// Restore selection (apply_operation resets it).
		let _ = self
			.runtime
			.handle_event(EditorEvent::SelectionSet { selection: sel });
	}

	/// Toggle italic on the current selection. Preserves the current
	/// selection.
	#[wasm_bindgen]
	pub fn toggle_italic(&mut self) {
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return;
		}
		let start = sel.start().offset();
		let end = sel.end().offset();
		let all_italic = self.runtime.document().is_italic_in_range(start, end);
		// italic is an Option<bool> internally — set to false to remove.
		let style = if all_italic {
			Style::new() // default (no italic flag set) — clears italic on merge
		} else {
			Style::new().italic()
		};
		self.runtime.apply_operation(Operation::format(
			Selection::range(Position::new(start), Position::new(end)),
			style,
		));
		let _ = self
			.runtime
			.handle_event(EditorEvent::SelectionSet { selection: sel });
	}

	/// Toggle underline on the current selection. Preserves the current
	/// selection.
	#[wasm_bindgen]
	pub fn toggle_underline(&mut self) {
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return;
		}
		let start = sel.start().offset();
		let end = sel.end().offset();
		let all_underline = self.runtime.document().is_underline_in_range(start, end);
		let style = if all_underline {
			Style::new() // clears underline
		} else {
			Style::new().underline()
		};
		self.runtime.apply_operation(Operation::format(
			Selection::range(Position::new(start), Position::new(end)),
			style,
		));
		let _ = self
			.runtime
			.handle_event(EditorEvent::SelectionSet { selection: sel });
	}

	/// Check if the current selection is all bold.
	#[wasm_bindgen]
	pub fn is_bold(&self) -> bool {
		let sel = self.runtime.selection();
		self.runtime
			.document()
			.is_bold_in_range(sel.start().offset(), sel.end().offset())
	}

	/// Check if the current selection is all italic.
	#[wasm_bindgen]
	pub fn is_italic(&self) -> bool {
		let sel = self.runtime.selection();
		self.runtime
			.document()
			.is_italic_in_range(sel.start().offset(), sel.end().offset())
	}

	/// Check if the current selection is all underline.
	#[wasm_bindgen]
	pub fn is_underline(&self) -> bool {
		let sel = self.runtime.selection();
		self.runtime
			.document()
			.is_underline_in_range(sel.start().offset(), sel.end().offset())
	}

	/// Toggle strikethrough on the current selection.
	#[wasm_bindgen]
	pub fn toggle_strikethrough(&mut self) {
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return;
		}
		let start = sel.start().offset();
		let end = sel.end().offset();
		// Check if all characters in range have strikethrough.
		let all_strike = {
			let runs = self.runtime.document().styled_runs();
			let mut all = true;
			for (_, style, rs, rl) in &runs {
				let re = rs + rl;
				if re <= start || *rs >= end {
					continue;
				}
				if !style.resolve().strikethrough {
					all = false;
					break;
				}
			}
			all
		};
		let style = if all_strike {
			Style::new() // clears strikethrough
		} else {
			Style::new().strikethrough()
		};
		self.runtime.apply_operation(Operation::format(
			Selection::range(Position::new(start), Position::new(end)),
			style,
		));
		let _ = self
			.runtime
			.handle_event(EditorEvent::SelectionSet { selection: sel });
	}

	/// Find all occurrences of `needle`. Returns a flat array: [start0, end0,
	/// start1, end1, …].
	#[wasm_bindgen]
	pub fn find_all(&self, needle: &str, case_sensitive: bool) -> Vec<usize> {
		self.runtime
			.document()
			.find_all(needle, case_sensitive)
			.into_iter()
			.flat_map(|(s, e)| [s, e])
			.collect()
	}

	/// Find the next occurrence of `needle` at or after `from_offset`.
	/// Returns `[start, end]` or an empty array if not found.
	#[wasm_bindgen]
	pub fn find_next(&self, needle: &str, from_offset: usize, case_sensitive: bool) -> Vec<usize> {
		self.runtime
			.document()
			.find_next(needle, from_offset, case_sensitive)
			.map_or_else(Vec::new, |(s, e)| vec![s, e])
	}

	/// Find the previous occurrence before `from_offset`.
	#[wasm_bindgen]
	pub fn find_prev(&self, needle: &str, from_offset: usize, case_sensitive: bool) -> Vec<usize> {
		self.runtime
			.document()
			.find_prev(needle, from_offset, case_sensitive)
			.map_or_else(Vec::new, |(s, e)| vec![s, e])
	}

	/// Replace the text in range `[start, end)` with `replacement`.
	///
	/// This is a delete + insert.
	#[wasm_bindgen]
	pub fn replace_range(&mut self, start: usize, end: usize, replacement: &str) {
		if start < end {
			self.delete_range(start, end);
		}
		self.runtime.apply_operation(Operation::insert(
			Position::new(start),
			replacement.to_string(),
		));
	}

	/// Replace all occurrences of `needle` with `replacement`.
	/// Returns the number of replacements made.
	#[wasm_bindgen]
	pub fn replace_all(&mut self, needle: &str, replacement: &str, case_sensitive: bool) -> usize {
		let matches = self.runtime.document().find_all(needle, case_sensitive);
		let count = matches.len();
		// Replace from end to start so offsets stay valid.
		for &(start, end) in matches.iter().rev() {
			self.delete_range(start, end);
			self.runtime.apply_operation(Operation::insert(
				Position::new(start),
				replacement.to_string(),
			));
		}
		count
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

	/// Set font size on the current selection.
	#[wasm_bindgen]
	pub fn set_font_size(&mut self, size: f32) {
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return;
		}
		let style = Style::new().font_size(size);
		self.runtime.apply_operation(Operation::format(sel, style));
		let _ = self
			.runtime
			.handle_event(EditorEvent::SelectionSet { selection: sel });
	}

	/// Set text color on the current selection.
	#[wasm_bindgen]
	pub fn set_color(&mut self, r: u8, g: u8, b: u8, a: u8) {
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return;
		}
		let style = Style::new().color(r, g, b, a);
		self.runtime.apply_operation(Operation::format(sel, style));
		let _ = self
			.runtime
			.handle_event(EditorEvent::SelectionSet { selection: sel });
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

	/// Return the start offset of the visual line containing `offset`.
	///
	/// This performs a full paragraph layout to determine where lines wrap,
	/// then returns the character offset where that visual line begins.
	#[wasm_bindgen]
	pub fn line_start_for_offset(&self, offset: usize) -> Result<usize, JsValue> {
		let (_, ctx) = self.canvas_and_context()?;
		let renderer = Canvas2dRenderer::new(ctx, self.width, self.height);
		let doc = self.runtime.document();
		let lc = LayoutConstants::new(self.width);
		let plain_text = doc.plain_text();
		let styled_runs = doc.styled_runs();
		let paragraphs = layout_paragraphs(
			&plain_text,
			&styled_runs,
			&lc.layout_config,
			&renderer,
			&lc.default_style,
		);
		let (para_idx, _) = find_para_and_line_for_offset(&paragraphs, offset);
		if let Some(para) = paragraphs.get(para_idx) {
			let local_offset = offset.saturating_sub(para.global_char_start);
			let line_start =
				canvist_core::layout::line_start_for_offset(&para.layout, local_offset);
			Ok(para.global_char_start + line_start)
		} else {
			Ok(0)
		}
	}

	/// Return the end offset of the visual line containing `offset`.
	#[wasm_bindgen]
	pub fn line_end_for_offset(&self, offset: usize) -> Result<usize, JsValue> {
		let (_, ctx) = self.canvas_and_context()?;
		let renderer = Canvas2dRenderer::new(ctx, self.width, self.height);
		let doc = self.runtime.document();
		let lc = LayoutConstants::new(self.width);
		let plain_text = doc.plain_text();
		let styled_runs = doc.styled_runs();
		let paragraphs = layout_paragraphs(
			&plain_text,
			&styled_runs,
			&lc.layout_config,
			&renderer,
			&lc.default_style,
		);
		let (para_idx, _) = find_para_and_line_for_offset(&paragraphs, offset);
		if let Some(para) = paragraphs.get(para_idx) {
			let local_offset = offset.saturating_sub(para.global_char_start);
			let line_end = canvist_core::layout::line_end_for_offset(&para.layout, local_offset);
			Ok(para.global_char_start + line_end)
		} else {
			Ok(0)
		}
	}

	/// Return the character offset on the line directly above `offset`.
	///
	/// Preserves the horizontal (x) pixel position of the caret when moving
	/// between lines.
	#[wasm_bindgen]
	pub fn offset_above(&self, offset: usize) -> Result<usize, JsValue> {
		let (_, ctx) = self.canvas_and_context()?;
		let renderer = Canvas2dRenderer::new(ctx, self.width, self.height);
		let doc = self.runtime.document();
		let lc = LayoutConstants::new(self.width);
		let plain_text = doc.plain_text();
		let styled_runs = doc.styled_runs();
		let paragraphs = layout_paragraphs(
			&plain_text,
			&styled_runs,
			&lc.layout_config,
			&renderer,
			&lc.default_style,
		);
		let (para_idx, line_idx) = find_para_and_line_for_offset(&paragraphs, offset);

		if let Some(para) = paragraphs.get(para_idx) {
			let local_offset = offset.saturating_sub(para.global_char_start);
			if line_idx > 0 {
				// Move up within the same paragraph.
				let fragments = build_fragments(&para.text, &para.local_runs, &lc.default_style);
				let new_local = canvist_core::layout::offset_above(
					&para.layout,
					local_offset,
					&fragments,
					&renderer,
				);
				return Ok(para.global_char_start + new_local);
			}
			// Move to the previous paragraph.
			if para_idx > 0 {
				if let Some(prev_para) = paragraphs.get(para_idx - 1) {
					let cur_fragments =
						build_fragments(&para.text, &para.local_runs, &lc.default_style);
					let cur_line = &para.layout.lines[0];
					let target_x = canvist_core::layout::x_offset_in_line(
						cur_line.start_offset,
						local_offset,
						&cur_fragments,
						&renderer,
					);
					// Find the character on the last line of the previous paragraph.
					if let Some(last_line) = prev_para.layout.lines.last() {
						let prev_fragments = build_fragments(
							&prev_para.text,
							&prev_para.local_runs,
							&lc.default_style,
						);
						let hit =
							hit_x_on_line_ext(last_line, target_x, &prev_fragments, &renderer);
						return Ok(prev_para.global_char_start + hit);
					}
				}
			}
		}
		Ok(0)
	}

	/// Return the character offset on the line directly below `offset`.
	#[wasm_bindgen]
	pub fn offset_below(&self, offset: usize) -> Result<usize, JsValue> {
		let (_, ctx) = self.canvas_and_context()?;
		let renderer = Canvas2dRenderer::new(ctx, self.width, self.height);
		let doc = self.runtime.document();
		let lc = LayoutConstants::new(self.width);
		let plain_text = doc.plain_text();
		let styled_runs = doc.styled_runs();
		let paragraphs = layout_paragraphs(
			&plain_text,
			&styled_runs,
			&lc.layout_config,
			&renderer,
			&lc.default_style,
		);
		let total_chars = doc.char_count();
		let (para_idx, line_idx) = find_para_and_line_for_offset(&paragraphs, offset);

		if let Some(para) = paragraphs.get(para_idx) {
			let local_offset = offset.saturating_sub(para.global_char_start);
			let last_line_idx = para.layout.lines.len().saturating_sub(1);
			if line_idx < last_line_idx {
				// Move down within the same paragraph.
				let fragments = build_fragments(&para.text, &para.local_runs, &lc.default_style);
				let new_local = canvist_core::layout::offset_below(
					&para.layout,
					local_offset,
					&fragments,
					&renderer,
				);
				return Ok(para.global_char_start + new_local);
			}
			// Move to the next paragraph.
			if para_idx + 1 < paragraphs.len() {
				if let Some(next_para) = paragraphs.get(para_idx + 1) {
					let cur_fragments =
						build_fragments(&para.text, &para.local_runs, &lc.default_style);
					let cur_line = para.layout.lines.last().unwrap();
					let target_x = canvist_core::layout::x_offset_in_line(
						cur_line.start_offset,
						local_offset,
						&cur_fragments,
						&renderer,
					);
					// Find the character on the first line of the next paragraph.
					if let Some(first_line) = next_para.layout.lines.first() {
						let next_fragments = build_fragments(
							&next_para.text,
							&next_para.local_runs,
							&lc.default_style,
						);
						let hit =
							hit_x_on_line_ext(first_line, target_x, &next_fragments, &renderer);
						return Ok(next_para.global_char_start + hit);
					}
				}
			}
		}
		Ok(total_chars)
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
	/// Maximum valid scroll offset.
	fn max_scroll_y(&self) -> f32 {
		// We can't call canvas_and_context here (infallible context),
		// so use a rough estimate: content_height is set externally.
		// A tighter bound is computed in JS after content_height().
		f32::MAX
	}

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
		let (_canvas, ctx) = self.canvas_and_context()?;

		let width = self.width;
		let height = self.height;

		let renderer = Canvas2dRenderer::new(ctx, width, height);
		let lc = LayoutConstants::new(width);

		// Convert screen coords to document coords via viewport.
		let viewport = Viewport::new(width, height);
		let (doc_x, doc_y) = viewport.screen_to_document(screen_x as f32, screen_y as f32);

		// Coordinates relative to the content area origin, accounting for scroll.
		let content_x = doc_x - lc.padding_x;
		let content_y = doc_y - lc.padding_y + self.scroll_y;

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
		let (_canvas, ctx) = self.canvas_and_context()?;

		// Use cached logical (CSS) dimensions — these are set by `set_size()`
		// and exclude DPR scaling so layout wrapping is correct.
		let width = self.width;
		let height = self.height;

		// Enable high-quality text rendering.
		ctx.set_image_smoothing_enabled(true);

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

		// Scroll offset applied to all Y coordinates.
		let sy = self.scroll_y;

		// Focus-aware colors.
		let selection_color = if self.focused {
			Color::new(66, 133, 244, 80)
		} else {
			Color::new(180, 180, 180, 60)
		};
		let caret_color = if self.focused {
			Color::BLACK
		} else {
			Color::new(160, 160, 160, 128)
		};

		// ── Selection highlights ─────────────────────────────────────────
		if !selection.is_collapsed() {
			let sel_start = selection.start().offset();
			let sel_end = selection.end().offset();

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
						+ line.x_offset + x_offset_in_para_line(
						&renderer,
						para,
						line.start_offset,
						local_sel_start,
					);
					let x_end = lc.padding_x
						+ line.x_offset + x_offset_in_para_line(
						&renderer,
						para,
						line.start_offset,
						local_sel_end,
					);

					let y = lc.padding_y + para.y_offset + line.y - sy;
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
						let x = lc.padding_x + last_line.x_offset + last_line.width;
						let y = lc.padding_y + para.y_offset + last_line.y - sy;
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

				let line_y = lc.padding_y + para.y_offset + line.y - sy;

				// Skip lines that are entirely outside the viewport.
				if line_y + line.height < 0.0 || line_y > height {
					continue;
				}

				let line_x = lc.padding_x + line.x_offset;

				if para.local_runs.is_empty() {
					let line_text: String = para_chars[line_start..line_end.min(para_chars.len())]
						.iter()
						.collect();
					renderer.draw_text(line_x, line_y, &line_text, &lc.default_style);
				} else {
					let mut x = line_x;

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

			let (caret_para_idx, caret_line_idx) =
				find_para_and_line_for_offset(&paragraphs, caret_offset);

			if let Some(para) = paragraphs.get(caret_para_idx) {
				if let Some(caret_line) = para.layout.lines.get(caret_line_idx) {
					let local_caret = caret_offset.saturating_sub(para.global_char_start);
					let caret_x = lc.padding_x
						+ caret_line.x_offset
						+ x_offset_in_para_line(
							&renderer,
							para,
							caret_line.start_offset,
							local_caret,
						);
					let caret_y = lc.padding_y + para.y_offset + caret_line.y - sy;
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

		// ── Scroll indicator ─────────────────────────────────────────────
		// Draw a thin scrollbar on the right edge when content overflows.
		let content_h = paragraphs
			.last()
			.map(|p| p.y_offset + p.layout.total_height + lc.padding_y * 2.0)
			.unwrap_or(height);
		if content_h > height {
			let track_x = width - 6.0;
			let track_h = height;
			let ratio = height / content_h;
			let thumb_h = (track_h * ratio).max(20.0);
			let thumb_y = (sy / (content_h - height)) * (track_h - thumb_h);
			// Track.
			renderer.fill_rect(
				Rect::new(track_x, 0.0, 6.0, track_h),
				Color::new(240, 240, 240, 128),
			);
			// Thumb.
			renderer.fill_rect(
				Rect::new(track_x, thumb_y, 6.0, thumb_h),
				Color::new(180, 180, 180, 180),
			);
		}

		Ok(())
	}
}
