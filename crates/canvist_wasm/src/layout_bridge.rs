//! Layout bridge — shared layout computation between hit-testing and rendering.
//!
//! This module extracts the paragraph layout pipeline and related helpers that
//! are used by both `render()` and `hit_test()` so they stay in sync.

use canvist_core::Color;
use canvist_core::Style;
use canvist_core::layout::{LayoutConfig, LayoutLine, TextFragment, TextMeasure, layout_paragraph};


pub(crate) struct LayoutConstants {
	/// Horizontal padding from canvas edge to content area (pixels).
	pub(crate) padding_x: f32,
	/// Vertical padding from canvas edge to content area (pixels).
	pub(crate) padding_y: f32,
	/// Default text style applied when no explicit styling is present.
	pub(crate) default_style: Style,
	/// Layout engine configuration derived from the above (embeds `max_width`
	/// computed from the canvas width and padding).
	pub(crate) layout_config: LayoutConfig,
}

impl LayoutConstants {
	/// Create layout constants for a canvas of the given pixel dimensions.
	#[allow(dead_code)]
	pub(crate) fn new(canvas_width: f32) -> Self {
		Self::with_zoom(canvas_width, 1.0)
	}

	/// Create layout constants with a specific zoom level and text colour.
	#[allow(dead_code)]
	pub(crate) fn with_zoom(canvas_width: f32, zoom: f32) -> Self {
		Self::with_zoom_and_color(canvas_width, zoom, Color::BLACK)
	}

	/// Create layout constants with zoom and explicit text colour.
	pub(crate) fn with_zoom_and_color(canvas_width: f32, zoom: f32, text_color: Color) -> Self {
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
pub(crate) const PARAGRAPH_SPACING: f32 = 8.0;

/// Computed layout for a single paragraph within a multi-paragraph document.
pub(crate) struct ParagraphLayoutInfo {
	/// The paragraph layout (line breaks, widths, heights).
	pub(crate) layout: canvist_core::layout::ParagraphLayout,
	/// Global character offset where this paragraph starts in the document.
	pub(crate) global_char_start: usize,
	/// Character count of this paragraph (excluding the `\n` separator).
	pub(crate) char_count: usize,
	/// Vertical offset of this paragraph's first line within the content area.
	pub(crate) y_offset: f32,
	/// Styled runs belonging to this paragraph, with offsets relative to the
	/// paragraph (not the document). Each entry is
	/// `(text, style, paragraph-local offset, char count)`.
	pub(crate) local_runs: Vec<(String, Style, usize, usize)>,
	/// The paragraph's plain text (no `\n`).
	pub(crate) text: String,
	/// Block type of this paragraph (Body, Heading1, etc.).
	pub(crate) block_type: canvist_core::BlockType,
}

/// Split the document into per-paragraph data and lay out each one.
///
/// Returns a vec of [`ParagraphLayoutInfo`] structs, one per paragraph. The
/// `\n` characters that separate paragraphs in `plain_text` are consumed as
/// boundaries but are not part of any paragraph's text or layout.
pub(crate) fn layout_paragraphs(
	plain_text: &str,
	styled_runs: &[(String, Style, usize, usize)],
	layout_config: &LayoutConfig,
	measurer: &dyn TextMeasure,
	default_style: &Style,
	block_types: Option<&[canvist_core::BlockType]>,
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

		// Determine block type for this paragraph.
		let bt = block_types
			.and_then(|bts| bts.get(para_idx).copied())
			.unwrap_or(canvist_core::BlockType::Body);

		// For headings, override the layout config with a larger font size.
		let para_layout_config = if bt != canvist_core::BlockType::Body {
			let heading_size = bt.default_font_size() * (layout_config.default_style.font_size.unwrap_or(16.0) / 16.0);
			let heading_style = layout_config.default_style.clone().font_size(heading_size);
			LayoutConfig {
				max_width: layout_config.max_width,
				default_style: heading_style,
				text_align: layout_config.text_align,
			}
		} else {
			layout_config.clone()
		};

		let layout = layout_paragraph(&fragments, &para_layout_config, measurer);

		result.push(ParagraphLayoutInfo {
			layout,
			global_char_start: para_start,
			char_count: para_char_count,
			y_offset,
			local_runs,
			text: para_text.to_string(),
			block_type: bt,
		});

		// Advance y_offset for next paragraph.
		let para_height = result
			.last()
			.map(|p| p.layout.total_height)
			.unwrap_or(0.0);
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
pub(crate) fn find_para_and_line_for_offset(
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
pub(crate) fn build_fragments<'a>(
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
pub(crate) fn hit_x_on_line_ext(
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
