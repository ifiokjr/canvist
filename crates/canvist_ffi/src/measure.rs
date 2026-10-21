//! Text measurement by the platform: CoreText on iOS, `Paint` on Android.
//!
//! Layout batches every segment a relayout is missing into one request and
//! caches the answers by (font, segment), so typing a character costs at
//! most one call into Swift or Kotlin, and usually none.

use std::sync::Arc;

use canvist_layout::FontMetrics;
use canvist_layout::FontSpec;
use canvist_layout::MeasureBatch;
use canvist_layout::MeasureOutput;
use canvist_layout::TextMeasurer;

/// A font layout uses, sent once per font id before the first request that
/// needs it. Ids stay valid until the editor loads another document, which
/// sends every font again.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FontFace {
	/// The id text runs and segments refer to the font by.
	pub id: u32,
	/// Family names in fallback order, unquoted, as CSS lists them: names
	/// such as `Inter`, or the generic `system-ui`, `sans-serif`, `serif`,
	/// `ui-monospace`, and `monospace`.
	pub families: Vec<String>,
	/// The size in points (iOS) or density-independent pixels (Android).
	pub size: f32,
	/// The weight, from 100 to 900, where 400 is regular and 700 bold.
	pub weight: u16,
	/// Whether the face is italic.
	pub italic: bool,
}

impl FontFace {
	fn new(id: usize, spec: &FontSpec) -> Self {
		Self {
			id: id as u32,
			families: spec.families().map(str::to_owned).collect(),
			size: spec.size,
			weight: spec.weight,
			italic: spec.italic,
		}
	}
}

/// One piece of text to measure in one font.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct TextSegment {
	/// The font's id.
	pub font: u32,
	/// The text. It never contains tabs, line breaks, or embeds.
	pub text: String,
}

/// Everything one relayout needs measured.
#[derive(Clone, Debug, Default, PartialEq, uniffi::Record)]
pub struct MeasureRequest {
	/// Fonts the platform has not been sent yet.
	pub fonts: Vec<FontFace>,
	/// Ids of fonts whose vertical metrics are needed.
	pub metrics: Vec<u32>,
	/// Segments whose advances are needed.
	pub segments: Vec<TextSegment>,
}

/// A font's extent around its baseline.
#[derive(Clone, Copy, Debug, Default, PartialEq, uniffi::Record)]
pub struct VerticalMetrics {
	/// Distance from the baseline up to the ascender line. Positive.
	pub ascent: f32,
	/// Distance from the baseline down to the descender line. Positive.
	pub descent: f32,
}

/// The answer to a [`MeasureRequest`].
#[derive(Clone, Debug, Default, PartialEq, uniffi::Record)]
pub struct MeasureResponse {
	/// One entry per id in [`MeasureRequest::metrics`], in order.
	pub metrics: Vec<VerticalMetrics>,
	/// One advance per Unicode scalar value of every segment, segment after
	/// segment. A segment's advances must add up to the width the platform
	/// draws the whole segment at, kerning and ligatures included; how a
	/// ligature's width is split among its characters is up to the platform.
	pub advances: Vec<f32>,
}

/// Measures text with the platform's own text engine, so the editor wraps
/// lines where the platform would and paints glyphs exactly where it
/// measured them. The iOS and Android views implement it; a host embedding
/// the editor some other way supplies its own.
///
/// The editor calls it synchronously, on the thread that called into the
/// editor, while holding the editor's lock: an implementation must not call
/// back into the editor.
#[uniffi::export(with_foreign)]
pub trait PlatformMeasurer: Send + Sync {
	/// Measure one batch.
	fn measure(&self, request: MeasureRequest) -> MeasureResponse;
}

/// Adapts a [`PlatformMeasurer`] to layout's [`TextMeasurer`], sending each
/// font once and counting the work for statistics.
pub(crate) struct HostMeasurer {
	platform: Arc<dyn PlatformMeasurer>,
	/// How many of the layout's fonts the platform has been sent.
	fonts_sent: usize,
	/// Segments measured over the editor's life.
	pub(crate) segments: u64,
	/// Requests made over the editor's life.
	pub(crate) calls: u64,
}

impl HostMeasurer {
	pub(crate) fn new(platform: Arc<dyn PlatformMeasurer>) -> Self {
		Self {
			platform,
			fonts_sent: 0,
			segments: 0,
			calls: 0,
		}
	}

	/// Send every font again with its new id: a new layout numbers fonts
	/// from zero.
	pub(crate) fn reset_fonts(&mut self) {
		self.fonts_sent = 0;
	}
}

impl TextMeasurer for HostMeasurer {
	fn measure(&mut self, batch: &MeasureBatch<'_>, out: &mut MeasureOutput) {
		let fonts = batch.fonts.fonts();
		let request = MeasureRequest {
			fonts: fonts
				.iter()
				.enumerate()
				.skip(self.fonts_sent)
				.map(|(id, spec)| FontFace::new(id, spec))
				.collect(),
			metrics: batch.metrics.iter().map(|id| id.index() as u32).collect(),
			segments: batch
				.segments
				.iter()
				.map(|segment| {
					TextSegment {
						font: segment.font.index() as u32,
						text: segment.text.to_owned(),
					}
				})
				.collect(),
		};

		self.fonts_sent = fonts.len();
		self.segments += request.segments.len() as u64;
		self.calls += 1;

		// A short answer leaves the outputs short, which layout reports as a
		// broken measurer contract rather than laying out garbage.
		let response = self.platform.measure(request);
		out.metrics.extend(response.metrics.iter().map(|metrics| {
			FontMetrics {
				ascent: metrics.ascent,
				descent: metrics.descent,
			}
		}));
		out.advances.extend_from_slice(&response.advances);
	}
}
