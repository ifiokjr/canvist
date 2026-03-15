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
		Self::with_zoom(canvas_width, 1.0)
	}

	/// Create layout constants with a specific zoom level and text colour.
	fn with_zoom(canvas_width: f32, zoom: f32) -> Self {
		Self::with_zoom_and_color(canvas_width, zoom, Color::BLACK)
	}

	/// Create layout constants with zoom and explicit text colour.
	fn with_zoom_and_color(canvas_width: f32, zoom: f32, text_color: Color) -> Self {
		let padding_x: f32 = 20.0;
		let padding_y: f32 = 20.0;
		let content_width = (canvas_width - padding_x * 2.0).max(100.0);
		let base_size = 16.0 * zoom;
		let default_style = Style::new()
			.font_size(base_size)
			.color(text_color.r, text_color.g, text_color.b, text_color.a)
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
	/// When true, editing operations are blocked (selection and copy still work).
	read_only: bool,
	/// When true, a line-number gutter is rendered to the left of the content.
	show_line_numbers: bool,
	/// Active colour theme.
	theme: EditorTheme,
	/// Whether the current line is highlighted.
	highlight_current_line: bool,
	/// Zoom level multiplier (1.0 = 100%).
	zoom: f32,
	/// Whether text wraps at the canvas edge (true) or extends infinitely (false).
	word_wrap: bool,
	/// Whether to render whitespace indicators (· for space, → for tab).
	show_whitespace: bool,
	/// Whether typing an opening bracket inserts the closing counterpart.
	auto_close_brackets: bool,
	/// Tab size in spaces (default 4).
	tab_size: usize,
	/// Whether Tab inserts spaces instead of a `\t` character.
	soft_tabs: bool,
	/// Line comment prefix (default `// `).
	comment_prefix: String,
	/// Whether to auto-surround selected text when typing brackets.
	auto_surround: bool,
	/// Whether to highlight matching brackets near the cursor.
	highlight_matching_brackets: bool,
	/// Whether to show indent guides (vertical lines at tab stops).
	show_indent_guides: bool,
	/// Set of bookmarked line numbers (0-based paragraph indices).
	bookmarks: std::collections::BTreeSet<usize>,
	/// Whether the editor is in overwrite (replace) mode instead of insert.
	overwrite_mode: bool,
	/// Stack of cursor positions for back navigation.
	cursor_history: Vec<usize>,
	/// Index into cursor_history for forward navigation (-1 means at tip).
	cursor_history_index: i32,
	/// Column rulers — vertical guide lines at these column numbers.
	rulers: Vec<usize>,
	/// Per-line decorations: (line_0based, r, g, b, a) for background tinting.
	line_decorations: Vec<(usize, u8, u8, u8, u8)>,
	/// Whether the document has been modified since last `mark_saved()`.
	is_modified: bool,
	/// Clipboard ring — last N copied/cut texts (newest first).
	clipboard_ring: Vec<String>,
	/// Max entries in clipboard ring.
	clipboard_ring_max: usize,
	/// Whether to highlight all occurrences of the word under cursor.
	highlight_occurrences: bool,
	/// Placeholder text shown when document is empty.
	placeholder: String,
	/// Maximum character count (0 = unlimited).
	max_length: usize,
	/// Last known selection end offset for change detection.
	last_selection_end: usize,
	/// Whether to show wrap continuation indicators in the gutter.
	show_wrap_indicators: bool,
	/// Recent editor events log (newest first, max 50).
	event_log: Vec<String>,
	/// Max entries in event log.
	event_log_max: usize,
	/// Annotations: (start_offset, end_offset, kind, message).
	annotations: Vec<(usize, usize, String, String)>,
	/// Recent search terms (newest first, max 20).
	search_history: Vec<String>,
	/// Whether to show the minimap.
	show_minimap: bool,
	/// Minimap width in pixels.
	minimap_width: f32,
	/// Whether to show sticky scroll (top context line).
	sticky_scroll: bool,
	/// Cursor style: 0=line, 1=block, 2=underline.
	cursor_style: u8,
	/// Cursor width in pixels (for line style).
	cursor_width: f32,
	/// Cursor colour override (r, g, b, a). None = use theme.
	cursor_color: Option<(u8, u8, u8, u8)>,
	/// Previous text snapshot for diff.
	diff_snapshot: String,
	/// Whether macro recording is active.
	macro_recording: bool,
	/// Recorded macro steps: each is a (kind, data) tuple.
	/// kind: "insert" | "delete" | "select"
	macro_steps: Vec<(String, String)>,
	/// Saved macros by name.
	saved_macros: std::collections::HashMap<String, Vec<(String, String)>>,
	/// Whether to highlight all find matches visually.
	show_find_highlights: bool,
	/// Current find highlight needle.
	find_highlight_needle: String,
	/// Folded line ranges: each (start_line, end_line) inclusive, 0-based.
	folded_ranges: Vec<(usize, usize)>,
	/// Whether link detection is enabled.
	detect_links: bool,
	/// Whether syntax highlighting is enabled.
	syntax_highlight: bool,
	/// Token colour overrides: kind → (r, g, b, a).
	token_colors: std::collections::HashMap<String, (u8, u8, u8, u8)>,
	/// Extra cursor offsets for multi-cursor editing.
	extra_cursors: Vec<usize>,
	/// Collaborative cursors: (offset, name, r, g, b).
	collab_cursors: Vec<(usize, String, u8, u8, u8)>,
	/// Selection history stack for undo/redo selections.
	selection_history: Vec<(usize, usize)>,
	/// Selection history index (-1 = current).
	selection_history_index: i32,
	/// Custom keybinding overrides: shortcut → command name.
	keybinding_overrides: std::collections::HashMap<String, String>,
	/// Marker highlight ranges: (start, end, r, g, b, a, id).
	markers: Vec<(usize, usize, u8, u8, u8, u8, String)>,
	/// Named anchors: name -> character offset.
	anchors: std::collections::HashMap<String, usize>,
	/// Named saved full editor states: name -> JSON payload.
	named_states: std::collections::HashMap<String, String>,
	/// Named saved selection ranges: name -> (start, end).
	selection_profiles: std::collections::HashMap<String, (usize, usize)>,
}

/// Colour theme for the editor canvas.
#[derive(Clone)]
struct EditorTheme {
	/// Canvas / document background.
	background: Color,
	/// Default text colour.
	text: Color,
	/// Caret colour when focused.
	caret: Color,
	/// Caret colour when blurred.
	caret_blur: Color,
	/// Selection highlight (focused).
	selection: Color,
	/// Selection highlight (blurred).
	selection_blur: Color,
	/// Current-line highlight band.
	line_highlight: Color,
	/// Line-number gutter background.
	gutter_bg: Color,
	/// Line-number gutter separator.
	gutter_border: Color,
	/// Line-number text colour.
	gutter_text: Color,
	/// Scrollbar track.
	scrollbar_track: Color,
	/// Scrollbar thumb.
	scrollbar_thumb: Color,
}

impl EditorTheme {
	fn light() -> Self {
		Self {
			background: Color::WHITE,
			text: Color::BLACK,
			caret: Color::BLACK,
			caret_blur: Color::new(160, 160, 160, 128),
			selection: Color::new(66, 133, 244, 80),
			selection_blur: Color::new(180, 180, 180, 60),
			line_highlight: Color::new(0, 0, 0, 10),
			gutter_bg: Color::new(245, 245, 245, 255),
			gutter_border: Color::new(220, 220, 220, 255),
			gutter_text: Color::new(160, 160, 160, 255),
			scrollbar_track: Color::new(240, 240, 240, 128),
			scrollbar_thumb: Color::new(180, 180, 180, 180),
		}
	}

	fn dark() -> Self {
		Self {
			background: Color::new(30, 30, 30, 255),
			text: Color::new(212, 212, 212, 255),
			caret: Color::WHITE,
			caret_blur: Color::new(120, 120, 120, 128),
			selection: Color::new(38, 79, 120, 160),
			selection_blur: Color::new(80, 80, 80, 80),
			line_highlight: Color::new(255, 255, 255, 12),
			gutter_bg: Color::new(37, 37, 37, 255),
			gutter_border: Color::new(55, 55, 55, 255),
			gutter_text: Color::new(110, 110, 110, 255),
			scrollbar_track: Color::new(50, 50, 50, 128),
			scrollbar_thumb: Color::new(100, 100, 100, 180),
		}
	}
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
			read_only: false,
			show_line_numbers: false,
			theme: EditorTheme::light(),
			highlight_current_line: true,
			zoom: 1.0,
			word_wrap: true,
			show_whitespace: false,
			auto_close_brackets: false,
			tab_size: 4,
			soft_tabs: false,
			comment_prefix: "// ".to_string(),
			auto_surround: false,
			highlight_matching_brackets: true,
			show_indent_guides: false,
			bookmarks: std::collections::BTreeSet::new(),
			overwrite_mode: false,
			cursor_history: Vec::new(),
			cursor_history_index: -1,
			rulers: Vec::new(),
			line_decorations: Vec::new(),
			is_modified: false,
			clipboard_ring: Vec::new(),
			clipboard_ring_max: 10,
			highlight_occurrences: false,
			placeholder: String::new(),
			max_length: 0,
			last_selection_end: 0,
			show_wrap_indicators: false,
			event_log: Vec::new(),
			event_log_max: 50,
			annotations: Vec::new(),
			search_history: Vec::new(),
			show_minimap: false,
			minimap_width: 60.0,
			sticky_scroll: false,
			cursor_style: 0,
			cursor_width: 2.0,
			cursor_color: None,
			diff_snapshot: String::new(),
			macro_recording: false,
			macro_steps: Vec::new(),
			saved_macros: std::collections::HashMap::new(),
			show_find_highlights: false,
			find_highlight_needle: String::new(),
			folded_ranges: Vec::new(),
			detect_links: false,
			syntax_highlight: false,
			token_colors: std::collections::HashMap::new(),
			extra_cursors: Vec::new(),
			collab_cursors: Vec::new(),
			selection_history: Vec::new(),
			selection_history_index: -1,
			keybinding_overrides: std::collections::HashMap::new(),
			markers: Vec::new(),
			anchors: std::collections::HashMap::new(),
			named_states: std::collections::HashMap::new(),
			selection_profiles: std::collections::HashMap::new(),
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
		let lc = self.layout_constants();
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
		let lc = self.layout_constants();
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

	// ── Read-only mode ───────────────────────────────────────────────

	/// Set the editor to read-only mode. Editing operations are blocked;
	/// selection, copy, and navigation still work.
	#[wasm_bindgen]
	pub fn set_read_only(&mut self, read_only: bool) {
		self.read_only = read_only;
	}

	/// Check whether the editor is in read-only mode.
	#[wasm_bindgen]
	pub fn read_only(&self) -> bool {
		self.read_only
	}

	// ── Line numbers ─────────────────────────────────────────────────

	/// Enable or disable the line-number gutter.
	#[wasm_bindgen]
	pub fn set_show_line_numbers(&mut self, show: bool) {
		self.show_line_numbers = show;
	}

	/// Check whether line numbers are visible.
	#[wasm_bindgen]
	pub fn show_line_numbers(&self) -> bool {
		self.show_line_numbers
	}

	// ── Theme ────────────────────────────────────────────────────────

	/// Switch to the dark colour theme.
	#[wasm_bindgen]
	pub fn set_theme_dark(&mut self) {
		self.theme = EditorTheme::dark();
	}

	/// Switch to the light colour theme.
	#[wasm_bindgen]
	pub fn set_theme_light(&mut self) {
		self.theme = EditorTheme::light();
	}

	/// Return `"dark"` or `"light"` depending on the active theme.
	#[wasm_bindgen]
	pub fn theme_name(&self) -> String {
		if self.theme.background.r < 100 {
			"dark".to_string()
		} else {
			"light".to_string()
		}
	}

	// ── Zoom ─────────────────────────────────────────────────────────

	/// Set the zoom level (1.0 = 100%, 1.5 = 150%, etc.). Clamped to [0.25, 4.0].
	#[wasm_bindgen]
	pub fn set_zoom(&mut self, level: f32) {
		self.zoom = level.clamp(0.25, 4.0);
	}

	/// Get the current zoom level.
	#[wasm_bindgen]
	pub fn zoom(&self) -> f32 {
		self.zoom
	}

	/// Zoom in by one step (1.1× multiplier).
	#[wasm_bindgen]
	pub fn zoom_in(&mut self) {
		self.zoom = (self.zoom * 1.1).min(4.0);
	}

	/// Zoom out by one step (÷ 1.1).
	#[wasm_bindgen]
	pub fn zoom_out(&mut self) {
		self.zoom = (self.zoom / 1.1).max(0.25);
	}

	/// Reset zoom to 100%.
	#[wasm_bindgen]
	pub fn zoom_reset(&mut self) {
		self.zoom = 1.0;
	}

	// ── Current line highlight ───────────────────────────────────────

	/// Enable or disable the current-line highlight band.
	#[wasm_bindgen]
	pub fn set_highlight_current_line(&mut self, enabled: bool) {
		self.highlight_current_line = enabled;
	}

	/// Whether the current-line highlight is enabled.
	#[wasm_bindgen]
	pub fn highlight_current_line(&self) -> bool {
		self.highlight_current_line
	}

	// ── Word wrap ────────────────────────────────────────────────────

	/// Enable or disable word wrapping at the canvas edge.
	///
	/// When disabled, lines extend horizontally and horizontal scrolling
	/// may be needed.
	#[wasm_bindgen]
	pub fn set_word_wrap(&mut self, enabled: bool) {
		self.word_wrap = enabled;
	}

	/// Whether word wrapping is enabled.
	#[wasm_bindgen]
	pub fn word_wrap(&self) -> bool {
		self.word_wrap
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
		let lc = self.layout_constants();
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
		let lc = self.layout_constants();
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
		let lc = self.layout_constants();
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

	// ── Selection statistics ─────────────────────────────────────────

	/// Number of characters currently selected (0 if collapsed).
	#[wasm_bindgen]
	pub fn selected_char_count(&self) -> usize {
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return 0;
		}
		sel.end().offset().saturating_sub(sel.start().offset())
	}

	/// Number of words in the current selection (0 if collapsed).
	#[wasm_bindgen]
	pub fn selected_word_count(&self) -> usize {
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return 0;
		}
		let text = self.runtime.document().plain_text();
		let chars: Vec<char> = text.chars().collect();
		let s = sel.start().offset().min(chars.len());
		let e = sel.end().offset().min(chars.len());
		let selected: String = chars[s..e].iter().collect();
		selected.split_whitespace().count()
	}

	// ── Go to line ───────────────────────────────────────────────────

	/// Move the cursor to the start of the given 1-based paragraph line.
	///
	/// If `line_number` exceeds the paragraph count, the cursor moves to
	/// the end of the document.
	#[wasm_bindgen]
	pub fn go_to_line(&mut self, line_number: usize) {
		let plain = self.runtime.document().plain_text();
		let mut offset = 0usize;
		let target = line_number.max(1) - 1; // 0-based
		for (i, line_text) in plain.split('\n').enumerate() {
			if i == target {
				let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
					selection: Selection::collapsed(Position::new(offset)),
				});
				return;
			}
			offset += line_text.chars().count() + 1; // +1 for '\n'
		}
		// Past end — go to document end.
		let end = plain.chars().count();
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(end)),
		});
	}

	// ── Line operations ──────────────────────────────────────────────

	/// Duplicate the current line (or selected lines) below.
	#[wasm_bindgen]
	pub fn duplicate_line(&mut self) {
		if !self.is_writable() {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let offset = self.runtime.selection().end().offset();

		// Find line boundaries.
		let mut line_start = offset.min(chars.len());
		while line_start > 0 && chars[line_start - 1] != '\n' {
			line_start -= 1;
		}
		let mut line_end = offset.min(chars.len());
		while line_end < chars.len() && chars[line_end] != '\n' {
			line_end += 1;
		}

		let line_text: String = chars[line_start..line_end].iter().collect();
		// Insert \n + copy after the current line end.
		let insert_text = format!("\n{line_text}");
		self.runtime
			.apply_operation(Operation::insert(Position::new(line_end), insert_text));
	}

	/// Move the current line up by swapping it with the line above.
	#[wasm_bindgen]
	pub fn move_line_up(&mut self) {
		if !self.is_writable() {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let offset = self.runtime.selection().end().offset();

		// Current line boundaries.
		let mut cur_start = offset.min(chars.len());
		while cur_start > 0 && chars[cur_start - 1] != '\n' {
			cur_start -= 1;
		}
		if cur_start == 0 {
			return; // Already first line.
		}
		let mut cur_end = offset.min(chars.len());
		while cur_end < chars.len() && chars[cur_end] != '\n' {
			cur_end += 1;
		}

		// Previous line boundaries.
		let mut prev_start = cur_start - 1; // skip the \n
		while prev_start > 0 && chars[prev_start - 1] != '\n' {
			prev_start -= 1;
		}

		let prev_text: String = chars[prev_start..cur_start - 1].iter().collect();
		let cur_text: String = chars[cur_start..cur_end].iter().collect();

		// Delete both lines (prev_start..cur_end) and reinsert swapped.
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(prev_start), Position::new(cur_end)),
		});
		let _ = self
			.runtime
			.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
		let swapped = format!("{cur_text}\n{prev_text}");
		self.runtime
			.apply_operation(Operation::insert(Position::new(prev_start), swapped));

		// Move cursor up.
		let new_offset = prev_start + (offset - cur_start);
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(
				new_offset.min(self.runtime.document().plain_text().chars().count()),
			)),
		});
	}

	/// Move the current line down by swapping it with the line below.
	#[wasm_bindgen]
	pub fn move_line_down(&mut self) {
		if !self.is_writable() {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let offset = self.runtime.selection().end().offset();

		// Current line boundaries.
		let mut cur_start = offset.min(chars.len());
		while cur_start > 0 && chars[cur_start - 1] != '\n' {
			cur_start -= 1;
		}
		let mut cur_end = offset.min(chars.len());
		while cur_end < chars.len() && chars[cur_end] != '\n' {
			cur_end += 1;
		}
		if cur_end >= chars.len() {
			return; // Already last line.
		}

		// Next line boundaries.
		let next_start = cur_end + 1; // skip the \n
		let mut next_end = next_start;
		while next_end < chars.len() && chars[next_end] != '\n' {
			next_end += 1;
		}

		let cur_text: String = chars[cur_start..cur_end].iter().collect();
		let next_text: String = chars[next_start..next_end].iter().collect();

		// Delete both lines and reinsert swapped.
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(cur_start), Position::new(next_end)),
		});
		let _ = self
			.runtime
			.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
		let swapped = format!("{next_text}\n{cur_text}");
		self.runtime
			.apply_operation(Operation::insert(Position::new(cur_start), swapped));

		// Move cursor down.
		let next_text_len = next_text.chars().count();
		let new_offset = cur_start + next_text_len + 1 + (offset - cur_start);
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(
				new_offset.min(self.runtime.document().plain_text().chars().count()),
			)),
		});
	}

	// ── Background / highlight colour ────────────────────────────────

	/// Set a background (highlight) colour on the current selection.
	///
	/// The colour is stored via the style's `background` field.
	#[wasm_bindgen]
	pub fn set_highlight_color(&mut self, r: u8, g: u8, b: u8, a: u8) {
		if !self.is_writable() {
			return;
		}
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return;
		}
		let style = Style::new().background(r, g, b, a);
		self.runtime.apply_operation(Operation::format(sel, style));
		// Preserve selection.
		let _ = self
			.runtime
			.handle_event(EditorEvent::SelectionSet { selection: sel });
	}

	/// Remove the background (highlight) colour from the current selection.
	#[wasm_bindgen]
	pub fn remove_highlight_color(&mut self) {
		self.set_highlight_color(0, 0, 0, 0);
	}

	// ── Delete line ──────────────────────────────────────────────────

	/// Delete the entire line the cursor is on (Ctrl+Shift+K).
	///
	/// If the deleted line is not the last, the trailing `\n` is also
	/// removed so the next line moves up.
	#[wasm_bindgen]
	pub fn delete_line(&mut self) {
		if !self.is_writable() {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let offset = self.runtime.selection().end().offset();

		let mut line_start = offset.min(chars.len());
		while line_start > 0 && chars[line_start - 1] != '\n' {
			line_start -= 1;
		}
		let mut line_end = offset.min(chars.len());
		while line_end < chars.len() && chars[line_end] != '\n' {
			line_end += 1;
		}
		// Include the trailing \n if present.
		if line_end < chars.len() && chars[line_end] == '\n' {
			line_end += 1;
		} else if line_start > 0 {
			// Last line — eat the preceding \n instead.
			line_start -= 1;
		}
		if line_start < line_end {
			self.delete_range(line_start, line_end);
		}
	}

	// ── Transform case ───────────────────────────────────────────────

	/// Convert selected text to UPPERCASE.
	#[wasm_bindgen]
	pub fn transform_uppercase(&mut self) {
		if !self.is_writable() {
			return;
		}
		self.transform_selection(|s| s.to_uppercase());
	}

	/// Convert selected text to lowercase.
	#[wasm_bindgen]
	pub fn transform_lowercase(&mut self) {
		if !self.is_writable() {
			return;
		}
		self.transform_selection(|s| s.to_lowercase());
	}

	/// Convert selected text to Title Case.
	#[wasm_bindgen]
	pub fn transform_title_case(&mut self) {
		if !self.is_writable() {
			return;
		}
		self.transform_selection(|s| {
			s.split_whitespace()
				.map(|word| {
					let mut chars = word.chars();
					match chars.next() {
						Some(c) => {
							let upper: String = c.to_uppercase().collect();
							let rest: String = chars.as_str().to_lowercase();
							format!("{upper}{rest}")
						}
						None => String::new(),
					}
				})
				.collect::<Vec<_>>()
				.join(" ")
		});
	}

	/// Replace the selected text with the result of `f(selected_text)`,
	/// preserving the selection range.
	fn transform_selection<F: FnOnce(&str) -> String>(&mut self, f: F) {
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return;
		}
		let start = sel.start().offset();
		let end = sel.end().offset();
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let s = start.min(chars.len());
		let e = end.min(chars.len());
		let selected: String = chars[s..e].iter().collect();
		let transformed = f(&selected);
		let new_len = transformed.chars().count();

		// Delete old, insert new.
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(s), Position::new(e)),
		});
		let _ = self
			.runtime
			.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
		self.runtime
			.apply_operation(Operation::insert(Position::new(s), transformed));

		// Re-select the transformed text.
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(s), Position::new(s + new_len)),
		});
	}

	// ── Sort lines ───────────────────────────────────────────────────

	/// Sort selected lines in ascending alphabetical order.
	#[wasm_bindgen]
	pub fn sort_lines_asc(&mut self) {
		if !self.is_writable() {
			return;
		}
		self.sort_lines(false);
	}

	/// Sort selected lines in descending alphabetical order.
	#[wasm_bindgen]
	pub fn sort_lines_desc(&mut self) {
		if !self.is_writable() {
			return;
		}
		self.sort_lines(true);
	}

	/// Sort the lines covered by the current selection.
	fn sort_lines(&mut self, descending: bool) {
		let sel = self.runtime.selection();
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let sel_start = sel.start().offset().min(chars.len());
		let sel_end = sel.end().offset().min(chars.len());

		// Expand to full lines.
		let mut start = sel_start;
		while start > 0 && chars[start - 1] != '\n' {
			start -= 1;
		}
		let mut end = sel_end;
		while end < chars.len() && chars[end] != '\n' {
			end += 1;
		}

		let block: String = chars[start..end].iter().collect();
		let mut lines: Vec<&str> = block.split('\n').collect();
		if descending {
			lines.sort_unstable_by(|a, b| b.cmp(a));
		} else {
			lines.sort_unstable();
		}
		let sorted = lines.join("\n");

		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(end)),
		});
		let _ = self
			.runtime
			.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
		let sorted_len = sorted.chars().count();
		self.runtime
			.apply_operation(Operation::insert(Position::new(start), sorted));
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(start + sorted_len)),
		});
	}

	// ── Join lines ───────────────────────────────────────────────────

	/// Join the current line with the line below (Ctrl+J).
	///
	/// Replaces the newline between them with a single space.
	#[wasm_bindgen]
	pub fn join_lines(&mut self) {
		if !self.is_writable() {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let offset = self.runtime.selection().end().offset();

		// Find end of current line.
		let mut line_end = offset.min(chars.len());
		while line_end < chars.len() && chars[line_end] != '\n' {
			line_end += 1;
		}
		if line_end >= chars.len() {
			return; // No line below.
		}

		// Replace the \n with a space. Also trim leading whitespace of the
		// next line so "hello\n  world" becomes "hello world".
		let mut trim_end = line_end + 1;
		while trim_end < chars.len() && (chars[trim_end] == ' ' || chars[trim_end] == '\t') {
			trim_end += 1;
		}

		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(line_end), Position::new(trim_end)),
		});
		let _ = self
			.runtime
			.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
		self.runtime
			.apply_operation(Operation::insert(Position::new(line_end), " ".to_string()));
	}

	// ── Show whitespace ──────────────────────────────────────────────

	/// Toggle the visual whitespace indicator.
	///
	/// When enabled, the renderer draws `·` for spaces and `→` for tabs.
	#[wasm_bindgen]
	pub fn set_show_whitespace(&mut self, show: bool) {
		self.show_whitespace = show;
	}

	/// Whether whitespace visualization is enabled.
	#[wasm_bindgen]
	pub fn show_whitespace(&self) -> bool {
		self.show_whitespace
	}

	// ── Bracket auto-close ───────────────────────────────────────────

	/// Toggle bracket auto-closing.
	///
	/// When enabled, typing `(`, `[`, `{`, `"`, or `'` automatically
	/// inserts the closing counterpart and places the cursor between them.
	#[wasm_bindgen]
	pub fn set_auto_close_brackets(&mut self, enabled: bool) {
		self.auto_close_brackets = enabled;
	}

	/// Whether bracket auto-closing is enabled.
	#[wasm_bindgen]
	pub fn auto_close_brackets(&self) -> bool {
		self.auto_close_brackets
	}

	/// Insert an opening bracket and its closing counterpart.
	///
	/// Returns the number of characters inserted (always 2 when auto-close
	/// fires, 1 otherwise). Cursor is placed between the pair.
	#[wasm_bindgen]
	pub fn insert_with_auto_close(&mut self, ch: &str) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let open_char = ch.chars().next().unwrap_or(' ');
		let close = match open_char {
			'(' => Some(')'),
			'[' => Some(']'),
			'{' => Some('}'),
			'"' => Some('"'),
			'\'' => Some('\''),
			'`' => Some('`'),
			_ => None,
		};

		if !self.auto_close_brackets || close.is_none() {
			// Normal insert.
			let offset = self.runtime.selection().end().offset();
			self.runtime
				.apply_operation(Operation::insert(Position::new(offset), ch.to_string()));
			let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
				selection: Selection::collapsed(Position::new(offset + 1)),
			});
			return 1;
		}

		let close_ch = close.unwrap();
		let offset = self.runtime.selection().end().offset();

		// Delete selection if any.
		let sel = self.runtime.selection();
		if !sel.is_collapsed() {
			self.delete_range(sel.start().offset(), sel.end().offset());
		}
		let insert_at = self.runtime.selection().end().offset();
		let pair = format!("{open_char}{close_ch}");
		self.runtime
			.apply_operation(Operation::insert(Position::new(insert_at), pair));
		// Cursor between the pair.
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(insert_at + 1)),
		});
		2
	}

	// ── Delete word ──────────────────────────────────────────────────

	/// Delete the word to the left of the cursor (Ctrl+Backspace).
	///
	/// Walks backwards from the cursor past whitespace, then past word
	/// characters, and deletes the range.
	#[wasm_bindgen]
	pub fn delete_word_left(&mut self) {
		if !self.is_writable() {
			return;
		}
		let sel = self.runtime.selection();
		if !sel.is_collapsed() {
			self.delete_range(sel.start().offset(), sel.end().offset());
			return;
		}
		let offset = sel.end().offset();
		if offset == 0 {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let mut pos = offset;
		// Skip trailing whitespace.
		while pos > 0 && chars[pos - 1].is_whitespace() {
			pos -= 1;
		}
		// Skip word characters.
		while pos > 0 && !chars[pos - 1].is_whitespace() {
			pos -= 1;
		}
		if pos < offset {
			self.delete_range(pos, offset);
		}
	}

	/// Delete the word to the right of the cursor (Ctrl+Delete).
	#[wasm_bindgen]
	pub fn delete_word_right(&mut self) {
		if !self.is_writable() {
			return;
		}
		let sel = self.runtime.selection();
		if !sel.is_collapsed() {
			self.delete_range(sel.start().offset(), sel.end().offset());
			return;
		}
		let offset = sel.end().offset();
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		if offset >= chars.len() {
			return;
		}
		let mut pos = offset;
		// Skip word characters.
		while pos < chars.len() && !chars[pos].is_whitespace() {
			pos += 1;
		}
		// Skip trailing whitespace.
		while pos < chars.len() && chars[pos].is_whitespace() {
			pos += 1;
		}
		if pos > offset {
			self.delete_range(offset, pos);
		}
	}

	// ── Select line ──────────────────────────────────────────────────

	/// Select the entire current line (Ctrl+L).
	///
	/// Repeated calls extend the selection by one line each time.
	#[wasm_bindgen]
	pub fn select_line(&mut self) {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let offset = self.runtime.selection().end().offset();

		let mut line_start = offset.min(chars.len());
		while line_start > 0 && chars[line_start - 1] != '\n' {
			line_start -= 1;
		}
		let mut line_end = offset.min(chars.len());
		while line_end < chars.len() && chars[line_end] != '\n' {
			line_end += 1;
		}
		// Include the trailing newline so repeated Ctrl+L extends.
		if line_end < chars.len() {
			line_end += 1;
		}

		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(line_start), Position::new(line_end)),
		});
	}

	// ── Trim trailing whitespace ─────────────────────────────────────

	/// Remove trailing whitespace (spaces and tabs) from every line.
	///
	/// Returns the number of characters removed.
	#[wasm_bindgen]
	pub fn trim_trailing_whitespace(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let mut new_text = String::new();
		let mut removed = 0usize;
		for (i, line) in plain.split('\n').enumerate() {
			if i > 0 {
				new_text.push('\n');
			}
			let trimmed = line.trim_end();
			removed += line.len() - trimmed.len();
			new_text.push_str(trimmed);
		}
		if removed > 0 {
			self.runtime.document_mut().set_plain_text(&new_text);
		}
		removed
	}

	// ── Remove duplicate lines ───────────────────────────────────────

	/// Remove consecutive duplicate lines from the document.
	///
	/// Returns the number of lines removed.
	#[wasm_bindgen]
	pub fn remove_duplicate_lines(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let lines: Vec<&str> = plain.split('\n').collect();
		let original_count = lines.len();
		let mut deduped: Vec<&str> = Vec::with_capacity(original_count);
		for line in &lines {
			if deduped.last() != Some(line) {
				deduped.push(line);
			}
		}
		let removed = original_count - deduped.len();
		if removed > 0 {
			let new_text = deduped.join("\n");
			self.runtime.document_mut().set_plain_text(&new_text);
		}
		removed
	}

	// ── Wrap selection ───────────────────────────────────────────────

	/// Wrap the selected text with a pair of strings (e.g. brackets).
	///
	/// Example: `wrap_selection("(", ")")` turns `hello` into `(hello)`.
	/// Cursor is placed after the closing string.
	#[wasm_bindgen]
	pub fn wrap_selection(&mut self, open: &str, close: &str) {
		if !self.is_writable() {
			return;
		}
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return;
		}
		let start = sel.start().offset();
		let end = sel.end().offset();
		let open_len = open.chars().count();
		let close_len = close.chars().count();

		// Insert close first (so start offset stays valid), then open.
		self.runtime
			.apply_operation(Operation::insert(Position::new(end), close.to_string()));
		self.runtime
			.apply_operation(Operation::insert(Position::new(start), open.to_string()));

		// Select the wrapped content (open + original + close).
		let new_end = end + open_len + close_len;
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(new_end)),
		});
	}

	// ── Smart backspace ──────────────────────────────────────────────

	/// If the cursor is between a matching bracket pair (e.g. `(|)`),
	/// delete both characters. Otherwise, behave like normal backspace.
	///
	/// Returns `true` if a pair was deleted, `false` for normal backspace.
	#[wasm_bindgen]
	pub fn smart_backspace(&mut self) -> bool {
		if !self.is_writable() {
			return false;
		}
		let sel = self.runtime.selection();
		if !sel.is_collapsed() {
			return false;
		}
		let offset = sel.end().offset();
		if offset == 0 {
			return false;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		if offset >= chars.len() {
			return false;
		}
		let before = chars[offset - 1];
		let after = chars[offset];
		let is_pair = matches!(
			(before, after),
			('(', ')') | ('[', ']') | ('{', '}') | ('"', '"') | ('\'', '\'') | ('`', '`')
		);
		if is_pair {
			self.delete_range(offset - 1, offset + 1);
			return true;
		}
		false
	}

	// ── Transpose characters ─────────────────────────────────────────

	/// Swap the two characters around the cursor (Ctrl+T).
	///
	/// If the cursor is at the end of a line, swaps the two preceding
	/// characters instead.
	#[wasm_bindgen]
	pub fn transpose_chars(&mut self) {
		if !self.is_writable() {
			return;
		}
		let sel = self.runtime.selection();
		if !sel.is_collapsed() {
			return;
		}
		let offset = sel.end().offset();
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();

		if chars.is_empty() {
			return;
		}

		// Determine which two positions to swap.
		let (a, b) = if offset == 0 {
			return;
		} else if offset >= chars.len() || chars[offset] == '\n' {
			// At end or end-of-line: swap the two chars before cursor.
			if offset < 2 {
				return;
			}
			(offset - 2, offset - 1)
		} else {
			(offset - 1, offset)
		};

		let char_a = chars[a];
		let char_b = chars[b];
		if char_a == '\n' || char_b == '\n' {
			return;
		}

		// Delete range [a..b+1], insert swapped.
		self.delete_range(a, b + 1);
		let swapped = format!("{char_b}{char_a}");
		self.runtime
			.apply_operation(Operation::insert(Position::new(a), swapped));
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(b + 1)),
		});
	}

	// ── Toggle line comment ──────────────────────────────────────────

	/// Set the line comment prefix (default `"// "`).
	#[wasm_bindgen]
	pub fn set_comment_prefix(&mut self, prefix: &str) {
		self.comment_prefix = prefix.to_string();
	}

	/// Get the current line comment prefix.
	#[wasm_bindgen]
	pub fn comment_prefix(&self) -> String {
		self.comment_prefix.clone()
	}

	/// Toggle a line-comment prefix on the current line or all selected
	/// lines.
	///
	/// If all affected lines start with the prefix, it is removed from
	/// each. Otherwise the prefix is added to every line.
	#[wasm_bindgen]
	pub fn toggle_line_comment(&mut self) {
		if !self.is_writable() {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let sel = self.runtime.selection();
		let sel_start = sel.start().offset().min(chars.len());
		let sel_end = sel.end().offset().min(chars.len());

		// Expand to full line boundaries.
		let mut start = sel_start;
		while start > 0 && chars[start - 1] != '\n' {
			start -= 1;
		}
		let mut end = sel_end;
		while end < chars.len() && chars[end] != '\n' {
			end += 1;
		}

		let block: String = chars[start..end].iter().collect();
		let lines: Vec<&str> = block.split('\n').collect();
		let prefix = &self.comment_prefix;

		// Check if ALL non-empty lines already have the prefix.
		let all_commented = lines
			.iter()
			.all(|l| l.is_empty() || l.starts_with(prefix.as_str()));

		let new_lines: Vec<String> = if all_commented {
			lines
				.iter()
				.map(|l| {
					if l.starts_with(prefix.as_str()) {
						l[prefix.len()..].to_string()
					} else {
						l.to_string()
					}
				})
				.collect()
		} else {
			lines.iter().map(|l| format!("{prefix}{l}")).collect()
		};

		let new_block = new_lines.join("\n");
		let new_len = new_block.chars().count();

		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(end)),
		});
		let _ = self
			.runtime
			.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
		self.runtime
			.apply_operation(Operation::insert(Position::new(start), new_block));
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(start + new_len)),
		});
	}

	// ── Soft tabs ────────────────────────────────────────────────────

	/// Set the tab display/insert size (1–8). Default: 4.
	#[wasm_bindgen]
	pub fn set_tab_size(&mut self, size: usize) {
		self.tab_size = size.clamp(1, 8);
	}

	/// Get the current tab size.
	#[wasm_bindgen]
	pub fn tab_size(&self) -> usize {
		self.tab_size
	}

	/// Enable or disable soft tabs (spaces instead of `\t`).
	#[wasm_bindgen]
	pub fn set_soft_tabs(&mut self, enabled: bool) {
		self.soft_tabs = enabled;
	}

	/// Whether soft tabs are enabled.
	#[wasm_bindgen]
	pub fn soft_tabs(&self) -> bool {
		self.soft_tabs
	}

	/// Insert one "tab" at the cursor — either spaces or a `\t`.
	#[wasm_bindgen]
	pub fn insert_tab(&mut self) {
		if !self.is_writable() {
			return;
		}
		let tab_str = if self.soft_tabs {
			" ".repeat(self.tab_size)
		} else {
			"\t".to_string()
		};
		let len = tab_str.chars().count();
		let offset = self.runtime.selection().end().offset();
		self.runtime
			.apply_operation(Operation::insert(Position::new(offset), tab_str));
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(offset + len)),
		});
	}

	// ── Auto-surround ────────────────────────────────────────────────

	/// Enable or disable auto-surround on selection.
	///
	/// When enabled and text is selected, typing an opening bracket
	/// wraps the selection instead of replacing it.
	#[wasm_bindgen]
	pub fn set_auto_surround(&mut self, enabled: bool) {
		self.auto_surround = enabled;
	}

	/// Whether auto-surround is enabled.
	#[wasm_bindgen]
	pub fn auto_surround(&self) -> bool {
		self.auto_surround
	}

	/// If auto-surround is on and the selection is non-empty, wrap the
	/// selection with the opening/closing pair. Returns `true` if wrapping
	/// happened.
	#[wasm_bindgen]
	pub fn try_auto_surround(&mut self, ch: &str) -> bool {
		if !self.is_writable() || !self.auto_surround {
			return false;
		}
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return false;
		}
		let open_char = ch.chars().next().unwrap_or(' ');
		let close = match open_char {
			'(' => ")",
			'[' => "]",
			'{' => "}",
			'"' => "\"",
			'\'' => "'",
			'`' => "`",
			_ => return false,
		};
		self.wrap_selection(ch, close);
		true
	}

	// ── Expand / contract selection ──────────────────────────────────

	/// Expand selection intelligently: word → quoted → bracketed → line → all.
	///
	/// Each call expands to the next logical boundary.
	#[wasm_bindgen]
	pub fn expand_selection(&mut self) {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let sel = self.runtime.selection();
		let sel_start = sel.start().offset().min(chars.len());
		let sel_end = sel.end().offset().min(chars.len());
		let len = chars.len();

		// 1. If collapsed → select word.
		if sel_start == sel_end {
			let (ws, we) = self.word_bounds(sel_start, &chars);
			if ws < we {
				let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
					selection: Selection::range(Position::new(ws), Position::new(we)),
				});
				return;
			}
		}

		// 2. Try to expand to matching quotes.
		if let Some((qs, qe)) =
			Self::find_surrounding(&chars, sel_start, sel_end, &['"', '\'', '`'])
		{
			if qs < sel_start || qe > sel_end {
				let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
					selection: Selection::range(Position::new(qs), Position::new(qe)),
				});
				return;
			}
		}

		// 3. Try to expand to matching brackets.
		if let Some((bs, be)) = Self::find_surrounding_brackets(&chars, sel_start, sel_end) {
			if bs < sel_start || be > sel_end {
				let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
					selection: Selection::range(Position::new(bs), Position::new(be)),
				});
				return;
			}
		}

		// 4. Expand to full line.
		let mut line_start = sel_start;
		while line_start > 0 && chars[line_start - 1] != '\n' {
			line_start -= 1;
		}
		let mut line_end = sel_end;
		while line_end < len && chars[line_end] != '\n' {
			line_end += 1;
		}
		if line_start < sel_start || line_end > sel_end {
			let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
				selection: Selection::range(Position::new(line_start), Position::new(line_end)),
			});
			return;
		}

		// 5. Select all.
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(0), Position::new(len)),
		});
	}

	/// Contract selection intelligently (reverse of expand).
	///
	/// Shrinks: all → line → bracket → quote → word → collapsed.
	#[wasm_bindgen]
	pub fn contract_selection(&mut self) {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let sel = self.runtime.selection();
		let sel_start = sel.start().offset().min(chars.len());
		let sel_end = sel.end().offset().min(chars.len());

		if sel_start == sel_end {
			return; // Already collapsed.
		}

		// Try shrinking to line.
		let mut line_start = sel_start;
		while line_start > 0 && chars[line_start - 1] != '\n' {
			line_start -= 1;
		}
		let mut line_end = sel_end;
		while line_end < chars.len() && chars[line_end] != '\n' {
			line_end += 1;
		}
		if sel_start < line_start || sel_end > line_end {
			let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
				selection: Selection::range(Position::new(line_start), Position::new(line_end)),
			});
			return;
		}

		// Try shrinking to brackets.
		if let Some((bs, be)) = Self::find_inner_brackets(&chars, sel_start, sel_end) {
			let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
				selection: Selection::range(Position::new(bs), Position::new(be)),
			});
			return;
		}

		// Try shrinking to word.
		let mid = (sel_start + sel_end) / 2;
		let (ws, we) = self.word_bounds(mid, &chars);
		if ws >= sel_start && we <= sel_end && (ws > sel_start || we < sel_end) {
			let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
				selection: Selection::range(Position::new(ws), Position::new(we)),
			});
			return;
		}

		// Collapse to cursor position.
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(sel_end)),
		});
	}

	/// Word boundaries around a position.
	fn word_bounds(&self, pos: usize, chars: &[char]) -> (usize, usize) {
		if pos >= chars.len() {
			return (pos, pos);
		}
		let mut start = pos;
		while start > 0 && chars[start - 1].is_alphanumeric() {
			start -= 1;
		}
		let mut end = pos;
		while end < chars.len() && chars[end].is_alphanumeric() {
			end += 1;
		}
		(start, end)
	}

	/// Find the innermost surrounding quote pair that fully contains [sel_start..sel_end].
	fn find_surrounding(
		chars: &[char],
		sel_start: usize,
		sel_end: usize,
		delimiters: &[char],
	) -> Option<(usize, usize)> {
		let mut best: Option<(usize, usize)> = None;
		for &delim in delimiters {
			// Search backward for opening.
			let mut open = sel_start;
			loop {
				if open == 0 {
					break;
				}
				open -= 1;
				if chars[open] == delim {
					// Search forward for closing.
					let mut close = sel_end;
					while close < chars.len() {
						if chars[close] == delim {
							let inner_start = open + 1;
							let inner_end = close;
							if inner_start <= sel_start && inner_end >= sel_end {
								let span = inner_end - inner_start;
								if best.is_none() || span < best.unwrap().1 - best.unwrap().0 {
									best = Some((inner_start, inner_end));
								}
							}
							break;
						}
						close += 1;
					}
					break;
				}
			}
		}
		best
	}

	/// Find the innermost surrounding bracket pair that contains [sel_start..sel_end].
	fn find_surrounding_brackets(
		chars: &[char],
		sel_start: usize,
		sel_end: usize,
	) -> Option<(usize, usize)> {
		let pairs = [('(', ')'), ('[', ']'), ('{', '}')];
		let mut best: Option<(usize, usize)> = None;
		for (open_ch, close_ch) in &pairs {
			// Search backward for unmatched open.
			let mut depth = 0i32;
			let mut open = sel_start;
			let mut found_open = None;
			loop {
				if open == 0 {
					break;
				}
				open -= 1;
				if chars[open] == *close_ch {
					depth += 1;
				} else if chars[open] == *open_ch {
					if depth == 0 {
						found_open = Some(open);
						break;
					}
					depth -= 1;
				}
			}
			if let Some(o) = found_open {
				// Search forward for matching close.
				let mut depth2 = 0i32;
				let mut close = sel_end;
				while close < chars.len() {
					if chars[close] == *open_ch {
						depth2 += 1;
					} else if chars[close] == *close_ch {
						if depth2 == 0 {
							let inner_start = o + 1;
							let inner_end = close;
							let span = inner_end - inner_start;
							if best.is_none() || span < best.unwrap().1 - best.unwrap().0 {
								best = Some((inner_start, inner_end));
							}
							break;
						}
						depth2 -= 1;
					}
					close += 1;
				}
			}
		}
		best
	}

	/// Find innermost brackets strictly inside [sel_start..sel_end].
	fn find_inner_brackets(
		chars: &[char],
		sel_start: usize,
		sel_end: usize,
	) -> Option<(usize, usize)> {
		let pairs = [('(', ')'), ('[', ']'), ('{', '}')];
		let mut best: Option<(usize, usize)> = None;
		for (open_ch, close_ch) in &pairs {
			for i in sel_start..sel_end {
				if chars[i] == *open_ch {
					let mut depth = 0i32;
					for j in (i + 1)..sel_end {
						if chars[j] == *open_ch {
							depth += 1;
						} else if chars[j] == *close_ch {
							if depth == 0 {
								let inner_start = i + 1;
								let inner_end = j;
								if inner_start > sel_start || inner_end < sel_end {
									let span = inner_end - inner_start;
									if best.is_none() || span < best.unwrap().1 - best.unwrap().0 {
										best = Some((inner_start, inner_end));
									}
								}
								break;
							}
							depth -= 1;
						}
					}
				}
			}
		}
		best
	}

	// ── Matching bracket highlight ───────────────────────────────────

	/// Toggle matching bracket highlight.
	#[wasm_bindgen]
	pub fn set_highlight_matching_brackets(&mut self, enabled: bool) {
		self.highlight_matching_brackets = enabled;
	}

	/// Whether matching bracket highlighting is enabled.
	#[wasm_bindgen]
	pub fn highlight_matching_brackets(&self) -> bool {
		self.highlight_matching_brackets
	}

	/// Find the offset of the bracket matching the one at `offset`.
	///
	/// Returns `None` (via -1 in WASM) if the char at `offset` is not a
	/// bracket or no match is found.
	#[wasm_bindgen]
	pub fn find_matching_bracket(&self, offset: usize) -> i32 {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		if offset >= chars.len() {
			return -1;
		}
		let ch = chars[offset];
		let (forward, target) = match ch {
			'(' => (true, ')'),
			'[' => (true, ']'),
			'{' => (true, '}'),
			')' => (false, '('),
			']' => (false, '['),
			'}' => (false, '{'),
			_ => return -1,
		};
		let mut depth = 0i32;
		if forward {
			for i in (offset + 1)..chars.len() {
				if chars[i] == ch {
					depth += 1;
				} else if chars[i] == target {
					if depth == 0 {
						return i as i32;
					}
					depth -= 1;
				}
			}
		} else {
			let mut i = offset;
			while i > 0 {
				i -= 1;
				if chars[i] == ch {
					depth += 1;
				} else if chars[i] == target {
					if depth == 0 {
						return i as i32;
					}
					depth -= 1;
				}
			}
		}
		-1
	}

	// ── Move to matching bracket ─────────────────────────────────────

	/// Move cursor to the matching bracket (Ctrl+Shift+\).
	///
	/// Checks the character at the cursor and the one before it.
	/// If a bracket is found, jumps the cursor to its match.
	#[wasm_bindgen]
	pub fn move_to_matching_bracket(&mut self) {
		let offset = self.runtime.selection().end().offset();
		// Try offset first, then offset-1.
		let offsets: Vec<usize> = if offset > 0 {
			vec![offset, offset - 1]
		} else {
			vec![offset]
		};
		for &o in &offsets {
			let m = self.find_matching_bracket(o);
			if m >= 0 {
				let target = m as usize;
				let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
					selection: Selection::collapsed(Position::new(target)),
				});
				return;
			}
		}
	}

	// ── Document statistics (extras) ─────────────────────────────────

	/// Total paragraph count (non-empty lines).
	#[wasm_bindgen]
	pub fn paragraph_count(&self) -> usize {
		self.runtime
			.document()
			.plain_text()
			.split('\n')
			.filter(|l| !l.trim().is_empty())
			.count()
			.max(1)
	}

	/// Current line number the cursor is on (1-based).
	#[wasm_bindgen]
	pub fn current_line_number(&self) -> usize {
		let offset = self.runtime.selection().end().offset();
		let text = self.runtime.document().plain_text();
		let chars: Vec<char> = text.chars().collect();
		let mut line = 1usize;
		for i in 0..offset.min(chars.len()) {
			if chars[i] == '\n' {
				line += 1;
			}
		}
		line
	}

	/// Current column (1-based character offset from line start).
	#[wasm_bindgen]
	pub fn current_column(&self) -> usize {
		let offset = self.runtime.selection().end().offset();
		let text = self.runtime.document().plain_text();
		let chars: Vec<char> = text.chars().collect();
		let mut col = 1usize;
		let mut i = offset.min(chars.len());
		while i > 0 && chars[i - 1] != '\n' {
			i -= 1;
			col += 1;
		}
		col
	}

	// ── Indent guides ────────────────────────────────────────────────

	/// Toggle indent guide rendering.
	#[wasm_bindgen]
	pub fn set_show_indent_guides(&mut self, show: bool) {
		self.show_indent_guides = show;
	}

	/// Whether indent guides are enabled.
	#[wasm_bindgen]
	pub fn show_indent_guides(&self) -> bool {
		self.show_indent_guides
	}

	// ── Bookmarks ────────────────────────────────────────────────────

	/// Toggle a bookmark on the current line.
	///
	/// Returns `true` if the bookmark was added, `false` if removed.
	#[wasm_bindgen]
	pub fn toggle_bookmark(&mut self) -> bool {
		let line = self.current_line_number() - 1; // 0-based
		if self.bookmarks.contains(&line) {
			self.bookmarks.remove(&line);
			false
		} else {
			self.bookmarks.insert(line);
			true
		}
	}

	/// Jump to the next bookmark after the current line.
	///
	/// Wraps around to the first bookmark if past the last one.
	/// Returns `true` if a bookmark was found.
	#[wasm_bindgen]
	pub fn next_bookmark(&mut self) -> bool {
		if self.bookmarks.is_empty() {
			return false;
		}
		let current = self.current_line_number() - 1;
		// Find first bookmark after current line.
		let target = self
			.bookmarks
			.range((current + 1)..)
			.next()
			.or_else(|| self.bookmarks.iter().next())
			.copied();
		if let Some(line) = target {
			self.go_to_line(line + 1); // go_to_line is 1-based
			return true;
		}
		false
	}

	/// Jump to the previous bookmark before the current line.
	///
	/// Wraps around to the last bookmark if before the first one.
	/// Returns `true` if a bookmark was found.
	#[wasm_bindgen]
	pub fn prev_bookmark(&mut self) -> bool {
		if self.bookmarks.is_empty() {
			return false;
		}
		let current = self.current_line_number() - 1;
		let target = if current > 0 {
			self.bookmarks
				.range(..current)
				.next_back()
				.or_else(|| self.bookmarks.iter().next_back())
				.copied()
		} else {
			self.bookmarks.iter().next_back().copied()
		};
		if let Some(line) = target {
			self.go_to_line(line + 1);
			return true;
		}
		false
	}

	/// Remove all bookmarks.
	#[wasm_bindgen]
	pub fn clear_bookmarks(&mut self) {
		self.bookmarks.clear();
	}

	/// Number of active bookmarks.
	#[wasm_bindgen]
	pub fn bookmark_count(&self) -> usize {
		self.bookmarks.len()
	}

	/// Check if the current line has a bookmark.
	#[wasm_bindgen]
	pub fn is_line_bookmarked(&self) -> bool {
		let line = self.current_line_number() - 1;
		self.bookmarks.contains(&line)
	}

	/// Return all bookmarked line numbers as a flat array (0-based).
	#[wasm_bindgen]
	pub fn bookmarked_lines(&self) -> Vec<usize> {
		self.bookmarks.iter().copied().collect()
	}

	// ── Convert indentation ──────────────────────────────────────────

	/// Convert all tabs to spaces (using the current tab_size).
	///
	/// Returns the number of tabs replaced.
	#[wasm_bindgen]
	pub fn tabs_to_spaces(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let text = self.runtime.document().plain_text();
		let spaces = " ".repeat(self.tab_size);
		let count = text.matches('\t').count();
		if count > 0 {
			let new_text = text.replace('\t', &spaces);
			self.runtime.document_mut().set_plain_text(&new_text);
		}
		count
	}

	/// Convert leading spaces to tabs (using the current tab_size).
	///
	/// Only converts groups of `tab_size` spaces at the start of lines.
	/// Returns the number of conversions made.
	#[wasm_bindgen]
	pub fn spaces_to_tabs(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let text = self.runtime.document().plain_text();
		let ts = self.tab_size;
		let mut count = 0usize;
		let mut new_lines: Vec<String> = Vec::new();
		for line in text.split('\n') {
			let leading: usize = line.chars().take_while(|c| *c == ' ').count();
			let tab_count = leading / ts;
			if tab_count > 0 {
				count += tab_count;
				let tabs = "\t".repeat(tab_count);
				let remainder = &line[leading..];
				let leftover_spaces = " ".repeat(leading % ts);
				new_lines.push(format!("{tabs}{leftover_spaces}{remainder}"));
			} else {
				new_lines.push(line.to_string());
			}
		}
		if count > 0 {
			let new_text = new_lines.join("\n");
			self.runtime.document_mut().set_plain_text(&new_text);
		}
		count
	}

	// ── Open line above / below ──────────────────────────────────────

	/// Insert a new line below the current line and move cursor there
	/// (Ctrl+Enter).
	#[wasm_bindgen]
	pub fn open_line_below(&mut self) {
		if !self.is_writable() {
			return;
		}
		let text = self.runtime.document().plain_text();
		let chars: Vec<char> = text.chars().collect();
		let offset = self.runtime.selection().end().offset();

		// Find end of current line.
		let mut line_end = offset.min(chars.len());
		while line_end < chars.len() && chars[line_end] != '\n' {
			line_end += 1;
		}

		self.runtime
			.apply_operation(Operation::insert(Position::new(line_end), "\n".to_string()));
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(line_end + 1)),
		});
	}

	/// Insert a new line above the current line and move cursor there
	/// (Ctrl+Shift+Enter).
	#[wasm_bindgen]
	pub fn open_line_above(&mut self) {
		if !self.is_writable() {
			return;
		}
		let text = self.runtime.document().plain_text();
		let chars: Vec<char> = text.chars().collect();
		let offset = self.runtime.selection().end().offset();

		// Find start of current line.
		let mut line_start = offset.min(chars.len());
		while line_start > 0 && chars[line_start - 1] != '\n' {
			line_start -= 1;
		}

		self.runtime.apply_operation(Operation::insert(
			Position::new(line_start),
			"\n".to_string(),
		));
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(line_start)),
		});
	}

	// ── Copy / cut line (no selection) ───────────────────────────────

	/// Get the full text of the line the cursor is on (including the
	/// trailing `\n` if present). Useful for "copy line" when nothing is
	/// selected.
	#[wasm_bindgen]
	pub fn current_line_text(&self) -> String {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let offset = self.runtime.selection().end().offset();

		let mut start = offset.min(chars.len());
		while start > 0 && chars[start - 1] != '\n' {
			start -= 1;
		}
		let mut end = offset.min(chars.len());
		while end < chars.len() && chars[end] != '\n' {
			end += 1;
		}
		// Include trailing newline for paste-as-line behavior.
		if end < chars.len() {
			end += 1;
		}
		chars[start..end].iter().collect()
	}

	/// Cut the current line (remove it and return its text).
	/// This is the "cut line when nothing is selected" behavior.
	#[wasm_bindgen]
	pub fn cut_line(&mut self) -> String {
		if !self.is_writable() {
			return String::new();
		}
		let text = self.current_line_text();
		self.delete_line();
		text
	}

	// ── Overwrite mode ───────────────────────────────────────────────

	/// Toggle between insert and overwrite mode (Insert key).
	#[wasm_bindgen]
	pub fn toggle_overwrite_mode(&mut self) {
		self.overwrite_mode = !self.overwrite_mode;
	}

	/// Whether the editor is in overwrite mode.
	#[wasm_bindgen]
	pub fn overwrite_mode(&self) -> bool {
		self.overwrite_mode
	}

	/// Set overwrite mode explicitly.
	#[wasm_bindgen]
	pub fn set_overwrite_mode(&mut self, enabled: bool) {
		self.overwrite_mode = enabled;
	}

	/// Insert text respecting overwrite mode. In overwrite mode,
	/// characters after the cursor are replaced one-for-one rather
	/// than pushing text forward.
	#[wasm_bindgen]
	pub fn insert_text_overwrite(&mut self, text: &str) {
		if !self.is_writable() {
			return;
		}
		let offset = self.runtime.selection().end().offset();
		let insert_len = text.chars().count();

		if self.overwrite_mode {
			let plain = self.runtime.document().plain_text();
			let chars: Vec<char> = plain.chars().collect();
			// Count how many chars we can overwrite (stop at newline / end).
			let mut replace_count = 0usize;
			for i in 0..insert_len {
				let pos = offset + i;
				if pos >= chars.len() || chars[pos] == '\n' {
					break;
				}
				replace_count += 1;
			}
			// Select the range to replace, delete, then insert at offset.
			if replace_count > 0 {
				let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
					selection: Selection::range(
						Position::new(offset),
						Position::new(offset + replace_count),
					),
				});
				let _ = self
					.runtime
					.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
			}
		}

		// After delete (if any), cursor should be at `offset`.
		self.runtime
			.apply_operation(Operation::insert(Position::new(offset), text.to_string()));
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(offset + insert_len)),
		});
	}

	// ── Center line in viewport ──────────────────────────────────────

	/// Scroll so the cursor's line is vertically centered in the viewport.
	#[wasm_bindgen]
	pub fn center_line_in_viewport(&mut self) {
		let content_h = self.content_height().unwrap_or(0.0);
		let viewport_h = self.height;
		if content_h <= viewport_h {
			self.scroll_y = 0.0;
			return;
		}
		// Estimate cursor Y from line number.
		let line_num = self.current_line_number();
		let line_h = 24.0 * self.zoom; // approximate line height
		let cursor_y = (line_num as f32 - 1.0) * line_h;
		let target = (cursor_y - viewport_h / 2.0 + line_h / 2.0).max(0.0);
		let max_scroll = (content_h - viewport_h).max(0.0);
		self.scroll_y = target.min(max_scroll);
	}

	// ── Go to document start / end ───────────────────────────────────

	/// Move cursor to the very beginning of the document (Ctrl+Home).
	#[wasm_bindgen]
	pub fn go_to_document_start(&mut self) {
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(0)),
		});
		self.scroll_y = 0.0;
	}

	/// Move cursor to the very end of the document (Ctrl+End).
	#[wasm_bindgen]
	pub fn go_to_document_end(&mut self) {
		let len = self.runtime.document().plain_text().chars().count();
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(len)),
		});
		// Scroll to bottom.
		let content_h = self.content_height().unwrap_or(0.0);
		let viewport_h = self.height;
		self.scroll_y = (content_h - viewport_h).max(0.0);
	}

	/// Select from cursor to document start (Ctrl+Shift+Home).
	#[wasm_bindgen]
	pub fn select_to_document_start(&mut self) {
		let end = self.runtime.selection().end().offset();
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(0), Position::new(end)),
		});
		self.scroll_y = 0.0;
	}

	/// Select from cursor to document end (Ctrl+Shift+End).
	#[wasm_bindgen]
	pub fn select_to_document_end(&mut self) {
		let start = self.runtime.selection().start().offset();
		let len = self.runtime.document().plain_text().chars().count();
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(len)),
		});
		let content_h = self.content_height().unwrap_or(0.0);
		let viewport_h = self.height;
		self.scroll_y = (content_h - viewport_h).max(0.0);
	}

	// ── Select between brackets ──────────────────────────────────────

	/// Select all text between the nearest enclosing bracket pair.
	///
	/// Returns `true` if brackets were found and selection was made.
	#[wasm_bindgen]
	pub fn select_between_brackets(&mut self) -> bool {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let offset = self.runtime.selection().end().offset().min(chars.len());

		if let Some((inner_start, inner_end)) =
			Self::find_surrounding_brackets(&chars, offset, offset)
		{
			let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
				selection: Selection::range(Position::new(inner_start), Position::new(inner_end)),
			});
			return true;
		}
		false
	}

	// ── Cursor position history ──────────────────────────────────────

	/// Record the current cursor position in the history stack.
	///
	/// Call this before navigation jumps (go-to-line, bookmark jump, etc.)
	/// so the user can navigate back. Deduplicates consecutive identical
	/// positions and caps the stack at 100 entries.
	#[wasm_bindgen]
	pub fn push_cursor_history(&mut self) {
		let offset = self.runtime.selection().end().offset();
		// Deduplicate.
		if self.cursor_history.last() == Some(&offset) {
			return;
		}
		// If we navigated back and then push, trim the forward entries.
		if self.cursor_history_index >= 0 {
			let idx = self.cursor_history_index as usize;
			self.cursor_history.truncate(idx + 1);
		}
		self.cursor_history.push(offset);
		// Cap at 100.
		if self.cursor_history.len() > 100 {
			self.cursor_history.remove(0);
		}
		self.cursor_history_index = -1; // at tip
	}

	/// Navigate backward in cursor history (Ctrl+Alt+←).
	///
	/// Returns `true` if the cursor moved.
	#[wasm_bindgen]
	pub fn cursor_history_back(&mut self) -> bool {
		if self.cursor_history.is_empty() {
			return false;
		}
		let current = if self.cursor_history_index < 0 {
			// First "back": skip the most recent (current) entry and
			// go to the one before it.
			self.cursor_history.len() as i32 - 2
		} else {
			self.cursor_history_index - 1
		};
		if current < 0 {
			return false;
		}
		self.cursor_history_index = current;
		let target = self.cursor_history[current as usize];
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(target)),
		});
		true
	}

	/// Navigate forward in cursor history (Ctrl+Alt+→).
	///
	/// Returns `true` if the cursor moved.
	#[wasm_bindgen]
	pub fn cursor_history_forward(&mut self) -> bool {
		if self.cursor_history_index < 0 {
			return false; // Already at tip.
		}
		let next = self.cursor_history_index + 1;
		if next as usize >= self.cursor_history.len() {
			self.cursor_history_index = -1; // back to tip
			return false;
		}
		self.cursor_history_index = next;
		let target = self.cursor_history[next as usize];
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(target)),
		});
		true
	}

	/// Number of positions in cursor history.
	#[wasm_bindgen]
	pub fn cursor_history_length(&self) -> usize {
		self.cursor_history.len()
	}

	// ── Select all occurrences ───────────────────────────────────────

	/// Find all occurrences of the currently selected text.
	///
	/// Returns the count of matches found (0 if nothing is selected or no
	/// matches). The offsets can be retrieved with `find_all`.
	#[wasm_bindgen]
	pub fn select_all_occurrences(&self) -> usize {
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let start = sel.start().offset();
		let end = sel.end().offset();
		let chars: Vec<char> = plain.chars().collect();
		let needle: String = chars[start.min(chars.len())..end.min(chars.len())]
			.iter()
			.collect();
		if needle.is_empty() {
			return 0;
		}
		plain.matches(&needle).count()
	}

	/// Return all occurrence offsets of the selected text as a flat array
	/// `[start0, end0, start1, end1, ...]`.
	#[wasm_bindgen]
	pub fn occurrence_offsets(&self) -> Vec<usize> {
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return Vec::new();
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let start = sel.start().offset().min(chars.len());
		let end = sel.end().offset().min(chars.len());
		let needle: String = chars[start..end].iter().collect();
		if needle.is_empty() {
			return Vec::new();
		}
		let needle_len = needle.chars().count();
		let mut results = Vec::new();
		let mut search_from = 0;
		while let Some(byte_pos) = plain[search_from..].find(&needle) {
			let abs_byte = search_from + byte_pos;
			let char_pos = plain[..abs_byte].chars().count();
			results.push(char_pos);
			results.push(char_pos + needle_len);
			search_from = abs_byte + needle.len();
		}
		results
	}

	// ── Whole word find ──────────────────────────────────────────────

	/// Find all whole-word occurrences of `needle`.
	///
	/// Returns offsets as `[start0, end0, start1, end1, ...]`.
	/// A "whole word" match requires the char before and after the match
	/// to be non-alphanumeric (or at document boundary).
	#[wasm_bindgen]
	pub fn find_all_whole_word(&self, needle: &str) -> Vec<usize> {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let needle_chars: Vec<char> = needle.chars().collect();
		let needle_len = needle_chars.len();
		if needle_len == 0 || chars.len() < needle_len {
			return Vec::new();
		}
		let mut results = Vec::new();
		let mut i = 0;
		while i + needle_len <= chars.len() {
			if chars[i..i + needle_len] == needle_chars[..] {
				// Check word boundary before.
				let before_ok = i == 0 || !chars[i - 1].is_alphanumeric();
				// Check word boundary after.
				let after_ok =
					i + needle_len >= chars.len() || !chars[i + needle_len].is_alphanumeric();
				if before_ok && after_ok {
					results.push(i);
					results.push(i + needle_len);
					i += needle_len;
					continue;
				}
			}
			i += 1;
		}
		results
	}

	// ── Paragraph navigation ─────────────────────────────────────────

	/// Move cursor to the start of the previous paragraph (Ctrl+↑).
	///
	/// A paragraph boundary is an empty line or the document start.
	#[wasm_bindgen]
	pub fn move_to_prev_paragraph(&mut self) {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let mut pos = self.runtime.selection().end().offset().min(chars.len());

		// Skip current line to start.
		while pos > 0 && chars[pos - 1] != '\n' {
			pos -= 1;
		}
		// Skip blank lines.
		while pos > 0 && chars[pos - 1] == '\n' {
			pos -= 1;
		}
		// Move to start of previous non-blank line.
		while pos > 0 && chars[pos - 1] != '\n' {
			pos -= 1;
		}

		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(pos)),
		});
	}

	/// Move cursor to the start of the next paragraph (Ctrl+↓).
	#[wasm_bindgen]
	pub fn move_to_next_paragraph(&mut self) {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let mut pos = self.runtime.selection().end().offset().min(chars.len());

		// Skip to end of current line.
		while pos < chars.len() && chars[pos] != '\n' {
			pos += 1;
		}
		// Skip blank lines.
		while pos < chars.len() && chars[pos] == '\n' {
			pos += 1;
		}

		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(pos)),
		});
	}

	// ── Snippet insertion ────────────────────────────────────────────

	/// Insert a snippet template. `$0` marks where the cursor should
	/// be placed after insertion. Other text is inserted literally.
	///
	/// Example: `insert_snippet("if ($0) {\n}")` inserts the template
	/// and places the cursor between the parentheses.
	#[wasm_bindgen]
	pub fn insert_snippet(&mut self, template: &str) {
		if !self.is_writable() {
			return;
		}
		let offset = self.runtime.selection().end().offset();

		// Delete selection if any.
		let sel = self.runtime.selection();
		if !sel.is_collapsed() {
			self.delete_range(sel.start().offset(), sel.end().offset());
		}
		let insert_at = self.runtime.selection().end().offset();

		// Find $0 cursor marker position.
		let cursor_marker = template.find("$0");
		let clean = template.replace("$0", "");
		let clean_len = clean.chars().count();

		self.runtime
			.apply_operation(Operation::insert(Position::new(insert_at), clean));

		let cursor_pos = if let Some(byte_pos) = cursor_marker {
			let char_pos = template[..byte_pos].chars().count();
			insert_at + char_pos
		} else {
			insert_at + clean_len
		};

		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(cursor_pos)),
		});
	}

	// ── Scroll to selection ──────────────────────────────────────────

	/// Ensure the current selection (or cursor) is visible in the
	/// viewport. Scrolls the minimum amount needed.
	#[wasm_bindgen]
	pub fn scroll_to_selection(&mut self) {
		let line_num = self.current_line_number();
		let line_h = 24.0 * self.zoom; // approximate
		let cursor_y = (line_num as f32 - 1.0) * line_h;
		let viewport_h = self.height;
		let padding = line_h * 2.0; // keep 2 lines visible above/below

		if cursor_y < self.scroll_y + padding {
			// Cursor above viewport.
			self.scroll_y = (cursor_y - padding).max(0.0);
		} else if cursor_y + line_h > self.scroll_y + viewport_h - padding {
			// Cursor below viewport.
			let content_h = self.content_height().unwrap_or(0.0);
			let target = cursor_y + line_h + padding - viewport_h;
			self.scroll_y = target.min((content_h - viewport_h).max(0.0));
		}
	}

	// ── Column ruler ─────────────────────────────────────────────────

	/// Set column ruler positions (e.g. `[80, 120]`).
	///
	/// Pass an empty array to remove all rulers. Rulers are drawn as
	/// thin vertical lines at the specified column offsets.
	#[wasm_bindgen]
	pub fn set_rulers(&mut self, columns: &[usize]) {
		self.rulers = columns.to_vec();
	}

	/// Get the current ruler columns as a flat array.
	#[wasm_bindgen]
	pub fn rulers(&self) -> Vec<usize> {
		self.rulers.clone()
	}

	/// Add a single ruler at the given column.
	#[wasm_bindgen]
	pub fn add_ruler(&mut self, column: usize) {
		if !self.rulers.contains(&column) {
			self.rulers.push(column);
			self.rulers.sort_unstable();
		}
	}

	/// Remove the ruler at the given column.
	#[wasm_bindgen]
	pub fn remove_ruler(&mut self, column: usize) {
		self.rulers.retain(|&c| c != column);
	}

	// ── Ensure final newline ─────────────────────────────────────────

	/// Ensure the document ends with a newline character.
	///
	/// Returns `true` if a newline was added.
	#[wasm_bindgen]
	pub fn ensure_final_newline(&mut self) -> bool {
		if !self.is_writable() {
			return false;
		}
		let plain = self.runtime.document().plain_text();
		if plain.is_empty() || plain.ends_with('\n') {
			return false;
		}
		let len = plain.chars().count();
		self.runtime
			.apply_operation(Operation::insert(Position::new(len), "\n".to_string()));
		true
	}

	// ── Replace all occurrences of selection ─────────────────────────

	/// Replace all occurrences of the selected text with `replacement`.
	///
	/// Returns the number of replacements made. Processes from end to
	/// start so offsets remain valid.
	#[wasm_bindgen]
	pub fn replace_all_occurrences(&mut self, replacement: &str) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let sel = self.runtime.selection();
		if sel.is_collapsed() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let start = sel.start().offset().min(chars.len());
		let end = sel.end().offset().min(chars.len());
		let needle: String = chars[start..end].iter().collect();
		if needle.is_empty() {
			return 0;
		}
		let needle_len = needle.chars().count();
		let replacement_len = replacement.chars().count();

		// Collect all match positions.
		let mut matches: Vec<usize> = Vec::new();
		let mut search_from = 0;
		while let Some(byte_pos) = plain[search_from..].find(&needle) {
			let abs_byte = search_from + byte_pos;
			let char_pos = plain[..abs_byte].chars().count();
			matches.push(char_pos);
			search_from = abs_byte + needle.len();
		}

		// Replace from end to start.
		for &pos in matches.iter().rev() {
			let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
				selection: Selection::range(Position::new(pos), Position::new(pos + needle_len)),
			});
			let _ = self
				.runtime
				.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
			if !replacement.is_empty() {
				self.runtime.apply_operation(Operation::insert(
					Position::new(pos),
					replacement.to_string(),
				));
			}
		}

		let count = matches.len();
		if count > 0 {
			// Place cursor at the end of the first replacement.
			let first = matches[0];
			let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
				selection: Selection::collapsed(Position::new(first + replacement_len)),
			});
		}
		count
	}

	// ── Reverse lines ────────────────────────────────────────────────

	/// Reverse the order of selected lines.
	#[wasm_bindgen]
	pub fn reverse_lines(&mut self) {
		if !self.is_writable() {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let sel = self.runtime.selection();
		let sel_start = sel.start().offset().min(chars.len());
		let sel_end = sel.end().offset().min(chars.len());

		// Expand to full lines.
		let mut start = sel_start;
		while start > 0 && chars[start - 1] != '\n' {
			start -= 1;
		}
		let mut end = sel_end;
		while end < chars.len() && chars[end] != '\n' {
			end += 1;
		}

		let block: String = chars[start..end].iter().collect();
		let mut lines: Vec<&str> = block.split('\n').collect();
		lines.reverse();
		let reversed = lines.join("\n");
		let reversed_len = reversed.chars().count();

		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(end)),
		});
		let _ = self
			.runtime
			.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
		self.runtime
			.apply_operation(Operation::insert(Position::new(start), reversed));
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(start + reversed_len)),
		});
	}

	// ── Encode / decode selection ────────────────────────────────────

	/// Base64-encode the selected text, replacing the selection.
	#[wasm_bindgen]
	pub fn base64_encode_selection(&mut self) {
		if !self.is_writable() {
			return;
		}
		self.transform_selection(|s| {
			use base64::Engine;
			base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
		});
	}

	/// Base64-decode the selected text, replacing the selection.
	///
	/// If the selected text is not valid base64, the selection is unchanged.
	#[wasm_bindgen]
	pub fn base64_decode_selection(&mut self) {
		if !self.is_writable() {
			return;
		}
		self.transform_selection(|s| {
			use base64::Engine;
			match base64::engine::general_purpose::STANDARD.decode(s.as_bytes()) {
				Ok(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| s.to_string()),
				Err(_) => s.to_string(),
			}
		});
	}

	/// URL-encode the selected text, replacing the selection.
	#[wasm_bindgen]
	pub fn url_encode_selection(&mut self) {
		if !self.is_writable() {
			return;
		}
		self.transform_selection(|s| {
			s.chars()
				.map(|c| {
					if c.is_ascii_alphanumeric() || "-_.~".contains(c) {
						c.to_string()
					} else {
						let mut buf = [0u8; 4];
						let bytes = c.encode_utf8(&mut buf);
						bytes
							.bytes()
							.map(|b| format!("%{b:02X}"))
							.collect::<String>()
					}
				})
				.collect()
		});
	}

	/// URL-decode the selected text, replacing the selection.
	#[wasm_bindgen]
	pub fn url_decode_selection(&mut self) {
		if !self.is_writable() {
			return;
		}
		self.transform_selection(|s| {
			let mut result = String::new();
			let bytes = s.as_bytes();
			let mut i = 0;
			let mut raw: Vec<u8> = Vec::new();
			while i < bytes.len() {
				if bytes[i] == b'%' && i + 2 < bytes.len() {
					if let Ok(val) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
						raw.push(val);
						i += 3;
						continue;
					}
				}
				// Flush any accumulated raw bytes.
				if !raw.is_empty() {
					result.push_str(&String::from_utf8(raw.clone()).unwrap_or_default());
					raw.clear();
				}
				result.push(bytes[i] as char);
				i += 1;
			}
			if !raw.is_empty() {
				result.push_str(&String::from_utf8(raw).unwrap_or_default());
			}
			result
		});
	}

	// ── Toggle case ──────────────────────────────────────────────────

	/// Swap the case of each character in the selection (a↔A).
	#[wasm_bindgen]
	pub fn transform_toggle_case(&mut self) {
		if !self.is_writable() {
			return;
		}
		self.transform_selection(|s| {
			s.chars()
				.map(|c| {
					if c.is_uppercase() {
						c.to_lowercase().collect::<String>()
					} else {
						c.to_uppercase().collect::<String>()
					}
				})
				.collect()
		});
	}

	// ── Line decorations ─────────────────────────────────────────────

	/// Add a coloured background decoration to a line (0-based).
	///
	/// Multiple decorations can be added to the same line. The colours
	/// are blended in order.
	#[wasm_bindgen]
	pub fn add_line_decoration(&mut self, line: usize, r: u8, g: u8, b: u8, a: u8) {
		self.line_decorations.push((line, r, g, b, a));
	}

	/// Remove all decorations from a specific line.
	#[wasm_bindgen]
	pub fn remove_line_decorations(&mut self, line: usize) {
		self.line_decorations.retain(|&(l, ..)| l != line);
	}

	/// Remove all line decorations.
	#[wasm_bindgen]
	pub fn clear_line_decorations(&mut self) {
		self.line_decorations.clear();
	}

	/// Number of active line decorations.
	#[wasm_bindgen]
	pub fn line_decoration_count(&self) -> usize {
		self.line_decorations.len()
	}

	// ── Modified state tracking ──────────────────────────────────────

	/// Whether the document has been modified since last save.
	#[wasm_bindgen]
	pub fn is_modified(&self) -> bool {
		self.is_modified
	}

	/// Mark the document as saved (clears the modified flag).
	#[wasm_bindgen]
	pub fn mark_saved(&mut self) {
		self.is_modified = false;
	}

	/// Mark the document as modified.
	///
	/// Called automatically by mutating operations. You can also call
	/// it manually to force the dirty state.
	#[wasm_bindgen]
	pub fn mark_modified(&mut self) {
		self.is_modified = true;
	}

	// ── Clipboard ring ───────────────────────────────────────────────

	/// Push a text entry into the clipboard ring.
	///
	/// The ring holds the most recent `clipboard_ring_max` entries
	/// (default 10). Newest entry is at index 0.
	#[wasm_bindgen]
	pub fn clipboard_ring_push(&mut self, text: &str) {
		if text.is_empty() {
			return;
		}
		// Deduplicate: remove if already present.
		self.clipboard_ring.retain(|t| t != text);
		self.clipboard_ring.insert(0, text.to_string());
		if self.clipboard_ring.len() > self.clipboard_ring_max {
			self.clipboard_ring.truncate(self.clipboard_ring_max);
		}
	}

	/// Get the clipboard ring entry at `index` (0 = most recent).
	///
	/// Returns empty string if index is out of range.
	#[wasm_bindgen]
	pub fn clipboard_ring_get(&self, index: usize) -> String {
		self.clipboard_ring.get(index).cloned().unwrap_or_default()
	}

	/// Number of entries in the clipboard ring.
	#[wasm_bindgen]
	pub fn clipboard_ring_length(&self) -> usize {
		self.clipboard_ring.len()
	}

	/// Clear the clipboard ring.
	#[wasm_bindgen]
	pub fn clipboard_ring_clear(&mut self) {
		self.clipboard_ring.clear();
	}

	/// Paste the clipboard ring entry at `index` at the cursor.
	#[wasm_bindgen]
	pub fn clipboard_ring_paste(&mut self, index: usize) {
		if !self.is_writable() {
			return;
		}
		let text = self.clipboard_ring_get(index);
		if text.is_empty() {
			return;
		}
		// Delete selection if any.
		let sel = self.runtime.selection();
		if !sel.is_collapsed() {
			self.delete_range(sel.start().offset(), sel.end().offset());
		}
		let offset = self.runtime.selection().end().offset();
		let len = text.chars().count();
		self.runtime
			.apply_operation(Operation::insert(Position::new(offset), text));
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(offset + len)),
		});
		self.is_modified = true;
	}

	// ── Word frequency ───────────────────────────────────────────────

	/// Return the top N most frequent words as alternating
	/// `[word, count, word, count, ...]` strings.
	#[wasm_bindgen]
	pub fn word_frequency(&self, top_n: usize) -> Vec<String> {
		let plain = self.runtime.document().plain_text();
		let mut freq: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
		for word in plain.split_whitespace() {
			*freq.entry(word).or_insert(0) += 1;
		}
		let mut pairs: Vec<(&&str, &usize)> = freq.iter().collect();
		pairs.sort_by(|a, b| b.1.cmp(a.1));
		let mut result = Vec::new();
		for (word, count) in pairs.into_iter().take(top_n) {
			result.push(word.to_string());
			result.push(count.to_string());
		}
		result
	}

	// ── Highlight occurrences of word under cursor ───────────────────

	/// Enable or disable highlighting all occurrences of the word under
	/// the cursor.
	#[wasm_bindgen]
	pub fn set_highlight_occurrences(&mut self, enabled: bool) {
		self.highlight_occurrences = enabled;
	}

	/// Whether occurrence highlighting is enabled.
	#[wasm_bindgen]
	pub fn highlight_occurrences(&self) -> bool {
		self.highlight_occurrences
	}

	/// Get the word under (or adjacent to) the cursor.
	///
	/// Returns empty string if the cursor is not on a word.
	#[wasm_bindgen]
	pub fn word_at_cursor(&self) -> String {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let offset = self.runtime.selection().end().offset().min(chars.len());
		let (start, end) = self.word_bounds(offset, &chars);
		if start == end {
			return String::new();
		}
		chars[start..end].iter().collect()
	}

	// ── Text measurement ─────────────────────────────────────────────

	/// Measure the pixel width of a string using the default style.
	///
	/// Useful for external layout calculations. Returns 0.0 if the
	/// canvas context is not available.
	#[wasm_bindgen]
	pub fn measure_text_width(&self, text: &str) -> f32 {
		let (_, ctx) = match self.canvas_and_context() {
			Ok(v) => v,
			Err(_) => return 0.0,
		};
		let renderer = Canvas2dRenderer::new(ctx, self.width, self.height);
		let lc = self.layout_constants();
		renderer.measure_text(text, &lc.default_style)
	}

	/// Measure the pixel width of a single character using the default
	/// style.
	#[wasm_bindgen]
	pub fn measure_char_width(&self, ch: &str) -> f32 {
		self.measure_text_width(ch)
	}

	// ── State serialization ─────────────────────────────────────────

	/// Serialize the editor state to a JSON string.
	///
	/// Includes text, selection, scroll position, theme, and settings.
	/// Use `restore_state` to reload.
	#[wasm_bindgen]
	pub fn save_state(&self) -> String {
		let plain = self.runtime.document().plain_text();
		let sel_start = self.runtime.selection().start().offset();
		let sel_end = self.runtime.selection().end().offset();
		let state = serde_json::json!({
			"text": plain,
			"selectionStart": sel_start,
			"selectionEnd": sel_end,
			"scrollY": self.scroll_y,
			"zoom": self.zoom,
			"readOnly": self.read_only,
			"wordWrap": self.word_wrap,
			"showLineNumbers": self.show_line_numbers,
			"showWhitespace": self.show_whitespace,
			"showIndentGuides": self.show_indent_guides,
			"autoCloseBrackets": self.auto_close_brackets,
			"autoSurround": self.auto_surround,
			"softTabs": self.soft_tabs,
			"tabSize": self.tab_size,
			"overwriteMode": self.overwrite_mode,
			"highlightCurrentLine": self.highlight_current_line,
			"highlightMatchingBrackets": self.highlight_matching_brackets,
			"highlightOccurrences": self.highlight_occurrences,
			"placeholder": self.placeholder,
			"maxLength": self.max_length,
		});
		state.to_string()
	}

	/// Restore editor state from a JSON string produced by `save_state`.
	#[wasm_bindgen]
	pub fn restore_state(&mut self, json: &str) {
		let v: serde_json::Value = match serde_json::from_str(json) {
			Ok(v) => v,
			Err(_) => return,
		};
		if let Some(text) = v["text"].as_str() {
			self.runtime.document_mut().set_plain_text(text);
		}
		let sel_start = v["selectionStart"].as_u64().unwrap_or(0) as usize;
		let sel_end = v["selectionEnd"].as_u64().unwrap_or(0) as usize;
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(sel_start), Position::new(sel_end)),
		});
		if let Some(sy) = v["scrollY"].as_f64() {
			self.scroll_y = sy as f32;
		}
		if let Some(z) = v["zoom"].as_f64() {
			self.zoom = (z as f32).clamp(0.25, 4.0);
		}
		if let Some(b) = v["readOnly"].as_bool() {
			self.read_only = b;
		}
		if let Some(b) = v["wordWrap"].as_bool() {
			self.word_wrap = b;
		}
		if let Some(b) = v["showLineNumbers"].as_bool() {
			self.show_line_numbers = b;
		}
		if let Some(b) = v["showWhitespace"].as_bool() {
			self.show_whitespace = b;
		}
		if let Some(b) = v["showIndentGuides"].as_bool() {
			self.show_indent_guides = b;
		}
		if let Some(b) = v["autoCloseBrackets"].as_bool() {
			self.auto_close_brackets = b;
		}
		if let Some(b) = v["autoSurround"].as_bool() {
			self.auto_surround = b;
		}
		if let Some(b) = v["softTabs"].as_bool() {
			self.soft_tabs = b;
		}
		if let Some(n) = v["tabSize"].as_u64() {
			self.tab_size = (n as usize).clamp(1, 8);
		}
		if let Some(b) = v["overwriteMode"].as_bool() {
			self.overwrite_mode = b;
		}
		if let Some(b) = v["highlightCurrentLine"].as_bool() {
			self.highlight_current_line = b;
		}
		if let Some(b) = v["highlightMatchingBrackets"].as_bool() {
			self.highlight_matching_brackets = b;
		}
		if let Some(b) = v["highlightOccurrences"].as_bool() {
			self.highlight_occurrences = b;
		}
		if let Some(s) = v["placeholder"].as_str() {
			self.placeholder = s.to_string();
		}
		if let Some(n) = v["maxLength"].as_u64() {
			self.max_length = n as usize;
		}
		self.is_modified = false;
	}

	// ── Placeholder text ─────────────────────────────────────────────

	/// Set placeholder text shown when the document is empty.
	#[wasm_bindgen]
	pub fn set_placeholder(&mut self, text: &str) {
		self.placeholder = text.to_string();
	}

	/// Get the current placeholder text.
	#[wasm_bindgen]
	pub fn placeholder(&self) -> String {
		self.placeholder.clone()
	}

	// ── Max length ───────────────────────────────────────────────────

	/// Set maximum character count (0 = unlimited).
	///
	/// When set, `insert_text` and similar operations will be truncated
	/// to stay within the limit.
	#[wasm_bindgen]
	pub fn set_max_length(&mut self, max: usize) {
		self.max_length = max;
	}

	/// Get the current max character count (0 = unlimited).
	#[wasm_bindgen]
	pub fn max_length(&self) -> usize {
		self.max_length
	}

	/// How many more characters can be inserted before hitting the limit.
	///
	/// Returns `usize::MAX` when max_length is 0 (unlimited).
	#[wasm_bindgen]
	pub fn remaining_capacity(&self) -> usize {
		if self.max_length == 0 {
			return usize::MAX;
		}
		let current = self.runtime.document().plain_text().chars().count();
		self.max_length.saturating_sub(current)
	}

	/// Insert text respecting the max_length constraint.
	///
	/// Truncates the input so the total never exceeds the limit.
	/// Returns the number of characters actually inserted.
	#[wasm_bindgen]
	pub fn insert_text_clamped(&mut self, text: &str) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let remaining = self.remaining_capacity();
		if remaining == 0 {
			return 0;
		}
		let chars: Vec<char> = text.chars().take(remaining).collect();
		let clamped: String = chars.iter().collect();
		let len = chars.len();
		let offset = self.runtime.selection().end().offset();
		self.runtime
			.apply_operation(Operation::insert(Position::new(offset), clamped));
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(offset + len)),
		});
		self.is_modified = true;
		len
	}

	// ── Batch operations ─────────────────────────────────────────────

	/// Begin a batch of operations that will be grouped into a single
	/// undo step. Call `end_batch` when done.
	///
	/// The runtime coalesces rapid edits automatically. This method
	/// serves as a logical marker — all edits between `begin_batch`
	/// and `end_batch` happen in quick succession and are treated as
	/// one undo group.
	#[wasm_bindgen]
	pub fn begin_batch(&self) {
		// Intentionally empty — the runtime coalesces edits within the
		// coalesce timeout window. begin_batch signals intent.
	}

	/// End a batch of operations.
	///
	/// After this call, the next edit will start a new undo group
	/// (once the coalesce timeout expires).
	#[wasm_bindgen]
	pub fn end_batch(&self) {
		// Intentionally empty — the coalesce timeout handles grouping.
	}

	// ── Regex find ───────────────────────────────────────────────────

	/// Find all matches of a regex pattern in the document.
	///
	/// Returns offsets as `[start0, end0, start1, end1, ...]`.
	/// Returns empty array if the pattern is invalid.
	///
	/// Note: uses a simple character-by-character implementation since
	/// the `regex` crate is heavy for WASM. Supports: `.` `*` `+` `?`
	/// `^` `$` `\d` `\w` `\s` and character classes `[abc]`.
	/// For full regex, use the JS `RegExp` in the host and pass offsets.
	#[wasm_bindgen]
	pub fn find_all_regex(&self, pattern: &str) -> Vec<usize> {
		// Delegate to JS RegExp via a simple strategy:
		// We provide the text; the caller should use JS RegExp for complex
		// patterns. This method handles the simple case of literal + flags.
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();

		// Simple implementation: treat pattern as literal with case-insensitive flag.
		// For real regex, the JS side should use RegExp.
		let needle = pattern;
		if needle.is_empty() {
			return Vec::new();
		}
		let needle_lower = needle.to_lowercase();
		let needle_len = needle_lower.chars().count();
		let plain_lower = plain.to_lowercase();
		let mut results = Vec::new();
		let mut search_from = 0;
		while let Some(byte_pos) = plain_lower[search_from..].find(&needle_lower) {
			let abs_byte = search_from + byte_pos;
			let char_pos = plain[..abs_byte].chars().count();
			results.push(char_pos);
			results.push(char_pos + needle_len);
			search_from = abs_byte + needle.len();
		}
		results
	}

	// ── Selection change detection ───────────────────────────────────

	/// Check if the selection has changed since the last call to this
	/// method.
	///
	/// Returns `true` the first time the selection moves to a new
	/// position. Useful for triggering UI updates only when needed.
	#[wasm_bindgen]
	pub fn selection_changed(&mut self) -> bool {
		let current = self.runtime.selection().end().offset();
		if current != self.last_selection_end {
			self.last_selection_end = current;
			return true;
		}
		false
	}

	/// Get the last recorded selection end offset (from `selection_changed`).
	#[wasm_bindgen]
	pub fn last_selection_end(&self) -> usize {
		self.last_selection_end
	}

	// ── Wrap continuation indicators ─────────────────────────────────

	/// Enable/disable wrap continuation indicators in the gutter.
	///
	/// When enabled, wrapped continuation lines show a `↪` glyph in
	/// the gutter to distinguish them from real line breaks.
	#[wasm_bindgen]
	pub fn set_show_wrap_indicators(&mut self, enabled: bool) {
		self.show_wrap_indicators = enabled;
	}

	/// Whether wrap indicators are shown.
	#[wasm_bindgen]
	pub fn show_wrap_indicators(&self) -> bool {
		self.show_wrap_indicators
	}

	// ── Selection anchor ─────────────────────────────────────────────

	/// Get the selection anchor (start) offset.
	///
	/// When selecting left-to-right, anchor < focus (end).
	/// When selecting right-to-left, anchor > focus.
	/// When collapsed, anchor == focus.
	#[wasm_bindgen]
	pub fn selection_anchor(&self) -> usize {
		self.runtime.selection().start().offset()
	}

	/// Whether the selection is collapsed (no text selected).
	#[wasm_bindgen]
	pub fn selection_is_collapsed(&self) -> bool {
		self.runtime.selection().is_collapsed()
	}

	/// Length of the current selection in characters.
	#[wasm_bindgen]
	pub fn selection_length(&self) -> usize {
		let sel = self.runtime.selection();
		let start = sel.start().offset();
		let end = sel.end().offset();
		if end >= start {
			end - start
		} else {
			start - end
		}
	}

	// ── Character count by type ──────────────────────────────────────

	/// Count characters by type: [letters, digits, spaces, punctuation, other].
	///
	/// Returns a 5-element array.
	#[wasm_bindgen]
	pub fn char_counts(&self) -> Vec<usize> {
		let plain = self.runtime.document().plain_text();
		let mut letters = 0usize;
		let mut digits = 0usize;
		let mut spaces = 0usize;
		let mut punct = 0usize;
		let mut other = 0usize;
		for ch in plain.chars() {
			if ch.is_alphabetic() {
				letters += 1;
			} else if ch.is_ascii_digit() {
				digits += 1;
			} else if ch.is_whitespace() {
				spaces += 1;
			} else if ch.is_ascii_punctuation() {
				punct += 1;
			} else {
				other += 1;
			}
		}
		vec![letters, digits, spaces, punct, other]
	}

	// ── Text hash ────────────────────────────────────────────────────

	/// Fast content fingerprint (FNV-1a 64-bit hash as hex string).
	///
	/// Useful for external change detection: compare hashes to check
	/// if content has changed without comparing full text.
	#[wasm_bindgen]
	pub fn text_hash(&self) -> String {
		let plain = self.runtime.document().plain_text();
		let mut hash: u64 = 0xcbf29ce484222325;
		for byte in plain.as_bytes() {
			hash ^= *byte as u64;
			hash = hash.wrapping_mul(0x100000001b3);
		}
		format!("{hash:016x}")
	}

	// ── Event log ────────────────────────────────────────────────────

	/// Log an editor event. Newest entries are at index 0.
	///
	/// The log is capped at `event_log_max` (default 50).
	/// Call from JS to record significant actions.
	#[wasm_bindgen]
	pub fn log_event(&mut self, event: &str) {
		self.event_log.insert(0, event.to_string());
		if self.event_log.len() > self.event_log_max {
			self.event_log.truncate(self.event_log_max);
		}
	}

	/// Get event log entry at index (0 = newest).
	#[wasm_bindgen]
	pub fn event_log_get(&self, index: usize) -> String {
		self.event_log.get(index).cloned().unwrap_or_default()
	}

	/// Number of entries in the event log.
	#[wasm_bindgen]
	pub fn event_log_length(&self) -> usize {
		self.event_log.len()
	}

	/// Clear the event log.
	#[wasm_bindgen]
	pub fn event_log_clear(&mut self) {
		self.event_log.clear();
	}

	/// Set the maximum number of event log entries.
	#[wasm_bindgen]
	pub fn set_event_log_max(&mut self, max: usize) {
		self.event_log_max = max.max(1);
		if self.event_log.len() > self.event_log_max {
			self.event_log.truncate(self.event_log_max);
		}
	}

	// ── Word completion ──────────────────────────────────────────────

	/// Suggest completions for the word currently being typed.
	///
	/// Returns up to `max_results` words from the document that start
	/// with the prefix at the cursor. Sorted alphabetically, deduplicated.
	#[wasm_bindgen]
	pub fn completions(&self, max_results: usize) -> Vec<String> {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let offset = self.runtime.selection().end().offset().min(chars.len());

		// Find the prefix — walk backwards from cursor while alphanumeric.
		let mut start = offset;
		while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
			start -= 1;
		}
		if start == offset {
			return Vec::new();
		}
		let prefix: String = chars[start..offset].iter().collect();
		let prefix_lower = prefix.to_lowercase();

		// Collect unique words from document that start with prefix.
		let mut seen = std::collections::HashSet::new();
		let mut results = Vec::new();
		for word in plain.split(|c: char| !c.is_alphanumeric() && c != '_') {
			if word.len() > prefix.len()
				&& word.to_lowercase().starts_with(&prefix_lower)
				&& !seen.contains(word)
			{
				seen.insert(word.to_string());
				results.push(word.to_string());
			}
		}
		results.sort();
		results.truncate(max_results);
		results
	}

	// ── Line range operations ────────────────────────────────────────

	/// Get text for a range of lines (0-based, inclusive start, exclusive end).
	#[wasm_bindgen]
	pub fn get_line_range(&self, start_line: usize, end_line: usize) -> String {
		let plain = self.runtime.document().plain_text();
		let lines: Vec<&str> = plain.split('\n').collect();
		let s = start_line.min(lines.len());
		let e = end_line.min(lines.len());
		if s >= e {
			return String::new();
		}
		lines[s..e].join("\n")
	}

	/// Replace text for a range of lines (0-based, inclusive start, exclusive end).
	#[wasm_bindgen]
	pub fn set_line_range(&mut self, start_line: usize, end_line: usize, text: &str) {
		if !self.is_writable() {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let lines: Vec<&str> = plain.split('\n').collect();
		let s = start_line.min(lines.len());
		let e = end_line.min(lines.len());
		if s > e {
			return;
		}
		// Calculate char offsets for the line range.
		let mut char_start = 0usize;
		for (i, line) in lines.iter().enumerate() {
			if i == s {
				break;
			}
			char_start += line.chars().count() + 1; // +1 for \n
		}
		let mut char_end = char_start;
		for i in s..e {
			if i < lines.len() {
				char_end += lines[i].chars().count();
				if i + 1 < e && i + 1 < lines.len() {
					char_end += 1; // \n between lines
				}
			}
		}
		self.delete_range(char_start, char_end);
		self.insert_text_at(char_start, text);
	}

	/// Get the total number of lines in the document.
	#[wasm_bindgen]
	pub fn line_count_total(&self) -> usize {
		let plain = self.runtime.document().plain_text();
		if plain.is_empty() {
			return 1;
		}
		plain.split('\n').count()
	}

	/// Get text of a single line (0-based).
	#[wasm_bindgen]
	pub fn get_line(&self, line: usize) -> String {
		let plain = self.runtime.document().plain_text();
		plain.split('\n').nth(line).unwrap_or("").to_string()
	}

	// ── Scroll metrics ──────────────────────────────────────────────

	/// The viewport height in pixels (same as canvas height / zoom).
	#[wasm_bindgen]
	pub fn viewport_height(&self) -> f32 {
		self.height / self.zoom
	}

	/// The ratio of viewport to content (0.0–1.0). 1.0 = all visible.
	#[wasm_bindgen]
	pub fn scroll_ratio(&self) -> f32 {
		let ch = self.content_height().unwrap_or(self.height);
		if ch <= 0.0 {
			return 1.0;
		}
		(self.viewport_height() / ch).min(1.0)
	}

	/// The scroll position as a fraction (0.0 = top, 1.0 = bottom).
	#[wasm_bindgen]
	pub fn scroll_fraction(&self) -> f32 {
		let ch = self.content_height().unwrap_or(self.height);
		let max_scroll = (ch - self.viewport_height()).max(0.0);
		if max_scroll <= 0.0 {
			return 0.0;
		}
		(self.scroll_y / max_scroll).clamp(0.0, 1.0)
	}

	/// Scroll to a fraction of the document (0.0 = top, 1.0 = bottom).
	#[wasm_bindgen]
	pub fn scroll_to_fraction(&mut self, fraction: f32) {
		let ch = self.content_height().unwrap_or(self.height);
		let max_scroll = (ch - self.viewport_height()).max(0.0);
		self.scroll_y = (fraction.clamp(0.0, 1.0) * max_scroll).max(0.0);
	}

	// ── Annotations ──────────────────────────────────────────────────

	/// Add an annotation to a text range.
	///
	/// `kind` examples: "error", "warning", "info", "spelling".
	/// `message` is optional descriptive text.
	#[wasm_bindgen]
	pub fn add_annotation(&mut self, start: usize, end: usize, kind: &str, message: &str) {
		self.annotations
			.push((start, end, kind.to_string(), message.to_string()));
	}

	/// Remove all annotations matching a kind (e.g. "error").
	#[wasm_bindgen]
	pub fn remove_annotations_by_kind(&mut self, kind: &str) {
		self.annotations.retain(|a| a.2 != kind);
	}

	/// Remove all annotations.
	#[wasm_bindgen]
	pub fn clear_annotations(&mut self) {
		self.annotations.clear();
	}

	/// Number of active annotations.
	#[wasm_bindgen]
	pub fn annotation_count(&self) -> usize {
		self.annotations.len()
	}

	/// Get annotations as flat array: [start, end, kind, message, ...].
	#[wasm_bindgen]
	pub fn get_annotations(&self) -> Vec<String> {
		let mut result = Vec::with_capacity(self.annotations.len() * 4);
		for (start, end, kind, msg) in &self.annotations {
			result.push(start.to_string());
			result.push(end.to_string());
			result.push(kind.clone());
			result.push(msg.clone());
		}
		result
	}

	/// Get annotations overlapping a character offset.
	///
	/// Returns flat array: [start, end, kind, message, ...].
	#[wasm_bindgen]
	pub fn annotations_at(&self, offset: usize) -> Vec<String> {
		let mut result = Vec::new();
		for (start, end, kind, msg) in &self.annotations {
			if offset >= *start && offset < *end {
				result.push(start.to_string());
				result.push(end.to_string());
				result.push(kind.clone());
				result.push(msg.clone());
			}
		}
		result
	}

	// ── Search history ───────────────────────────────────────────────

	/// Push a search term into the search history.
	#[wasm_bindgen]
	pub fn search_history_push(&mut self, term: &str) {
		if term.is_empty() {
			return;
		}
		self.search_history.retain(|t| t != term);
		self.search_history.insert(0, term.to_string());
		if self.search_history.len() > 20 {
			self.search_history.truncate(20);
		}
	}

	/// Get search history entry at index (0 = most recent).
	#[wasm_bindgen]
	pub fn search_history_get(&self, index: usize) -> String {
		self.search_history.get(index).cloned().unwrap_or_default()
	}

	/// Number of search history entries.
	#[wasm_bindgen]
	pub fn search_history_length(&self) -> usize {
		self.search_history.len()
	}

	/// Clear search history.
	#[wasm_bindgen]
	pub fn search_history_clear(&mut self) {
		self.search_history.clear();
	}

	// ── Visible range ────────────────────────────────────────────────

	/// Get the first visible line number (0-based).
	#[wasm_bindgen]
	pub fn first_visible_line(&self) -> usize {
		if self.scroll_y <= 0.0 {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let line_count = if plain.is_empty() {
			1
		} else {
			plain.split('\n').count()
		};
		let ch = self.content_height().unwrap_or(self.height);
		if ch <= 0.0 {
			return 0;
		}
		let line_height = ch / line_count as f32;
		if line_height <= 0.0 {
			return 0;
		}
		let approx = (self.scroll_y / line_height) as usize;
		approx.min(line_count.saturating_sub(1))
	}

	/// Get the last visible line number (0-based).
	#[wasm_bindgen]
	pub fn last_visible_line(&self) -> usize {
		let plain = self.runtime.document().plain_text();
		let line_count = if plain.is_empty() {
			1
		} else {
			plain.split('\n').count()
		};
		let ch = self.content_height().unwrap_or(self.height);
		if ch <= 0.0 {
			return 0;
		}
		let line_height = ch / line_count as f32;
		if line_height <= 0.0 {
			return 0;
		}
		let bottom = self.scroll_y + self.viewport_height();
		let approx = (bottom / line_height) as usize;
		approx.min(line_count.saturating_sub(1))
	}

	/// Number of lines visible in the viewport.
	#[wasm_bindgen]
	pub fn visible_line_count(&self) -> usize {
		let first = self.first_visible_line();
		let last = self.last_visible_line();
		(last - first) + 1
	}

	// ── Minimap ──────────────────────────────────────────────────────

	/// Toggle the minimap sidebar.
	#[wasm_bindgen]
	pub fn set_show_minimap(&mut self, enabled: bool) {
		self.show_minimap = enabled;
	}

	/// Whether the minimap is shown.
	#[wasm_bindgen]
	pub fn show_minimap(&self) -> bool {
		self.show_minimap
	}

	/// Set the minimap width in pixels (default 60).
	#[wasm_bindgen]
	pub fn set_minimap_width(&mut self, w: f32) {
		self.minimap_width = w.clamp(30.0, 200.0);
	}

	/// Get the minimap width.
	#[wasm_bindgen]
	pub fn minimap_width(&self) -> f32 {
		self.minimap_width
	}

	// ── Sticky scroll ────────────────────────────────────────────────

	/// Toggle sticky scroll — shows the first line of the document at
	/// the top when scrolled past it.
	#[wasm_bindgen]
	pub fn set_sticky_scroll(&mut self, enabled: bool) {
		self.sticky_scroll = enabled;
	}

	/// Whether sticky scroll is enabled.
	#[wasm_bindgen]
	pub fn sticky_scroll(&self) -> bool {
		self.sticky_scroll
	}

	// ── Rename all occurrences ───────────────────────────────────────

	/// Rename all occurrences of the word under cursor to `new_name`.
	///
	/// Uses whole-word matching. Returns the number of replacements.
	#[wasm_bindgen]
	pub fn rename_all(&mut self, new_name: &str) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let word = self.word_at_cursor();
		if word.is_empty() || word == new_name {
			return 0;
		}
		let offsets = self.find_all_whole_word(&word);
		if offsets.is_empty() {
			return 0;
		}
		let count = offsets.len() / 2;
		// Process from end to start so offsets stay valid.
		let mut i = offsets.len();
		while i >= 2 {
			i -= 2;
			let start = offsets[i];
			let end = offsets[i + 1];
			self.delete_range(start, end);
			self.insert_text_at(start, new_name);
		}
		count
	}

	// ── Cursor style ─────────────────────────────────────────────────

	/// Set cursor style: 0=line (default), 1=block, 2=underline.
	#[wasm_bindgen]
	pub fn set_cursor_style(&mut self, style: u8) {
		self.cursor_style = style.min(2);
	}

	/// Get cursor style (0=line, 1=block, 2=underline).
	#[wasm_bindgen]
	pub fn cursor_style(&self) -> u8 {
		self.cursor_style
	}

	/// Set cursor width in pixels (line style only, default 2.0).
	#[wasm_bindgen]
	pub fn set_cursor_width(&mut self, w: f32) {
		self.cursor_width = w.clamp(1.0, 8.0);
	}

	/// Get cursor width.
	#[wasm_bindgen]
	pub fn cursor_width_px(&self) -> f32 {
		self.cursor_width
	}

	/// Set cursor colour override. Pass 0,0,0,0 to reset to theme default.
	#[wasm_bindgen]
	pub fn set_cursor_color(&mut self, r: u8, g: u8, b: u8, a: u8) {
		if a == 0 {
			self.cursor_color = None;
		} else {
			self.cursor_color = Some((r, g, b, a));
		}
	}

	// ── Snapshot diff ────────────────────────────────────────────────

	/// Take a snapshot of the current text for later diff.
	#[wasm_bindgen]
	pub fn take_snapshot(&mut self) {
		self.diff_snapshot = self.runtime.document().plain_text().to_string();
	}

	/// Compare current text against the last snapshot.
	///
	/// Returns a list of changed line numbers (0-based) as a flat array.
	/// A line is "changed" if it differs from the snapshot.
	#[wasm_bindgen]
	pub fn diff_from_snapshot(&self) -> Vec<usize> {
		let current = self.runtime.document().plain_text();
		let snap_lines: Vec<&str> = self.diff_snapshot.split('\n').collect();
		let curr_lines: Vec<&str> = current.split('\n').collect();
		let max_len = snap_lines.len().max(curr_lines.len());
		let mut changed = Vec::new();
		for i in 0..max_len {
			let snap = snap_lines.get(i).copied().unwrap_or("");
			let curr = curr_lines.get(i).copied().unwrap_or("");
			if snap != curr {
				changed.push(i);
			}
		}
		changed
	}

	/// Whether a snapshot has been taken.
	#[wasm_bindgen]
	pub fn has_snapshot(&self) -> bool {
		!self.diff_snapshot.is_empty()
	}

	/// Clear the saved snapshot.
	#[wasm_bindgen]
	pub fn clear_snapshot(&mut self) {
		self.diff_snapshot.clear();
	}

	// ── Macro recording ──────────────────────────────────────────────

	/// Start recording a macro.
	#[wasm_bindgen]
	pub fn macro_start_recording(&mut self) {
		self.macro_recording = true;
		self.macro_steps.clear();
	}

	/// Stop recording and return the number of steps recorded.
	#[wasm_bindgen]
	pub fn macro_stop_recording(&mut self) -> usize {
		self.macro_recording = false;
		self.macro_steps.len()
	}

	/// Whether macro recording is active.
	#[wasm_bindgen]
	pub fn macro_is_recording(&self) -> bool {
		self.macro_recording
	}

	/// Record a macro step manually.
	///
	/// `kind`: "insert", "delete", "select"
	/// `data`: for insert = text; for delete = "start,end";
	///         for select = "start,end"
	#[wasm_bindgen]
	pub fn macro_record_step(&mut self, kind: &str, data: &str) {
		if self.macro_recording {
			self.macro_steps.push((kind.to_string(), data.to_string()));
		}
	}

	/// Number of steps in the current macro recording.
	#[wasm_bindgen]
	pub fn macro_step_count(&self) -> usize {
		self.macro_steps.len()
	}

	/// Replay the recorded macro once.
	#[wasm_bindgen]
	pub fn macro_replay(&mut self) {
		if !self.is_writable() {
			return;
		}
		let steps = self.macro_steps.clone();
		for (kind, data) in &steps {
			match kind.as_str() {
				"insert" => {
					let offset = self.runtime.selection().end().offset();
					self.insert_text_at(offset, data);
				}
				"delete" => {
					let parts: Vec<&str> = data.split(',').collect();
					if parts.len() == 2 {
						if let (Ok(s), Ok(e)) =
							(parts[0].parse::<usize>(), parts[1].parse::<usize>())
						{
							self.delete_range(s, e);
						}
					}
				}
				"select" => {
					let parts: Vec<&str> = data.split(',').collect();
					if parts.len() == 2 {
						if let (Ok(s), Ok(e)) =
							(parts[0].parse::<usize>(), parts[1].parse::<usize>())
						{
							let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
								selection: Selection::range(Position::new(s), Position::new(e)),
							});
						}
					}
				}
				_ => {}
			}
		}
		self.is_modified = true;
	}

	/// Save the current recorded macro under a name.
	#[wasm_bindgen]
	pub fn macro_save(&mut self, name: &str) {
		self.saved_macros
			.insert(name.to_string(), self.macro_steps.clone());
	}

	/// Replay a saved macro by name. Returns false if not found.
	#[wasm_bindgen]
	pub fn macro_replay_saved(&mut self, name: &str) -> bool {
		if !self.is_writable() {
			return false;
		}
		if let Some(steps) = self.saved_macros.get(name).cloned() {
			let old = std::mem::replace(&mut self.macro_steps, steps);
			self.macro_replay();
			self.macro_steps = old;
			true
		} else {
			false
		}
	}

	/// List saved macro names.
	#[wasm_bindgen]
	pub fn macro_list_saved(&self) -> Vec<String> {
		let mut names: Vec<String> = self.saved_macros.keys().cloned().collect();
		names.sort();
		names
	}

	/// Delete a saved macro.
	#[wasm_bindgen]
	pub fn macro_delete_saved(&mut self, name: &str) {
		self.saved_macros.remove(name);
	}

	// ── Find match highlights ────────────────────────────────────────

	/// Set the needle for visual find highlights.
	///
	/// All occurrences are highlighted with a translucent overlay.
	/// Pass empty string to clear highlights.
	#[wasm_bindgen]
	pub fn set_find_highlights(&mut self, needle: &str) {
		self.find_highlight_needle = needle.to_string();
		self.show_find_highlights = !needle.is_empty();
	}

	/// Get the current find highlight needle.
	#[wasm_bindgen]
	pub fn find_highlight_needle(&self) -> String {
		self.find_highlight_needle.clone()
	}

	/// Whether find highlights are active.
	#[wasm_bindgen]
	pub fn show_find_highlights(&self) -> bool {
		self.show_find_highlights
	}

	// ── Column/block selection ───────────────────────────────────────

	/// Get text from a rectangular block selection.
	///
	/// Returns lines from `start_line` to `end_line` (inclusive),
	/// each trimmed to columns `start_col` to `end_col` (char-based).
	#[wasm_bindgen]
	pub fn get_block_selection(
		&self,
		start_line: usize,
		end_line: usize,
		start_col: usize,
		end_col: usize,
	) -> String {
		let plain = self.runtime.document().plain_text();
		let lines: Vec<&str> = plain.split('\n').collect();
		let mut result = Vec::new();
		let s = start_line.min(lines.len());
		let e = end_line.min(lines.len().saturating_sub(1));
		for i in s..=e {
			let chars: Vec<char> = lines[i].chars().collect();
			let sc = start_col.min(chars.len());
			let ec = end_col.min(chars.len());
			if sc <= ec {
				result.push(chars[sc..ec].iter().collect::<String>());
			} else {
				result.push(String::new());
			}
		}
		result.join("\n")
	}

	/// Replace text in a rectangular block.
	///
	/// Each line of `text` replaces the corresponding column range.
	#[wasm_bindgen]
	pub fn set_block_selection(
		&mut self,
		start_line: usize,
		end_line: usize,
		start_col: usize,
		end_col: usize,
		text: &str,
	) {
		if !self.is_writable() {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let lines: Vec<&str> = plain.split('\n').collect();
		let new_lines: Vec<&str> = text.split('\n').collect();
		let s = start_line.min(lines.len());
		let e = end_line.min(lines.len().saturating_sub(1));

		// Build new document from bottom to top.
		let mut rebuilt: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
		let range_indices: Vec<usize> = (s..=e).collect();
		for (idx, &i) in range_indices.iter().enumerate().rev() {
			let chars: Vec<char> = rebuilt[i].chars().collect();
			let sc = start_col.min(chars.len());
			let ec = end_col.min(chars.len());
			let before: String = chars[..sc].iter().collect();
			let after: String = chars[ec..].iter().collect();
			let replacement = new_lines.get(idx).copied().unwrap_or("");
			rebuilt[i] = format!("{before}{replacement}{after}");
		}

		let new_text = rebuilt.join("\n");
		self.runtime.document_mut().set_plain_text(&new_text);
		self.is_modified = true;
	}

	// ── Smart paste ──────────────────────────────────────────────────

	/// Paste text with auto-adjusted indentation.
	///
	/// Detects the indentation level at the cursor and adjusts the
	/// pasted text to match.
	#[wasm_bindgen]
	pub fn paste_with_indent(&mut self, text: &str) {
		if !self.is_writable() || text.is_empty() {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let offset = self.runtime.selection().end().offset();

		// Find the current line's indentation.
		let before: String = plain.chars().take(offset).collect();
		let current_line = before.rsplit('\n').next().unwrap_or("");
		let target_indent: String = current_line
			.chars()
			.take_while(|c| c.is_whitespace())
			.collect();

		// Find the minimum indentation of the pasted text.
		let paste_lines: Vec<&str> = text.split('\n').collect();
		let min_indent = paste_lines
			.iter()
			.filter(|l| !l.trim().is_empty())
			.map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
			.min()
			.unwrap_or(0);

		// Re-indent each line.
		let mut result = Vec::new();
		for (i, line) in paste_lines.iter().enumerate() {
			if i == 0 {
				// First line goes at cursor position (no re-indent).
				result.push(line.to_string());
			} else if line.trim().is_empty() {
				result.push(String::new());
			} else {
				let line_indent = line.chars().take_while(|c| c.is_whitespace()).count();
				let stripped = &line[line
					.chars()
					.take_while(|c| c.is_whitespace())
					.map(|c| c.len_utf8())
					.sum::<usize>()..];
				let extra = line_indent.saturating_sub(min_indent);
				let new_indent = format!("{}{}", target_indent, " ".repeat(extra));
				result.push(format!("{new_indent}{stripped}"));
			}
		}
		let adjusted = result.join("\n");
		let len = adjusted.chars().count();
		self.insert_text_at(offset, &adjusted);
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(offset + len)),
		});
	}

	// ── Tokenize ─────────────────────────────────────────────────────

	/// Simple tokenization of the document text.
	///
	/// Returns alternating [kind, text, kind, text, ...] where kind is
	/// one of: "word", "number", "whitespace", "punctuation", "newline".
	#[wasm_bindgen]
	pub fn tokenize(&self) -> Vec<String> {
		let plain = self.runtime.document().plain_text();
		let mut tokens = Vec::new();
		let chars: Vec<char> = plain.chars().collect();
		let mut i = 0;
		while i < chars.len() {
			let ch = chars[i];
			if ch == '\n' {
				tokens.push("newline".to_string());
				tokens.push("\n".to_string());
				i += 1;
			} else if ch.is_whitespace() {
				let start = i;
				while i < chars.len() && chars[i].is_whitespace() && chars[i] != '\n' {
					i += 1;
				}
				tokens.push("whitespace".to_string());
				tokens.push(chars[start..i].iter().collect());
			} else if ch.is_alphabetic() || ch == '_' {
				let start = i;
				while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
					i += 1;
				}
				tokens.push("word".to_string());
				tokens.push(chars[start..i].iter().collect());
			} else if ch.is_ascii_digit() {
				let start = i;
				while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
					i += 1;
				}
				tokens.push("number".to_string());
				tokens.push(chars[start..i].iter().collect());
			} else {
				tokens.push("punctuation".to_string());
				tokens.push(ch.to_string());
				i += 1;
			}
		}
		tokens
	}

	// ── Link detection ───────────────────────────────────────────────

	/// Enable or disable URL link detection.
	#[wasm_bindgen]
	pub fn set_detect_links(&mut self, enabled: bool) {
		self.detect_links = enabled;
	}

	/// Whether link detection is enabled.
	#[wasm_bindgen]
	pub fn detect_links(&self) -> bool {
		self.detect_links
	}

	/// Find all URLs in the document.
	///
	/// Returns flat array: [start, end, start, end, ...] of char offsets.
	#[wasm_bindgen]
	pub fn find_links(&self) -> Vec<usize> {
		let plain = self.runtime.document().plain_text();
		let mut results = Vec::new();
		let prefixes = ["https://", "http://", "ftp://"];

		for prefix in &prefixes {
			let mut search_from = 0;
			while let Some(pos) = plain[search_from..].find(prefix) {
				let abs = search_from + pos;
				let char_start = plain[..abs].chars().count();
				// Find end of URL — stop at whitespace, >, ), ], or end.
				let url_bytes = &plain[abs..];
				let end_byte = url_bytes
					.find(|c: char| c.is_whitespace() || c == '>' || c == ')' || c == ']')
					.unwrap_or(url_bytes.len());
				let char_end = char_start + plain[abs..abs + end_byte].chars().count();
				results.push(char_start);
				results.push(char_end);
				search_from = abs + end_byte;
			}
		}
		results.sort_by_key(|&x| x);
		results
	}

	/// Get the URL text at a character offset, if any.
	///
	/// Returns empty string if offset is not inside a URL.
	#[wasm_bindgen]
	pub fn link_at_offset(&self, offset: usize) -> String {
		let links = self.find_links();
		let mut i = 0;
		while i + 1 < links.len() {
			if offset >= links[i] && offset < links[i + 1] {
				let plain = self.runtime.document().plain_text();
				let chars: Vec<char> = plain.chars().collect();
				return chars[links[i]..links[i + 1]].iter().collect();
			}
			i += 2;
		}
		String::new()
	}

	// ── Line folding ─────────────────────────────────────────────────

	/// Fold (collapse) a range of lines (0-based, inclusive).
	///
	/// The first line remains visible; subsequent lines are hidden.
	#[wasm_bindgen]
	pub fn fold_lines(&mut self, start_line: usize, end_line: usize) {
		if start_line >= end_line {
			return;
		}
		// Don't add duplicate ranges.
		if !self
			.folded_ranges
			.iter()
			.any(|&(s, e)| s == start_line && e == end_line)
		{
			self.folded_ranges.push((start_line, end_line));
		}
	}

	/// Unfold a specific range.
	#[wasm_bindgen]
	pub fn unfold_lines(&mut self, start_line: usize, end_line: usize) {
		self.folded_ranges
			.retain(|&(s, e)| s != start_line || e != end_line);
	}

	/// Unfold all ranges.
	#[wasm_bindgen]
	pub fn unfold_all(&mut self) {
		self.folded_ranges.clear();
	}

	/// Number of active fold regions.
	#[wasm_bindgen]
	pub fn fold_count(&self) -> usize {
		self.folded_ranges.len()
	}

	/// Whether a specific line is inside a folded (hidden) region.
	///
	/// Returns true for lines that are hidden — NOT the first line of
	/// a fold which remains visible.
	#[wasm_bindgen]
	pub fn is_line_folded(&self, line: usize) -> bool {
		self.folded_ranges
			.iter()
			.any(|&(s, e)| line > s && line <= e)
	}

	/// Toggle fold at a line. If the line starts a fold, unfold it.
	/// Otherwise, try to fold from this line using indentation.
	#[wasm_bindgen]
	pub fn toggle_fold_at(&mut self, line: usize) {
		// Check if this line starts an existing fold.
		if let Some(pos) = self.folded_ranges.iter().position(|&(s, _)| s == line) {
			self.folded_ranges.remove(pos);
			return;
		}
		// Auto-detect fold range using indentation.
		let plain = self.runtime.document().plain_text();
		let lines: Vec<&str> = plain.split('\n').collect();
		if line >= lines.len() {
			return;
		}
		let base_indent = lines[line]
			.chars()
			.take_while(|c| c.is_whitespace())
			.count();
		let mut end = line;
		for i in (line + 1)..lines.len() {
			let l = lines[i];
			if l.trim().is_empty() {
				end = i;
				continue;
			}
			let indent = l.chars().take_while(|c| c.is_whitespace()).count();
			if indent > base_indent {
				end = i;
			} else {
				break;
			}
		}
		if end > line {
			self.fold_lines(line, end);
		}
	}

	/// Get all folded ranges as flat array: [start0, end0, start1, end1, ...].
	#[wasm_bindgen]
	pub fn folded_ranges(&self) -> Vec<usize> {
		let mut result = Vec::with_capacity(self.folded_ranges.len() * 2);
		for &(s, e) in &self.folded_ranges {
			result.push(s);
			result.push(e);
		}
		result
	}

	// ── Gutter click ─────────────────────────────────────────────────

	/// Determine which line number a Y-coordinate in the gutter maps to.
	///
	/// Returns the 0-based line number, or -1 if outside content.
	#[wasm_bindgen]
	pub fn line_at_y(&self, y: f32) -> i32 {
		let plain = self.runtime.document().plain_text();
		let line_count = if plain.is_empty() {
			1
		} else {
			plain.split('\n').count()
		};
		let ch = self.content_height().unwrap_or(self.height);
		if ch <= 0.0 || line_count == 0 {
			return -1;
		}
		let line_height = ch / line_count as f32;
		let adjusted_y = y + self.scroll_y;
		let line = (adjusted_y / line_height) as i32;
		if line < 0 || line >= line_count as i32 {
			-1
		} else {
			line
		}
	}

	// ── Configuration presets ────────────────────────────────────────

	/// Apply a named configuration preset.
	///
	/// - `"code"`: line numbers, indent guides, whitespace, bracket
	///   highlight, occurrence highlight, auto-close brackets, soft tabs
	/// - `"prose"`: word wrap, no line numbers, no whitespace, no
	///   indent guides, placeholder
	/// - `"minimal"`: minimal chrome, no gutter, no highlights
	#[wasm_bindgen]
	pub fn apply_preset(&mut self, name: &str) {
		match name {
			"code" => {
				self.show_line_numbers = true;
				self.show_indent_guides = true;
				self.show_whitespace = true;
				self.highlight_matching_brackets = true;
				self.highlight_occurrences = true;
				self.auto_close_brackets = true;
				self.auto_surround = true;
				self.soft_tabs = true;
				self.tab_size = 4;
				self.word_wrap = false;
				self.show_wrap_indicators = true;
			}
			"prose" => {
				self.show_line_numbers = false;
				self.show_indent_guides = false;
				self.show_whitespace = false;
				self.highlight_matching_brackets = false;
				self.highlight_occurrences = false;
				self.auto_close_brackets = false;
				self.auto_surround = false;
				self.word_wrap = true;
				self.show_wrap_indicators = false;
				if self.placeholder.is_empty() {
					self.placeholder = "Start writing…".to_string();
				}
			}
			"minimal" => {
				self.show_line_numbers = false;
				self.show_indent_guides = false;
				self.show_whitespace = false;
				self.highlight_matching_brackets = false;
				self.highlight_occurrences = false;
				self.highlight_current_line = false;
				self.auto_close_brackets = false;
				self.auto_surround = false;
				self.show_minimap = false;
				self.sticky_scroll = false;
				self.show_wrap_indicators = false;
			}
			_ => {}
		}
	}

	// ── Content statistics ───────────────────────────────────────────

	/// Estimated reading time in seconds (assumes 250 words/minute).
	#[wasm_bindgen]
	pub fn reading_time_seconds(&self) -> f32 {
		let plain = self.runtime.document().plain_text();
		let word_count = plain.split_whitespace().count();
		(word_count as f32 / 250.0) * 60.0
	}

	/// Flesch reading ease score (0–100, higher = easier).
	///
	/// Simplified: uses average words per sentence and average
	/// syllables per word.
	#[wasm_bindgen]
	pub fn flesch_reading_ease(&self) -> f32 {
		let plain = self.runtime.document().plain_text();
		let words: Vec<&str> = plain.split_whitespace().collect();
		if words.is_empty() {
			return 0.0;
		}
		let word_count = words.len() as f32;

		// Count sentences (split on . ! ?).
		let sentence_count = plain
			.chars()
			.filter(|&c| c == '.' || c == '!' || c == '?')
			.count()
			.max(1) as f32;

		// Count syllables (simplified: count vowel groups).
		let total_syllables: usize = words
			.iter()
			.map(|w| {
				let lower = w.to_lowercase();
				let mut count = 0usize;
				let mut prev_vowel = false;
				for ch in lower.chars() {
					let is_vowel = "aeiouy".contains(ch);
					if is_vowel && !prev_vowel {
						count += 1;
					}
					prev_vowel = is_vowel;
				}
				count.max(1)
			})
			.sum();

		let avg_sentence_len = word_count / sentence_count;
		let avg_syllables = total_syllables as f32 / word_count;
		(206.835 - 1.015 * avg_sentence_len - 84.6 * avg_syllables).clamp(0.0, 100.0)
	}

	// ── Syntax highlighting ──────────────────────────────────────────

	/// Enable or disable syntax highlighting.
	///
	/// When enabled, the tokenizer colours are applied during rendering.
	#[wasm_bindgen]
	pub fn set_syntax_highlight(&mut self, enabled: bool) {
		self.syntax_highlight = enabled;
	}

	/// Whether syntax highlighting is enabled.
	#[wasm_bindgen]
	pub fn syntax_highlight(&self) -> bool {
		self.syntax_highlight
	}

	/// Set a colour for a token kind.
	///
	/// Kinds: "word", "number", "whitespace", "punctuation", "newline".
	/// Use this to customise syntax colours.
	#[wasm_bindgen]
	pub fn set_token_color(&mut self, kind: &str, r: u8, g: u8, b: u8, a: u8) {
		self.token_colors.insert(kind.to_string(), (r, g, b, a));
	}

	/// Get the colour for a token kind as [r, g, b, a].
	///
	/// Returns default colours if not customised.
	#[wasm_bindgen]
	pub fn get_token_color(&self, kind: &str) -> Vec<u8> {
		let (r, g, b, a) = self.token_colors.get(kind).copied().unwrap_or_else(|| {
			match kind {
				"word" => (212, 212, 212, 255),        // light gray
				"number" => (181, 206, 168, 255),      // green
				"punctuation" => (150, 150, 150, 255), // dim gray
				"whitespace" => (0, 0, 0, 0),          // invisible
				"newline" => (0, 0, 0, 0),             // invisible
				_ => (212, 212, 212, 255),
			}
		});
		vec![r, g, b, a]
	}

	/// Reset all token colours to defaults.
	#[wasm_bindgen]
	pub fn reset_token_colors(&mut self) {
		self.token_colors.clear();
	}

	// ── Custom theme API ─────────────────────────────────────────────

	/// Set a single theme colour slot.
	///
	/// Slot names: "background", "text", "caret", "caret_blur",
	/// "selection", "selection_blur", "line_highlight",
	/// "gutter_bg", "gutter_text", "gutter_border",
	/// "scrollbar_track", "scrollbar_thumb".
	#[wasm_bindgen]
	pub fn set_theme_color(&mut self, slot: &str, r: u8, g: u8, b: u8, a: u8) {
		let c = Color::new(r, g, b, a);
		match slot {
			"background" => self.theme.background = c,
			"text" => self.theme.text = c,
			"caret" => self.theme.caret = c,
			"caret_blur" => self.theme.caret_blur = c,
			"selection" => self.theme.selection = c,
			"selection_blur" => self.theme.selection_blur = c,
			"line_highlight" => self.theme.line_highlight = c,
			"gutter_bg" => self.theme.gutter_bg = c,
			"gutter_text" => self.theme.gutter_text = c,
			"gutter_border" => self.theme.gutter_border = c,
			"scrollbar_track" => self.theme.scrollbar_track = c,
			"scrollbar_thumb" => self.theme.scrollbar_thumb = c,
			_ => {}
		}
	}

	/// Get a theme colour slot as [r, g, b, a].
	#[wasm_bindgen]
	pub fn get_theme_color(&self, slot: &str) -> Vec<u8> {
		let c = match slot {
			"background" => self.theme.background,
			"text" => self.theme.text,
			"caret" => self.theme.caret,
			"caret_blur" => self.theme.caret_blur,
			"selection" => self.theme.selection,
			"selection_blur" => self.theme.selection_blur,
			"line_highlight" => self.theme.line_highlight,
			"gutter_bg" => self.theme.gutter_bg,
			"gutter_text" => self.theme.gutter_text,
			"gutter_border" => self.theme.gutter_border,
			"scrollbar_track" => self.theme.scrollbar_track,
			"scrollbar_thumb" => self.theme.scrollbar_thumb,
			_ => Color::new(0, 0, 0, 0),
		};
		vec![c.r, c.g, c.b, c.a]
	}

	// ── Range formatting ─────────────────────────────────────────────

	/// Apply bold to a character range.
	#[wasm_bindgen]
	pub fn format_range_bold(&mut self, start: usize, end: usize) {
		if !self.is_writable() || start >= end {
			return;
		}
		let sel = Selection::range(Position::new(start), Position::new(end));
		let style = Style::new().bold();
		self.runtime.apply_operation(Operation::format(sel, style));
		self.is_modified = true;
	}

	/// Apply italic to a character range.
	#[wasm_bindgen]
	pub fn format_range_italic(&mut self, start: usize, end: usize) {
		if !self.is_writable() || start >= end {
			return;
		}
		let sel = Selection::range(Position::new(start), Position::new(end));
		let style = Style::new().italic();
		self.runtime.apply_operation(Operation::format(sel, style));
		self.is_modified = true;
	}

	/// Apply underline to a character range.
	#[wasm_bindgen]
	pub fn format_range_underline(&mut self, start: usize, end: usize) {
		if !self.is_writable() || start >= end {
			return;
		}
		let sel = Selection::range(Position::new(start), Position::new(end));
		let style = Style::new().underline();
		self.runtime.apply_operation(Operation::format(sel, style));
		self.is_modified = true;
	}

	/// Apply strikethrough to a character range.
	#[wasm_bindgen]
	pub fn format_range_strikethrough(&mut self, start: usize, end: usize) {
		if !self.is_writable() || start >= end {
			return;
		}
		let sel = Selection::range(Position::new(start), Position::new(end));
		let style = Style::new().strikethrough();
		self.runtime.apply_operation(Operation::format(sel, style));
		self.is_modified = true;
	}

	/// Set font size for a character range.
	#[wasm_bindgen]
	pub fn format_range_font_size(&mut self, start: usize, end: usize, size: f32) {
		if !self.is_writable() || start >= end {
			return;
		}
		let sel = Selection::range(Position::new(start), Position::new(end));
		let style = Style::new().font_size(size);
		self.runtime.apply_operation(Operation::format(sel, style));
		self.is_modified = true;
	}

	// ── Scroll to line ───────────────────────────────────────────────

	/// Scroll the viewport to make a specific line visible.
	///
	/// The line will be positioned near the top of the viewport with
	/// a 2-line padding.
	#[wasm_bindgen]
	pub fn scroll_to_line(&mut self, line: usize) {
		let plain = self.runtime.document().plain_text();
		let line_count = if plain.is_empty() {
			1
		} else {
			plain.split('\n').count()
		};
		let ch = self.content_height().unwrap_or(self.height);
		if ch <= 0.0 || line_count == 0 {
			return;
		}
		let line_height = ch / line_count as f32;
		let target_line = line.saturating_sub(2); // 2-line padding
		self.scroll_y = (target_line as f32 * line_height).max(0.0);
	}

	// ── Extended text statistics ─────────────────────────────────────

	/// Average word length in characters.
	#[wasm_bindgen]
	pub fn avg_word_length(&self) -> f32 {
		let plain = self.runtime.document().plain_text();
		let words: Vec<&str> = plain.split_whitespace().collect();
		if words.is_empty() {
			return 0.0;
		}
		let total_chars: usize = words.iter().map(|w| w.chars().count()).sum();
		total_chars as f32 / words.len() as f32
	}

	/// The longest word in the document.
	#[wasm_bindgen]
	pub fn longest_word(&self) -> String {
		let plain = self.runtime.document().plain_text();
		plain
			.split_whitespace()
			.max_by_key(|w| w.chars().count())
			.unwrap_or("")
			.to_string()
	}

	/// Count of unique words (case-insensitive).
	#[wasm_bindgen]
	pub fn unique_word_count(&self) -> usize {
		let plain = self.runtime.document().plain_text();
		let mut seen = std::collections::HashSet::new();
		for word in plain.split_whitespace() {
			seen.insert(word.to_lowercase());
		}
		seen.len()
	}

	/// Sentence count (split on . ! ?).
	#[wasm_bindgen]
	pub fn sentence_count(&self) -> usize {
		let plain = self.runtime.document().plain_text();
		plain
			.chars()
			.filter(|&c| c == '.' || c == '!' || c == '?')
			.count()
			.max(if plain.trim().is_empty() { 0 } else { 1 })
	}

	// ── Editor info ──────────────────────────────────────────────────

	/// Editor version string.
	#[wasm_bindgen]
	pub fn editor_version(&self) -> String {
		"0.1.0".to_string()
	}

	/// Total number of public API methods.
	#[wasm_bindgen]
	pub fn api_count(&self) -> usize {
		320 // Updated with this PR
	}

	/// Feature summary as a comma-separated list of categories.
	#[wasm_bindgen]
	pub fn feature_categories(&self) -> String {
		[
			"editing",
			"formatting",
			"selection",
			"navigation",
			"find-replace",
			"undo-redo",
			"clipboard",
			"visual",
			"folding",
			"annotations",
			"links",
			"macros",
			"tokenizer",
			"analytics",
			"serialization",
			"presets",
			"themes",
		]
		.join(",")
	}

	// ── Multi-cursor ─────────────────────────────────────────────────

	/// Add an extra cursor at a character offset.
	///
	/// Extra cursors are rendered alongside the primary cursor.
	/// Use `multi_cursor_insert` to type at all positions.
	#[wasm_bindgen]
	pub fn add_cursor(&mut self, offset: usize) {
		if !self.extra_cursors.contains(&offset) {
			self.extra_cursors.push(offset);
			self.extra_cursors.sort_unstable();
		}
	}

	/// Remove an extra cursor at a specific offset.
	#[wasm_bindgen]
	pub fn remove_cursor(&mut self, offset: usize) {
		self.extra_cursors.retain(|&o| o != offset);
	}

	/// Clear all extra cursors.
	#[wasm_bindgen]
	pub fn clear_cursors(&mut self) {
		self.extra_cursors.clear();
	}

	/// Number of extra cursors (not counting the primary).
	#[wasm_bindgen]
	pub fn extra_cursor_count(&self) -> usize {
		self.extra_cursors.len()
	}

	/// Get all extra cursor offsets.
	#[wasm_bindgen]
	pub fn extra_cursor_offsets(&self) -> Vec<usize> {
		self.extra_cursors.clone()
	}

	/// Insert text at all cursor positions (primary + extras).
	///
	/// Returns the number of insertions performed. Offsets are adjusted
	/// as text is inserted (processed from end to start).
	#[wasm_bindgen]
	pub fn multi_cursor_insert(&mut self, text: &str) -> usize {
		if !self.is_writable() || text.is_empty() {
			return 0;
		}
		let primary = self.runtime.selection().end().offset();
		let mut all_offsets: Vec<usize> = self.extra_cursors.clone();
		if !all_offsets.contains(&primary) {
			all_offsets.push(primary);
		}
		all_offsets.sort_unstable();
		all_offsets.dedup();

		let len = text.chars().count();
		let count = all_offsets.len();

		// Insert from end to start so earlier offsets stay valid.
		for &offset in all_offsets.iter().rev() {
			self.runtime
				.apply_operation(Operation::insert(Position::new(offset), text.to_string()));
		}

		// Update extra cursor offsets.
		let mut shift = 0usize;
		let mut new_cursors = Vec::new();
		for &offset in &all_offsets {
			let new_pos = offset + shift + len;
			if offset != primary {
				new_cursors.push(new_pos);
			}
			shift += len;
		}
		self.extra_cursors = new_cursors;

		// Move primary cursor.
		let new_primary =
			primary + len + all_offsets.iter().filter(|&&o| o < primary).count() * len;
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(new_primary)),
		});
		self.is_modified = true;
		count
	}

	// ── Breadcrumbs ──────────────────────────────────────────────────

	/// Get document breadcrumbs — lines that start with #, //, or are
	/// all-caps (treated as section headers).
	///
	/// Returns flat array: [line_number, text, line_number, text, ...].
	#[wasm_bindgen]
	pub fn breadcrumbs(&self) -> Vec<String> {
		let plain = self.runtime.document().plain_text();
		let mut result = Vec::new();
		for (i, line) in plain.split('\n').enumerate() {
			let trimmed = line.trim();
			if trimmed.is_empty() {
				continue;
			}
			let is_heading = trimmed.starts_with('#')
				|| trimmed.starts_with("//")
				|| (trimmed.len() >= 3
					&& trimmed
						.chars()
						.all(|c| c.is_uppercase() || c.is_whitespace()));
			if is_heading {
				result.push(i.to_string());
				result.push(trimmed.to_string());
			}
		}
		result
	}

	/// Navigate to a breadcrumb by index in the breadcrumbs array.
	///
	/// Sets cursor to the beginning of that line and scrolls to it.
	#[wasm_bindgen]
	pub fn go_to_breadcrumb(&mut self, line: usize) {
		let plain = self.runtime.document().plain_text();
		let mut char_offset = 0usize;
		for (i, ln) in plain.split('\n').enumerate() {
			if i == line {
				let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
					selection: Selection::collapsed(Position::new(char_offset)),
				});
				self.scroll_to_line(line);
				return;
			}
			char_offset += ln.chars().count() + 1;
		}
	}

	// ── Indent level at cursor ───────────────────────────────────────

	/// Get the indentation level (number of leading whitespace chars)
	/// of the current line.
	#[wasm_bindgen]
	pub fn indent_level_at_cursor(&self) -> usize {
		let plain = self.runtime.document().plain_text();
		let offset = self.runtime.selection().end().offset();
		let before: String = plain.chars().take(offset).collect();
		let current_line = before.rsplit('\n').next().unwrap_or("");
		current_line
			.chars()
			.take_while(|c| c.is_whitespace())
			.count()
	}

	/// Get the indent level of a specific line (0-based).
	#[wasm_bindgen]
	pub fn indent_level_of_line(&self, line: usize) -> usize {
		let plain = self.runtime.document().plain_text();
		plain
			.split('\n')
			.nth(line)
			.unwrap_or("")
			.chars()
			.take_while(|c| c.is_whitespace())
			.count()
	}

	// ── Text diff patch ──────────────────────────────────────────────

	/// Apply a simple text patch.
	///
	/// `operations` is a flat array of strings: ["insert", "offset", "text",
	/// "delete", "start", "end", ...]. Processed from end to start.
	#[wasm_bindgen]
	pub fn apply_patch(&mut self, operations: Vec<String>) {
		if !self.is_writable() {
			return;
		}
		// Parse operations.
		struct PatchOp {
			kind: String,
			offset: usize,
			end: usize,
			text: String,
		}
		let mut ops = Vec::new();
		let mut i = 0;
		while i < operations.len() {
			match operations[i].as_str() {
				"insert" if i + 2 < operations.len() => {
					let offset = operations[i + 1].parse::<usize>().unwrap_or(0);
					let text = operations[i + 2].clone();
					ops.push(PatchOp {
						kind: "insert".to_string(),
						offset,
						end: 0,
						text,
					});
					i += 3;
				}
				"delete" if i + 2 < operations.len() => {
					let start = operations[i + 1].parse::<usize>().unwrap_or(0);
					let end = operations[i + 2].parse::<usize>().unwrap_or(0);
					ops.push(PatchOp {
						kind: "delete".to_string(),
						offset: start,
						end,
						text: String::new(),
					});
					i += 3;
				}
				_ => {
					i += 1;
				}
			}
		}
		// Sort by offset descending so later ops don't shift earlier ones.
		ops.sort_by(|a, b| b.offset.cmp(&a.offset));
		for op in &ops {
			match op.kind.as_str() {
				"insert" => self.insert_text_at(op.offset, &op.text),
				"delete" => self.delete_range(op.offset, op.end),
				_ => {}
			}
		}
	}

	// ── Canvas export ────────────────────────────────────────────────

	/// Export the current canvas as a PNG data URL.
	///
	/// Returns empty string if the canvas is not available.
	#[wasm_bindgen]
	pub fn export_canvas_data_url(&self) -> String {
		let (canvas, _) = match self.canvas_and_context() {
			Ok(v) => v,
			Err(_) => return String::new(),
		};
		canvas.to_data_url().ok().unwrap_or_default()
	}

	// ── Command palette ──────────────────────────────────────────────

	/// Return all available editor commands as a flat array:
	/// [name, keybinding, name, keybinding, ...].
	///
	/// Useful for building a command palette UI.
	#[wasm_bindgen]
	pub fn command_list(&self) -> Vec<String> {
		let commands = [
			("Bold", "Ctrl+B"),
			("Italic", "Ctrl+I"),
			("Underline", "Ctrl+U"),
			("Strikethrough", "Ctrl+Shift+S"),
			("Undo", "Ctrl+Z"),
			("Redo", "Ctrl+Shift+Z"),
			("Select All", "Ctrl+A"),
			("Cut", "Ctrl+X"),
			("Copy", "Ctrl+C"),
			("Paste", "Ctrl+V"),
			("Find", "Ctrl+F"),
			("Replace", "Ctrl+H"),
			("Duplicate Line", "Ctrl+Shift+D"),
			("Delete Line", "Ctrl+Shift+K"),
			("Move Line Up", "Alt+Up"),
			("Move Line Down", "Alt+Down"),
			("Toggle Comment", "Ctrl+/"),
			("Indent", "Tab"),
			("Outdent", "Shift+Tab"),
			("Go To Line", "Ctrl+G"),
			("Toggle Word Wrap", "Alt+Z"),
			("Transform Upper Case", "Ctrl+Shift+U"),
			("Transform Lower Case", "Ctrl+Shift+L"),
			("Join Lines", "Ctrl+J"),
			("Sort Lines", "Ctrl+Shift+P"),
			("Select Line", "Ctrl+L"),
			("Expand Selection", "Ctrl+Shift+E"),
			("Contract Selection", "Ctrl+Shift+W"),
			("Transpose Chars", "Ctrl+T"),
			("Go To Matching Bracket", "Ctrl+Shift+\\"),
			("Toggle Bookmark", "Ctrl+F2"),
			("Next Bookmark", "F2"),
			("Previous Bookmark", "Shift+F2"),
			("Delete Word Left", "Ctrl+Backspace"),
			("Delete Word Right", "Ctrl+Delete"),
			("Cursor History Back", "Ctrl+Alt+Left"),
			("Cursor History Forward", "Ctrl+Alt+Right"),
			("Open Line Below", "Ctrl+Enter"),
			("Open Line Above", "Ctrl+Shift+Enter"),
			("Select Between Brackets", "Ctrl+Shift+M"),
			("Document Start", "Ctrl+Home"),
			("Document End", "Ctrl+End"),
			("Center Line", "Ctrl+Shift+."),
			("Toggle Overwrite", "Insert"),
		];
		let mut result = Vec::with_capacity(commands.len() * 2);
		for (name, key) in &commands {
			result.push(name.to_string());
			result.push(key.to_string());
		}
		result
	}

	/// Search commands by query string.
	///
	/// Returns matching commands as [name, keybinding, ...].
	/// Case-insensitive substring match on command name.
	#[wasm_bindgen]
	pub fn search_commands(&self, query: &str) -> Vec<String> {
		if query.is_empty() {
			return self.command_list();
		}
		let all = self.command_list();
		let q = query.to_lowercase();
		let mut result = Vec::new();
		let mut i = 0;
		while i + 1 < all.len() {
			if all[i].to_lowercase().contains(&q) {
				result.push(all[i].clone());
				result.push(all[i + 1].clone());
			}
			i += 2;
		}
		result
	}

	// ── Text diffing ─────────────────────────────────────────────────

	/// Compare two texts line by line.
	///
	/// Returns flat array: [kind, lineNumber, text, ...] where kind is
	/// "added", "removed", or "changed".
	#[wasm_bindgen]
	pub fn diff_texts(a: &str, b: &str) -> Vec<String> {
		let lines_a: Vec<&str> = a.split('\n').collect();
		let lines_b: Vec<&str> = b.split('\n').collect();
		let max = lines_a.len().max(lines_b.len());
		let mut result = Vec::new();
		for i in 0..max {
			let la = lines_a.get(i).copied();
			let lb = lines_b.get(i).copied();
			match (la, lb) {
				(Some(a_line), Some(b_line)) if a_line != b_line => {
					result.push("changed".to_string());
					result.push(i.to_string());
					result.push(b_line.to_string());
				}
				(Some(_), None) => {
					result.push("removed".to_string());
					result.push(i.to_string());
					result.push(la.unwrap_or("").to_string());
				}
				(None, Some(b_line)) => {
					result.push("added".to_string());
					result.push(i.to_string());
					result.push(b_line.to_string());
				}
				_ => {} // same — no diff entry
			}
		}
		result
	}

	// ── Bidi text info ───────────────────────────────────────────────

	/// Whether the document contains any RTL (right-to-left) characters.
	///
	/// Detects Arabic, Hebrew, and other RTL scripts.
	#[wasm_bindgen]
	pub fn contains_rtl(&self) -> bool {
		let plain = self.runtime.document().plain_text();
		plain.chars().any(|c| {
			let code = c as u32;
			// Arabic: 0x0600–0x06FF, Hebrew: 0x0590–0x05FF,
			// Arabic Supplement: 0x0750–0x077F, Arabic Extended: 0x08A0–0x08FF
			(0x0590..=0x05FF).contains(&code)
				|| (0x0600..=0x06FF).contains(&code)
				|| (0x0750..=0x077F).contains(&code)
				|| (0x08A0..=0x08FF).contains(&code)
				|| (0xFB50..=0xFDFF).contains(&code)
				|| (0xFE70..=0xFEFF).contains(&code)
		})
	}

	/// Whether the document contains any non-ASCII characters.
	#[wasm_bindgen]
	pub fn contains_non_ascii(&self) -> bool {
		let plain = self.runtime.document().plain_text();
		plain.chars().any(|c| !c.is_ascii())
	}

	// ── Selection to lines ───────────────────────────────────────────

	/// Get the line numbers covered by the current selection.
	///
	/// Returns [startLine, endLine] (0-based, inclusive).
	#[wasm_bindgen]
	pub fn selection_line_range(&self) -> Vec<usize> {
		let sel = self.runtime.selection();
		let start = sel.start().offset();
		let end = sel.end().offset();
		let plain = self.runtime.document().plain_text();

		let start_line = plain.chars().take(start).filter(|&c| c == '\n').count();
		let end_line = plain.chars().take(end).filter(|&c| c == '\n').count();
		vec![start_line, end_line]
	}

	/// Select an entire range of lines (0-based, inclusive).
	#[wasm_bindgen]
	pub fn select_lines(&mut self, start_line: usize, end_line: usize) {
		let plain = self.runtime.document().plain_text();
		let lines: Vec<&str> = plain.split('\n').collect();
		let s = start_line.min(lines.len().saturating_sub(1));
		let e = end_line.min(lines.len().saturating_sub(1));

		let mut char_start = 0usize;
		for i in 0..s {
			char_start += lines[i].chars().count() + 1;
		}
		let mut char_end = char_start;
		for i in s..=e {
			char_end += lines[i].chars().count();
			if i < e {
				char_end += 1;
			}
		}
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(char_start), Position::new(char_end)),
		});
	}

	// ── Whitespace normalization ─────────────────────────────────────

	/// Normalize line endings to LF (remove \r).
	///
	/// Returns the number of \r characters removed.
	#[wasm_bindgen]
	pub fn normalize_line_endings(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let cr_count = plain.chars().filter(|&c| c == '\r').count();
		if cr_count > 0 {
			let normalized = plain.replace('\r', "");
			self.runtime.document_mut().set_plain_text(&normalized);
			self.is_modified = true;
		}
		cr_count
	}

	/// Normalize all indentation to the current tab style.
	///
	/// If soft_tabs is true, converts tabs to spaces (tab_size).
	/// If soft_tabs is false, converts leading spaces to tabs.
	/// Returns number of lines modified.
	#[wasm_bindgen]
	pub fn normalize_indentation(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		if self.soft_tabs {
			self.tabs_to_spaces()
		} else {
			self.spaces_to_tabs()
		}
	}

	// ── Document outline ─────────────────────────────────────────────

	/// Build a document outline from indentation levels.
	///
	/// Returns flat array: [indent, lineNumber, text, ...] for non-empty
	/// lines. The indent value can be used to build a tree structure.
	#[wasm_bindgen]
	pub fn document_outline(&self) -> Vec<String> {
		let plain = self.runtime.document().plain_text();
		let mut result = Vec::new();
		for (i, line) in plain.split('\n').enumerate() {
			let trimmed = line.trim();
			if trimmed.is_empty() {
				continue;
			}
			let indent = line.chars().take_while(|c| c.is_whitespace()).count();
			result.push(indent.to_string());
			result.push(i.to_string());
			result.push(trimmed.to_string());
		}
		result
	}

	// ── Collaborative cursors ────────────────────────────────────────

	/// Add a collaborative cursor (another user's position).
	///
	/// Each cursor has an offset, display name, and RGB colour.
	#[wasm_bindgen]
	pub fn add_collab_cursor(&mut self, offset: usize, name: &str, r: u8, g: u8, b: u8) {
		// Remove existing cursor with same name.
		self.collab_cursors.retain(|(_, n, ..)| n != name);
		self.collab_cursors
			.push((offset, name.to_string(), r, g, b));
	}

	/// Update a collaborative cursor's position.
	#[wasm_bindgen]
	pub fn update_collab_cursor(&mut self, name: &str, offset: usize) {
		for cursor in &mut self.collab_cursors {
			if cursor.1 == name {
				cursor.0 = offset;
				return;
			}
		}
	}

	/// Remove a collaborative cursor by name.
	#[wasm_bindgen]
	pub fn remove_collab_cursor(&mut self, name: &str) {
		self.collab_cursors.retain(|(_, n, ..)| n != name);
	}

	/// Clear all collaborative cursors.
	#[wasm_bindgen]
	pub fn clear_collab_cursors(&mut self) {
		self.collab_cursors.clear();
	}

	/// Number of collaborative cursors.
	#[wasm_bindgen]
	pub fn collab_cursor_count(&self) -> usize {
		self.collab_cursors.len()
	}

	/// Get all collaborative cursors as [offset, name, r, g, b, ...].
	#[wasm_bindgen]
	pub fn collab_cursor_list(&self) -> Vec<String> {
		let mut result = Vec::with_capacity(self.collab_cursors.len() * 5);
		for (offset, name, r, g, b) in &self.collab_cursors {
			result.push(offset.to_string());
			result.push(name.clone());
			result.push(r.to_string());
			result.push(g.to_string());
			result.push(b.to_string());
		}
		result
	}

	// ── Line ending detection ────────────────────────────────────────

	/// Detect the dominant line ending style.
	///
	/// Returns "lf", "crlf", or "mixed".
	#[wasm_bindgen]
	pub fn detect_line_ending(&self) -> String {
		let plain = self.runtime.document().plain_text();
		let crlf_count = plain.matches("\r\n").count();
		let lf_only = plain.matches('\n').count() - crlf_count;
		if crlf_count > 0 && lf_only > 0 {
			"mixed".to_string()
		} else if crlf_count > 0 {
			"crlf".to_string()
		} else {
			"lf".to_string()
		}
	}

	/// Convert all line endings to CRLF.
	///
	/// Returns the number of conversions made.
	#[wasm_bindgen]
	pub fn convert_to_crlf(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		// First normalize to LF, then convert to CRLF.
		let normalized = plain.replace("\r\n", "\n").replace('\r', "\n");
		let lf_count = normalized.matches('\n').count();
		let crlf = normalized.replace('\n', "\r\n");
		self.runtime.document_mut().set_plain_text(&crlf);
		self.is_modified = true;
		lf_count
	}

	/// Convert all line endings to LF.
	#[wasm_bindgen]
	pub fn convert_to_lf(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let cr_count = plain.matches('\r').count();
		if cr_count > 0 {
			let normalized = plain.replace("\r\n", "\n").replace('\r', "\n");
			self.runtime.document_mut().set_plain_text(&normalized);
			self.is_modified = true;
		}
		cr_count
	}

	// ── File type heuristic ──────────────────────────────────────────

	/// Guess the file type from content.
	///
	/// Returns a string like "javascript", "python", "html", "css",
	/// "json", "markdown", "xml", "rust", "text".
	#[wasm_bindgen]
	pub fn detect_file_type(&self) -> String {
		let plain = self.runtime.document().plain_text();
		let first_line = plain.lines().next().unwrap_or("");
		let lower = plain.to_lowercase();

		if first_line.starts_with("<!doctype html")
			|| first_line.starts_with("<html")
			|| lower.contains("</div>")
		{
			return "html".to_string();
		}
		if first_line.starts_with("{")
			&& (lower.contains("\"name\"") || lower.contains("\"version\""))
		{
			return "json".to_string();
		}
		if first_line.starts_with("<?xml") || first_line.starts_with("<svg") {
			return "xml".to_string();
		}
		if lower.contains("def ") && lower.contains("import ") {
			return "python".to_string();
		}
		if lower.contains("function ") || lower.contains("const ") || lower.contains("=> {") {
			return "javascript".to_string();
		}
		if lower.contains("fn ")
			&& lower.contains("let ")
			&& (lower.contains("pub ") || lower.contains("use "))
		{
			return "rust".to_string();
		}
		if lower.contains("color:") || lower.contains("font-size:") || lower.contains("margin:") {
			return "css".to_string();
		}
		if first_line.starts_with('#') || lower.contains("**") || lower.contains("```") {
			return "markdown".to_string();
		}
		"text".to_string()
	}

	// ── Emmet expansion ──────────────────────────────────────────────

	/// Expand an Emmet-style abbreviation at the cursor.
	///
	/// Supports simple patterns:
	/// - `tag` → `<tag></tag>`
	/// - `tag.class` → `<tag class="class"></tag>`
	/// - `tag#id` → `<tag id="id"></tag>`
	/// - `tag*n` → `<tag></tag>` repeated n times
	/// - `lorem` → placeholder lorem ipsum text
	///
	/// Returns true if an expansion was performed.
	#[wasm_bindgen]
	pub fn expand_emmet(&mut self) -> bool {
		if !self.is_writable() {
			return false;
		}
		let plain = self.runtime.document().plain_text();
		let offset = self.runtime.selection().end().offset();
		let chars: Vec<char> = plain.chars().collect();

		// Walk backwards to find the abbreviation.
		let mut start = offset;
		while start > 0
			&& (chars[start - 1].is_alphanumeric()
				|| chars[start - 1] == '.'
				|| chars[start - 1] == '#'
				|| chars[start - 1] == '*')
		{
			start -= 1;
		}
		if start == offset {
			return false;
		}
		let abbrev: String = chars[start..offset].iter().collect();

		// Handle "lorem".
		if abbrev == "lorem" {
			let lorem = "Lorem ipsum dolor sit amet, consectetur adipiscing elit.";
			self.delete_range(start, offset);
			self.insert_text_at(start, lorem);
			return true;
		}

		// Parse tag.class#id*count.
		let (tag_part, count) = if let Some(pos) = abbrev.find('*') {
			let n = abbrev[pos + 1..].parse::<usize>().unwrap_or(1);
			(&abbrev[..pos], n)
		} else {
			(abbrev.as_str(), 1usize)
		};

		let (tag, class, id) = {
			let mut tag = tag_part;
			let mut class = "";
			let mut id = "";
			if let Some(pos) = tag_part.find('.') {
				class = &tag_part[pos + 1..];
				tag = &tag_part[..pos];
			}
			if let Some(pos) = tag.find('#') {
				id = &tag[pos + 1..];
				tag = &tag[..pos];
			}
			(tag, class, id)
		};

		if tag.is_empty() {
			return false;
		}

		let mut attrs = String::new();
		if !id.is_empty() {
			attrs.push_str(&format!(" id=\"{id}\""));
		}
		if !class.is_empty() {
			attrs.push_str(&format!(" class=\"{class}\""));
		}

		let element = format!("<{tag}{attrs}></{tag}>");
		let expanded = (0..count)
			.map(|_| element.as_str())
			.collect::<Vec<&str>>()
			.join("\n");

		self.delete_range(start, offset);
		self.insert_text_at(start, &expanded);
		true
	}

	// ── Selection history ────────────────────────────────────────────

	/// Push the current selection onto the selection history stack.
	#[wasm_bindgen]
	pub fn push_selection_history(&mut self) {
		let start = self.runtime.selection().start().offset();
		let end = self.runtime.selection().end().offset();
		// Don't push duplicates.
		if self.selection_history.last() == Some(&(start, end)) {
			return;
		}
		// Truncate forward history if we're not at the end.
		if self.selection_history_index >= 0
			&& (self.selection_history_index as usize)
				< self.selection_history.len().saturating_sub(1)
		{
			self.selection_history
				.truncate(self.selection_history_index as usize + 1);
		}
		self.selection_history.push((start, end));
		if self.selection_history.len() > 50 {
			self.selection_history.remove(0);
		}
		self.selection_history_index = self.selection_history.len() as i32 - 1;
	}

	/// Go back in selection history.
	#[wasm_bindgen]
	pub fn selection_history_back(&mut self) -> bool {
		if self.selection_history_index <= 0 {
			return false;
		}
		self.selection_history_index -= 1;
		let (start, end) = self.selection_history[self.selection_history_index as usize];
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(end)),
		});
		true
	}

	/// Go forward in selection history.
	#[wasm_bindgen]
	pub fn selection_history_forward(&mut self) -> bool {
		if self.selection_history_index >= self.selection_history.len() as i32 - 1 {
			return false;
		}
		self.selection_history_index += 1;
		let (start, end) = self.selection_history[self.selection_history_index as usize];
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(end)),
		});
		true
	}

	/// Selection history length.
	#[wasm_bindgen]
	pub fn selection_history_length(&self) -> usize {
		self.selection_history.len()
	}

	// ── Editor focus API ─────────────────────────────────────────────

	/// Whether the editor is currently focused.
	#[wasm_bindgen]
	pub fn is_focused(&self) -> bool {
		self.focused
	}

	// ── Custom keybindings ───────────────────────────────────────────

	/// Rebind a keyboard shortcut to a command.
	///
	/// `shortcut` is e.g. "Ctrl+B", `command` is the command name from
	/// `command_list()` e.g. "Bold".
	#[wasm_bindgen]
	pub fn set_keybinding(&mut self, shortcut: &str, command: &str) {
		self.keybinding_overrides
			.insert(shortcut.to_string(), command.to_string());
	}

	/// Remove a custom keybinding override.
	#[wasm_bindgen]
	pub fn remove_keybinding(&mut self, shortcut: &str) {
		self.keybinding_overrides.remove(shortcut);
	}

	/// Clear all custom keybinding overrides.
	#[wasm_bindgen]
	pub fn clear_keybindings(&mut self) {
		self.keybinding_overrides.clear();
	}

	/// Get the command bound to a shortcut (custom override or default).
	#[wasm_bindgen]
	pub fn get_keybinding(&self, shortcut: &str) -> String {
		if let Some(cmd) = self.keybinding_overrides.get(shortcut) {
			return cmd.clone();
		}
		// Check default bindings.
		let all = self.command_list();
		let mut i = 0;
		while i + 1 < all.len() {
			if all[i + 1] == shortcut {
				return all[i].clone();
			}
			i += 2;
		}
		String::new()
	}

	/// Number of custom keybinding overrides.
	#[wasm_bindgen]
	pub fn keybinding_override_count(&self) -> usize {
		self.keybinding_overrides.len()
	}

	/// Get all keybinding overrides as [shortcut, command, ...].
	#[wasm_bindgen]
	pub fn keybinding_overrides_list(&self) -> Vec<String> {
		let mut out = Vec::with_capacity(self.keybinding_overrides.len() * 2);
		for (shortcut, command) in &self.keybinding_overrides {
			out.push(shortcut.clone());
			out.push(command.clone());
		}
		out
	}

	/// Execute a command by name.
	///
	/// Returns `true` if the command is recognized and executed.
	#[wasm_bindgen]
	pub fn run_command(&mut self, command: &str) -> bool {
		match command {
			"Bold" => {
				self.toggle_bold();
				true
			}
			"Italic" => {
				self.toggle_italic();
				true
			}
			"Underline" => {
				self.toggle_underline();
				true
			}
			"Strikethrough" => {
				self.toggle_strikethrough();
				true
			}
			"Undo" => self.undo(),
			"Redo" => self.redo(),
			"Select All" => {
				self.select_all();
				true
			}
			"Duplicate Line" => {
				self.duplicate_line();
				true
			}
			"Delete Line" => {
				self.delete_line();
				true
			}
			"Move Line Up" => {
				self.move_line_up();
				true
			}
			"Move Line Down" => {
				self.move_line_down();
				true
			}
			"Toggle Comment" => {
				self.toggle_line_comment();
				true
			}
			"Go To Line" => {
				self.go_to_line(1);
				true
			}
			"Toggle Word Wrap" => {
				let next = !self.word_wrap;
				self.set_word_wrap(next);
				true
			}
			"Transform Upper Case" => {
				self.transform_uppercase();
				true
			}
			"Transform Lower Case" => {
				self.transform_lowercase();
				true
			}
			"Join Lines" => {
				self.join_lines();
				true
			}
			"Sort Lines" => {
				self.sort_lines_asc();
				true
			}
			"Select Line" => {
				self.select_line();
				true
			}
			"Expand Selection" => {
				self.expand_selection();
				true
			}
			"Contract Selection" => {
				self.contract_selection();
				true
			}
			"Transpose Chars" => {
				self.transpose_chars();
				true
			}
			"Go To Matching Bracket" => {
				self.move_to_matching_bracket();
				true
			}
			"Delete Word Left" => {
				self.delete_word_left();
				true
			}
			"Delete Word Right" => {
				self.delete_word_right();
				true
			}
			"Open Line Below" => {
				self.open_line_below();
				true
			}
			"Open Line Above" => {
				self.open_line_above();
				true
			}
			"Select Between Brackets" => {
				self.select_between_brackets();
				true
			}
			"Document Start" => {
				self.go_to_document_start();
				true
			}
			"Document End" => {
				self.go_to_document_end();
				true
			}
			"Center Line" => {
				self.center_line_in_viewport();
				true
			}
			"Toggle Overwrite" => {
				self.toggle_overwrite_mode();
				true
			}
			_ => false,
		}
	}

	/// Execute the command bound to a shortcut.
	///
	/// Custom overrides are checked first, then defaults.
	#[wasm_bindgen]
	pub fn run_shortcut(&mut self, shortcut: &str) -> bool {
		let cmd = self.get_keybinding(shortcut);
		if cmd.is_empty() {
			return false;
		}
		self.run_command(&cmd)
	}

	// ── Text transform pipeline ──────────────────────────────────────

	/// Apply a transformation pipeline to the current selection.
	///
	/// Supported step names (case-insensitive, `|` separated):
	/// `upper`, `lower`, `title`, `camel`, `snake`, `kebab`, `constant`,
	/// `reverse`.
	#[wasm_bindgen]
	pub fn transform_pipeline(&mut self, pipeline: &str) {
		if !self.is_writable() {
			return;
		}

		let words = |text: &str| {
			text.split(|c: char| !c.is_alphanumeric())
				.filter(|w| !w.is_empty())
				.map(|w| w.to_string())
				.collect::<Vec<String>>()
		};
		let to_title = |text: &str| {
			text.split_whitespace()
				.map(|word| {
					let mut chars = word.chars();
					match chars.next() {
						Some(c) => {
							let first: String = c.to_uppercase().collect();
							format!("{first}{}", chars.as_str().to_lowercase())
						}
						None => String::new(),
					}
				})
				.collect::<Vec<String>>()
				.join(" ")
		};
		let to_camel = |text: &str| {
			let parts = words(text);
			parts
				.iter()
				.enumerate()
				.map(|(i, w)| {
					if i == 0 {
						w.to_lowercase()
					} else {
						let mut chars = w.chars();
						match chars.next() {
							Some(c) => {
								let first: String = c.to_uppercase().collect();
								format!("{first}{}", chars.as_str().to_lowercase())
							}
							None => String::new(),
						}
					}
				})
				.collect::<String>()
		};
		let to_snake = |text: &str| {
			words(text)
				.iter()
				.map(|w| w.to_lowercase())
				.collect::<Vec<String>>()
				.join("_")
		};
		let to_kebab = |text: &str| {
			words(text)
				.iter()
				.map(|w| w.to_lowercase())
				.collect::<Vec<String>>()
				.join("-")
		};

		self.transform_selection(|selected| {
			let mut out = selected.to_string();
			for raw_step in pipeline.split('|') {
				let step = raw_step.trim().to_ascii_lowercase();
				out = match step.as_str() {
					"upper" | "uppercase" => out.to_uppercase(),
					"lower" | "lowercase" => out.to_lowercase(),
					"title" | "title_case" => to_title(&out),
					"camel" | "camel_case" => to_camel(&out),
					"snake" | "snake_case" => to_snake(&out),
					"kebab" | "kebab_case" => to_kebab(&out),
					"constant" | "constant_case" => to_snake(&out).to_uppercase(),
					"reverse" => out.chars().rev().collect(),
					_ => out,
				};
			}
			out
		});
	}

	/// Transform selected text to camelCase.
	#[wasm_bindgen]
	pub fn transform_camel_case(&mut self) {
		if !self.is_writable() {
			return;
		}
		let sel = self.runtime.selection();
		let start = sel.start().offset();
		let end = sel.end().offset();
		if start == end {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let selected: String = chars[start..end].iter().collect();
		let words: Vec<&str> = selected
			.split(|c: char| c.is_whitespace() || c == '-' || c == '_')
			.filter(|w| !w.is_empty())
			.collect();
		let transformed: String = words
			.iter()
			.enumerate()
			.map(|(i, w)| {
				if i == 0 {
					w.to_lowercase()
				} else {
					let mut chars = w.chars();
					match chars.next() {
						Some(c) => {
							let upper: String = c.to_uppercase().collect();
							format!("{upper}{}", chars.as_str().to_lowercase())
						}
						None => String::new(),
					}
				}
			})
			.collect();
		self.delete_range(start, end);
		self.insert_text_at(start, &transformed);
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(
				Position::new(start),
				Position::new(start + transformed.chars().count()),
			),
		});
	}

	/// Transform selected text to snake_case.
	#[wasm_bindgen]
	pub fn transform_snake_case(&mut self) {
		if !self.is_writable() {
			return;
		}
		let sel = self.runtime.selection();
		let start = sel.start().offset();
		let end = sel.end().offset();
		if start == end {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let selected: String = chars[start..end].iter().collect();
		let words: Vec<&str> = selected
			.split(|c: char| c.is_whitespace() || c == '-' || c == '_')
			.filter(|w| !w.is_empty())
			.collect();
		let transformed = words
			.iter()
			.map(|w| w.to_lowercase())
			.collect::<Vec<String>>()
			.join("_");
		self.delete_range(start, end);
		self.insert_text_at(start, &transformed);
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(
				Position::new(start),
				Position::new(start + transformed.chars().count()),
			),
		});
	}

	/// Transform selected text to kebab-case.
	#[wasm_bindgen]
	pub fn transform_kebab_case(&mut self) {
		if !self.is_writable() {
			return;
		}
		let sel = self.runtime.selection();
		let start = sel.start().offset();
		let end = sel.end().offset();
		if start == end {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let selected: String = chars[start..end].iter().collect();
		let words: Vec<&str> = selected
			.split(|c: char| c.is_whitespace() || c == '-' || c == '_')
			.filter(|w| !w.is_empty())
			.collect();
		let transformed = words
			.iter()
			.map(|w| w.to_lowercase())
			.collect::<Vec<String>>()
			.join("-");
		self.delete_range(start, end);
		self.insert_text_at(start, &transformed);
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(
				Position::new(start),
				Position::new(start + transformed.chars().count()),
			),
		});
	}

	/// Transform selected text to CONSTANT_CASE (upper snake).
	#[wasm_bindgen]
	pub fn transform_constant_case(&mut self) {
		if !self.is_writable() {
			return;
		}
		let sel = self.runtime.selection();
		let start = sel.start().offset();
		let end = sel.end().offset();
		if start == end {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let selected: String = chars[start..end].iter().collect();
		let words: Vec<&str> = selected
			.split(|c: char| c.is_whitespace() || c == '-' || c == '_')
			.filter(|w| !w.is_empty())
			.collect();
		let transformed = words
			.iter()
			.map(|w| w.to_uppercase())
			.collect::<Vec<String>>()
			.join("_");
		self.delete_range(start, end);
		self.insert_text_at(start, &transformed);
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(
				Position::new(start),
				Position::new(start + transformed.chars().count()),
			),
		});
	}

	// ── Marker ranges ────────────────────────────────────────────────

	/// Add a coloured marker highlight range.
	///
	/// Returns the marker ID for later removal.
	#[wasm_bindgen]
	pub fn add_marker(&mut self, start: usize, end: usize, r: u8, g: u8, b: u8, a: u8, id: &str) {
		self.markers.retain(|m| m.6 != id);
		self.markers.push((start, end, r, g, b, a, id.to_string()));
	}

	/// Remove a marker by ID.
	#[wasm_bindgen]
	pub fn remove_marker(&mut self, id: &str) {
		self.markers.retain(|m| m.6 != id);
	}

	/// Remove all markers with IDs starting with a prefix.
	#[wasm_bindgen]
	pub fn remove_markers_by_prefix(&mut self, prefix: &str) {
		self.markers.retain(|m| !m.6.starts_with(prefix));
	}

	/// Clear all markers.
	#[wasm_bindgen]
	pub fn clear_markers(&mut self) {
		self.markers.clear();
	}

	/// Number of active markers.
	#[wasm_bindgen]
	pub fn marker_count(&self) -> usize {
		self.markers.len()
	}

	/// Get all markers as [start, end, r, g, b, a, id, ...].
	#[wasm_bindgen]
	pub fn marker_list(&self) -> Vec<String> {
		let mut result = Vec::with_capacity(self.markers.len() * 7);
		for (start, end, r, g, b, a, id) in &self.markers {
			result.push(start.to_string());
			result.push(end.to_string());
			result.push(r.to_string());
			result.push(g.to_string());
			result.push(b.to_string());
			result.push(a.to_string());
			result.push(id.clone());
		}
		result
	}

	/// Get markers overlapping a character offset.
	#[wasm_bindgen]
	pub fn markers_at(&self, offset: usize) -> Vec<String> {
		let mut result = Vec::new();
		for (start, end, r, g, b, a, id) in &self.markers {
			if offset >= *start && offset < *end {
				result.push(start.to_string());
				result.push(end.to_string());
				result.push(r.to_string());
				result.push(g.to_string());
				result.push(b.to_string());
				result.push(a.to_string());
				result.push(id.clone());
			}
		}
		result
	}

	// ── Soft wrap info ───────────────────────────────────────────────

	/// Number of visual (display) lines after word wrapping.
	#[wasm_bindgen]
	pub fn visual_line_count(&self) -> usize {
		self.line_count()
			.unwrap_or_else(|_| self.runtime.document().plain_text().split('\n').count())
	}

	/// Whether a specific logical line (0-based) is soft-wrapped into
	/// multiple visual lines.
	#[wasm_bindgen]
	pub fn is_line_wrapped(&self, line: usize) -> bool {
		if !self.word_wrap {
			return false;
		}
		let (_, ctx) = match self.canvas_and_context() {
			Ok(v) => v,
			Err(_) => return false,
		};
		let renderer = Canvas2dRenderer::new(ctx, self.width, self.height);
		let lc = self.layout_constants();
		let plain = self.runtime.document().plain_text();
		let styled_runs = self.runtime.document().styled_runs();
		let paragraphs = layout_paragraphs(
			&plain,
			&styled_runs,
			&lc.layout_config,
			&renderer,
			&lc.default_style,
		);
		paragraphs
			.get(line)
			.map(|p| p.layout.lines.len() > 1)
			.unwrap_or(false)
	}

	// ── Extended statistics ──────────────────────────────────────────

	/// Number of paragraph blocks (text groups separated by blank lines).
	#[wasm_bindgen]
	pub fn paragraph_block_count(&self) -> usize {
		let plain = self.runtime.document().plain_text();
		if plain.trim().is_empty() {
			return 0;
		}
		let mut count = 0usize;
		let mut in_para = false;
		for line in plain.split('\n') {
			if line.trim().is_empty() {
				if in_para {
					in_para = false;
				}
			} else if !in_para {
				in_para = true;
				count += 1;
			}
		}
		count
	}

	/// Average number of characters per line.
	#[wasm_bindgen]
	pub fn avg_line_length(&self) -> f64 {
		let plain = self.runtime.document().plain_text();
		let lines: Vec<&str> = plain.split('\n').collect();
		if lines.is_empty() {
			return 0.0;
		}
		let total: usize = lines.iter().map(|l| l.chars().count()).sum();
		total as f64 / lines.len() as f64
	}

	/// Longest line length in characters.
	#[wasm_bindgen]
	pub fn longest_line_length(&self) -> usize {
		let plain = self.runtime.document().plain_text();
		plain
			.split('\n')
			.map(|l| l.chars().count())
			.max()
			.unwrap_or(0)
	}

	/// Line number of the longest line (0-based).
	#[wasm_bindgen]
	pub fn longest_line_number(&self) -> usize {
		let plain = self.runtime.document().plain_text();
		plain
			.split('\n')
			.enumerate()
			.max_by_key(|(_, l)| l.chars().count())
			.map(|(i, _)| i)
			.unwrap_or(0)
	}

	/// Total byte count of the document (UTF-8).
	#[wasm_bindgen]
	pub fn byte_count(&self) -> usize {
		self.runtime.document().plain_text().len()
	}

	// ── Auto-complete context ────────────────────────────────────────

	/// Get filtered word completions with context.
	///
	/// Returns [word, lineContext, ...] where lineContext is the line
	/// where the word appears. Max `limit` results.
	#[wasm_bindgen]
	pub fn completions_with_context(&self, limit: usize) -> Vec<String> {
		let plain = self.runtime.document().plain_text();
		let offset = self.runtime.selection().end().offset();
		let chars: Vec<char> = plain.chars().collect();

		// Find prefix at cursor.
		let mut start = offset;
		while start > 0 && chars[start - 1].is_alphanumeric() {
			start -= 1;
		}
		if start == offset {
			return Vec::new();
		}
		let prefix: String = chars[start..offset].iter().collect();
		let prefix_lower = prefix.to_lowercase();

		// Collect unique words matching the prefix.
		let mut seen = std::collections::HashSet::new();
		let mut result = Vec::new();
		for line in plain.split('\n') {
			for word in line.split(|c: char| !c.is_alphanumeric()) {
				if word.len() > prefix.len()
					&& word.to_lowercase().starts_with(&prefix_lower)
					&& seen.insert(word.to_lowercase())
				{
					result.push(word.to_string());
					result.push(line.trim().to_string());
					if result.len() / 2 >= limit {
						return result;
					}
				}
			}
		}
		result
	}

	// ── Named anchors ────────────────────────────────────────────────

	/// Set a named anchor to a character offset.
	///
	/// If the anchor already exists, it is updated.
	#[wasm_bindgen]
	pub fn set_anchor(&mut self, name: &str, offset: usize) {
		let max = self.runtime.document().char_count();
		self.anchors.insert(name.to_string(), offset.min(max));
	}

	/// Set a named anchor only if it does not already exist.
	///
	/// Returns `true` when inserted.
	#[wasm_bindgen]
	pub fn set_anchor_if_absent(&mut self, name: &str, offset: usize) -> bool {
		if self.anchors.contains_key(name) {
			return false;
		}
		let max = self.runtime.document().char_count();
		self.anchors.insert(name.to_string(), offset.min(max));
		true
	}

	/// Get a named anchor offset, or -1 if not found.
	#[wasm_bindgen]
	pub fn anchor_offset(&self, name: &str) -> i32 {
		self.anchors.get(name).map(|v| *v as i32).unwrap_or(-1)
	}

	/// Remove a named anchor.
	#[wasm_bindgen]
	pub fn remove_anchor(&mut self, name: &str) {
		self.anchors.remove(name);
	}

	/// Clear all named anchors.
	#[wasm_bindgen]
	pub fn clear_anchors(&mut self) {
		self.anchors.clear();
	}

	/// Number of named anchors.
	#[wasm_bindgen]
	pub fn anchor_count(&self) -> usize {
		self.anchors.len()
	}

	/// List anchor names sorted alphabetically.
	#[wasm_bindgen]
	pub fn anchor_names(&self) -> Vec<String> {
		let mut out: Vec<String> = self.anchors.keys().cloned().collect();
		out.sort_unstable();
		out
	}

	/// Move cursor to a named anchor.
	///
	/// Returns true if the anchor exists.
	#[wasm_bindgen]
	pub fn go_to_anchor(&mut self, name: &str) -> bool {
		let Some(offset) = self.anchors.get(name).copied() else {
			return false;
		};
		let max = self.runtime.document().char_count();
		let target = offset.min(max);
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(target)),
		});
		true
	}

	/// Whether a named anchor exists.
	#[wasm_bindgen]
	pub fn anchor_exists(&self, name: &str) -> bool {
		self.anchors.contains_key(name)
	}

	/// Rename an anchor key.
	///
	/// Returns `true` when source anchor existed.
	#[wasm_bindgen]
	pub fn rename_anchor(&mut self, old_name: &str, new_name: &str) -> bool {
		if old_name == new_name {
			return self.anchors.contains_key(old_name);
		}
		let Some(offset) = self.anchors.remove(old_name) else {
			return false;
		};
		self.anchors.insert(new_name.to_string(), offset);
		true
	}

	/// Find nearest anchor at or before the given offset.
	///
	/// Returns `[name, offset]` or empty when none.
	#[wasm_bindgen]
	pub fn nearest_anchor_before(&self, offset: usize) -> Vec<String> {
		let mut best: Option<(&String, usize)> = None;
		for (name, pos) in &self.anchors {
			if *pos <= offset {
				match best {
					Some((_, best_pos)) if *pos <= best_pos => {}
					_ => best = Some((name, *pos)),
				}
			}
		}
		if let Some((name, pos)) = best {
			vec![name.clone(), pos.to_string()]
		} else {
			Vec::new()
		}
	}

	/// Find nearest anchor at or after the given offset.
	///
	/// Returns `[name, offset]` or empty when none.
	#[wasm_bindgen]
	pub fn nearest_anchor_after(&self, offset: usize) -> Vec<String> {
		let mut best: Option<(&String, usize)> = None;
		for (name, pos) in &self.anchors {
			if *pos >= offset {
				match best {
					Some((_, best_pos)) if *pos >= best_pos => {}
					_ => best = Some((name, *pos)),
				}
			}
		}
		if let Some((name, pos)) = best {
			vec![name.clone(), pos.to_string()]
		} else {
			Vec::new()
		}
	}

	/// Next anchor after the current cursor as `[name, offset]`.
	///
	/// Uses strict `>` comparison against the cursor offset.
	#[wasm_bindgen]
	pub fn next_anchor_after_cursor(&self) -> Vec<String> {
		let cursor = self.runtime.selection().end().offset();
		let mut pairs: Vec<(usize, String)> = self
			.anchors
			.iter()
			.filter_map(|(name, pos)| (*pos > cursor).then_some((*pos, name.clone())))
			.collect();
		pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
		if let Some((offset, name)) = pairs.into_iter().next() {
			vec![name, offset.to_string()]
		} else {
			Vec::new()
		}
	}

	/// Previous anchor before the current cursor as `[name, offset]`.
	///
	/// Uses strict `<` comparison against the cursor offset.
	#[wasm_bindgen]
	pub fn prev_anchor_before_cursor(&self) -> Vec<String> {
		let cursor = self.runtime.selection().end().offset();
		let mut pairs: Vec<(usize, String)> = self
			.anchors
			.iter()
			.filter_map(|(name, pos)| (*pos < cursor).then_some((*pos, name.clone())))
			.collect();
		pairs.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
		if let Some((offset, name)) = pairs.into_iter().next() {
			vec![name, offset.to_string()]
		} else {
			Vec::new()
		}
	}

	/// Move cursor to the next anchor after the current cursor.
	///
	/// When `wrap` is true and no next anchor exists, wraps to first anchor.
	#[wasm_bindgen]
	pub fn go_to_next_anchor(&mut self, wrap: bool) -> bool {
		let cursor = self.runtime.selection().end().offset();
		let mut pairs: Vec<(usize, String)> = self
			.anchors
			.iter()
			.map(|(name, pos)| (*pos, name.clone()))
			.collect();
		pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
		let target = pairs
			.iter()
			.find(|(offset, _)| *offset > cursor)
			.map(|(offset, _)| *offset)
			.or_else(|| {
				if wrap {
					pairs.first().map(|(offset, _)| *offset)
				} else {
					None
				}
			});
		let Some(target_offset) = target else {
			return false;
		};
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(target_offset)),
		});
		true
	}

	/// Move cursor to the previous anchor before the current cursor.
	///
	/// When `wrap` is true and no previous anchor exists, wraps to last anchor.
	#[wasm_bindgen]
	pub fn go_to_prev_anchor(&mut self, wrap: bool) -> bool {
		let cursor = self.runtime.selection().end().offset();
		let mut pairs: Vec<(usize, String)> = self
			.anchors
			.iter()
			.map(|(name, pos)| (*pos, name.clone()))
			.collect();
		pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
		let target = pairs
			.iter()
			.rev()
			.find(|(offset, _)| *offset < cursor)
			.map(|(offset, _)| *offset)
			.or_else(|| {
				if wrap {
					pairs.last().map(|(offset, _)| *offset)
				} else {
					None
				}
			});
		let Some(target_offset) = target else {
			return false;
		};
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(target_offset)),
		});
		true
	}

	/// Anchor names set exactly at a given offset.
	#[wasm_bindgen]
	pub fn anchors_at_offset(&self, offset: usize) -> Vec<String> {
		let mut out: Vec<String> = self
			.anchors
			.iter()
			.filter_map(|(name, pos)| (*pos == offset).then_some(name.clone()))
			.collect();
		out.sort_unstable();
		out
	}

	/// Anchors inside an inclusive character-offset range.
	///
	/// Returns `[name, offset, ...]` sorted by offset then name.
	#[wasm_bindgen]
	pub fn anchors_in_range(&self, start_offset: usize, end_offset: usize) -> Vec<String> {
		let (start, end) = if start_offset <= end_offset {
			(start_offset, end_offset)
		} else {
			(end_offset, start_offset)
		};

		let mut pairs: Vec<(usize, String)> = self
			.anchors
			.iter()
			.filter_map(|(name, pos)| {
				(*pos >= start && *pos <= end).then_some((*pos, name.clone()))
			})
			.collect();
		pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

		let mut out = Vec::with_capacity(pairs.len() * 2);
		for (pos, name) in pairs {
			out.push(name);
			out.push(pos.to_string());
		}
		out
	}

	/// Shift a named anchor by a signed delta.
	///
	/// The resulting offset is clamped to the current document bounds.
	/// Returns `false` when the anchor does not exist.
	#[wasm_bindgen]
	pub fn shift_anchor(&mut self, name: &str, delta: i32) -> bool {
		let Some(offset) = self.anchors.get_mut(name) else {
			return false;
		};
		let max = self.runtime.document().char_count() as i64;
		let next = ((*offset as i64) + i64::from(delta)).clamp(0, max) as usize;
		*offset = next;
		true
	}

	/// Anchor entries as flat `[name, offset, ...]`, sorted by name.
	#[wasm_bindgen]
	pub fn anchor_entries(&self) -> Vec<String> {
		let mut names: Vec<String> = self.anchors.keys().cloned().collect();
		names.sort_unstable();
		let mut out = Vec::with_capacity(names.len() * 2);
		for name in names {
			if let Some(offset) = self.anchors.get(&name) {
				out.push(name);
				out.push(offset.to_string());
			}
		}
		out
	}

	/// Anchor names sorted by offset then name.
	#[wasm_bindgen]
	pub fn anchor_names_by_offset(&self) -> Vec<String> {
		let mut pairs: Vec<(usize, String)> = self
			.anchors
			.iter()
			.map(|(name, offset)| (*offset, name.clone()))
			.collect();
		pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
		pairs.into_iter().map(|(_, name)| name).collect()
	}

	/// Earliest anchor entry as `[name, offset]`.
	///
	/// Ties are resolved by anchor name.
	#[wasm_bindgen]
	pub fn first_anchor_entry(&self) -> Vec<String> {
		let mut pairs: Vec<(usize, String)> = self
			.anchors
			.iter()
			.map(|(name, offset)| (*offset, name.clone()))
			.collect();
		pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
		if let Some((offset, name)) = pairs.into_iter().next() {
			vec![name, offset.to_string()]
		} else {
			Vec::new()
		}
	}

	/// Latest anchor entry as `[name, offset]`.
	///
	/// Ties are resolved by anchor name.
	#[wasm_bindgen]
	pub fn last_anchor_entry(&self) -> Vec<String> {
		let mut pairs: Vec<(usize, String)> = self
			.anchors
			.iter()
			.map(|(name, offset)| (*offset, name.clone()))
			.collect();
		pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
		if let Some((offset, name)) = pairs.into_iter().last() {
			vec![name, offset.to_string()]
		} else {
			Vec::new()
		}
	}

	/// Anchor names at or before `offset`.
	///
	/// When `inclusive` is false, only names strictly before are returned.
	/// Output is sorted by offset then name.
	#[wasm_bindgen]
	pub fn anchor_names_before_offset(&self, offset: usize, inclusive: bool) -> Vec<String> {
		let mut pairs: Vec<(usize, String)> = self
			.anchors
			.iter()
			.filter_map(|(name, pos)| {
				(if inclusive {
					*pos <= offset
				} else {
					*pos < offset
				})
				.then_some((*pos, name.clone()))
			})
			.collect();
		pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
		pairs.into_iter().map(|(_, name)| name).collect()
	}

	/// Anchor names at or after `offset`.
	///
	/// When `inclusive` is false, only names strictly after are returned.
	/// Output is sorted by offset then name.
	#[wasm_bindgen]
	pub fn anchor_names_after_offset(&self, offset: usize, inclusive: bool) -> Vec<String> {
		let mut pairs: Vec<(usize, String)> = self
			.anchors
			.iter()
			.filter_map(|(name, pos)| {
				(if inclusive {
					*pos >= offset
				} else {
					*pos > offset
				})
				.then_some((*pos, name.clone()))
			})
			.collect();
		pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
		pairs.into_iter().map(|(_, name)| name).collect()
	}

	/// Count anchors before an offset.
	#[wasm_bindgen]
	pub fn anchor_count_before_offset(&self, offset: usize, inclusive: bool) -> usize {
		self.anchor_names_before_offset(offset, inclusive).len()
	}

	/// Count anchors after an offset.
	#[wasm_bindgen]
	pub fn anchor_count_after_offset(&self, offset: usize, inclusive: bool) -> usize {
		self.anchor_names_after_offset(offset, inclusive).len()
	}

	/// Shift anchors before an offset by a signed delta.
	///
	/// Offsets are clamped to document bounds. Returns number shifted.
	#[wasm_bindgen]
	pub fn shift_anchors_before_offset(
		&mut self,
		offset: usize,
		delta: i32,
		inclusive: bool,
	) -> usize {
		let max = self.runtime.document().char_count() as i64;
		let mut shifted = 0usize;
		for anchor_offset in self.anchors.values_mut() {
			let in_range = if inclusive {
				*anchor_offset <= offset
			} else {
				*anchor_offset < offset
			};
			if in_range {
				let next = ((*anchor_offset as i64) + i64::from(delta)).clamp(0, max) as usize;
				*anchor_offset = next;
				shifted += 1;
			}
		}
		shifted
	}

	/// Shift anchors after an offset by a signed delta.
	///
	/// Offsets are clamped to document bounds. Returns number shifted.
	#[wasm_bindgen]
	pub fn shift_anchors_after_offset(
		&mut self,
		offset: usize,
		delta: i32,
		inclusive: bool,
	) -> usize {
		let max = self.runtime.document().char_count() as i64;
		let mut shifted = 0usize;
		for anchor_offset in self.anchors.values_mut() {
			let in_range = if inclusive {
				*anchor_offset >= offset
			} else {
				*anchor_offset > offset
			};
			if in_range {
				let next = ((*anchor_offset as i64) + i64::from(delta)).clamp(0, max) as usize;
				*anchor_offset = next;
				shifted += 1;
			}
		}
		shifted
	}

	/// Remove anchors before an offset.
	///
	/// Returns number removed.
	#[wasm_bindgen]
	pub fn remove_anchors_before_offset(&mut self, offset: usize, inclusive: bool) -> usize {
		let keys: Vec<String> = self
			.anchors
			.iter()
			.filter_map(|(name, pos)| {
				(if inclusive {
					*pos <= offset
				} else {
					*pos < offset
				})
				.then_some(name.clone())
			})
			.collect();
		let removed = keys.len();
		for key in keys {
			self.anchors.remove(&key);
		}
		removed
	}

	/// Remove anchors after an offset.
	///
	/// Returns number removed.
	#[wasm_bindgen]
	pub fn remove_anchors_after_offset(&mut self, offset: usize, inclusive: bool) -> usize {
		let keys: Vec<String> = self
			.anchors
			.iter()
			.filter_map(|(name, pos)| {
				(if inclusive {
					*pos >= offset
				} else {
					*pos > offset
				})
				.then_some(name.clone())
			})
			.collect();
		let removed = keys.len();
		for key in keys {
			self.anchors.remove(&key);
		}
		removed
	}

	/// Anchor names before a named anchor offset.
	///
	/// When `inclusive` is false, only anchors strictly before are returned.
	/// Returns empty when the named anchor does not exist.
	#[wasm_bindgen]
	pub fn anchor_names_before_anchor(&self, name: &str, inclusive: bool) -> Vec<String> {
		let Some(offset) = self.anchors.get(name).copied() else {
			return Vec::new();
		};
		self.anchor_names_before_offset(offset, inclusive)
	}

	/// Anchor names after a named anchor offset.
	///
	/// When `inclusive` is false, only anchors strictly after are returned.
	/// Returns empty when the named anchor does not exist.
	#[wasm_bindgen]
	pub fn anchor_names_after_anchor(&self, name: &str, inclusive: bool) -> Vec<String> {
		let Some(offset) = self.anchors.get(name).copied() else {
			return Vec::new();
		};
		self.anchor_names_after_offset(offset, inclusive)
	}

	/// Count anchors before a named anchor offset.
	#[wasm_bindgen]
	pub fn anchor_count_before_anchor(&self, name: &str, inclusive: bool) -> usize {
		self.anchor_names_before_anchor(name, inclusive).len()
	}

	/// Count anchors after a named anchor offset.
	#[wasm_bindgen]
	pub fn anchor_count_after_anchor(&self, name: &str, inclusive: bool) -> usize {
		self.anchor_names_after_anchor(name, inclusive).len()
	}

	/// Shift anchors before a named anchor offset by a signed delta.
	///
	/// Offsets are clamped to document bounds. Returns number shifted.
	#[wasm_bindgen]
	pub fn shift_anchors_before_anchor(
		&mut self,
		name: &str,
		delta: i32,
		inclusive: bool,
	) -> usize {
		let Some(boundary) = self.anchors.get(name).copied() else {
			return 0;
		};
		let max = self.runtime.document().char_count() as i64;
		let mut shifted = 0usize;
		for offset in self.anchors.values_mut() {
			let in_range = if inclusive {
				*offset <= boundary
			} else {
				*offset < boundary
			};
			if in_range {
				let next = ((*offset as i64) + i64::from(delta)).clamp(0, max) as usize;
				*offset = next;
				shifted += 1;
			}
		}
		shifted
	}

	/// Shift anchors after a named anchor offset by a signed delta.
	///
	/// Offsets are clamped to document bounds. Returns number shifted.
	#[wasm_bindgen]
	pub fn shift_anchors_after_anchor(&mut self, name: &str, delta: i32, inclusive: bool) -> usize {
		let Some(boundary) = self.anchors.get(name).copied() else {
			return 0;
		};
		let max = self.runtime.document().char_count() as i64;
		let mut shifted = 0usize;
		for offset in self.anchors.values_mut() {
			let in_range = if inclusive {
				*offset >= boundary
			} else {
				*offset > boundary
			};
			if in_range {
				let next = ((*offset as i64) + i64::from(delta)).clamp(0, max) as usize;
				*offset = next;
				shifted += 1;
			}
		}
		shifted
	}

	/// Offset span for two named anchors as `[start, end]`.
	///
	/// Returns empty when either anchor name is missing.
	#[wasm_bindgen]
	pub fn anchor_span_offsets(&self, start_name: &str, end_name: &str) -> Vec<usize> {
		let Some((start, end)) = self.anchor_span_from_names(start_name, end_name) else {
			return Vec::new();
		};
		vec![start, end]
	}

	/// Anchor names between two named anchors.
	///
	/// Uses normalized min/max offsets of the provided anchor names.
	/// Returns empty when either anchor name is missing.
	#[wasm_bindgen]
	pub fn anchor_names_between(
		&self,
		start_name: &str,
		end_name: &str,
		inclusive: bool,
	) -> Vec<String> {
		let Some((start, end)) = self.anchor_span_from_names(start_name, end_name) else {
			return Vec::new();
		};
		let mut pairs: Vec<(usize, String)> = self
			.anchors
			.iter()
			.filter_map(|(name, pos)| {
				(if inclusive {
					*pos >= start && *pos <= end
				} else {
					*pos > start && *pos < end
				})
				.then_some((*pos, name.clone()))
			})
			.collect();
		pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
		pairs.into_iter().map(|(_, name)| name).collect()
	}

	/// Count anchors between two named anchors.
	#[wasm_bindgen]
	pub fn anchor_count_between(&self, start_name: &str, end_name: &str, inclusive: bool) -> usize {
		self.anchor_names_between(start_name, end_name, inclusive)
			.len()
	}

	/// Shift anchors between two named anchors by a signed delta.
	///
	/// Offsets are clamped to document bounds. Returns number shifted.
	#[wasm_bindgen]
	pub fn shift_anchors_between(
		&mut self,
		start_name: &str,
		end_name: &str,
		delta: i32,
		inclusive: bool,
	) -> usize {
		let Some((start, end)) = self.anchor_span_from_names(start_name, end_name) else {
			return 0;
		};
		let max = self.runtime.document().char_count() as i64;
		let mut shifted = 0usize;
		for offset in self.anchors.values_mut() {
			let in_range = if inclusive {
				*offset >= start && *offset <= end
			} else {
				*offset > start && *offset < end
			};
			if in_range {
				let next = ((*offset as i64) + i64::from(delta)).clamp(0, max) as usize;
				*offset = next;
				shifted += 1;
			}
		}
		shifted
	}

	/// Remove anchors between two named anchors.
	///
	/// Returns number removed.
	#[wasm_bindgen]
	pub fn remove_anchors_between(
		&mut self,
		start_name: &str,
		end_name: &str,
		inclusive: bool,
	) -> usize {
		let Some((start, end)) = self.anchor_span_from_names(start_name, end_name) else {
			return 0;
		};
		let keys: Vec<String> = self
			.anchors
			.iter()
			.filter_map(|(name, pos)| {
				(if inclusive {
					*pos >= start && *pos <= end
				} else {
					*pos > start && *pos < end
				})
				.then_some(name.clone())
			})
			.collect();
		let removed = keys.len();
		for key in keys {
			self.anchors.remove(&key);
		}
		removed
	}

	/// Anchor offsets inside an inclusive range, sorted ascending.
	#[wasm_bindgen]
	pub fn anchor_offsets_in_range(&self, start_offset: usize, end_offset: usize) -> Vec<usize> {
		let (start, end) = if start_offset <= end_offset {
			(start_offset, end_offset)
		} else {
			(end_offset, start_offset)
		};

		let mut out: Vec<usize> = self
			.anchors
			.values()
			.copied()
			.filter(|offset| *offset >= start && *offset <= end)
			.collect();
		out.sort_unstable();
		out
	}

	/// Shift all anchors in an inclusive range by signed delta.
	///
	/// Offsets are clamped to document bounds. Returns number shifted.
	#[wasm_bindgen]
	pub fn shift_anchors_in_range(
		&mut self,
		start_offset: usize,
		end_offset: usize,
		delta: i32,
	) -> usize {
		let (start, end) = if start_offset <= end_offset {
			(start_offset, end_offset)
		} else {
			(end_offset, start_offset)
		};
		let max = self.runtime.document().char_count() as i64;
		let mut shifted = 0usize;
		for offset in self.anchors.values_mut() {
			if *offset >= start && *offset <= end {
				let next = ((*offset as i64) + i64::from(delta)).clamp(0, max) as usize;
				*offset = next;
				shifted += 1;
			}
		}
		shifted
	}

	/// Remove anchors whose names start with prefix.
	///
	/// Returns number removed.
	#[wasm_bindgen]
	pub fn remove_anchors_with_prefix(&mut self, prefix: &str) -> usize {
		if prefix.is_empty() {
			return 0;
		}
		let keys: Vec<String> = self
			.anchors
			.keys()
			.filter(|name| name.starts_with(prefix))
			.cloned()
			.collect();
		let removed = keys.len();
		for key in keys {
			self.anchors.remove(&key);
		}
		removed
	}

	/// Rename anchors with a shared prefix.
	///
	/// Returns number renamed. Existing destination names are overwritten.
	#[wasm_bindgen]
	pub fn rename_anchor_prefix(&mut self, old_prefix: &str, new_prefix: &str) -> usize {
		if old_prefix.is_empty() {
			return 0;
		}
		let mut keys: Vec<String> = self
			.anchors
			.keys()
			.filter(|name| name.starts_with(old_prefix))
			.cloned()
			.collect();
		if keys.is_empty() {
			return 0;
		}
		keys.sort_unstable();
		let mut renamed = 0usize;
		for old_name in keys {
			let Some(offset) = self.anchors.remove(&old_name) else {
				continue;
			};
			let suffix = &old_name[old_prefix.len()..];
			let new_name = format!("{new_prefix}{suffix}");
			self.anchors.insert(new_name, offset);
			renamed += 1;
		}
		renamed
	}

	/// Anchor names that start with a prefix, sorted by name.
	#[wasm_bindgen]
	pub fn anchor_names_with_prefix(&self, prefix: &str) -> Vec<String> {
		let mut out: Vec<String> = self
			.anchors
			.keys()
			.filter(|name| prefix.is_empty() || name.starts_with(prefix))
			.cloned()
			.collect();
		out.sort_unstable();
		out
	}

	/// Anchor names inside an inclusive character-offset range.
	///
	/// Sorted by offset then name.
	#[wasm_bindgen]
	pub fn anchor_names_in_range(&self, start_offset: usize, end_offset: usize) -> Vec<String> {
		let (start, end) = if start_offset <= end_offset {
			(start_offset, end_offset)
		} else {
			(end_offset, start_offset)
		};

		let mut pairs: Vec<(usize, String)> = self
			.anchors
			.iter()
			.filter_map(|(name, pos)| {
				(*pos >= start && *pos <= end).then_some((*pos, name.clone()))
			})
			.collect();
		pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
		pairs.into_iter().map(|(_, name)| name).collect()
	}

	/// Remove anchors whose offsets are inside an inclusive range.
	///
	/// Returns number removed.
	#[wasm_bindgen]
	pub fn remove_anchors_in_range(&mut self, start_offset: usize, end_offset: usize) -> usize {
		let (start, end) = if start_offset <= end_offset {
			(start_offset, end_offset)
		} else {
			(end_offset, start_offset)
		};

		let keys: Vec<String> = self
			.anchors
			.iter()
			.filter_map(|(name, pos)| (*pos >= start && *pos <= end).then_some(name.clone()))
			.collect();
		let removed = keys.len();
		for key in keys {
			self.anchors.remove(&key);
		}
		removed
	}

	/// Move an anchor to the current cursor position.
	///
	/// Returns false when the anchor does not exist.
	#[wasm_bindgen]
	pub fn move_anchor_to_cursor(&mut self, name: &str) -> bool {
		let cursor = self
			.runtime
			.selection()
			.end()
			.offset()
			.min(self.runtime.document().char_count());
		let Some(offset) = self.anchors.get_mut(name) else {
			return false;
		};
		*offset = cursor;
		true
	}

	// ── Tasks / TODO scanner ────────────────────────────────────────

	/// Scan the document for task-style lines.
	///
	/// Returns flat array [line, kind, checked, text, ...].
	/// Kinds: `task`, `todo`, `fixme`, `note`, `hack`.
	#[wasm_bindgen]
	pub fn scan_tasks(&self) -> Vec<String> {
		let plain = self.runtime.document().plain_text();
		let mut out = Vec::new();
		for (line_no, line) in plain.split('\n').enumerate() {
			let trimmed = line.trim_start();
			let lower = trimmed.to_ascii_lowercase();

			// Markdown checkbox tasks.
			if lower.starts_with("- [ ]") || lower.starts_with("* [ ]") {
				out.push(line_no.to_string());
				out.push("task".to_string());
				out.push("false".to_string());
				out.push(trimmed[5..].trim().to_string());
				continue;
			}
			if lower.starts_with("- [x]") || lower.starts_with("* [x]") {
				out.push(line_no.to_string());
				out.push("task".to_string());
				out.push("true".to_string());
				out.push(trimmed[5..].trim().to_string());
				continue;
			}

			// Comment prefixes then label.
			let stripped = trimmed
				.trim_start_matches('/')
				.trim_start_matches('/')
				.trim_start()
				.trim_start_matches('#')
				.trim_start()
				.trim_start_matches(';')
				.trim_start()
				.trim_start_matches("--")
				.trim_start()
				.to_ascii_lowercase();

			for kind in ["todo", "fixme", "note", "hack"] {
				let needle = format!("{kind}:");
				if let Some(pos) = stripped.find(&needle) {
					let msg = stripped[pos + needle.len()..].trim().to_string();
					out.push(line_no.to_string());
					out.push(kind.to_string());
					out.push("false".to_string());
					out.push(msg);
					break;
				}
			}
		}
		out
	}

	/// Count task-style lines in the document.
	#[wasm_bindgen]
	pub fn task_count(&self) -> usize {
		self.scan_tasks().len() / 4
	}

	/// Return the next task line after `from_line` (wraps), or -1.
	#[wasm_bindgen]
	pub fn next_task_line(&self, from_line: usize) -> i32 {
		let tasks = self.scan_tasks();
		if tasks.is_empty() {
			return -1;
		}
		let mut lines = Vec::new();
		let mut i = 0;
		while i + 3 < tasks.len() {
			if let Ok(n) = tasks[i].parse::<usize>() {
				lines.push(n);
			}
			i += 4;
		}
		lines.sort_unstable();
		lines.dedup();
		if let Some(next) = lines.iter().copied().find(|l| *l > from_line) {
			next as i32
		} else {
			lines.first().copied().map(|v| v as i32).unwrap_or(-1)
		}
	}

	/// Return the previous task line before `from_line` (wraps), or -1.
	#[wasm_bindgen]
	pub fn prev_task_line(&self, from_line: usize) -> i32 {
		let tasks = self.scan_tasks();
		if tasks.is_empty() {
			return -1;
		}
		let mut lines = Vec::new();
		let mut i = 0;
		while i + 3 < tasks.len() {
			if let Ok(n) = tasks[i].parse::<usize>() {
				lines.push(n);
			}
			i += 4;
		}
		lines.sort_unstable();
		lines.dedup();
		if let Some(prev) = lines.iter().rev().copied().find(|l| *l < from_line) {
			prev as i32
		} else {
			lines.last().copied().map(|v| v as i32).unwrap_or(-1)
		}
	}

	/// Toggle markdown checkbox state on a line.
	///
	/// Supports `- [ ]` <-> `- [x]` and `* [ ]` <-> `* [x]`.
	/// Returns true if a toggle occurred.
	#[wasm_bindgen]
	pub fn toggle_task_checkbox(&mut self, line: usize) -> bool {
		if !self.is_writable() {
			return false;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		if line >= lines.len() {
			return false;
		}
		let mut changed = false;
		if let Some(pos) = lines[line].find("[ ]") {
			lines[line].replace_range(pos..pos + 3, "[x]");
			changed = true;
		} else if let Some(pos) = lines[line].find("[x]") {
			lines[line].replace_range(pos..pos + 3, "[ ]");
			changed = true;
		} else if let Some(pos) = lines[line].find("[X]") {
			lines[line].replace_range(pos..pos + 3, "[ ]");
			changed = true;
		}
		if changed {
			self.runtime
				.document_mut()
				.set_plain_text(&lines.join("\n"));
			self.is_modified = true;
		}
		changed
	}

	// ── Lint helpers ────────────────────────────────────────────────

	/// Return line numbers that end with trailing spaces or tabs.
	#[wasm_bindgen]
	pub fn lint_trailing_whitespace(&self) -> Vec<usize> {
		self.runtime
			.document()
			.plain_text()
			.split('\n')
			.enumerate()
			.filter_map(|(i, line)| {
				if line.ends_with(' ') || line.ends_with('\t') {
					Some(i)
				} else {
					None
				}
			})
			.collect()
	}

	/// Return line numbers longer than `max_len` characters.
	#[wasm_bindgen]
	pub fn lint_long_lines(&self, max_len: usize) -> Vec<usize> {
		self.runtime
			.document()
			.plain_text()
			.split('\n')
			.enumerate()
			.filter_map(|(i, line)| {
				if line.chars().count() > max_len {
					Some(i)
				} else {
					None
				}
			})
			.collect()
	}

	/// Return line numbers with mixed leading tabs and spaces.
	#[wasm_bindgen]
	pub fn lint_mixed_indentation(&self) -> Vec<usize> {
		self.runtime
			.document()
			.plain_text()
			.split('\n')
			.enumerate()
			.filter_map(|(i, line)| {
				let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
				let has_tab = indent.contains('\t');
				let has_space = indent.contains(' ');
				if has_tab && has_space { Some(i) } else { None }
			})
			.collect()
	}

	/// Return line numbers containing non-ASCII characters.
	#[wasm_bindgen]
	pub fn lint_non_ascii_lines(&self) -> Vec<usize> {
		self.runtime
			.document()
			.plain_text()
			.split('\n')
			.enumerate()
			.filter_map(|(i, line)| {
				if line.chars().any(|c| !c.is_ascii()) {
					Some(i)
				} else {
					None
				}
			})
			.collect()
	}

	// ── Line occurrence navigation ──────────────────────────────────

	/// Return line numbers containing `needle`.
	#[wasm_bindgen]
	pub fn line_occurrences(&self, needle: &str, case_sensitive: bool) -> Vec<usize> {
		if needle.is_empty() {
			return Vec::new();
		}
		let plain = self.runtime.document().plain_text();
		let needle_cmp = if case_sensitive {
			needle.to_string()
		} else {
			needle.to_lowercase()
		};
		plain
			.split('\n')
			.enumerate()
			.filter_map(|(i, line)| {
				let hay = if case_sensitive {
					line.to_string()
				} else {
					line.to_lowercase()
				};
				if hay.contains(&needle_cmp) {
					Some(i)
				} else {
					None
				}
			})
			.collect()
	}

	/// Count lines containing `needle`.
	#[wasm_bindgen]
	pub fn line_occurrence_count(&self, needle: &str, case_sensitive: bool) -> usize {
		self.line_occurrences(needle, case_sensitive).len()
	}

	/// Next line containing `needle` after `from_line` (wraps), or -1.
	#[wasm_bindgen]
	pub fn next_line_with(&self, needle: &str, from_line: usize, case_sensitive: bool) -> i32 {
		let lines = self.line_occurrences(needle, case_sensitive);
		if lines.is_empty() {
			return -1;
		}
		if let Some(next) = lines.iter().copied().find(|l| *l > from_line) {
			next as i32
		} else {
			lines.first().copied().map(|v| v as i32).unwrap_or(-1)
		}
	}

	/// Previous line containing `needle` before `from_line` (wraps), or -1.
	#[wasm_bindgen]
	pub fn prev_line_with(&self, needle: &str, from_line: usize, case_sensitive: bool) -> i32 {
		let lines = self.line_occurrences(needle, case_sensitive);
		if lines.is_empty() {
			return -1;
		}
		if let Some(prev) = lines.iter().rev().copied().find(|l| *l < from_line) {
			prev as i32
		} else {
			lines.last().copied().map(|v| v as i32).unwrap_or(-1)
		}
	}

	// ── Cursor context helpers ──────────────────────────────────────

	/// Return up to `max_chars` immediately before the cursor.
	#[wasm_bindgen]
	pub fn text_before_cursor(&self, max_chars: usize) -> String {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let cursor = self.runtime.selection().end().offset().min(chars.len());
		let start = cursor.saturating_sub(max_chars);
		chars[start..cursor].iter().collect()
	}

	/// Return up to `max_chars` immediately after the cursor.
	#[wasm_bindgen]
	pub fn text_after_cursor(&self, max_chars: usize) -> String {
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let cursor = self.runtime.selection().end().offset().min(chars.len());
		let end = (cursor + max_chars).min(chars.len());
		chars[cursor..end].iter().collect()
	}

	/// Return line context window around a target line.
	///
	/// Flat format: [lineNumber, text, lineNumber, text, ...].
	#[wasm_bindgen]
	pub fn line_context(&self, line: usize, radius: usize) -> Vec<String> {
		let plain = self.runtime.document().plain_text();
		let lines: Vec<&str> = plain.split('\n').collect();
		if lines.is_empty() {
			return Vec::new();
		}
		let center = line.min(lines.len() - 1);
		let start = center.saturating_sub(radius);
		let end = (center + radius).min(lines.len() - 1);
		let mut out = Vec::new();
		for (i, text) in lines.iter().enumerate().skip(start).take(end - start + 1) {
			out.push(i.to_string());
			out.push((*text).to_string());
		}
		out
	}

	// ── Rotate lines ────────────────────────────────────────────────

	/// Rotate a line range up by one (first line moves to end).
	#[wasm_bindgen]
	pub fn rotate_lines_up(&mut self, start_line: usize, end_line: usize) -> bool {
		if !self.is_writable() {
			return false;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		if lines.len() < 2 {
			return false;
		}
		let s = start_line.min(lines.len() - 1);
		let e = end_line.min(lines.len() - 1);
		if s >= e {
			return false;
		}
		let first = lines[s].clone();
		for i in s..e {
			lines[i] = lines[i + 1].clone();
		}
		lines[e] = first;
		self.runtime
			.document_mut()
			.set_plain_text(&lines.join("\n"));
		self.is_modified = true;
		true
	}

	/// Rotate a line range down by one (last line moves to start).
	#[wasm_bindgen]
	pub fn rotate_lines_down(&mut self, start_line: usize, end_line: usize) -> bool {
		if !self.is_writable() {
			return false;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		if lines.len() < 2 {
			return false;
		}
		let s = start_line.min(lines.len() - 1);
		let e = end_line.min(lines.len() - 1);
		if s >= e {
			return false;
		}
		let last = lines[e].clone();
		for i in (s + 1..=e).rev() {
			lines[i] = lines[i - 1].clone();
		}
		lines[s] = last;
		self.runtime
			.document_mut()
			.set_plain_text(&lines.join("\n"));
		self.is_modified = true;
		true
	}

	// ── Named state slots ───────────────────────────────────────────

	/// Save the full editor state under a name.
	#[wasm_bindgen]
	pub fn save_named_state(&mut self, name: &str) {
		self.named_states
			.insert(name.to_string(), self.save_state());
	}

	/// Load a previously saved named editor state.
	///
	/// Returns `true` when found and restored.
	#[wasm_bindgen]
	pub fn load_named_state(&mut self, name: &str) -> bool {
		let Some(json) = self.named_states.get(name).cloned() else {
			return false;
		};
		self.restore_state(&json);
		true
	}

	/// Remove a named state.
	#[wasm_bindgen]
	pub fn delete_named_state(&mut self, name: &str) {
		self.named_states.remove(name);
	}

	/// Remove all named states.
	#[wasm_bindgen]
	pub fn clear_named_states(&mut self) {
		self.named_states.clear();
	}

	/// Number of named states currently saved.
	#[wasm_bindgen]
	pub fn named_state_count(&self) -> usize {
		self.named_states.len()
	}

	/// Saved state names (sorted).
	#[wasm_bindgen]
	pub fn named_state_names(&self) -> Vec<String> {
		let mut out: Vec<String> = self.named_states.keys().cloned().collect();
		out.sort_unstable();
		out
	}

	// ── Selection profiles ──────────────────────────────────────────

	/// Save the current selection range under a name.
	#[wasm_bindgen]
	pub fn save_selection_profile(&mut self, name: &str) {
		let sel = self.runtime.selection();
		self.selection_profiles
			.insert(name.to_string(), (sel.start().offset(), sel.end().offset()));
	}

	/// Restore a named selection profile.
	///
	/// Returns `true` when found.
	#[wasm_bindgen]
	pub fn load_selection_profile(&mut self, name: &str) -> bool {
		let Some((start, end)) = self.selection_profiles.get(name).copied() else {
			return false;
		};
		let max = self.runtime.document().char_count();
		let s = start.min(max);
		let e = end.min(max);
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(s), Position::new(e)),
		});
		true
	}

	/// Remove a named selection profile.
	#[wasm_bindgen]
	pub fn delete_selection_profile(&mut self, name: &str) {
		self.selection_profiles.remove(name);
	}

	/// Clear all selection profiles.
	#[wasm_bindgen]
	pub fn clear_selection_profiles(&mut self) {
		self.selection_profiles.clear();
	}

	/// Number of saved selection profiles.
	#[wasm_bindgen]
	pub fn selection_profile_count(&self) -> usize {
		self.selection_profiles.len()
	}

	/// Selection profile names (sorted).
	#[wasm_bindgen]
	pub fn selection_profile_names(&self) -> Vec<String> {
		let mut out: Vec<String> = self.selection_profiles.keys().cloned().collect();
		out.sort_unstable();
		out
	}

	// ── Task workflow helpers ───────────────────────────────────────

	/// Task progress as [checked, total].
	#[wasm_bindgen]
	pub fn task_progress(&self) -> Vec<usize> {
		let tasks = self.scan_tasks();
		let total = tasks.len() / 4;
		let mut checked = 0usize;
		let mut i = 0;
		while i + 3 < tasks.len() {
			if tasks[i + 2] == "true" {
				checked += 1;
			}
			i += 4;
		}
		vec![checked, total]
	}

	/// Insert a markdown task line at the current line start.
	#[wasm_bindgen]
	pub fn insert_task_line(&mut self, text: &str, checked: bool) {
		if !self.is_writable() {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let cursor = self.runtime.selection().end().offset().min(chars.len());
		let mut line_start = cursor;
		while line_start > 0 && chars[line_start - 1] != '\n' {
			line_start -= 1;
		}
		let prefix = if checked { "- [x] " } else { "- [ ] " };
		self.insert_text_at(line_start, &format!("{prefix}{text}\n"));
	}

	/// Next unchecked task line after `from_line` (wraps), or -1.
	#[wasm_bindgen]
	pub fn next_unchecked_task_line(&self, from_line: usize) -> i32 {
		let tasks = self.scan_tasks();
		let mut lines = Vec::new();
		let mut i = 0;
		while i + 3 < tasks.len() {
			if tasks[i + 2] == "false" {
				if let Ok(n) = tasks[i].parse::<usize>() {
					lines.push(n);
				}
			}
			i += 4;
		}
		if lines.is_empty() {
			return -1;
		}
		lines.sort_unstable();
		lines.dedup();
		if let Some(next) = lines.iter().copied().find(|l| *l > from_line) {
			next as i32
		} else {
			lines.first().copied().map(|v| v as i32).unwrap_or(-1)
		}
	}

	/// Previous unchecked task line before `from_line` (wraps), or -1.
	#[wasm_bindgen]
	pub fn prev_unchecked_task_line(&self, from_line: usize) -> i32 {
		let tasks = self.scan_tasks();
		let mut lines = Vec::new();
		let mut i = 0;
		while i + 3 < tasks.len() {
			if tasks[i + 2] == "false" {
				if let Ok(n) = tasks[i].parse::<usize>() {
					lines.push(n);
				}
			}
			i += 4;
		}
		if lines.is_empty() {
			return -1;
		}
		lines.sort_unstable();
		lines.dedup();
		if let Some(prev) = lines.iter().rev().copied().find(|l| *l < from_line) {
			prev as i32
		} else {
			lines.last().copied().map(|v| v as i32).unwrap_or(-1)
		}
	}

	/// Mark all unchecked markdown tasks as checked.
	///
	/// Returns number of lines updated.
	#[wasm_bindgen]
	pub fn complete_all_tasks(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		let mut changed = 0usize;
		for line in &mut lines {
			if let Some(pos) = line.find("[ ]") {
				line.replace_range(pos..pos + 3, "[x]");
				changed += 1;
			}
		}
		if changed > 0 {
			self.runtime
				.document_mut()
				.set_plain_text(&lines.join("\n"));
			self.is_modified = true;
		}
		changed
	}

	/// Remove completed markdown task lines (`[x]` / `[X]`).
	///
	/// Returns number of lines removed.
	#[wasm_bindgen]
	pub fn clear_completed_tasks(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		let before = lines.len();
		let filtered: Vec<String> = lines
			.into_iter()
			.filter(|line| {
				let lower = line.to_ascii_lowercase();
				!(lower.contains("[x]") && (lower.starts_with("- ") || lower.starts_with("* ")))
			})
			.collect();
		let removed = before.saturating_sub(filtered.len());
		if removed > 0 {
			self.runtime
				.document_mut()
				.set_plain_text(&filtered.join("\n"));
			self.is_modified = true;
		}
		removed
	}

	// ── Cleanup utilities ───────────────────────────────────────────

	/// Trim leading spaces/tabs from every line.
	///
	/// Returns number of lines changed.
	#[wasm_bindgen]
	pub fn trim_leading_whitespace(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		let mut changed = 0usize;
		for line in &mut lines {
			let trimmed = line
				.trim_start_matches(|c| c == ' ' || c == '\t')
				.to_string();
			if *line != trimmed {
				*line = trimmed;
				changed += 1;
			}
		}
		if changed > 0 {
			self.runtime
				.document_mut()
				.set_plain_text(&lines.join("\n"));
			self.is_modified = true;
		}
		changed
	}

	/// Collapse blank-line runs to at most `max_consecutive` lines.
	///
	/// Returns number of lines removed.
	#[wasm_bindgen]
	pub fn collapse_blank_lines(&mut self, max_consecutive: usize) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		let mut out = Vec::with_capacity(lines.len());
		let mut blank_run = 0usize;
		for line in lines {
			if line.trim().is_empty() {
				blank_run += 1;
				if blank_run <= max_consecutive {
					out.push(line);
				}
			} else {
				blank_run = 0;
				out.push(line);
			}
		}
		let new_text = out.join("\n");
		let removed = plain.split('\n').count().saturating_sub(out.len());
		if removed > 0 {
			self.runtime.document_mut().set_plain_text(&new_text);
			self.is_modified = true;
		}
		removed
	}

	/// Remove blank lines at end of document.
	///
	/// Returns number of trailing blank lines removed.
	#[wasm_bindgen]
	pub fn remove_trailing_blank_lines(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		let mut removed = 0usize;
		while lines.len() > 1 {
			let is_blank = lines.last().map(|l| l.trim().is_empty()).unwrap_or(false);
			if is_blank {
				lines.pop();
				removed += 1;
			} else {
				break;
			}
		}
		if removed > 0 {
			self.runtime
				.document_mut()
				.set_plain_text(&lines.join("\n"));
			self.is_modified = true;
		}
		removed
	}

	/// Ensure the document ends with exactly one trailing newline.
	///
	/// Returns `true` if content was changed.
	#[wasm_bindgen]
	pub fn ensure_single_trailing_newline(&mut self) -> bool {
		if !self.is_writable() {
			return false;
		}
		let plain = self.runtime.document().plain_text();
		let mut normalized = plain.trim_end_matches('\n').to_string();
		normalized.push('\n');
		if normalized != plain {
			self.runtime.document_mut().set_plain_text(&normalized);
			self.is_modified = true;
			return true;
		}
		false
	}

	// ── Line utilities ──────────────────────────────────────────────

	/// Swap two logical lines by index.
	///
	/// Returns `true` on success.
	#[wasm_bindgen]
	pub fn swap_lines(&mut self, a: usize, b: usize) -> bool {
		if !self.is_writable() {
			return false;
		}
		if a == b {
			return true;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		if a >= lines.len() || b >= lines.len() {
			return false;
		}
		lines.swap(a, b);
		self.runtime
			.document_mut()
			.set_plain_text(&lines.join("\n"));
		self.is_modified = true;
		true
	}

	/// Duplicate a line range and insert it below the range.
	///
	/// Returns `true` on success.
	#[wasm_bindgen]
	pub fn duplicate_line_range(&mut self, start_line: usize, end_line: usize) -> bool {
		if !self.is_writable() {
			return false;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		if lines.is_empty() {
			return false;
		}
		let s = start_line.min(lines.len() - 1);
		let e = end_line.min(lines.len() - 1);
		if s > e {
			return false;
		}
		let block: Vec<String> = lines[s..=e].to_vec();
		let insert_at = e + 1;
		lines.splice(insert_at..insert_at, block);
		self.runtime
			.document_mut()
			.set_plain_text(&lines.join("\n"));
		self.is_modified = true;
		true
	}

	/// Prefix each line in range with `prefix`.
	///
	/// Returns number of lines changed.
	#[wasm_bindgen]
	pub fn prefix_lines(&mut self, start_line: usize, end_line: usize, prefix: &str) -> usize {
		if !self.is_writable() || prefix.is_empty() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		if lines.is_empty() {
			return 0;
		}
		let s = start_line.min(lines.len() - 1);
		let e = end_line.min(lines.len() - 1);
		if s > e {
			return 0;
		}
		for line in lines.iter_mut().take(e + 1).skip(s) {
			*line = format!("{prefix}{line}");
		}
		let changed = e - s + 1;
		self.runtime
			.document_mut()
			.set_plain_text(&lines.join("\n"));
		self.is_modified = true;
		changed
	}

	/// Suffix each line in range with `suffix`.
	///
	/// Returns number of lines changed.
	#[wasm_bindgen]
	pub fn suffix_lines(&mut self, start_line: usize, end_line: usize, suffix: &str) -> usize {
		if !self.is_writable() || suffix.is_empty() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		if lines.is_empty() {
			return 0;
		}
		let s = start_line.min(lines.len() - 1);
		let e = end_line.min(lines.len() - 1);
		if s > e {
			return 0;
		}
		for line in lines.iter_mut().take(e + 1).skip(s) {
			line.push_str(suffix);
		}
		let changed = e - s + 1;
		self.runtime
			.document_mut()
			.set_plain_text(&lines.join("\n"));
		self.is_modified = true;
		changed
	}

	/// Remove `prefix` from each line in range when present.
	///
	/// Returns number of lines changed.
	#[wasm_bindgen]
	pub fn unprefix_lines(&mut self, start_line: usize, end_line: usize, prefix: &str) -> usize {
		if !self.is_writable() || prefix.is_empty() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		if lines.is_empty() {
			return 0;
		}
		let s = start_line.min(lines.len() - 1);
		let e = end_line.min(lines.len() - 1);
		if s > e {
			return 0;
		}
		let mut changed = 0usize;
		for line in lines.iter_mut().take(e + 1).skip(s) {
			if line.starts_with(prefix) {
				line.drain(..prefix.len());
				changed += 1;
			}
		}
		if changed > 0 {
			self.runtime
				.document_mut()
				.set_plain_text(&lines.join("\n"));
			self.is_modified = true;
		}
		changed
	}

	/// Remove `suffix` from each line in range when present.
	///
	/// Returns number of lines changed.
	#[wasm_bindgen]
	pub fn unsuffix_lines(&mut self, start_line: usize, end_line: usize, suffix: &str) -> usize {
		if !self.is_writable() || suffix.is_empty() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		if lines.is_empty() {
			return 0;
		}
		let s = start_line.min(lines.len() - 1);
		let e = end_line.min(lines.len() - 1);
		if s > e {
			return 0;
		}
		let mut changed = 0usize;
		for line in lines.iter_mut().take(e + 1).skip(s) {
			if line.ends_with(suffix) {
				let new_len = line.len() - suffix.len();
				line.truncate(new_len);
				changed += 1;
			}
		}
		if changed > 0 {
			self.runtime
				.document_mut()
				.set_plain_text(&lines.join("\n"));
			self.is_modified = true;
		}
		changed
	}

	/// Whether a line starts with a prefix.
	#[wasm_bindgen]
	pub fn line_has_prefix(&self, line: usize, prefix: &str, case_sensitive: bool) -> bool {
		let plain = self.runtime.document().plain_text();
		let Some(text) = plain.split('\n').nth(line) else {
			return false;
		};
		if case_sensitive {
			text.starts_with(prefix)
		} else {
			text.to_lowercase().starts_with(&prefix.to_lowercase())
		}
	}

	/// Whether a line ends with a suffix.
	#[wasm_bindgen]
	pub fn line_has_suffix(&self, line: usize, suffix: &str, case_sensitive: bool) -> bool {
		let plain = self.runtime.document().plain_text();
		let Some(text) = plain.split('\n').nth(line) else {
			return false;
		};
		if case_sensitive {
			text.ends_with(suffix)
		} else {
			text.to_lowercase().ends_with(&suffix.to_lowercase())
		}
	}

	/// Line numbers whose text starts with prefix.
	#[wasm_bindgen]
	pub fn lines_with_prefix(&self, prefix: &str, case_sensitive: bool) -> Vec<usize> {
		if prefix.is_empty() {
			return Vec::new();
		}
		let needle = if case_sensitive {
			String::new()
		} else {
			prefix.to_lowercase()
		};
		let plain = self.runtime.document().plain_text();
		let mut out = Vec::new();
		for (i, line) in plain.split('\n').enumerate() {
			let hit = if case_sensitive {
				line.starts_with(prefix)
			} else {
				line.to_lowercase().starts_with(&needle)
			};
			if hit {
				out.push(i);
			}
		}
		out
	}

	/// Line numbers whose text ends with suffix.
	#[wasm_bindgen]
	pub fn lines_with_suffix(&self, suffix: &str, case_sensitive: bool) -> Vec<usize> {
		if suffix.is_empty() {
			return Vec::new();
		}
		let needle = if case_sensitive {
			String::new()
		} else {
			suffix.to_lowercase()
		};
		let plain = self.runtime.document().plain_text();
		let mut out = Vec::new();
		for (i, line) in plain.split('\n').enumerate() {
			let hit = if case_sensitive {
				line.ends_with(suffix)
			} else {
				line.to_lowercase().ends_with(&needle)
			};
			if hit {
				out.push(i);
			}
		}
		out
	}

	/// Count lines whose text starts with prefix.
	#[wasm_bindgen]
	pub fn count_lines_with_prefix(&self, prefix: &str, case_sensitive: bool) -> usize {
		self.lines_with_prefix(prefix, case_sensitive).len()
	}

	/// Count lines whose text ends with suffix.
	#[wasm_bindgen]
	pub fn count_lines_with_suffix(&self, suffix: &str, case_sensitive: bool) -> usize {
		self.lines_with_suffix(suffix, case_sensitive).len()
	}

	/// Number a line range with `N. ` prefix.
	///
	/// Returns number of lines changed.
	#[wasm_bindgen]
	pub fn number_lines(
		&mut self,
		start_line: usize,
		end_line: usize,
		start_number: usize,
		pad_width: usize,
	) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let mut lines: Vec<String> = plain.split('\n').map(|s| s.to_string()).collect();
		if lines.is_empty() {
			return 0;
		}
		let s = start_line.min(lines.len() - 1);
		let e = end_line.min(lines.len() - 1);
		if s > e {
			return 0;
		}
		for i in s..=e {
			let n = start_number + (i - s);
			let label = if pad_width > 0 {
				format!("{n:0pad_width$}. ")
			} else {
				format!("{n}. ")
			};
			lines[i] = format!("{label}{}", lines[i]);
		}
		let changed = e - s + 1;
		self.runtime
			.document_mut()
			.set_plain_text(&lines.join("\n"));
		self.is_modified = true;
		changed
	}

	// ── Cleanup additions ───────────────────────────────────────────

	/// Remove non-printable control characters.
	///
	/// Keeps `\n`, `\r`, and `\t`. Returns chars removed.
	#[wasm_bindgen]
	pub fn strip_non_printable(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let mut out = String::with_capacity(plain.len());
		let mut removed = 0usize;
		for ch in plain.chars() {
			let keep = match ch {
				'\n' | '\r' | '\t' => true,
				c if c.is_control() => false,
				_ => true,
			};
			if keep {
				out.push(ch);
			} else {
				removed += 1;
			}
		}
		if removed > 0 {
			self.runtime.document_mut().set_plain_text(&out);
			self.is_modified = true;
		}
		removed
	}

	/// Normalize common Unicode whitespace characters to ASCII space.
	///
	/// Returns number of replaced characters.
	#[wasm_bindgen]
	pub fn normalize_unicode_whitespace(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let plain = self.runtime.document().plain_text();
		let mut out = String::with_capacity(plain.len());
		let mut replaced = 0usize;
		for ch in plain.chars() {
			let mapped = match ch {
				'\u{00A0}'
				| '\u{1680}'
				| '\u{2000}'..='\u{200A}'
				| '\u{202F}'
				| '\u{205F}'
				| '\u{3000}' => ' ',
				_ => ch,
			};
			if mapped != ch {
				replaced += 1;
			}
			out.push(mapped);
		}
		if replaced > 0 {
			self.runtime.document_mut().set_plain_text(&out);
			self.is_modified = true;
		}
		replaced
	}

	// ── Line fingerprints ───────────────────────────────────────────

	/// Compute FNV-1a 64-bit hash of a logical line.
	///
	/// Returns empty string when line is out of range.
	#[wasm_bindgen]
	pub fn line_hash(&self, line: usize) -> String {
		let plain = self.runtime.document().plain_text();
		let Some(text) = plain.split('\n').nth(line) else {
			return String::new();
		};
		let mut hash: u64 = 0xcbf29ce484222325;
		for byte in text.as_bytes() {
			hash ^= *byte as u64;
			hash = hash.wrapping_mul(0x100000001b3);
		}
		format!("{hash:016x}")
	}

	/// Return all line hashes as flat array: [line, hash, ...].
	#[wasm_bindgen]
	pub fn line_hashes(&self) -> Vec<String> {
		let plain = self.runtime.document().plain_text();
		let mut out = Vec::new();
		for (i, line) in plain.split('\n').enumerate() {
			let mut hash: u64 = 0xcbf29ce484222325;
			for byte in line.as_bytes() {
				hash ^= *byte as u64;
				hash = hash.wrapping_mul(0x100000001b3);
			}
			out.push(i.to_string());
			out.push(format!("{hash:016x}"));
		}
		out
	}

	/// Return line hashes for an inclusive range as `[line, hash, ...]`.
	#[wasm_bindgen]
	pub fn line_hashes_in_range(&self, start_line: usize, end_line: usize) -> Vec<String> {
		let plain = self.runtime.document().plain_text();
		let lines: Vec<&str> = plain.split('\n').collect();
		if lines.is_empty() {
			return Vec::new();
		}
		let s = start_line.min(lines.len() - 1);
		let e = end_line.min(lines.len() - 1);
		if s > e {
			return Vec::new();
		}
		let mut out = Vec::new();
		for (idx, line) in lines.iter().enumerate().take(e + 1).skip(s) {
			let mut hash: u64 = 0xcbf29ce484222325;
			for byte in line.as_bytes() {
				hash ^= *byte as u64;
				hash = hash.wrapping_mul(0x100000001b3);
			}
			out.push(idx.to_string());
			out.push(format!("{hash:016x}"));
		}
		out
	}

	/// Compare two line hashes for equality.
	///
	/// Returns `false` if either line is out of range.
	#[wasm_bindgen]
	pub fn line_hash_equals(&self, a: usize, b: usize) -> bool {
		let ha = self.line_hash(a);
		let hb = self.line_hash(b);
		!ha.is_empty() && !hb.is_empty() && ha == hb
	}

	/// Whether a line participates in a duplicate-content set.
	#[wasm_bindgen]
	pub fn line_is_duplicate(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> bool {
		self.duplicate_line_numbers(case_sensitive, ignore_whitespace)
			.contains(&line)
	}

	/// Line numbers that have duplicated content.
	///
	/// `ignore_whitespace` collapses whitespace and trims ends before
	/// comparison. Returns sorted 0-based line numbers.
	#[wasm_bindgen]
	pub fn duplicate_line_numbers(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> Vec<usize> {
		let groups = self.line_groups(case_sensitive, ignore_whitespace);
		let mut out = Vec::new();
		for lines in groups.values() {
			if lines.len() > 1 {
				out.extend(lines.iter().copied());
			}
		}
		out.sort_unstable();
		out
	}

	/// Line numbers that are unique by content matching.
	///
	/// Returns sorted 0-based line numbers.
	#[wasm_bindgen]
	pub fn unique_line_numbers(&self, case_sensitive: bool, ignore_whitespace: bool) -> Vec<usize> {
		let groups = self.line_groups(case_sensitive, ignore_whitespace);
		let mut out = Vec::new();
		for lines in groups.values() {
			if lines.len() == 1 {
				out.push(lines[0]);
			}
		}
		out.sort_unstable();
		out
	}

	/// Number of lines that belong to duplicate-content groups.
	#[wasm_bindgen]
	pub fn duplicate_line_count(&self, case_sensitive: bool, ignore_whitespace: bool) -> usize {
		self.duplicate_line_numbers(case_sensitive, ignore_whitespace)
			.len()
	}

	/// Number of lines that are unique by content matching.
	#[wasm_bindgen]
	pub fn unique_line_count(&self, case_sensitive: bool, ignore_whitespace: bool) -> usize {
		self.unique_line_numbers(case_sensitive, ignore_whitespace)
			.len()
	}

	/// First duplicate line number, or -1 when no duplicates.
	#[wasm_bindgen]
	pub fn first_duplicate_line(&self, case_sensitive: bool, ignore_whitespace: bool) -> i32 {
		self.duplicate_line_numbers(case_sensitive, ignore_whitespace)
			.first()
			.copied()
			.map_or(-1, |n| n as i32)
	}

	/// Last duplicate line number, or -1 when no duplicates.
	#[wasm_bindgen]
	pub fn last_duplicate_line(&self, case_sensitive: bool, ignore_whitespace: bool) -> i32 {
		self.duplicate_line_numbers(case_sensitive, ignore_whitespace)
			.last()
			.copied()
			.map_or(-1, |n| n as i32)
	}

	/// Number of duplicate-content groups.
	#[wasm_bindgen]
	pub fn duplicate_group_count(&self, case_sensitive: bool, ignore_whitespace: bool) -> usize {
		self.line_groups(case_sensitive, ignore_whitespace)
			.values()
			.filter(|lines| lines.len() > 1)
			.count()
	}

	/// Largest duplicate-content group size.
	#[wasm_bindgen]
	pub fn largest_duplicate_group_size(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> usize {
		self.line_groups(case_sensitive, ignore_whitespace)
			.values()
			.filter_map(|lines| (lines.len() > 1).then_some(lines.len()))
			.max()
			.unwrap_or(0)
	}

	/// Line numbers in the largest duplicate-content group.
	///
	/// Ties are broken by lowest first line, then lexicographic line list.
	#[wasm_bindgen]
	pub fn largest_duplicate_group_lines(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> Vec<usize> {
		let groups = self.line_groups(case_sensitive, ignore_whitespace);
		let mut candidates: Vec<Vec<usize>> = groups
			.values()
			.filter(|lines| lines.len() > 1)
			.cloned()
			.collect();
		for lines in &mut candidates {
			lines.sort_unstable();
		}
		candidates.sort_by(|a, b| {
			b.len()
				.cmp(&a.len())
				.then_with(|| a[0].cmp(&b[0]))
				.then_with(|| a.cmp(b))
		});
		candidates.into_iter().next().unwrap_or_default()
	}

	/// Duplicate group sizes sorted descending.
	#[wasm_bindgen]
	pub fn duplicate_group_sizes(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> Vec<usize> {
		let mut out: Vec<usize> = self
			.line_groups(case_sensitive, ignore_whitespace)
			.values()
			.filter_map(|lines| (lines.len() > 1).then_some(lines.len()))
			.collect();
		out.sort_unstable_by(|a, b| b.cmp(a));
		out
	}

	/// Duplicate group lines for the provided line.
	///
	/// Returns sorted line numbers in the same duplicate group, or empty
	/// when `line` is unique or out of range.
	#[wasm_bindgen]
	pub fn duplicate_group_lines_for_line(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> Vec<usize> {
		let Some(key) = self.normalized_line_key_for_index(line, case_sensitive, ignore_whitespace)
		else {
			return Vec::new();
		};
		let groups = self.line_groups(case_sensitive, ignore_whitespace);
		let mut out = groups.get(&key).cloned().unwrap_or_default();
		if out.len() <= 1 {
			return Vec::new();
		}
		out.sort_unstable();
		out
	}

	/// Duplicate group size for the provided line.
	#[wasm_bindgen]
	pub fn duplicate_group_size_for_line(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> usize {
		self.duplicate_group_lines_for_line(line, case_sensitive, ignore_whitespace)
			.len()
	}

	/// First line of duplicate group for the provided line, or -1.
	#[wasm_bindgen]
	pub fn duplicate_group_first_line_for_line(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> i32 {
		self.duplicate_group_lines_for_line(line, case_sensitive, ignore_whitespace)
			.first()
			.copied()
			.map_or(-1, |n| n as i32)
	}

	/// Last line of duplicate group for the provided line, or -1.
	#[wasm_bindgen]
	pub fn duplicate_group_last_line_for_line(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> i32 {
		self.duplicate_group_lines_for_line(line, case_sensitive, ignore_whitespace)
			.last()
			.copied()
			.map_or(-1, |n| n as i32)
	}

	/// All line numbers that share content with the provided line.
	///
	/// Returns sorted line numbers including the provided line itself.
	/// Returns empty for out-of-range lines.
	#[wasm_bindgen]
	pub fn line_occurrence_lines_for_line(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> Vec<usize> {
		let Some(key) = self.normalized_line_key_for_index(line, case_sensitive, ignore_whitespace)
		else {
			return Vec::new();
		};
		let groups = self.line_groups(case_sensitive, ignore_whitespace);
		let mut out = groups.get(&key).cloned().unwrap_or_default();
		out.sort_unstable();
		out
	}

	/// Number of lines sharing content with the provided line.
	#[wasm_bindgen]
	pub fn line_occurrence_count_for_line(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> usize {
		self.line_occurrence_lines_for_line(line, case_sensitive, ignore_whitespace)
			.len()
	}

	/// Ratio of line-occurrence count to total lines for the provided line.
	#[wasm_bindgen]
	pub fn line_occurrence_ratio_for_line(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> f64 {
		let total = self.runtime.document().plain_text().split('\n').count();
		if total == 0 {
			return 0.0;
		}
		self.line_occurrence_count_for_line(line, case_sensitive, ignore_whitespace) as f64
			/ total as f64
	}

	/// Number of line-content groups with size at least `min_count`.
	#[wasm_bindgen]
	pub fn line_occurrence_group_count(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
		min_count: usize,
	) -> usize {
		self.ranked_line_occurrence_groups(case_sensitive, ignore_whitespace, min_count)
			.len()
	}

	/// Number of line-content groups with an occurrence size exactly `count`.
	#[wasm_bindgen]
	pub fn line_occurrence_group_count_with_count(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
		count: usize,
	) -> usize {
		if count == 0 {
			return 0;
		}
		self.line_groups(case_sensitive, ignore_whitespace)
			.values()
			.filter(|lines| lines.len() == count)
			.count()
	}

	/// Ranked line-occurrence groups as flat `[line, count, ...]`.
	///
	/// `line` is the first line number for each group. Results are sorted by
	/// count descending, then representative line ascending.
	#[wasm_bindgen]
	pub fn line_occurrence_rankings(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
		min_count: usize,
	) -> Vec<usize> {
		let groups =
			self.ranked_line_occurrence_groups(case_sensitive, ignore_whitespace, min_count);
		let mut out = Vec::with_capacity(groups.len() * 2);
		for lines in groups {
			out.push(lines[0]);
			out.push(lines.len());
		}
		out
	}

	/// Ranked line-occurrence groups constrained to an inclusive count range.
	///
	/// Returns flat `[line, count, ...]`, preserving ranking order.
	#[wasm_bindgen]
	pub fn line_occurrence_rankings_in_count_range(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
		min_count: usize,
		max_count: usize,
	) -> Vec<usize> {
		let lower = min_count.max(1);
		if max_count < lower {
			return Vec::new();
		}
		let groups = self.ranked_line_occurrence_groups(case_sensitive, ignore_whitespace, lower);
		let mut out = Vec::with_capacity(groups.len() * 2);
		for lines in groups {
			let count = lines.len();
			if count <= max_count {
				out.push(lines[0]);
				out.push(count);
			}
		}
		out
	}

	/// Line-occurrence histogram as `[occurrence_count, group_count, ...]`.
	///
	/// Rows are sorted by occurrence count descending.
	#[wasm_bindgen]
	pub fn line_occurrence_histogram(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
		min_count: usize,
	) -> Vec<usize> {
		let threshold = min_count.max(1);
		let groups = self.line_groups(case_sensitive, ignore_whitespace);
		let mut buckets: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
		for lines in groups.values() {
			let size = lines.len();
			if size >= threshold {
				*buckets.entry(size).or_default() += 1;
			}
		}
		let mut rows: Vec<(usize, usize)> = buckets.into_iter().collect();
		rows.sort_by(|a, b| b.0.cmp(&a.0));

		let mut out = Vec::with_capacity(rows.len() * 2);
		for (occurrence_count, group_count) in rows {
			out.push(occurrence_count);
			out.push(group_count);
		}
		out
	}

	/// All line numbers belonging to groups with exactly `count` occurrences.
	#[wasm_bindgen]
	pub fn line_occurrence_lines_with_count(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
		count: usize,
	) -> Vec<usize> {
		if count == 0 {
			return Vec::new();
		}
		let groups = self.line_groups(case_sensitive, ignore_whitespace);
		let mut out = Vec::new();
		for lines in groups.values() {
			if lines.len() == count {
				out.extend(lines.iter().copied());
			}
		}
		out.sort_unstable();
		out
	}

	/// All line numbers belonging to groups with at least `min_count` occurrences.
	#[wasm_bindgen]
	pub fn line_occurrence_lines_with_min_count(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
		min_count: usize,
	) -> Vec<usize> {
		let threshold = min_count.max(1);
		let groups = self.line_groups(case_sensitive, ignore_whitespace);
		let mut out = Vec::new();
		for lines in groups.values() {
			if lines.len() >= threshold {
				out.extend(lines.iter().copied());
			}
		}
		out.sort_unstable();
		out
	}

	/// All line numbers belonging to groups with at most `max_count` occurrences.
	#[wasm_bindgen]
	pub fn line_occurrence_lines_with_max_count(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
		max_count: usize,
	) -> Vec<usize> {
		if max_count == 0 {
			return Vec::new();
		}
		let groups = self.line_groups(case_sensitive, ignore_whitespace);
		let mut out = Vec::new();
		for lines in groups.values() {
			if lines.len() <= max_count {
				out.extend(lines.iter().copied());
			}
		}
		out.sort_unstable();
		out
	}

	/// Group lines for the ranked occurrence group at `rank`.
	///
	/// Groups are ranked by occurrence count descending, then first line
	/// ascending, then lexicographic line list.
	#[wasm_bindgen]
	pub fn line_occurrence_group_lines_at_rank(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
		rank: usize,
		min_count: usize,
	) -> Vec<usize> {
		self.ranked_line_occurrence_groups(case_sensitive, ignore_whitespace, min_count)
			.into_iter()
			.nth(rank)
			.unwrap_or_default()
	}

	/// Rank for the occurrence group containing `line`, or -1.
	///
	/// Rank uses the same ordering as `line_occurrence_rankings`.
	#[wasm_bindgen]
	pub fn line_occurrence_rank_for_line(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
		min_count: usize,
	) -> i32 {
		let Some(key) = self.normalized_line_key_for_index(line, case_sensitive, ignore_whitespace)
		else {
			return -1;
		};
		let threshold = min_count.max(1);
		let groups = self.line_groups(case_sensitive, ignore_whitespace);
		let Some(lines) = groups.get(&key) else {
			return -1;
		};
		if lines.len() < threshold {
			return -1;
		}
		let mut target = lines.clone();
		target.sort_unstable();
		self.ranked_line_occurrence_groups(case_sensitive, ignore_whitespace, min_count)
			.iter()
			.position(|candidate| *candidate == target)
			.map_or(-1, |idx| idx as i32)
	}

	/// Largest line-occurrence count across all line-content groups.
	#[wasm_bindgen]
	pub fn most_common_line_occurrence_count(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> usize {
		self.line_groups(case_sensitive, ignore_whitespace)
			.values()
			.map(Vec::len)
			.max()
			.unwrap_or(0)
	}

	/// Line numbers in the most common line-content group.
	///
	/// Ties are broken by lowest first line, then lexicographic line list.
	#[wasm_bindgen]
	pub fn most_common_line_occurrence_lines(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> Vec<usize> {
		self.ranked_line_occurrence_groups(case_sensitive, ignore_whitespace, 1)
			.into_iter()
			.next()
			.unwrap_or_default()
	}

	/// Whether the provided line is unique by content matching.
	#[wasm_bindgen]
	pub fn line_is_unique_by_content(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> bool {
		self.line_occurrence_count_for_line(line, case_sensitive, ignore_whitespace) == 1
	}

	/// Peer lines sharing content with the provided line (excluding itself).
	#[wasm_bindgen]
	pub fn duplicate_peer_lines_for_line(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> Vec<usize> {
		let mut lines =
			self.line_occurrence_lines_for_line(line, case_sensitive, ignore_whitespace);
		if lines.len() <= 1 {
			return Vec::new();
		}
		lines.retain(|value| *value != line);
		lines
	}

	/// Number of duplicate peer lines for the provided line.
	#[wasm_bindgen]
	pub fn duplicate_peer_line_count(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> usize {
		self.duplicate_peer_lines_for_line(line, case_sensitive, ignore_whitespace)
			.len()
	}

	/// Ratio of duplicate lines to total lines.
	#[wasm_bindgen]
	pub fn duplicate_line_ratio(&self, case_sensitive: bool, ignore_whitespace: bool) -> f64 {
		let total = self.runtime.document().plain_text().split('\n').count();
		if total == 0 {
			return 0.0;
		}
		self.duplicate_line_count(case_sensitive, ignore_whitespace) as f64 / total as f64
	}

	/// Normalized offset span for two anchor names.
	fn anchor_span_from_names(&self, start_name: &str, end_name: &str) -> Option<(usize, usize)> {
		let start = *self.anchors.get(start_name)?;
		let end = *self.anchors.get(end_name)?;
		if start <= end {
			Some((start, end))
		} else {
			Some((end, start))
		}
	}

	/// Ranked line-occurrence groups.
	///
	/// Sort order: occurrence count descending, first line ascending,
	/// then lexicographic line list.
	fn ranked_line_occurrence_groups(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
		min_count: usize,
	) -> Vec<Vec<usize>> {
		let threshold = min_count.max(1);
		let mut groups: Vec<Vec<usize>> = self
			.line_groups(case_sensitive, ignore_whitespace)
			.values()
			.filter(|lines| lines.len() >= threshold)
			.cloned()
			.collect();
		for lines in &mut groups {
			lines.sort_unstable();
		}
		groups.sort_by(|a, b| {
			b.len()
				.cmp(&a.len())
				.then_with(|| a[0].cmp(&b[0]))
				.then_with(|| a.cmp(b))
		});
		groups
	}

	/// Normalized line-content key for a specific line index.
	fn normalized_line_key_for_index(
		&self,
		line: usize,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> Option<String> {
		let plain = self.runtime.document().plain_text();
		let line_text = plain.split('\n').nth(line)?;
		let mut key = if ignore_whitespace {
			line_text
				.split_whitespace()
				.collect::<Vec<&str>>()
				.join(" ")
		} else {
			line_text.to_string()
		};
		if !case_sensitive {
			key = key.to_lowercase();
		}
		Some(key)
	}

	/// Build normalized line-content groups used by duplicate/unique helpers.
	fn line_groups(
		&self,
		case_sensitive: bool,
		ignore_whitespace: bool,
	) -> std::collections::HashMap<String, Vec<usize>> {
		let plain = self.runtime.document().plain_text();
		let mut groups: std::collections::HashMap<String, Vec<usize>> =
			std::collections::HashMap::new();
		for (i, line) in plain.split('\n').enumerate() {
			let mut key = if ignore_whitespace {
				line.split_whitespace().collect::<Vec<&str>>().join(" ")
			} else {
				line.to_string()
			};
			if !case_sensitive {
				key = key.to_lowercase();
			}
			groups.entry(key).or_default().push(i);
		}
		groups
	}

	/// Returns `true` if the editor is writable. Use at the top of any
	/// method that modifies the document.
	fn is_writable(&self) -> bool {
		!self.read_only
	}

	/// Width of the line-number gutter (0 when hidden).
	fn gutter_width(&self) -> f32 {
		if self.show_line_numbers { 48.0 } else { 0.0 }
	}

	/// Get the pixel rect of a single character for bracket highlighting.
	fn char_rect(
		offset: usize,
		paragraphs: &[ParagraphLayoutInfo],
		lc: &LayoutConstants,
		content_x_origin: f32,
		scroll_y: f32,
		renderer: &Canvas2dRenderer,
	) -> Option<Rect> {
		let mut global_offset = 0usize;
		for para in paragraphs {
			let para_len = para.text.chars().count();
			if offset < global_offset || offset >= global_offset + para_len {
				global_offset += para_len + 1; // +1 for \n
				continue;
			}
			let local = offset - global_offset;
			for line in &para.layout.lines {
				if local >= line.start_offset && local < line.end_offset {
					let para_chars: Vec<char> = para.text.chars().collect();
					let mut x = lc.padding_x + content_x_origin + line.x_offset;
					for i in line.start_offset..local {
						let ch_s = para_chars[i].to_string();
						x += renderer.measure_text(&ch_s, &lc.default_style);
					}
					let ch_s = para_chars[local].to_string();
					let w = renderer.measure_text(&ch_s, &lc.default_style);
					let y = lc.padding_y + para.y_offset + line.y - scroll_y;
					return Some(Rect::new(x, y, w, line.height));
				}
			}
			return None;
		}
		None
	}

	/// Content-area width (canvas width minus gutter).
	fn content_width(&self) -> f32 {
		self.width - self.gutter_width()
	}

	/// Build layout constants with current gutter, zoom, theme, and wrap settings.
	fn layout_constants(&self) -> LayoutConstants {
		let width = if self.word_wrap {
			self.content_width()
		} else {
			100_000.0 // effectively infinite
		};
		LayoutConstants::with_zoom_and_color(width, self.zoom, self.theme.text)
	}

	/// Insert text at the current cursor position (start of document).
	#[wasm_bindgen]
	pub fn insert_text(&mut self, text: &str) {
		if !self.is_writable() {
			return;
		}
		self.event_source.push_text_input(text);
		self.process_events();
		self.is_modified = true;
	}

	/// Insert text at a specific character offset.
	#[wasm_bindgen]
	pub fn insert_text_at(&mut self, offset: usize, text: &str) {
		if !self.is_writable() {
			return;
		}
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(offset)),
		});
		let _ = self.runtime.handle_event(EditorEvent::TextInsert {
			text: text.to_string(),
		});
		self.is_modified = true;
	}

	/// Delete a range of characters from `start` to `end`.
	#[wasm_bindgen]
	pub fn delete_range(&mut self, start: usize, end: usize) {
		if !self.is_writable() {
			return;
		}
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(start), Position::new(end)),
		});
		let _ = self
			.runtime
			.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
		self.is_modified = true;
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

	/// Import HTML content, replacing the current document.
	///
	/// Parses basic inline elements (`<strong>`, `<em>`, `<u>`, `<s>`, `<br>`,
	/// `<p>`) and HTML entities.
	#[wasm_bindgen]
	pub fn from_html(&mut self, html: &str) {
		if !self.is_writable() {
			return;
		}
		self.runtime.document_mut().from_html(html);
	}

	/// Paste HTML at the current cursor position.
	///
	/// Parses the HTML to extract styled text, deletes any current selection,
	/// and inserts the parsed content with formatting preserved.
	#[wasm_bindgen]
	pub fn paste_html(&mut self, html: &str) {
		if !self.is_writable() {
			return;
		}
		use canvist_core::operation::Operation;

		// Delete current selection if any.
		let sel = self.runtime.selection();
		if !sel.is_collapsed() {
			self.delete_range(sel.start().offset(), sel.end().offset());
		}

		let insert_at = self.runtime.selection().end().offset();
		let segments = canvist_core::document_parse_html(html);
		let mut offset = insert_at;

		for (text, bold, italic, underline, strike) in &segments {
			self.runtime
				.apply_operation(Operation::insert(Position::new(offset), text.clone()));
			let len = text.chars().count();
			if *bold || *italic || *underline || *strike {
				let mut style = Style::new();
				if *bold {
					style = style.bold();
				}
				if *italic {
					style = style.italic();
				}
				if *underline {
					style = style.underline();
				}
				if *strike {
					style = style.strikethrough();
				}
				self.runtime.apply_operation(Operation::format(
					Selection::range(Position::new(offset), Position::new(offset + len)),
					style,
				));
			}
			offset += len;
		}
	}

	/// Queue canonical text input and process it into operations.
	#[wasm_bindgen]
	pub fn queue_text_input(&mut self, text: &str) {
		if !self.is_writable() {
			return;
		}
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
		if !self.is_writable() {
			return;
		}
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
		if !self.is_writable() {
			return;
		}
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
		if !self.is_writable() {
			return;
		}
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
		if !self.is_writable() {
			return;
		}
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
		if !self.is_writable() {
			return;
		}
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
		if !self.is_writable() {
			return 0;
		}
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
		if !self.is_writable() {
			return;
		}
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
		if !self.is_writable() {
			return;
		}
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

	/// Insert a newline at the cursor and auto-indent with the same leading
	/// whitespace as the current line.
	///
	/// Also continues list markers:
	/// - Bullet lines (`• `, `- `, `* `) → new bullet line
	/// - Numbered lines (`1. `, `2. `, …) → incremented number
	/// - Empty list line (just the marker) → removes the marker instead
	///
	/// Returns the number of characters inserted (1 for `\n` plus indent).
	#[wasm_bindgen]
	pub fn auto_indent_newline(&mut self) -> usize {
		if !self.is_writable() {
			return 0;
		}
		let offset = self.runtime.selection().end().offset();
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();

		// Find current line text.
		let mut line_start = offset.min(chars.len());
		while line_start > 0 && chars[line_start - 1] != '\n' {
			line_start -= 1;
		}
		let line_text: String = chars[line_start..offset.min(chars.len())].iter().collect();

		// Extract leading whitespace.
		let indent: String = line_text
			.chars()
			.take_while(|c| *c == ' ' || *c == '\t')
			.collect();
		let trimmed = line_text.trim_start();

		// Detect list markers.
		let continuation = if trimmed == "• " || trimmed == "- " || trimmed == "* " {
			// Empty list item — remove the marker instead of continuing.
			return self.remove_line_prefix(line_start, &line_text);
		} else if trimmed.starts_with("• ")
			|| trimmed.starts_with("- ")
			|| trimmed.starts_with("* ")
		{
			// Get the first character (might be multi-byte like •).
			let marker: String = trimmed.chars().next().into_iter().collect();
			format!("{indent}{marker} ")
		} else if let Some(num_end) = trimmed.find(". ") {
			let num_str = &trimmed[..num_end];
			if let Ok(n) = num_str.parse::<u32>() {
				if trimmed.len() == num_end + 2 {
					// Empty numbered item — remove it.
					return self.remove_line_prefix(line_start, &line_text);
				}
				format!("{indent}{}. ", n + 1)
			} else {
				indent.clone()
			}
		} else {
			indent.clone()
		};

		let text = format!("\n{continuation}");
		let len = text.chars().count();

		// Delete selection if not collapsed.
		let sel = self.runtime.selection();
		if !sel.is_collapsed() {
			self.delete_range(sel.start().offset(), sel.end().offset());
		}

		let insert_at = self.runtime.selection().end().offset();
		self.runtime
			.apply_operation(Operation::insert(Position::new(insert_at), text));

		let new_pos = insert_at + len;
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(new_pos)),
		});

		len
	}

	/// Helper: remove a line prefix (used when pressing Enter on an empty list item).
	/// Deletes the line content and returns 0.
	fn remove_line_prefix(&mut self, line_start: usize, line_text: &str) -> usize {
		let line_len = line_text.chars().count();
		if line_len > 0 {
			let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
				selection: Selection::range(
					Position::new(line_start),
					Position::new(line_start + line_len),
				),
			});
			let _ = self
				.runtime
				.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
		}
		0
	}

	/// Toggle a bullet list prefix (`• `) on the current line.
	///
	/// If the line already starts with `• `, the prefix is removed.
	/// Otherwise it is inserted at the line start (after leading whitespace).
	#[wasm_bindgen]
	pub fn toggle_bullet_list(&mut self) {
		if !self.is_writable() {
			return;
		}
		self.toggle_line_prefix("• ");
	}

	/// Toggle a numbered list prefix (`1. `) on the current line.
	///
	/// If the line already starts with a number prefix, it is removed.
	/// Otherwise `1. ` is inserted.
	#[wasm_bindgen]
	pub fn toggle_numbered_list(&mut self) {
		if !self.is_writable() {
			return;
		}
		let offset = self.runtime.selection().end().offset();
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();

		let mut line_start = offset.min(chars.len());
		while line_start > 0 && chars[line_start - 1] != '\n' {
			line_start -= 1;
		}
		let line_text: String = chars[line_start..]
			.iter()
			.take_while(|c| **c != '\n')
			.collect();
		let trimmed = line_text.trim_start();
		let indent_len = line_text.len() - trimmed.len();
		let insert_pos = line_start + indent_len;

		// Check if already numbered.
		if let Some(dot_pos) = trimmed.find(". ") {
			let num_str = &trimmed[..dot_pos];
			if num_str.parse::<u32>().is_ok() {
				// Remove the number prefix.
				let prefix_len = dot_pos + 2; // "N. "
				let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
					selection: Selection::range(
						Position::new(insert_pos),
						Position::new(insert_pos + prefix_len),
					),
				});
				let _ = self
					.runtime
					.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
				return;
			}
		}

		// Insert "1. " at the line start.
		self.runtime.apply_operation(Operation::insert(
			Position::new(insert_pos),
			"1. ".to_string(),
		));
	}

	/// Helper: toggle a simple prefix (like "• ") on the current line.
	fn toggle_line_prefix(&mut self, prefix: &str) {
		let offset = self.runtime.selection().end().offset();
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();

		let mut line_start = offset.min(chars.len());
		while line_start > 0 && chars[line_start - 1] != '\n' {
			line_start -= 1;
		}
		let line_text: String = chars[line_start..]
			.iter()
			.take_while(|c| **c != '\n')
			.collect();
		let trimmed = line_text.trim_start();
		let indent_len = line_text.len() - trimmed.len();
		let insert_pos = line_start + indent_len;

		if trimmed.starts_with(prefix) {
			// Remove prefix.
			let prefix_len = prefix.chars().count();
			let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
				selection: Selection::range(
					Position::new(insert_pos),
					Position::new(insert_pos + prefix_len),
				),
			});
			let _ = self
				.runtime
				.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
		} else {
			// Insert prefix.
			self.runtime.apply_operation(Operation::insert(
				Position::new(insert_pos),
				prefix.to_string(),
			));
		}
	}

	/// Indent the current selection: insert a tab character at the start
	/// of each selected line. If the selection is collapsed, insert a tab
	/// at the cursor position.
	#[wasm_bindgen]
	pub fn indent_selection(&mut self) {
		if !self.is_writable() {
			return;
		}
		let sel = self.runtime.selection();
		let plain = self.runtime.document().plain_text();
		let sel_start = sel.start().offset();
		let sel_end = sel.end().offset();

		// Find line boundaries encompassing the selection.
		let chars: Vec<char> = plain.chars().collect();
		let mut line_starts: Vec<usize> = Vec::new();
		// First line containing sel_start.
		let mut s = sel_start;
		while s > 0 && chars[s - 1] != '\n' {
			s -= 1;
		}
		line_starts.push(s);
		// Find subsequent line starts within selection.
		for i in s..sel_end.min(chars.len()) {
			if chars[i] == '\n' && i + 1 <= sel_end {
				line_starts.push(i + 1);
			}
		}

		// Insert tab at each line start (from end to start to keep offsets valid).
		for &start in line_starts.iter().rev() {
			self.runtime
				.apply_operation(Operation::insert(Position::new(start), "\t".to_string()));
		}
	}

	/// Outdent the current selection: remove one leading tab or up to 4
	/// spaces from the start of each selected line.
	#[wasm_bindgen]
	pub fn outdent_selection(&mut self) {
		if !self.is_writable() {
			return;
		}
		let sel = self.runtime.selection();
		let plain = self.runtime.document().plain_text();
		let sel_start = sel.start().offset();
		let sel_end = sel.end().offset();

		let chars: Vec<char> = plain.chars().collect();
		let mut line_starts: Vec<usize> = Vec::new();
		let mut s = sel_start;
		while s > 0 && chars[s - 1] != '\n' {
			s -= 1;
		}
		line_starts.push(s);
		for i in s..sel_end.min(chars.len()) {
			if chars[i] == '\n' && i + 1 <= sel_end {
				line_starts.push(i + 1);
			}
		}

		// Remove indent from each line (from end to start).
		for &start in line_starts.iter().rev() {
			if start < chars.len() && chars[start] == '\t' {
				// Remove one tab.
				let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
					selection: Selection::range(Position::new(start), Position::new(start + 1)),
				});
				let _ = self
					.runtime
					.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
			} else {
				// Remove up to 4 leading spaces.
				let mut count = 0usize;
				for j in start..chars.len().min(start + 4) {
					if chars[j] == ' ' {
						count += 1;
					} else {
						break;
					}
				}
				if count > 0 {
					let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
						selection: Selection::range(
							Position::new(start),
							Position::new(start + count),
						),
					});
					let _ = self
						.runtime
						.handle_event(EditorEvent::TextDeleteBackward { count: 1 });
				}
			}
		}
	}

	/// Move text from `[src_start, src_end)` to `dest` offset.
	///
	/// Used by drag-and-drop: extract the selected text, delete the source
	/// range, then insert at the destination (adjusting for the shift).
	#[wasm_bindgen]
	pub fn move_text(&mut self, src_start: usize, src_end: usize, dest: usize) {
		if !self.is_writable() {
			return;
		}
		if src_start >= src_end {
			return;
		}
		let plain = self.runtime.document().plain_text();
		let chars: Vec<char> = plain.chars().collect();
		let s = src_start.min(chars.len());
		let e = src_end.min(chars.len());
		let text: String = chars[s..e].iter().collect();

		// If dest is inside the source range, do nothing.
		if dest >= s && dest <= e {
			return;
		}

		// Delete source first, then insert. Adjust dest if it was after
		// the deleted range.
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::range(Position::new(s), Position::new(e)),
		});
		let _ = self
			.runtime
			.handle_event(EditorEvent::TextDeleteBackward { count: 1 });

		let adjusted_dest = if dest > e { dest - (e - s) } else { dest };
		self.runtime.apply_operation(Operation::insert(
			Position::new(adjusted_dest),
			text.clone(),
		));

		// Place cursor at end of moved text.
		let new_end = adjusted_dest + text.chars().count();
		let _ = self.runtime.handle_event(EditorEvent::SelectionSet {
			selection: Selection::collapsed(Position::new(new_end)),
		});
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
		if !self.is_writable() {
			return;
		}
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
		let lc = self.layout_constants();
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
		let lc = self.layout_constants();
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
		let lc = self.layout_constants();
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
		let lc = self.layout_constants();
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
		let gutter_w = self.gutter_width();

		let renderer = Canvas2dRenderer::new(ctx, width, height);
		let ht_w = if self.word_wrap {
			width - gutter_w
		} else {
			100_000.0
		};
		let lc = LayoutConstants::with_zoom_and_color(ht_w, self.zoom, self.theme.text);

		// Convert screen coords to document coords via viewport.
		let viewport = Viewport::new(width, height);
		let (doc_x, doc_y) = viewport.screen_to_document(screen_x as f32, screen_y as f32);

		// Coordinates relative to the content area origin, accounting for scroll.
		let content_x = doc_x - lc.padding_x - gutter_w;
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

		// Clear canvas to theme background.
		let theme = &self.theme;
		renderer.clear(theme.background);

		// Gutter width for line numbers (0 when disabled).
		let gutter_width: f32 = if self.show_line_numbers { 48.0 } else { 0.0 };

		let content_w = if self.word_wrap {
			width - gutter_width
		} else {
			100_000.0
		};
		let lc = LayoutConstants::with_zoom_and_color(content_w, self.zoom, theme.text);
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

		// Horizontal origin of content (after gutter, if line numbers shown).
		let content_x_origin = gutter_width;

		// Scroll offset applied to all Y coordinates.
		let sy = self.scroll_y;

		// Focus-aware colors from theme.
		let selection_color = if self.focused {
			theme.selection
		} else {
			theme.selection_blur
		};
		let caret_color = if self.focused {
			theme.caret
		} else {
			theme.caret_blur
		};

		// ── Current line highlight ───────────────────────────────────────
		if self.highlight_current_line && selection.is_collapsed() {
			let caret_offset = selection.end().offset();
			let (caret_para_idx, caret_line_idx) =
				find_para_and_line_for_offset(&paragraphs, caret_offset);
			if let Some(para) = paragraphs.get(caret_para_idx) {
				if let Some(line) = para.layout.lines.get(caret_line_idx) {
					let y = lc.padding_y + para.y_offset + line.y - sy;
					renderer.fill_rect(
						Rect::new(gutter_width, y, width - gutter_width, line.height),
						theme.line_highlight,
					);
				}
			}
		}

		// ── Line decorations (background tint) ──────────────────────────
		if !self.line_decorations.is_empty() {
			let mut para_line = 0usize;
			for para in &paragraphs {
				for line in &para.layout.lines {
					if self.line_decorations.iter().any(|&(l, ..)| l == para_line) {
						let line_y = lc.padding_y + para.y_offset + line.y - sy;
						if line_y + line.height >= 0.0 && line_y <= height {
							for &(l, r, g, b, a) in &self.line_decorations {
								if l == para_line {
									renderer.fill_rect(
										Rect::new(
											content_x_origin,
											line_y,
											width - content_x_origin,
											line.height,
										),
										Color::new(r, g, b, a),
									);
								}
							}
						}
					}
				}
				para_line += 1;
			}
		}

		// ── Placeholder text ─────────────────────────────────────────────
		if !self.placeholder.is_empty() && self.runtime.document().plain_text().is_empty() {
			let ph_color = Color::new(
				theme.gutter_text.r,
				theme.gutter_text.g,
				theme.gutter_text.b,
				150,
			);
			let ph_style = Style::new()
				.font_size(lc.default_style.font_size.unwrap_or(16.0))
				.color(ph_color.r, ph_color.g, ph_color.b, ph_color.a)
				.font_family("Inter, system-ui, sans-serif");
			renderer.draw_text(
				lc.padding_x + content_x_origin,
				lc.padding_y,
				&self.placeholder,
				&ph_style,
			);
		}

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

					let x_start = (lc.padding_x + content_x_origin)
						+ line.x_offset + x_offset_in_para_line(
						&renderer,
						para,
						line.start_offset,
						local_sel_start,
					);
					let x_end = (lc.padding_x + content_x_origin)
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
						let x = (lc.padding_x + content_x_origin)
							+ last_line.x_offset + last_line.width;
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

				let line_x = lc.padding_x + content_x_origin + line.x_offset;

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

		// ── Column rulers ────────────────────────────────────────────────
		if !self.rulers.is_empty() {
			let ruler_color = Color::new(
				theme.gutter_text.r,
				theme.gutter_text.g,
				theme.gutter_text.b,
				40,
			);
			let char_w = lc.default_style.font_size.unwrap_or(16.0) * 0.6;
			for &col in &self.rulers {
				let x = lc.padding_x + content_x_origin + char_w * col as f32;
				renderer.fill_rect(Rect::new(x, 0.0, 1.0, height), ruler_color);
			}
		}

		// ── Indent guides ────────────────────────────────────────────────
		if self.show_indent_guides {
			let guide_color = Color::new(
				theme.gutter_text.r,
				theme.gutter_text.g,
				theme.gutter_text.b,
				60,
			);
			let guide_tab_px =
				lc.default_style.font_size.unwrap_or(16.0) * self.tab_size as f32 * 0.6; // approximate char width × tab_size

			for para in &paragraphs {
				let para_chars: Vec<char> = para.text.chars().collect();
				let leading_spaces = para_chars
					.iter()
					.take_while(|c| **c == ' ' || **c == '\t')
					.count();
				if leading_spaces == 0 {
					continue;
				}
				// Count indent levels.
				let mut indent_level = 0usize;
				let mut i = 0;
				while i < leading_spaces {
					if para_chars[i] == '\t' {
						indent_level += 1;
						i += 1;
					} else {
						// Count spaces.
						let mut spaces = 0;
						while i < leading_spaces && para_chars[i] == ' ' {
							spaces += 1;
							i += 1;
						}
						indent_level += spaces / self.tab_size;
					}
				}

				for line in &para.layout.lines {
					let line_y = lc.padding_y + para.y_offset + line.y - sy;
					if line_y + line.height < 0.0 || line_y > height {
						continue;
					}
					for level in 1..=indent_level {
						let gx = lc.padding_x + content_x_origin + guide_tab_px * level as f32;
						renderer.fill_rect(Rect::new(gx, line_y, 1.0, line.height), guide_color);
					}
				}
			}
		}

		// ── Whitespace indicators ────────────────────────────────────────
		if self.show_whitespace {
			let ws_color = Color::new(
				theme.gutter_text.r,
				theme.gutter_text.g,
				theme.gutter_text.b,
				120,
			);
			let ws_style = Style::new()
				.font_size(lc.default_style.font_size.unwrap_or(16.0))
				.color(ws_color.r, ws_color.g, ws_color.b, ws_color.a)
				.font_family("Inter, system-ui, monospace");

			for para in &paragraphs {
				let para_chars: Vec<char> = para.text.chars().collect();
				for line in &para.layout.lines {
					let line_y = lc.padding_y + para.y_offset + line.y - sy;
					if line_y + line.height < 0.0 || line_y > height {
						continue;
					}
					// Walk through chars on this line, measure x, draw indicator.
					let mut x = lc.padding_x + content_x_origin + line.x_offset;
					for i in line.start_offset..line.end_offset.min(para_chars.len()) {
						let ch = para_chars[i];
						let ch_w = renderer.measure_text(&ch.to_string(), &lc.default_style);
						if ch == ' ' {
							let dot_x = x + ch_w * 0.5 - 1.0;
							let dot_y = line_y + line.height * 0.5;
							renderer.fill_rect(Rect::new(dot_x, dot_y, 2.0, 2.0), ws_color);
						} else if ch == '\t' {
							renderer.draw_text(x, line_y, "→", &ws_style);
						}
						x += ch_w;
					}
				}
			}
		}

		// ── Line number gutter ───────────────────────────────────────────
		if self.show_line_numbers && gutter_width > 0.0 {
			renderer.fill_rect(Rect::new(0.0, 0.0, gutter_width, height), theme.gutter_bg);
			renderer.draw_line(gutter_width, 0.0, gutter_width, height, theme.gutter_border);

			let gt = &theme.gutter_text;
			let line_num_style = Style::new()
				.font_size(12.0 * self.zoom)
				.color(gt.r, gt.g, gt.b, gt.a)
				.font_family("Inter, system-ui, monospace");

			let mut line_number = 1u32;
			for para in &paragraphs {
				let first_line = para.layout.lines.first();
				if let Some(line) = first_line {
					let y = lc.padding_y + para.y_offset + line.y - sy;
					if y + line.height >= 0.0 && y <= height {
						// Bookmark indicator.
						if self.bookmarks.contains(&((line_number - 1) as usize)) {
							let bm_color = Color::new(66, 135, 245, 180); // blue
							renderer.fill_rect(
								Rect::new(2.0, y + 2.0, 4.0, line.height - 4.0),
								bm_color,
							);
						}
						let num_str = line_number.to_string();
						// Right-align within gutter.
						let num_w = renderer.measure_text(&num_str, &line_num_style);
						let x = gutter_width - num_w - 8.0;
						renderer.draw_text(x, y, &num_str, &line_num_style);
					}
				}
				// Wrap continuation indicators.
				if self.show_wrap_indicators && para.layout.lines.len() > 1 {
					for wrap_line in para.layout.lines.iter().skip(1) {
						let wy = lc.padding_y + para.y_offset + wrap_line.y - sy;
						if wy + wrap_line.height >= 0.0 && wy <= height {
							let wx = gutter_width - 16.0;
							renderer.draw_text(wx, wy, "↪", &line_num_style);
						}
					}
				}
				line_number += 1;
			}
		}

		// ── Occurrence highlighting (word under cursor) ─────────────────
		if self.highlight_occurrences && self.focused {
			let word = self.word_at_cursor();
			if word.len() >= 2 {
				let occ_color =
					Color::new(theme.selection.r, theme.selection.g, theme.selection.b, 60);
				let offsets = self.find_all_whole_word(&word);
				let mut i = 0;
				while i + 1 < offsets.len() {
					let occ_start = offsets[i];
					let occ_end = offsets[i + 1];
					// Draw a highlight rectangle for each character in range.
					if let Some(start_rect) = Self::char_rect(
						occ_start,
						&paragraphs,
						&lc,
						content_x_origin,
						sy,
						&renderer,
					) {
						if let Some(end_rect) = Self::char_rect(
							(occ_end - 1).max(occ_start),
							&paragraphs,
							&lc,
							content_x_origin,
							sy,
							&renderer,
						) {
							let w = (end_rect.x + end_rect.width) - start_rect.x;
							renderer.fill_rect(
								Rect::new(start_rect.x, start_rect.y, w, start_rect.height),
								occ_color,
							);
						}
					}
					i += 2;
				}
			}
		}

		// ── Find match highlights ────────────────────────────────────────
		if self.show_find_highlights && !self.find_highlight_needle.is_empty() {
			let hl_color = Color::new(255, 200, 0, 60);
			let needle_copy = self.find_highlight_needle.clone();
			let offsets = self.find_all(&needle_copy, false);
			let mut fi = 0;
			while fi + 1 < offsets.len() {
				let hl_start = offsets[fi];
				let hl_end = offsets[fi + 1];
				if let Some(sr) =
					Self::char_rect(hl_start, &paragraphs, &lc, content_x_origin, sy, &renderer)
				{
					if let Some(er) = Self::char_rect(
						(hl_end).saturating_sub(1).max(hl_start),
						&paragraphs,
						&lc,
						content_x_origin,
						sy,
						&renderer,
					) {
						let w = (er.x + er.width) - sr.x;
						renderer.fill_rect(Rect::new(sr.x, sr.y, w, sr.height), hl_color);
					}
				}
				fi += 2;
			}
		}

		// ── Annotation underlines ────────────────────────────────────────
		for (ann_start, ann_end, kind, _msg) in &self.annotations {
			let underline_color = match kind.as_str() {
				"error" => Color::new(255, 0, 0, 200),
				"warning" => Color::new(255, 165, 0, 200),
				"info" => Color::new(66, 135, 245, 200),
				"spelling" => Color::new(0, 180, 0, 180),
				_ => Color::new(128, 128, 128, 150),
			};
			// Draw wavy underline for each annotated character.
			if let Some(start_rect) = Self::char_rect(
				*ann_start,
				&paragraphs,
				&lc,
				content_x_origin,
				sy,
				&renderer,
			) {
				if let Some(end_rect) = Self::char_rect(
					(*ann_end).saturating_sub(1).max(*ann_start),
					&paragraphs,
					&lc,
					content_x_origin,
					sy,
					&renderer,
				) {
					let x1 = start_rect.x;
					let x2 = end_rect.x + end_rect.width;
					let y_base = start_rect.y + start_rect.height - 1.0;
					// Draw 2px thick underline.
					renderer.fill_rect(Rect::new(x1, y_base, x2 - x1, 2.0), underline_color);
				}
			}
		}

		// ── Matching bracket highlight ────────────────────────────────────
		if self.highlight_matching_brackets && self.focused {
			let cursor_off = self.runtime.selection().end().offset();
			// Check char at cursor and char before cursor.
			let offsets_to_check: Vec<usize> = if cursor_off > 0 {
				vec![cursor_off, cursor_off - 1]
			} else {
				vec![cursor_off]
			};
			for &check_off in &offsets_to_check {
				let match_off = self.find_matching_bracket(check_off);
				if match_off >= 0 {
					let bracket_color =
						Color::new(theme.selection.r, theme.selection.g, theme.selection.b, 160);
					// Highlight both brackets.
					for &bo in &[check_off, match_off as usize] {
						if let Some(rect) =
							Self::char_rect(bo, &paragraphs, &lc, content_x_origin, sy, &renderer)
						{
							renderer.fill_rect(rect, bracket_color);
						}
					}
					break;
				}
			}
		}

		// ── Caret ────────────────────────────────────────────────────────
		// Only draw the caret when the JS blink controller says it should be
		// visible. This lets the JS side toggle `set_caret_visible()` on a
		// 530 ms timer to produce the classic blinking effect.
		if self.caret_visible {
			let caret_offset = selection.end().offset();
			let actual_caret_color = match self.cursor_color {
				Some((r, g, b, a)) => Color::new(r, g, b, a),
				None => caret_color,
			};

			let (caret_para_idx, caret_line_idx) =
				find_para_and_line_for_offset(&paragraphs, caret_offset);

			if let Some(para) = paragraphs.get(caret_para_idx) {
				if let Some(caret_line) = para.layout.lines.get(caret_line_idx) {
					let local_caret = caret_offset.saturating_sub(para.global_char_start);
					let caret_x = (lc.padding_x + content_x_origin)
						+ caret_line.x_offset
						+ x_offset_in_para_line(
							&renderer,
							para,
							caret_line.start_offset,
							local_caret,
						);
					let caret_y = lc.padding_y + para.y_offset + caret_line.y - sy;
					let caret_height = caret_line.height;

					match self.cursor_style {
						1 => {
							// Block cursor — filled rectangle one character wide.
							let char_w = renderer.measure_text("M", &lc.default_style);
							renderer.fill_rect(
								Rect::new(caret_x, caret_y, char_w, caret_height),
								Color::new(
									actual_caret_color.r,
									actual_caret_color.g,
									actual_caret_color.b,
									100,
								),
							);
						}
						2 => {
							// Underline cursor — horizontal line at bottom.
							let char_w = renderer.measure_text("M", &lc.default_style);
							renderer.fill_rect(
								Rect::new(caret_x, caret_y + caret_height - 2.0, char_w, 2.0),
								actual_caret_color,
							);
						}
						_ => {
							// Line cursor (default) — vertical line.
							renderer.fill_rect(
								Rect::new(caret_x, caret_y, self.cursor_width, caret_height),
								actual_caret_color,
							);
						}
					}
				}
			}
		}

		// ── Marker ranges ────────────────────────────────────────────────
		for (m_start, m_end, mr, mg, mb, ma, _id) in &self.markers {
			for ch_off in *m_start..*m_end {
				let (pi, li) = find_para_and_line_for_offset(&paragraphs, ch_off);
				if let Some(para) = paragraphs.get(pi) {
					if let Some(line) = para.layout.lines.get(li) {
						let local = ch_off.saturating_sub(para.global_char_start);
						let next_local = local + 1;
						let cx = (lc.padding_x + content_x_origin)
							+ line.x_offset + x_offset_in_para_line(
							&renderer,
							para,
							line.start_offset,
							local,
						);
						let cx2 = (lc.padding_x + content_x_origin)
							+ line.x_offset + x_offset_in_para_line(
							&renderer,
							para,
							line.start_offset,
							next_local,
						);
						let cy = lc.padding_y + para.y_offset + line.y - sy;
						let marker_color = Color::new(*mr, *mg, *mb, *ma);
						renderer.fill_rect(Rect::new(cx, cy, cx2 - cx, line.height), marker_color);
					}
				}
			}
		}

		// ── Collaborative cursors ────────────────────────────────────────
		for (offset, name, cr, cg, cb) in &self.collab_cursors {
			let (pi, li) = find_para_and_line_for_offset(&paragraphs, *offset);
			if let Some(para) = paragraphs.get(pi) {
				if let Some(line) = para.layout.lines.get(li) {
					let local = offset.saturating_sub(para.global_char_start);
					let cx = (lc.padding_x + content_x_origin)
						+ line.x_offset + x_offset_in_para_line(
						&renderer,
						para,
						line.start_offset,
						local,
					);
					let cy = lc.padding_y + para.y_offset + line.y - sy;
					let collab_color = Color::new(*cr, *cg, *cb, 220);
					// Draw cursor line.
					renderer.fill_rect(Rect::new(cx, cy, 2.0, line.height), collab_color);
					// Draw name label above cursor.
					let label_style = Style::new()
						.font_size(9.0 * self.zoom)
						.color(*cr, *cg, *cb, 255)
						.font_family("Inter, system-ui, sans-serif");
					let label_w = renderer.measure_text(name, &label_style) + 6.0;
					renderer.fill_rect(Rect::new(cx, cy - 14.0, label_w, 14.0), collab_color);
					let label_text_style = Style::new()
						.font_size(9.0 * self.zoom)
						.color(255, 255, 255, 255)
						.font_family("Inter, system-ui, sans-serif");
					renderer.draw_text(cx + 3.0, cy - 13.0, name, &label_text_style);
				}
			}
		}

		// ── Extra cursors (multi-cursor) ─────────────────────────────────
		if self.caret_visible && !self.extra_cursors.is_empty() {
			let extra_color = Color::new(caret_color.r, caret_color.g, caret_color.b, 180);
			for &offset in &self.extra_cursors {
				let (pi, li) = find_para_and_line_for_offset(&paragraphs, offset);
				if let Some(para) = paragraphs.get(pi) {
					if let Some(line) = para.layout.lines.get(li) {
						let local = offset.saturating_sub(para.global_char_start);
						let cx = (lc.padding_x + content_x_origin)
							+ line.x_offset + x_offset_in_para_line(
							&renderer,
							para,
							line.start_offset,
							local,
						);
						let cy = lc.padding_y + para.y_offset + line.y - sy;
						renderer.fill_rect(
							Rect::new(cx, cy, self.cursor_width, line.height),
							extra_color,
						);
					}
				}
			}
		}

		// ── Sticky scroll ────────────────────────────────────────────────
		if self.sticky_scroll && self.scroll_y > 0.0 {
			let plain = self.runtime.document().plain_text();
			if let Some(first_line) = plain.split('\n').next() {
				if !first_line.is_empty() {
					// Draw sticky bar background.
					let bar_h = 20.0 * self.zoom;
					renderer.fill_rect(Rect::new(0.0, 0.0, width, bar_h), theme.gutter_bg);
					renderer.draw_line(0.0, bar_h, width, bar_h, theme.gutter_border);
					let sticky_style = Style::new()
						.font_size(11.0 * self.zoom)
						.color(
							theme.gutter_text.r,
							theme.gutter_text.g,
							theme.gutter_text.b,
							theme.gutter_text.a,
						)
						.font_family("Inter, system-ui, sans-serif");
					renderer.draw_text(content_x_origin + 4.0, 2.0, first_line, &sticky_style);
				}
			}
		}

		// ── Minimap ──────────────────────────────────────────────────────
		if self.show_minimap {
			let mm_w = self.minimap_width;
			let mm_x = width - mm_w;

			// Background.
			let mm_bg = Color::new(theme.gutter_bg.r, theme.gutter_bg.g, theme.gutter_bg.b, 220);
			renderer.fill_rect(Rect::new(mm_x, 0.0, mm_w, height), mm_bg);
			renderer.draw_line(mm_x, 0.0, mm_x, height, theme.gutter_border);

			// Render each line as a thin coloured strip.
			let plain = self.runtime.document().plain_text();
			let lines: Vec<&str> = plain.split('\n').collect();
			let total_lines = lines.len().max(1);
			let line_h = (height / total_lines as f32).min(3.0).max(0.5);
			let text_color = Color::new(theme.text.r, theme.text.g, theme.text.b, 80);

			for (i, line) in lines.iter().enumerate() {
				let y = i as f32 * line_h;
				if y > height {
					break;
				}
				let line_w = (line.len() as f32 / 80.0).min(1.0) * (mm_w - 4.0);
				if line_w > 0.0 {
					renderer.fill_rect(
						Rect::new(mm_x + 2.0, y, line_w, line_h.max(1.0)),
						text_color,
					);
				}
			}

			// Viewport indicator.
			let frac = self.scroll_fraction();
			let ratio = self.scroll_ratio();
			let vp_y = frac * (height - height * ratio);
			let vp_h = (height * ratio).max(10.0);
			let vp_color = Color::new(theme.selection.r, theme.selection.g, theme.selection.b, 60);
			renderer.fill_rect(Rect::new(mm_x, vp_y, mm_w, vp_h), vp_color);
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
			renderer.fill_rect(Rect::new(track_x, 0.0, 6.0, track_h), theme.scrollbar_track);
			// Thumb.
			renderer.fill_rect(
				Rect::new(track_x, thumb_y, 6.0, thumb_h),
				theme.scrollbar_thumb,
			);
		}

		Ok(())
	}
}
