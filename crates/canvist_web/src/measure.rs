//! Text measurement by the browser, one JavaScript call per relayout.
//!
//! Layout batches every segment it is missing into one
//! [`MeasureBatch`](canvist_layout::MeasureBatch) and caches the results by
//! (font, segment), so this measurer crosses into JavaScript once per batch.
//! The JavaScript side (`CanvasMeasurer` in `@canvist/web`) runs
//! `measureText` on a canvas configured exactly like the painting canvas,
//! so a segment is as wide here as `fillText` paints it and as the browser
//! lays it out in server-rendered HTML.

use canvist_layout::FontId;
use canvist_layout::FontMetrics;
use canvist_layout::LineBox;
use canvist_layout::MeasureBatch;
use canvist_layout::MeasureOutput;
use canvist_layout::TextMeasurer;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const HOST_MEASURER: &str = r#"
/** The JavaScript half of text measurement; `@canvist/web` implements it with a canvas. */
export interface HostMeasurer {
	addFont(id: number, css: string): void;
	paintFont(id: number): string;
	measure(
		metrics: Uint32Array,
		lineFonts: Uint32Array,
		lineHeights: Float32Array,
		fonts: Uint32Array,
		text: string,
		lengths: Uint32Array,
	): Float32Array;
}
"#;

#[wasm_bindgen]
extern "C" {
	/// The JavaScript half of measurement.
	///
	/// - `addFont(id, css)` registers a layout font by id, with its CSS `font`
	///   shorthand. Ids only grow, and each is added once.
	/// - `paintFont(id)` is the `font` string to draw font `id` with on a
	///   canvas: one the browser resolves to the face its DOM uses for `css`,
	///   which is not always `css` itself (WebKit's canvas and DOM resolve
	///   some system font keywords differently).
	/// - `measure(metrics, lineFonts, lineHeights, fonts, text, lengths)`
	///   measures one batch. `metrics` lists font ids whose content-area ascent
	///   and descent are needed; `lineFonts` and `lineHeights` list inline
	///   boxes (a font at a unitless line height) whose space above and below
	///   the baseline is needed. Segment `i` is the next `lengths[i]` UTF-16
	///   code units of `text`, in font `fonts[i]`. It returns each font's
	///   ascent and descent, then each inline box's above and below, then one
	///   advance per Unicode scalar value of every segment, in order.
	#[wasm_bindgen(typescript_type = "HostMeasurer")]
	pub type HostMeasurer;

	#[wasm_bindgen(method, js_name = addFont)]
	fn add_font(this: &HostMeasurer, id: u32, css: &str);

	#[wasm_bindgen(method, js_name = paintFont)]
	fn paint_font(this: &HostMeasurer, id: u32) -> String;

	#[wasm_bindgen(method)]
	fn measure(
		this: &HostMeasurer,
		metrics: &[u32],
		line_fonts: &[u32],
		line_heights: &[f32],
		fonts: &[u32],
		text: &str,
		lengths: &[u32],
	) -> Vec<f32>;
}

/// A [`TextMeasurer`] backed by the browser through a [`HostMeasurer`].
pub(crate) struct BrowserMeasurer {
	host: HostMeasurer,
	/// How many of the batch's fonts the host already knows.
	fonts_sent: usize,
	/// Buffers reused across batches.
	metrics: Vec<u32>,
	line_fonts: Vec<u32>,
	line_heights: Vec<f32>,
	fonts: Vec<u32>,
	text: String,
	lengths: Vec<u32>,
	/// How many segments and batches were measured, for statistics.
	segments: u64,
	calls: u64,
}

impl BrowserMeasurer {
	pub(crate) fn new(host: HostMeasurer) -> Self {
		Self {
			host,
			fonts_sent: 0,
			metrics: Vec::new(),
			line_fonts: Vec::new(),
			line_heights: Vec::new(),
			fonts: Vec::new(),
			text: String::new(),
			lengths: Vec::new(),
			segments: 0,
			calls: 0,
		}
	}

	/// The canvas `font` string for font `id`.
	pub(crate) fn paint_font(&self, id: FontId) -> String {
		self.host.paint_font(id.index() as u32)
	}

	/// Segments measured over the measurer's life.
	pub(crate) fn segments(&self) -> u64 {
		self.segments
	}

	/// Batches measured over the measurer's life.
	pub(crate) fn calls(&self) -> u64 {
		self.calls
	}
}

impl TextMeasurer for BrowserMeasurer {
	fn measure(&mut self, batch: &MeasureBatch<'_>, out: &mut MeasureOutput) {
		let fonts = batch.fonts.fonts();

		for (id, font) in fonts.iter().enumerate().skip(self.fonts_sent) {
			self.host.add_font(id as u32, &font.css());
		}

		self.fonts_sent = fonts.len();
		self.metrics.clear();
		self.metrics
			.extend(batch.metrics.iter().map(|id| id.index() as u32));
		self.line_fonts.clear();
		self.line_heights.clear();

		for &(font, line_height) in batch.line_boxes {
			self.line_fonts.push(font.index() as u32);
			self.line_heights.push(line_height);
		}

		self.fonts.clear();
		self.text.clear();
		self.lengths.clear();

		for segment in batch.segments {
			self.fonts.push(segment.font.index() as u32);
			self.text.push_str(segment.text);
			self.lengths
				.push(segment.text.encode_utf16().count() as u32);
		}

		let values = self.host.measure(
			&self.metrics,
			&self.line_fonts,
			&self.line_heights,
			&self.fonts,
			&self.text,
			&self.lengths,
		);
		// A short answer leaves the outputs short, which layout reports as a
		// broken measurer contract.
		let (metrics, rest) = values.split_at(values.len().min(self.metrics.len() * 2));
		let (line_boxes, advances) = rest.split_at(rest.len().min(self.line_fonts.len() * 2));

		out.metrics.extend(metrics.chunks_exact(2).map(|pair| {
			FontMetrics {
				ascent: pair[0],
				descent: pair[1],
			}
		}));
		out.line_boxes
			.extend(line_boxes.chunks_exact(2).map(|pair| {
				LineBox {
					above: pair[0],
					below: pair[1],
				}
			}));
		out.advances.extend_from_slice(advances);
		self.segments += batch.segments.len() as u64;
		self.calls += 1;
	}
}
