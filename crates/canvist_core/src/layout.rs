//! Text layout computation.
//!
//! This module handles line breaking and paragraph layout. It takes styled text
//! runs and computes the positions where lines should wrap, producing a list of
//! [`LayoutLine`]s that the renderer can draw.
//!
//! # Architecture
//!
//! The layout engine is intentionally decoupled from rendering. It produces
//! abstract geometry (widths, heights, offsets) that any backend can consume.

use serde::Deserialize;
use serde::Serialize;

use crate::Style;

/// Configuration for how text should be laid out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
	/// Maximum width available for text in logical pixels.
	pub max_width: f32,

	/// Default style applied when a run has no explicit style.
	pub default_style: Style,

	/// Text alignment for this layout pass.
	///
	/// Controls the horizontal positioning of each line within the available
	/// width. Defaults to [`TextAlign::Left`].
	pub text_align: crate::style::TextAlign,
}

impl Default for LayoutConfig {
	fn default() -> Self {
		Self {
			max_width: 800.0,
			default_style: Style::new(),
			text_align: crate::style::TextAlign::Left,
		}
	}
}

impl LayoutConfig {
	/// Create a new layout config with the given maximum width.
	#[must_use]
	pub fn new(max_width: f32) -> Self {
		Self {
			max_width,
			..Default::default()
		}
	}
}

/// A run of text to be laid out, with its associated style.
#[derive(Debug, Clone)]
pub struct TextFragment<'a> {
	/// The text content of this fragment.
	pub text: &'a str,
	/// The resolved style for this fragment.
	pub style: &'a Style,
}

/// A single laid-out line of text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutLine {
	/// Character offset where this line starts (relative to the paragraph).
	pub start_offset: usize,
	/// Character offset where this line ends (exclusive).
	pub end_offset: usize,
	/// Width of this line in logical pixels.
	pub width: f32,
	/// Height of this line in logical pixels.
	pub height: f32,
	/// Vertical offset from the top of the paragraph.
	pub y: f32,
	/// Horizontal offset from the left edge, used for center/right alignment.
	///
	/// For left-aligned text this is `0.0`. For center alignment it is
	/// `(max_width - line.width) / 2.0`, and for right alignment it is
	/// `max_width - line.width`.
	pub x_offset: f32,
}

/// The result of laying out a paragraph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParagraphLayout {
	/// The lines computed for this paragraph.
	pub lines: Vec<LayoutLine>,
	/// Total height of the paragraph in logical pixels.
	pub total_height: f32,
}

/// Compute the layout for a sequence of text fragments.
///
/// This is a simplified line-breaking algorithm that uses Unicode line break
/// opportunities. A production implementation would integrate with a proper
/// text shaper (e.g. `HarfBuzz`) for accurate glyph widths.
///
/// # Arguments
///
/// - `fragments` — ordered text fragments with styles
/// - `config` — layout constraints
/// - `measurer` — text measurement backend for accurate glyph widths
///
/// # Returns
///
/// A [`ParagraphLayout`] with computed line breaks and geometry.
pub fn layout_paragraph(
	fragments: &[TextFragment<'_>],
	config: &LayoutConfig,
	measurer: &dyn TextMeasure,
) -> ParagraphLayout {
	// Concatenate all fragment text for line-break analysis.
	let full_text: String = fragments.iter().map(|f| f.text).collect();

	if full_text.is_empty() {
		return ParagraphLayout {
			lines: vec![LayoutLine {
				start_offset: 0,
				end_offset: 0,
				width: 0.0,
				height: default_line_height(config),
				y: 0.0,
				x_offset: 0.0,
			}],
			total_height: default_line_height(config),
		};
	}

	let line_height = default_line_height(config);

	let mut lines = Vec::new();
	let mut line_start = 0usize;
	let mut y = 0.0f32;

	let chars: Vec<char> = full_text.chars().collect();
	let total_chars = chars.len();

	while line_start < total_chars {
		// Walk forward, measuring character widths until we exceed max_width.
		let mut line_width = 0.0f32;
		let mut tentative_end = line_start;
		let mut last_space = None;

		while tentative_end < total_chars {
			let ch = chars[tentative_end];
			let style = fragment_style_at(fragments, tentative_end);
			let char_width = measurer.measure_char(ch, style);

			if line_width + char_width > config.max_width && tentative_end > line_start {
				break;
			}

			line_width += char_width;
			tentative_end += 1;

			if ch == ' ' {
				last_space = Some(tentative_end);
			}
		}

		// Try to break at a space if we're not at the end.
		let line_end = if tentative_end < total_chars {
			if let Some(break_at) = last_space {
				break_at
			} else {
				// No space found — force break at measured width.
				tentative_end
			}
		} else {
			tentative_end
		};

		// Measure the final line width accurately.
		let width: f32 = (line_start..line_end)
			.map(|i| {
				let style = fragment_style_at(fragments, i);
				measurer.measure_char(chars[i], style)
			})
			.sum();

		// Compute horizontal offset for text alignment.
		let x_offset: f32 = match config.text_align {
			crate::style::TextAlign::Center => (config.max_width - width).max(0.0) / 2.0,
			crate::style::TextAlign::Right => (config.max_width - width).max(0.0),
			// Left and Justify both start at x=0 (justify spacing is future work).
			_ => 0.0,
		};

		lines.push(LayoutLine {
			start_offset: line_start,
			end_offset: line_end,
			width,
			height: line_height,
			y,
			x_offset,
		});

		y += line_height;
		line_start = line_end;
	}

	ParagraphLayout {
		total_height: y,
		lines,
	}
}

/// Trait for measuring text width.
///
/// Backends like `Canvas2D` `measureText` or `HarfBuzz` implement this to
/// provide accurate glyph widths to the layout engine.
pub trait TextMeasure {
	/// Measure the width of a single character in the given style.
	fn measure_char(&self, ch: char, style: &Style) -> f32;

	/// Measure the width of a string in the given style.
	fn measure_text(&self, text: &str, style: &Style) -> f32 {
		text.chars().map(|ch| self.measure_char(ch, style)).sum()
	}
}

/// Heuristic text measurer using font size × 0.6 as average character width.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeuristicTextMeasure;

impl TextMeasure for HeuristicTextMeasure {
	fn measure_char(&self, _ch: char, style: &Style) -> f32 {
		let resolved = style.resolve();
		resolved.font_size * 0.6
	}
}

/// Map a point (x, y) to a character offset within a laid-out paragraph.
///
/// Walks through the layout lines to find which line the y coordinate falls on,
/// then walks character-by-character using the measurer to find the closest
/// character offset.
pub fn hit_test_point(
	x: f32,
	y: f32,
	layout: &ParagraphLayout,
	fragments: &[TextFragment<'_>],
	measurer: &dyn TextMeasure,
) -> usize {
	if layout.lines.is_empty() {
		return 0;
	}

	// Find which line the y coordinate falls on.
	let line = layout
		.lines
		.iter()
		.find(|l| y >= l.y && y < l.y + l.height)
		.unwrap_or(layout.lines.last().unwrap());

	let line_start = line.start_offset;
	let line_end = line.end_offset;

	if line_start >= line_end {
		return line.start_offset;
	}

	// Build a flat list of (char, style) for the line range.
	let styled_chars: Vec<(char, &Style)> = {
		let full_text: String = fragments.iter().map(|f| f.text).collect();
		let chars: Vec<char> = full_text.chars().collect();
		(line_start..line_end)
			.map(|i| {
				let style = fragment_style_at(fragments, i);
				(chars[i], style)
			})
			.collect()
	};

	// Walk character by character, accumulating widths.
	let mut accumulated = 0.0f32;
	for (i, &(ch, style)) in styled_chars.iter().enumerate() {
		let char_width = measurer.measure_char(ch, style);
		let midpoint = accumulated + char_width * 0.5;

		if x < midpoint {
			return line_start + i;
		}

		accumulated += char_width;
	}

	line_end
}

/// Find which line a character offset falls on.
///
/// Returns the line index. If the offset equals a line boundary, it belongs
/// to the line starting there (not the one ending).
#[must_use]
pub fn line_index_for_offset(layout: &ParagraphLayout, offset: usize) -> usize {
	for (i, line) in layout.lines.iter().enumerate() {
		if offset < line.end_offset || i == layout.lines.len() - 1 {
			return i;
		}
	}
	0
}

/// Return the start (inclusive) character offset of the line containing
/// `offset`.
#[must_use]
pub fn line_start_for_offset(layout: &ParagraphLayout, offset: usize) -> usize {
	let idx = line_index_for_offset(layout, offset);
	layout.lines.get(idx).map_or(0, |l| l.start_offset)
}

/// Return the end (exclusive) character offset of the line containing
/// `offset`.
#[must_use]
pub fn line_end_for_offset(layout: &ParagraphLayout, offset: usize) -> usize {
	let idx = line_index_for_offset(layout, offset);
	layout.lines.get(idx).map_or(0, |l| l.end_offset)
}

/// Compute the x-pixel position of `offset` within a laid-out line.
///
/// `line_start` is the line's `start_offset`, and `offset` must be within
/// the same line.
#[must_use]
pub fn x_offset_in_line(
	line_start: usize,
	offset: usize,
	fragments: &[TextFragment<'_>],
	measurer: &dyn TextMeasure,
) -> f32 {
	if offset <= line_start {
		return 0.0;
	}
	let full_text: String = fragments.iter().map(|f| f.text).collect();
	let chars: Vec<char> = full_text.chars().collect();
	let end = offset.min(chars.len());
	let start = line_start.min(end);
	let mut x = 0.0f32;
	for (i, &ch) in chars.iter().enumerate().take(end).skip(start) {
		let style = fragment_style_at(fragments, i);
		x += measurer.measure_char(ch, style);
	}
	x
}

/// Find the character offset on the line directly above `offset`.
///
/// Uses the x-pixel position of the current caret to pick the closest
/// character on the previous line.
#[must_use]
pub fn offset_above(
	layout: &ParagraphLayout,
	offset: usize,
	fragments: &[TextFragment<'_>],
	measurer: &dyn TextMeasure,
) -> usize {
	let idx = line_index_for_offset(layout, offset);
	if idx == 0 {
		return layout.lines.first().map_or(0, |l| l.start_offset);
	}
	let cur_line = &layout.lines[idx];
	let target_x = x_offset_in_line(cur_line.start_offset, offset, fragments, measurer);
	let prev_line = &layout.lines[idx - 1];
	// Clamp: if hit_x lands at the line boundary (end_offset), pull back by
	// one so the offset unambiguously belongs to this line and not the next.
	let result = hit_x_on_line(prev_line, target_x, fragments, measurer);
	if result >= prev_line.end_offset && prev_line.end_offset > prev_line.start_offset {
		prev_line.end_offset - 1
	} else {
		result
	}
}

/// Find the character offset on the line directly below `offset`.
#[must_use]
pub fn offset_below(
	layout: &ParagraphLayout,
	offset: usize,
	fragments: &[TextFragment<'_>],
	measurer: &dyn TextMeasure,
) -> usize {
	let idx = line_index_for_offset(layout, offset);
	if idx >= layout.lines.len().saturating_sub(1) {
		return layout.lines.last().map_or(0, |l| l.end_offset);
	}
	let cur_line = &layout.lines[idx];
	let target_x = x_offset_in_line(cur_line.start_offset, offset, fragments, measurer);
	let next_line = &layout.lines[idx + 1];
	hit_x_on_line(next_line, target_x, fragments, measurer)
}

/// Find the character offset closest to a given x position on a single line.
fn hit_x_on_line(
	line: &LayoutLine,
	target_x: f32,
	fragments: &[TextFragment<'_>],
	measurer: &dyn TextMeasure,
) -> usize {
	let full_text: String = fragments.iter().map(|f| f.text).collect();
	let chars: Vec<char> = full_text.chars().collect();
	let mut x = 0.0f32;
	for (i, &ch) in chars
		.iter()
		.enumerate()
		.take(line.end_offset.min(chars.len()))
		.skip(line.start_offset)
	{
		let style = fragment_style_at(fragments, i);
		let w = measurer.measure_char(ch, style);
		if x + w * 0.5 > target_x {
			return i;
		}
		x += w;
	}
	line.end_offset
}

/// Find the style for a character at a given offset within the fragments.
fn fragment_style_at<'a>(fragments: &[TextFragment<'a>], offset: usize) -> &'a Style {
	let mut pos = 0;
	for frag in fragments {
		let len = frag.text.chars().count();
		if offset < pos + len {
			return frag.style;
		}
		pos += len;
	}
	// Fallback to last fragment's style or a static default.
	static DEFAULT: std::sync::LazyLock<Style> = std::sync::LazyLock::new(Style::new);
	fragments.last().map_or(&*DEFAULT, |f| f.style)
}
/// Compute the default line height from the config.
fn default_line_height(config: &LayoutConfig) -> f32 {
	let resolved = config.default_style.resolve();
	resolved.font_size * resolved.line_height
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn empty_text_produces_one_line() {
		let layout = layout_paragraph(&[], &LayoutConfig::default(), &HeuristicTextMeasure);
		assert_eq!(layout.lines.len(), 1);
		assert_eq!(layout.lines[0].start_offset, 0);
		assert_eq!(layout.lines[0].end_offset, 0);
	}

	#[test]
	fn short_text_fits_one_line() {
		let style = Style::new().font_size(16.0);
		let fragments = [TextFragment {
			text: "Hello",
			style: &style,
		}];
		let config = LayoutConfig::new(800.0);
		let layout = layout_paragraph(&fragments, &config, &HeuristicTextMeasure);

		assert_eq!(layout.lines.len(), 1);
		assert_eq!(layout.lines[0].start_offset, 0);
		assert_eq!(layout.lines[0].end_offset, 5);
	}

	#[test]
	fn long_text_wraps() {
		let style = Style::new().font_size(16.0);
		let text = "The quick brown fox jumps over the lazy dog and keeps running far away";
		let fragments = [TextFragment {
			text,
			style: &style,
		}];
		// Narrow width to force wrapping.
		let config = LayoutConfig::new(100.0);
		let layout = layout_paragraph(&fragments, &config, &HeuristicTextMeasure);

		assert!(layout.lines.len() > 1, "expected multiple lines, got 1");
		assert!(layout.total_height > 0.0);

		// Lines should be contiguous.
		for window in layout.lines.windows(2) {
			assert_eq!(window[0].end_offset, window[1].start_offset);
		}
	}

	#[test]
	fn lines_have_increasing_y() {
		let style = Style::new().font_size(16.0);
		let text = "word ".repeat(50);
		let fragments = [TextFragment {
			text: &text,
			style: &style,
		}];
		let config = LayoutConfig::new(200.0);
		let layout = layout_paragraph(&fragments, &config, &HeuristicTextMeasure);

		for window in layout.lines.windows(2) {
			assert!(window[1].y > window[0].y);
		}
	}

	#[test]
	fn line_start_end_for_offset_single_line() {
		let style = Style::new().font_size(16.0);
		let fragments = [TextFragment {
			text: "Hello",
			style: &style,
		}];
		let config = LayoutConfig::new(800.0);
		let layout = layout_paragraph(&fragments, &config, &HeuristicTextMeasure);

		assert_eq!(line_start_for_offset(&layout, 3), 0);
		assert_eq!(line_end_for_offset(&layout, 3), 5);
	}

	#[test]
	fn line_start_end_for_offset_multi_line() {
		let style = Style::new().font_size(16.0);
		// At 16px × 0.6 = 9.6px per char, and 100px width → ~10 chars/line.
		let text = "aaaaaaaaaa bbbbbbbbb";
		let fragments = [TextFragment {
			text,
			style: &style,
		}];
		let config = LayoutConfig::new(100.0);
		let layout = layout_paragraph(&fragments, &config, &HeuristicTextMeasure);

		assert!(
			layout.lines.len() >= 2,
			"expected at least 2 lines, got {}",
			layout.lines.len()
		);

		// Offset in first line.
		let first_line_end = layout.lines[0].end_offset;
		assert_eq!(line_start_for_offset(&layout, 3), 0);
		assert_eq!(line_end_for_offset(&layout, 3), first_line_end);

		// Offset in second line.
		let second_line_start = layout.lines[1].start_offset;
		let second_line_end = layout.lines[1].end_offset;
		assert_eq!(
			line_start_for_offset(&layout, second_line_start + 1),
			second_line_start
		);
		assert_eq!(
			line_end_for_offset(&layout, second_line_start + 1),
			second_line_end
		);
	}

	#[test]
	fn offset_above_on_first_line_returns_line_start() {
		let style = Style::new().font_size(16.0);
		let text = "short";
		let fragments = [TextFragment {
			text,
			style: &style,
		}];
		let config = LayoutConfig::new(800.0);
		let layout = layout_paragraph(&fragments, &config, &HeuristicTextMeasure);

		let result = offset_above(&layout, 3, &fragments, &HeuristicTextMeasure);
		assert_eq!(result, 0); // Already on first line, goes to start.
	}

	#[test]
	fn offset_below_on_last_line_returns_line_end() {
		let style = Style::new().font_size(16.0);
		let text = "short";
		let fragments = [TextFragment {
			text,
			style: &style,
		}];
		let config = LayoutConfig::new(800.0);
		let layout = layout_paragraph(&fragments, &config, &HeuristicTextMeasure);

		let result = offset_below(&layout, 3, &fragments, &HeuristicTextMeasure);
		assert_eq!(result, 5); // Already on last line, goes to end.
	}

	#[test]
	fn offset_above_below_stays_on_correct_line() {
		let style = Style::new().font_size(16.0);
		let text = "aaaaaaaaaa bbbbbbbbb ccccccccc";
		let fragments = [TextFragment {
			text,
			style: &style,
		}];
		let config = LayoutConfig::new(100.0);
		let layout = layout_paragraph(&fragments, &config, &HeuristicTextMeasure);

		if layout.lines.len() >= 2 {
			// Find the last line.
			let last_idx = layout.lines.len() - 1;
			let last_line = &layout.lines[last_idx];
			let mid = last_line.start_offset + 2;
			// Go up — should land on the previous line.
			let up = offset_above(&layout, mid, &fragments, &HeuristicTextMeasure);
			let up_line_idx = line_index_for_offset(&layout, up);
			assert!(
				up_line_idx < last_idx,
				"offset_above should move to a previous line: up={up} is on line {up_line_idx}, \
				 last line is {last_idx}"
			);
			// Go down from there — should move forward to a later line.
			let down = offset_below(&layout, up, &fragments, &HeuristicTextMeasure);
			let down_line_idx = line_index_for_offset(&layout, down);
			assert!(
				down_line_idx > up_line_idx,
				"offset_below should advance to a later line: down={down} on line \
				 {down_line_idx}, was on line {up_line_idx}"
			);
		}
	}

	#[test]
	fn x_offset_in_line_basic() {
		let style = Style::new().font_size(16.0);
		let fragments = [TextFragment {
			text: "Hello",
			style: &style,
		}];
		// Each char width = 16 * 0.6 = 9.6
		let x0 = x_offset_in_line(0, 0, &fragments, &HeuristicTextMeasure);
		assert!((x0 - 0.0).abs() < 0.01);

		let x3 = x_offset_in_line(0, 3, &fragments, &HeuristicTextMeasure);
		assert!((x3 - 28.8).abs() < 0.1, "expected ~28.8, got {x3}");
	}

	#[test]
	fn left_alignment_x_offset_is_zero() {
		let style = Style::new().font_size(16.0);
		let fragments = [TextFragment {
			text: "Hello",
			style: &style,
		}];
		let config = LayoutConfig {
			max_width: 800.0,
			default_style: style.clone(),
			text_align: crate::style::TextAlign::Left,
		};
		let layout = layout_paragraph(&fragments, &config, &HeuristicTextMeasure);
		assert_eq!(layout.lines.len(), 1);
		assert!((layout.lines[0].x_offset - 0.0).abs() < 0.01);
	}

	#[test]
	fn center_alignment_offsets_line() {
		let style = Style::new().font_size(16.0);
		let fragments = [TextFragment {
			text: "Hi",
			style: &style,
		}];
		let config = LayoutConfig {
			max_width: 400.0,
			default_style: style.clone(),
			text_align: crate::style::TextAlign::Center,
		};
		let layout = layout_paragraph(&fragments, &config, &HeuristicTextMeasure);
		assert_eq!(layout.lines.len(), 1);
		// "Hi" = 2 chars × 9.6 = 19.2px wide. Center offset = (400 - 19.2) / 2 = 190.4
		let expected = (400.0 - layout.lines[0].width) / 2.0;
		assert!(
			(layout.lines[0].x_offset - expected).abs() < 0.1,
			"center x_offset: expected ~{expected}, got {}",
			layout.lines[0].x_offset
		);
	}

	#[test]
	fn right_alignment_offsets_line() {
		let style = Style::new().font_size(16.0);
		let fragments = [TextFragment {
			text: "Hi",
			style: &style,
		}];
		let config = LayoutConfig {
			max_width: 400.0,
			default_style: style.clone(),
			text_align: crate::style::TextAlign::Right,
		};
		let layout = layout_paragraph(&fragments, &config, &HeuristicTextMeasure);
		assert_eq!(layout.lines.len(), 1);
		let expected = 400.0 - layout.lines[0].width;
		assert!(
			(layout.lines[0].x_offset - expected).abs() < 0.1,
			"right x_offset: expected ~{expected}, got {}",
			layout.lines[0].x_offset
		);
	}

	#[test]
	fn layout_line_x_offset_default_is_zero() {
		// Existing tests that use LayoutConfig::new() should get x_offset = 0.
		let style = Style::new().font_size(16.0);
		let fragments = [TextFragment {
			text: "Test text",
			style: &style,
		}];
		let config = LayoutConfig::new(800.0);
		let layout = layout_paragraph(&fragments, &config, &HeuristicTextMeasure);
		for line in &layout.lines {
			assert!(
				(line.x_offset - 0.0).abs() < 0.01,
				"default alignment should have x_offset=0, got {}",
				line.x_offset
			);
		}
	}
}
