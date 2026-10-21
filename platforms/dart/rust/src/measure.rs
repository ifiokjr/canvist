//! Text measurement by the Dart host, one synchronous call per relayout.
//!
//! Layout batches every segment it is missing into one [`MeasureBatch`] and
//! caches the answers by (font, segment), so a relayout crosses into Dart at
//! most once. Dart passes the address of a `NativeCallable.isolateLocal`
//! function when it creates a view; [`HostMeasurer`] calls it on the thread
//! that called into Rust, which is the isolate's own thread because every
//! view call is a synchronous bridge call. The Flutter package answers with
//! `dart:ui` paragraphs, so text is exactly as wide as Flutter paints it.
//!
//! This is the one module with `unsafe`: calling a function pointer and
//! lending it raw buffers. Both are confined to [`HostMeasurer::measure`].
#![allow(
	unsafe_code,
	reason = "calls the Dart measurer through a C function pointer"
)]

use canvist_layout::FontMetrics;
use canvist_layout::MeasureBatch;
use canvist_layout::MeasureOutput;
use canvist_layout::TextMeasurer;
use serde_json::json;

/// One measurement batch, laid out as the Dart `MeasureRequest` struct reads
/// it. Every pointer is valid for its length for the duration of the call.
#[repr(C)]
struct MeasureRequest {
	/// The id of the first font in `fonts_json`.
	first_font: u32,
	/// UTF-8 JSON: the fonts the host has not seen yet, as
	/// `[{"family", "size", "weight", "italic"}]` with consecutive ids.
	fonts_json: *const u8,
	fonts_json_len: usize,
	/// Fonts whose ascent and descent are needed.
	metrics: *const u32,
	metrics_len: usize,
	/// The font of each segment.
	segment_fonts: *const u32,
	/// The length of each segment in UTF-16 code units.
	segment_lengths: *const u32,
	segments_len: usize,
	/// Every segment's text, concatenated, as UTF-16.
	text: *const u16,
	text_len: usize,
	/// Where the host writes each requested font's ascent and descent, then
	/// one advance per Unicode scalar value of every segment.
	out: *mut f32,
	out_len: usize,
}

/// The C signature of the Dart measurer.
type MeasureFn = extern "C" fn(request: *const MeasureRequest);

/// A [`TextMeasurer`] backed by a Dart function.
pub(crate) struct HostMeasurer {
	measure: MeasureFn,
	/// How many of the layout's fonts the host already knows.
	fonts_sent: usize,
	/// Buffers reused across batches.
	metrics: Vec<u32>,
	fonts: Vec<u32>,
	lengths: Vec<u32>,
	text: Vec<u16>,
	out: Vec<f32>,
}

impl HostMeasurer {
	/// A measurer calling the Dart function at `address`.
	///
	/// # Safety
	///
	/// `address` must be the address of a function with the C signature
	/// `void measure(const MeasureRequest*)` that stays callable, on the
	/// thread that calls [`TextMeasurer::measure`], for as long as this
	/// measurer lives, and that writes no further than `out_len` floats.
	pub(crate) unsafe fn new(address: usize) -> Option<Self> {
		if address == 0 {
			return None;
		}

		// SAFETY: the caller guarantees `address` is a function with this
		// signature; it is non-null, checked above.
		let measure = unsafe { std::mem::transmute::<usize, MeasureFn>(address) };

		Some(Self {
			measure,
			fonts_sent: 0,
			metrics: Vec::new(),
			fonts: Vec::new(),
			lengths: Vec::new(),
			text: Vec::new(),
			out: Vec::new(),
		})
	}

	/// Forget which fonts the host knows, for a host that cleared its own
	/// caches.
	pub(crate) fn resend_fonts(&mut self) {
		self.fonts_sent = 0;
	}
}

impl TextMeasurer for HostMeasurer {
	fn measure(&mut self, batch: &MeasureBatch<'_>, out: &mut MeasureOutput) {
		let fonts = batch.fonts.fonts();
		let fonts_json = json!(
			fonts[self.fonts_sent.min(fonts.len())..]
				.iter()
				.map(|font| {
					json!({
						"family": &*font.family,
						"size": font.size,
						"weight": font.weight,
						"italic": font.italic,
					})
				})
				.collect::<Vec<_>>()
		)
		.to_string();

		self.metrics.clear();
		self.metrics
			.extend(batch.metrics.iter().map(|id| id.index() as u32));
		self.fonts.clear();
		self.lengths.clear();
		self.text.clear();
		let mut advances = 0;

		for segment in batch.segments {
			let start = self.text.len();
			self.text.extend(segment.text.encode_utf16());
			self.fonts.push(segment.font.index() as u32);
			self.lengths.push((self.text.len() - start) as u32);
			advances += segment.text.chars().count();
		}

		self.out.clear();
		self.out.resize(self.metrics.len() * 2 + advances, 0.0);
		let request = MeasureRequest {
			first_font: self.fonts_sent as u32,
			fonts_json: fonts_json.as_ptr(),
			fonts_json_len: fonts_json.len(),
			metrics: self.metrics.as_ptr(),
			metrics_len: self.metrics.len(),
			segment_fonts: self.fonts.as_ptr(),
			segment_lengths: self.lengths.as_ptr(),
			segments_len: self.fonts.len(),
			text: self.text.as_ptr(),
			text_len: self.text.len(),
			out: self.out.as_mut_ptr(),
			out_len: self.out.len(),
		};

		// The host saw every font, even if it fails below: it registers fonts
		// before measuring.
		self.fonts_sent = fonts.len();
		(self.measure)(&raw const request);

		let (metrics, advances) = self.out.split_at(self.metrics.len() * 2);
		out.metrics.extend(metrics.chunks_exact(2).map(|pair| {
			FontMetrics {
				ascent: pair[0],
				descent: pair[1],
			}
		}));
		out.advances.extend_from_slice(advances);
	}
}
