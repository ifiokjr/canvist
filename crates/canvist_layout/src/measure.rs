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
use crate::LineBox;

mod fixed;
#[cfg(feature = "parley")]
mod parley;

pub use fixed::FixedMeasurer;
#[cfg(feature = "parley")]
pub use parley::ParleyMeasurer;

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
	/// Inline boxes whose exact line box is needed: a font and a unitless
	/// line height, as CSS `line-height: <number>` multiplies the font size.
	/// A measurer may answer them, or leave [`MeasureOutput::line_boxes`]
	/// empty for layout to derive them from the metrics as CSS 2.1 does.
	pub line_boxes: &'a [(FontId, f32)],
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
/// - `line_boxes` is empty, or holds one entry per
///   [`MeasureBatch::line_boxes`] request, in order.
/// - `advances` holds one entry per `char` of every segment, segment after
///   segment. A segment's advances must sum to the width the host paints the
///   whole segment at (so kerning and ligatures inside it count). How the
///   width of a ligature or cluster is split among its characters is up to
///   the host; layout only reads sums at grapheme boundaries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeasureOutput {
	/// Vertical metrics, one per requested font.
	pub metrics: Vec<FontMetrics>,
	/// Line boxes, one per requested inline box, or none.
	pub line_boxes: Vec<LineBox>,
	/// Per-character advances, concatenated across segments.
	pub advances: Vec<f32>,
}

/// Measures text for layout. Implemented once per platform: the browser's
/// `measureText` on the web, [`ParleyMeasurer`] on desktop and for SSR,
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
	/// Line boxes by font and line height bits.
	line_boxes: FxHashMap<(FontId, u32), LineBox>,
	pending_line_boxes: Vec<(FontId, f32)>,
	pending: Vec<(FontId, Box<str>)>,
	pending_set: FxHashSet<(FontId, Box<str>)>,
	round_trips: u64,
	hits: u64,
	misses: u64,
}

/// How well the measurement cache is doing, from
/// [`DocumentLayout::measure_stats`](crate::DocumentLayout::measure_stats).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MeasureStats {
	/// Segment lookups answered from the cache, or already queued by an
	/// earlier lookup in the same relayout.
	pub hits: u64,
	/// Segment lookups that queued a measurement.
	pub misses: u64,
	/// How many times the measurer was called.
	pub round_trips: u64,
	/// Cached segments and their approximate heap size, per font, indexed by
	/// [`FontId::index`].
	pub fonts: Vec<FontCacheStats>,
}

impl MeasureStats {
	/// The share of lookups answered without measuring, from 0 to 1, or 1
	/// when nothing was looked up.
	#[must_use]
	pub fn hit_rate(&self) -> f64 {
		let total = self.hits + self.misses;

		if total == 0 {
			return 1.0;
		}

		self.hits as f64 / total as f64
	}

	/// The approximate heap size of every cached measurement, in bytes.
	#[must_use]
	pub fn bytes(&self) -> usize {
		self.fonts.iter().map(|font| font.bytes).sum()
	}
}

/// The cached measurements of one font.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FontCacheStats {
	/// How many segments are cached.
	pub segments: usize,
	/// Their approximate heap size in bytes: keys, advances, and the hash
	/// table's slots.
	pub bytes: usize,
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

	/// Queue the line box of `font` at unitless `line_height` unless it is
	/// known.
	pub(crate) fn request_line_box(&mut self, font: FontId, line_height: f32) {
		let key = (font, line_height.to_bits());

		if !self.line_boxes.contains_key(&key)
			&& !self
				.pending_line_boxes
				.iter()
				.any(|&(pending, height)| (pending, height.to_bits()) == key)
		{
			self.pending_line_boxes.push((font, line_height));
		}
	}

	/// The line box of `font` at unitless `line_height`, once measured.
	pub(crate) fn line_box(&self, font: FontId, line_height: f32) -> Option<LineBox> {
		self.line_boxes.get(&(font, line_height.to_bits())).copied()
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
		if text.is_empty() {
			return;
		}

		if self.get(font, text).is_some() {
			self.hits += 1;
			return;
		}

		let key = (font, Box::<str>::from(text));

		if self.pending_set.insert(key.clone()) {
			self.misses += 1;
			self.pending.push(key);
		} else {
			self.hits += 1;
		}
	}

	/// Whether anything is queued.
	pub(crate) fn has_pending(&self) -> bool {
		!self.pending.is_empty()
			|| !self.pending_metrics.is_empty()
			|| !self.pending_line_boxes.is_empty()
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
		self.line_boxes.clear();
		self.pending_line_boxes.clear();
		self.pending.clear();
		self.pending_set.clear();
	}

	/// Forget the measurements and metrics of `fonts` only, queueing their
	/// metrics again. Measurements of every other font stay cached.
	pub(crate) fn forget_fonts(&mut self, fonts: &[FontId]) {
		for &font in fonts {
			let index = font.index();

			if index >= self.segments.len() {
				continue;
			}

			self.segments[index] = FxHashMap::default();
			self.metrics[index] = None;

			if !self.pending_metrics.contains(&font) {
				self.pending_metrics.push(font);
			}
		}

		self.line_boxes.retain(|(font, _), _| !fonts.contains(font));
		self.pending_line_boxes
			.retain(|(font, _)| !fonts.contains(font));
		self.pending.retain(|(font, _)| !fonts.contains(font));
		self.pending_set.retain(|(font, _)| !fonts.contains(font));
	}

	/// How many times the measurer has been called.
	pub(crate) fn round_trips(&self) -> u64 {
		self.round_trips
	}

	/// Hit and miss counts, and the size of each font's cache.
	pub(crate) fn stats(&self) -> MeasureStats {
		MeasureStats {
			hits: self.hits,
			misses: self.misses,
			round_trips: self.round_trips,
			fonts: self.segments.iter().map(font_cache_stats).collect(),
		}
	}

	/// Measure everything queued with one call to `measurer`.
	pub(crate) fn flush(&mut self, measurer: &mut dyn TextMeasurer) -> Result<(), LayoutError> {
		if !self.has_pending() {
			return Ok(());
		}

		let pending = std::mem::take(&mut self.pending);
		let pending_metrics = std::mem::take(&mut self.pending_metrics);
		let pending_line_boxes = std::mem::take(&mut self.pending_line_boxes);
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
				line_boxes: &pending_line_boxes,
				segments: &requests,
			},
			&mut out,
		);
		self.round_trips += 1;

		let expected: usize = pending.iter().map(|(_, text)| text.chars().count()).sum();

		if !out.line_boxes.is_empty() && out.line_boxes.len() != pending_line_boxes.len() {
			return Err(LayoutError::LineBoxContract {
				requested: pending_line_boxes.len() as u32,
				returned: out.line_boxes.len() as u32,
			});
		}

		if out.metrics.len() != pending_metrics.len() || out.advances.len() != expected {
			return Err(LayoutError::MeasurerContract {
				requested: [pending_metrics.len() as u32, expected as u32],
				returned: [out.metrics.len() as u32, out.advances.len() as u32],
			});
		}

		for (font, metrics) in pending_metrics.iter().zip(out.metrics) {
			self.metrics[font.index()] = Some(metrics);
		}

		for (index, &(font, line_height)) in pending_line_boxes.iter().enumerate() {
			let line_box = if let Some(line_box) = out.line_boxes.get(index) {
				*line_box
			} else {
				let metrics = self.metrics(font).ok_or(LayoutError::MissingMeasurement)?;
				let size = self.fonts.get(font).map_or(0.0, |spec| spec.size);
				LineBox::from_metrics(metrics, line_height * size)
			};
			self.line_boxes
				.insert((font, line_height.to_bits()), line_box);
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

/// The approximate heap size of one font's segment cache: each slot of the
/// table (key, value, and a control byte), plus each entry's key text and
/// its shared measurement (two reference counts, the advances, and the
/// width).
fn font_cache_stats(segments: &FxHashMap<Box<str>, Arc<Measured>>) -> FontCacheStats {
	const ARC_COUNTS: usize = 2 * size_of::<usize>();

	let slot = size_of::<Box<str>>() + size_of::<Arc<Measured>>() + 1;
	let entries: usize = segments
		.iter()
		.map(|(text, measured)| {
			text.len()
				+ ARC_COUNTS
				+ size_of::<Measured>()
				+ measured.advances.len() * size_of::<f32>()
		})
		.sum();

	FontCacheStats {
		segments: segments.len(),
		bytes: segments.capacity() * slot + entries,
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

		let stats = cache.stats();
		assert_eq!((stats.hits, stats.misses), (2, 2));
		assert_eq!(stats.fonts[a.index()].segments, 1);
		assert!(stats.bytes() > 0);
	}

	/// A fixed measurer that reports every line box as `[above, below]`.
	struct Calibrated([f32; 2], Option<usize>);

	impl TextMeasurer for Calibrated {
		fn measure(&mut self, batch: &MeasureBatch<'_>, out: &mut MeasureOutput) {
			FixedMeasurer::new().measure(batch, out);
			let count = self.1.unwrap_or(batch.line_boxes.len());
			out.line_boxes = vec![
				LineBox {
					above: self.0[0],
					below: self.0[1],
				};
				count
			];
		}
	}

	#[test]
	fn line_boxes_come_from_the_measurer_or_the_metrics() {
		const LINE_HEIGHT: f32 = 1.5;

		let mut cache = MeasureCache::default();
		let id = cache.font(&font(10.0));
		cache.request_line_box(id, LINE_HEIGHT);
		cache
			.flush(&mut FixedMeasurer::new())
			.expect("valid output");
		let derived = cache.line_box(id, LINE_HEIGHT).expect("derived");
		let metrics = cache.metrics(id).expect("measured");
		assert_eq!(derived, LineBox::from_metrics(metrics, LINE_HEIGHT * 10.0));

		let mut cache = MeasureCache::default();
		let id = cache.font(&font(10.0));
		cache.request_line_box(id, LINE_HEIGHT);
		cache.request_line_box(id, LINE_HEIGHT);
		cache
			.flush(&mut Calibrated([17.5, 6.5], None))
			.expect("valid output");
		assert_eq!(
			cache.line_box(id, LINE_HEIGHT).map(|line| line.above),
			Some(17.5)
		);

		let mut cache = MeasureCache::default();
		let id = cache.font(&font(10.0));
		cache.request_line_box(id, LINE_HEIGHT);
		cache.request_line_box(id, LINE_HEIGHT * 2.0);
		assert!(matches!(
			cache.flush(&mut Calibrated([1.0, 1.0], Some(1))),
			Err(LayoutError::LineBoxContract {
				requested: 2,
				returned: 1
			})
		));
	}

	#[test]
	fn forgets_one_font() {
		let mut cache = MeasureCache::default();
		let mut measurer = FixedMeasurer::new();
		let a = cache.font(&font(10.0));
		let b = cache.font(&font(LARGE_SIZE));

		cache.request(a, "kept");
		cache.request(b, "dropped");
		cache.flush(&mut measurer).expect("valid output");
		cache.forget_fonts(&[b]);

		assert!(cache.get(a, "kept").is_some());
		assert!(cache.get(b, "dropped").is_none());
		assert_eq!(cache.metrics(b), None);
		assert!(cache.metrics(a).is_some());
		assert!(
			cache.has_pending(),
			"the forgotten font's metrics are queued"
		);

		cache.flush(&mut measurer).expect("valid output");
		assert_eq!(
			measurer.segments(),
			2,
			"nothing but the metrics was measured again"
		);
		assert!(cache.metrics(b).is_some());
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
