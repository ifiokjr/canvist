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
}

impl Default for LayoutConfig {
	fn default() -> Self {
		Self {
			max_width: 800.0,
			default_style: Style::new(),
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
		let width = (line_start..line_end)
			.map(|i| {
				let style = fragment_style_at(fragments, i);
				measurer.measure_char(chars[i], style)
			})
			.sum();

		lines.push(LayoutLine {
			start_offset: line_start,
			end_offset: line_end,
			width,
			height: line_height,
			y,
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
}
