//! A deterministic measurer for tests, benchmarks, and headless editing.

use super::MeasureBatch;
use super::MeasureOutput;
use super::TextMeasurer;
use crate::FontMetrics;
use crate::FontSpec;

/// The advance of most characters, in ems.
const NARROW_ADVANCE_EM: f32 = 0.6;

/// The advance of East Asian wide characters and emoji, in ems.
const WIDE_ADVANCE_EM: f32 = 1.0;

/// The lightest weight treated as bold, matching CSS `font-weight: 600`.
const BOLD_WEIGHT: u16 = 600;

/// How much wider bold text advances, so bold text visibly reflows.
const BOLD_WIDTH_SCALE: f32 = 1.1;

/// The height above the baseline, in ems.
const ASCENT_EM: f32 = 0.8;

/// The depth below the baseline, in ems.
const DESCENT_EM: f32 = 0.2;

/// Measures text without fonts, so layouts are identical on every machine.
///
/// - Most characters advance `0.6em`; East Asian wide characters and emoji
///   advance `1em`; combining marks, joiners, and variation selectors advance
///   nothing.
/// - Weights of 600 and above are 10% wider, so bold text visibly reflows.
/// - Ascent is `0.8em` and descent `0.2em`.
///
/// It also counts its calls, so tests can check that a relayout costs one
/// round trip.
#[derive(Clone, Debug, Default)]
pub struct FixedMeasurer {
	calls: usize,
	segments: usize,
}

impl FixedMeasurer {
	/// A measurer with zeroed counters.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// How many batches have been measured.
	#[must_use]
	pub fn calls(&self) -> usize {
		self.calls
	}

	/// How many segments have been measured in total.
	#[must_use]
	pub fn segments(&self) -> usize {
		self.segments
	}

	/// The advance of `c` in `font`.
	#[must_use]
	pub fn advance(c: char, font: &FontSpec) -> f32 {
		let ems = if is_zero_width(c) {
			0.0
		} else if is_wide(c) {
			WIDE_ADVANCE_EM
		} else {
			NARROW_ADVANCE_EM
		};

		let bold = if font.weight >= BOLD_WEIGHT {
			BOLD_WIDTH_SCALE
		} else {
			1.0
		};

		font.size * ems * bold
	}

	/// The vertical metrics of `font`.
	#[must_use]
	pub fn metrics(font: &FontSpec) -> FontMetrics {
		FontMetrics {
			ascent: font.size * ASCENT_EM,
			descent: font.size * DESCENT_EM,
		}
	}
}

impl TextMeasurer for FixedMeasurer {
	fn measure(&mut self, batch: &MeasureBatch<'_>, out: &mut MeasureOutput) {
		self.calls += 1;
		self.segments += batch.segments.len();

		for id in batch.metrics {
			out.metrics
				.push(batch.font(*id).map(Self::metrics).unwrap_or_default());
		}

		for segment in batch.segments {
			let Some(font) = batch.font(segment.font) else {
				out.advances.extend(segment.text.chars().map(|_| 0.0));
				continue;
			};

			out.advances
				.extend(segment.text.chars().map(|c| Self::advance(c, font)));
		}
	}
}

/// Characters that render inside the previous cluster.
fn is_zero_width(c: char) -> bool {
	matches!(
		c,
		'\u{0300}'..='\u{036F}'
			| '\u{0591}'..='\u{05C7}'
			| '\u{0610}'..='\u{061A}'
			| '\u{064B}'..='\u{065F}'
			| '\u{0670}'
			| '\u{200B}'..='\u{200F}'
			| '\u{202A}'..='\u{202E}'
			| '\u{2060}'..='\u{2069}'
			| '\u{20D0}'..='\u{20FF}'
			| '\u{FE00}'..='\u{FE0F}'
			| '\u{FE20}'..='\u{FE2F}'
			| '\u{FEFF}'
			| '\u{1F3FB}'..='\u{1F3FF}'
			| '\u{E0020}'..='\u{E007F}'
	)
}

/// East Asian wide characters and emoji.
fn is_wide(c: char) -> bool {
	matches!(
		c,
		'\u{1100}'..='\u{115F}'
			| '\u{2E80}'..='\u{A4CF}'
			| '\u{AC00}'..='\u{D7A3}'
			| '\u{F900}'..='\u{FAFF}'
			| '\u{FE30}'..='\u{FE4F}'
			| '\u{FF00}'..='\u{FF60}'
			| '\u{FFE0}'..='\u{FFE6}'
			| '\u{1F000}'..='\u{1FAFF}'
			| '\u{20000}'..='\u{3FFFD}'
	)
}

#[cfg(test)]
#[allow(
	clippy::float_cmp,
	reason = "the values compared are exactly representable"
)]
mod tests {
	use std::sync::Arc;

	use super::*;

	#[test]
	fn advances_by_character_class() {
		let font = FontSpec {
			family: Arc::from("Test"),
			size: 10.0,
			weight: 400,
			italic: false,
		};

		assert_eq!(FixedMeasurer::advance('a', &font), 6.0);
		assert_eq!(FixedMeasurer::advance('中', &font), 10.0);
		assert_eq!(FixedMeasurer::advance('😀', &font), 10.0);
		assert_eq!(FixedMeasurer::advance('\u{301}', &font), 0.0);
		assert_eq!(FixedMeasurer::advance('\u{200D}', &font), 0.0);

		let bold = FontSpec {
			weight: 700,
			..font
		};
		assert!((FixedMeasurer::advance('a', &bold) - 6.6).abs() < 1e-5);
	}
}
