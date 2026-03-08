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
///
/// # Returns
///
/// A [`ParagraphLayout`] with computed line breaks and geometry.
pub fn layout_paragraph(fragments: &[TextFragment<'_>], config: &LayoutConfig) -> ParagraphLayout {
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

	let avg_char_width = estimate_char_width(config);
	let line_height = default_line_height(config);
	let max_chars_per_line = (config.max_width / avg_char_width).floor().max(1.0) as usize;

	let mut lines = Vec::new();
	let mut line_start = 0usize;
	let mut y = 0.0f32;

	let chars: Vec<char> = full_text.chars().collect();
	let total_chars = chars.len();

	while line_start < total_chars {
		let remaining = total_chars - line_start;
		let tentative_end = line_start + remaining.min(max_chars_per_line);

		// Try to break at a space if we're not at the end.
		let line_end = if tentative_end < total_chars {
			// Look backward for a space to break at.
			let mut break_at = tentative_end;
			while break_at > line_start && chars[break_at - 1] != ' ' {
				break_at -= 1;
			}
			if break_at == line_start {
				// No space found — force break at max width.
				tentative_end
			} else {
				break_at
			}
		} else {
			tentative_end
		};

		let line_char_count = line_end - line_start;
		let width = line_char_count as f32 * avg_char_width;

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

/// Estimate the average character width based on the default font size.
fn estimate_char_width(config: &LayoutConfig) -> f32 {
	let resolved = config.default_style.resolve();
	// Rough heuristic: average character width ≈ 0.6 × font size.
	resolved.font_size * 0.6
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
		let layout = layout_paragraph(&[], &LayoutConfig::default());
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
		let layout = layout_paragraph(&fragments, &config);

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
		let layout = layout_paragraph(&fragments, &config);

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
		let layout = layout_paragraph(&fragments, &config);

		for window in layout.lines.windows(2) {
			assert!(window[1].y > window[0].y);
		}
	}
}
