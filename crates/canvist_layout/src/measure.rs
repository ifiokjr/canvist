//! Text measurement: the [`TextMeasurer`] trait hosts implement, and the
//! cache that batches requests so a relayout costs at most one host call.
//!
//! Layout splits text into *segments*: a word plus its trailing spaces, cut
//! further wherever the font or bidi level changes. Each segment is measured
//! once per font and cached for the life of the layout. A relayout first
//! collects every segment it is missing, then calls
//! [`TextMeasurer::measure`] once with all of them.

use std::sync::Arc;

use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use crate::FontBook;
use crate::FontId;
use crate::FontMetrics;
use crate::FontSpec;
use crate::LayoutError;

mod fixed;

pub use fixed::FixedMeasurer;

/// One segment to measure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentRequest<'a> {
	/// The font, resolvable through [`MeasureBatch::font`].
	pub font: FontId,
	/// The text. Never contains tabs, line breaks, or embed characters.
	pub text: &'a str,
}

/// Everything one relayout needs measured.
#[derive(Clone, Copy, Debug)]
pub struct MeasureBatch<'a> {
	/// Every font the layout knows.
	pub fonts: &'a FontBook,
	/// Fonts whose vertical metrics are needed.
	pub metrics: &'a [FontId],
	/// Segments whose advances are needed.
	pub segments: &'a [SegmentRequest<'a>],
}

impl MeasureBatch<'_> {
	/// The font with `id`.
	#[must_use]
	pub fn font(&self, id: FontId) -> Option<&FontSpec> {
		self.fonts.get(id)
	}
}

/// What a measurer returns for a [`MeasureBatch`].
///
/// - `metrics` holds one entry per [`MeasureBatch::metrics`] font, in order.
/// - `advances` holds one entry per `char` of every segment, segment after
///   segment. A segment's advances must sum to the width the host paints the
///   whole segment at (so kerning and ligatures inside it count). How the
///   width of a ligature or cluster is split among its characters is up to
///   the host; layout only reads sums at grapheme boundaries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeasureOutput {
	/// Vertical metrics, one per requested font.
	pub metrics: Vec<FontMetrics>,
	/// Per-character advances, concatenated across segments.
	pub advances: Vec<f32>,
}

/// Measures text for layout. Implemented once per platform: the browser's
/// `measureText` on the web, `parley` on desktop and for SSR,
/// CoreText on iOS, and so on.
pub trait TextMeasurer {
	/// Measure a batch, appending results to `out`, which arrives empty.
	///
	/// Layout calls this at most once per relayout pass, so a host that
	/// crosses an FFI or JavaScript boundary pays for one crossing.
	fn measure(&mut self, batch: &MeasureBatch<'_>, out: &mut MeasureOutput);
}

impl<T: TextMeasurer + ?Sized> TextMeasurer for &mut T {
	fn measure(&mut self, batch: &MeasureBatch<'_>, out: &mut MeasureOutput) {
		(**self).measure(batch, out);
	}
}

/// A measured segment.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Measured {
	/// Advance of each `char`.
	pub(crate) advances: Box<[f32]>,
	/// Sum of `advances`.
	pub(crate) width: f32,
}

/// Measurements cached by (font, segment text), plus the queue of misses
/// waiting for the next [`MeasureCache::flush`].
#[derive(Clone, Debug, Default)]
pub(crate) struct MeasureCache {
	fonts: FontBook,
	metrics: Vec<Option<FontMetrics>>,
	segments: Vec<FxHashMap<Box<str>, Arc<Measured>>>,
	pending_metrics: Vec<FontId>,
	pending: Vec<(FontId, Box<str>)>,
	pending_set: FxHashSet<(FontId, Box<str>)>,
	round_trips: u64,
}

impl MeasureCache {
	pub(crate) fn fonts(&self) -> &FontBook {
		&self.fonts
	}

	/// Intern `spec`, queueing its metrics if they are unknown.
	pub(crate) fn font(&mut self, spec: &FontSpec) -> FontId {
		let id = self.fonts.intern(spec);

		if id.index() >= self.metrics.len() {
			self.metrics.resize(id.index() + 1, None);
			self.segments
				.resize_with(id.index() + 1, FxHashMap::default);
			self.pending_metrics.push(id);
		}

		id
	}

	/// The metrics of `font`, once measured.
	pub(crate) fn metrics(&self, font: FontId) -> Option<FontMetrics> {
		self.metrics.get(font.index()).copied().flatten()
	}

	/// The measurement of `text` in `font`, once measured.
	pub(crate) fn get(&self, font: FontId, text: &str) -> Option<&Arc<Measured>> {
		self.segments.get(font.index())?.get(text)
	}

	/// Queue `text` for measurement unless it is cached or already queued.
	pub(crate) fn request(&mut self, font: FontId, text: &str) {
		if text.is_empty() || self.get(font, text).is_some() {
			return;
		}

		let key = (font, Box::<str>::from(text));

		if self.pending_set.insert(key.clone()) {
			self.pending.push(key);
		}
	}

	/// Whether anything is queued.
	pub(crate) fn has_pending(&self) -> bool {
		!self.pending.is_empty() || !self.pending_metrics.is_empty()
	}

	/// Forget every measurement but keep the fonts and their ids, so the
	/// next relayout measures everything again. Hosts call this when a font
	/// finishes loading, because measurements taken with its fallback are
	/// wrong.
	pub(crate) fn forget_measurements(&mut self) {
		let fonts = self.fonts.len();

		self.metrics = vec![None; fonts];
		self.segments = vec![FxHashMap::default(); fonts];
		self.pending_metrics = (0..fonts as u32).map(FontId).collect();
		self.pending.clear();
		self.pending_set.clear();
	}

	/// How many times the measurer has been called.
	pub(crate) fn round_trips(&self) -> u64 {
		self.round_trips
	}

	/// Measure everything queued with one call to `measurer`.
	pub(crate) fn flush(&mut self, measurer: &mut dyn TextMeasurer) -> Result<(), LayoutError> {
		if !self.has_pending() {
			return Ok(());
		}

		let pending = std::mem::take(&mut self.pending);
		let pending_metrics = std::mem::take(&mut self.pending_metrics);
		self.pending_set.clear();

		let requests: Vec<SegmentRequest<'_>> = pending
			.iter()
			.map(|(font, text)| SegmentRequest { font: *font, text })
			.collect();
		let mut out = MeasureOutput::default();
		measurer.measure(
			&MeasureBatch {
				fonts: &self.fonts,
				metrics: &pending_metrics,
				segments: &requests,
			},
			&mut out,
		);
		self.round_trips += 1;

		let expected: usize = pending.iter().map(|(_, text)| text.chars().count()).sum();

		if out.metrics.len() != pending_metrics.len() || out.advances.len() != expected {
			return Err(LayoutError::MeasurerContract {
				requested: [pending_metrics.len() as u32, expected as u32],
				returned: [out.metrics.len() as u32, out.advances.len() as u32],
			});
		}

		for (font, metrics) in pending_metrics.iter().zip(out.metrics) {
			self.metrics[font.index()] = Some(metrics);
		}

		let mut advances = out.advances.into_iter();

		for (font, text) in pending {
			let advances: Box<[f32]> = advances.by_ref().take(text.chars().count()).collect();
			let width = advances.iter().sum();
			self.segments[font.index()].insert(text, Arc::new(Measured { advances, width }));
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A second font size, so two fonts are batched together.
	const LARGE_SIZE: f32 = 20.0;

	fn font(size: f32) -> FontSpec {
		FontSpec {
			family: Arc::from("Test"),
			size,
			weight: 400,
			italic: false,
		}
	}

	#[test]
	fn batches_misses_into_one_call() {
		let mut cache = MeasureCache::default();
		let mut measurer = FixedMeasurer::new();
		let a = cache.font(&font(10.0));
		let b = cache.font(&font(LARGE_SIZE));

		cache.request(a, "hello ");
		cache.request(a, "hello ");
		cache.request(b, "world");
		cache.flush(&mut measurer).expect("valid output");

		assert_eq!(measurer.calls(), 1);
		assert_eq!(measurer.segments(), 2, "duplicates are measured once");
		assert_eq!(cache.get(a, "hello ").map(|m| m.width), Some(36.0));
		assert_eq!(cache.get(b, "world").map(|m| m.advances.len()), Some(5));
		assert_eq!(cache.metrics(b).map(|m| m.ascent), Some(16.0));

		cache.request(a, "hello ");
		assert!(!cache.has_pending(), "hits are not queued again");
	}

	#[test]
	fn rejects_short_output() {
		struct Broken;

		impl TextMeasurer for Broken {
			fn measure(&mut self, _: &MeasureBatch<'_>, out: &mut MeasureOutput) {
				out.advances.push(1.0);
			}
		}

		let mut cache = MeasureCache::default();
		let id = cache.font(&font(10.0));
		cache.request(id, "abc");

		assert!(matches!(
			cache.flush(&mut Broken),
			Err(LayoutError::MeasurerContract { .. })
		));
	}
}
