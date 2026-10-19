//! Painting a display list on an opaque Canvas2D context.
//!
//! The painter's job is to draw exactly what the browser would draw for the
//! same document as HTML:
//!
//! - **An opaque canvas.** The context is created with `alpha: false` and
//!   every repaint fills the theme's background first, so the browser can
//!   antialias text the way it does for DOM text on an opaque layer.
//! - **Pinned text properties.** `fontKerning`, `fontVariantCaps`,
//!   `textRendering`, `wordSpacing`, and `letterSpacing` are set to the
//!   values the CSS contract pins on `.cv-doc`, so `fillText` shapes like the
//!   HTML does.
//! - **The browser's pixel grid.** The backing store is the element's size
//!   in device pixels. Baselines and rectangle edges snap to the grid the
//!   browser snaps DOM text and backgrounds to ([`PixelGrid`]); horizontal
//!   glyph positions keep their fractions, as in the DOM.
//! - **Dirty rectangles.** A repaint can be limited to a few rectangles: they
//!   are cleared and every item that touches them is drawn again, clipped.

use canvist_layout::Color;
use canvist_layout::DecorationKind;
use canvist_layout::DisplayItem;
use canvist_layout::DisplayList;
use canvist_layout::FontId;
use canvist_layout::Rect;
use canvist_layout::SlotKind;
use canvist_layout::TextRun;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::CanvasRenderingContext2d;
use web_sys::ContextAttributes2d;
use web_sys::HtmlCanvasElement;

/// How far glyphs may reach outside their line box, as a multiple of the
/// largest font size on screen: accents above tight line heights,
/// descenders, and italic overhangs. Dirty rectangles grow by this much, so
/// clearing one never leaves half a glyph of a neighbour behind.
const OVERHANG_PER_FONT_SIZE: f32 = 0.5;

/// The task checkbox's border and checked fill, matching the HTML
/// renderer's `.cv-check` rules.
const CHECKBOX_BORDER: &str = "#8a8580";
const CHECKBOX_FILL: &str = "#2e2b29";
const CHECKBOX_RADIUS: f64 = 3.0;

/// The squiggle's swing, as in the SVG renderer.
const SQUIGGLE_AMPLITUDE_PER_THICKNESS: f32 = 1.5;
const SQUIGGLE_WAVELENGTH_PER_AMPLITUDE: f32 = 4.0;

#[wasm_bindgen]
extern "C" {
	/// The Canvas2D text properties `web-sys` does not bind yet.
	#[wasm_bindgen(extends = CanvasRenderingContext2d)]
	type TextContext;

	#[wasm_bindgen(method, setter = fontKerning)]
	fn set_font_kerning(this: &TextContext, value: &str);

	#[wasm_bindgen(method, setter = fontVariantCaps)]
	fn set_font_variant_caps(this: &TextContext, value: &str);

	#[wasm_bindgen(method, setter = textRendering)]
	fn set_text_rendering(this: &TextContext, value: &str);

	#[wasm_bindgen(method, setter = wordSpacing)]
	fn set_word_spacing(this: &TextContext, value: &str);

	#[wasm_bindgen(method, setter = letterSpacing)]
	fn set_letter_spacing(this: &TextContext, value: &str);

	#[wasm_bindgen(method, setter = direction)]
	fn set_direction(this: &TextContext, value: &str);
}

/// The canvas's size: its CSS box and its backing store in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Surface {
	/// The CSS width and height of the canvas element.
	pub(crate) width: f32,
	pub(crate) height: f32,
	/// Device pixels per CSS pixel.
	pub(crate) scale: f32,
	/// The backing store's size, from `devicePixelContentBoxSize` where the
	/// browser reports it, so the canvas maps one to one onto the screen.
	pub(crate) pixel_width: u32,
	pub(crate) pixel_height: u32,
}

/// The grid a browser snaps DOM text baselines and background edges to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PixelGrid {
	/// Whole device pixels: Firefox, WebKit, and Chromium on real
	/// high-density screens, where it lays out in device units.
	#[default]
	Device,
	/// Whole CSS pixels, then scaled: Chromium when it lays out in CSS units
	/// on a high-density screen, as under device emulation.
	Css,
}

/// Which part of the canvas to repaint.
pub(crate) enum Region<'a> {
	/// Everything.
	All,
	/// Only these rectangles, in document coordinates.
	Rects(&'a [Rect]),
}

/// Context state the painter last set, so unchanged properties are not set
/// again for every run.
#[derive(Default)]
struct State {
	font: Option<FontId>,
	fill: Option<Color>,
	letter_spacing: Option<u32>,
	rtl: Option<bool>,
}

/// Paints display lists on one canvas.
pub(crate) struct Painter {
	canvas: HtmlCanvasElement,
	context: CanvasRenderingContext2d,
	surface: Surface,
	/// The CSS `font` shorthand of each font id, built once.
	fonts: Vec<String>,
	/// Families whose text is laid out but not drawn, like DOM text during a
	/// `font-display` block period, and whether each font id lists one.
	hidden_families: Vec<String>,
	hidden: Vec<bool>,
	grid: PixelGrid,
	/// Colour strings, built once per colour.
	colors: rustc_hash::FxHashMap<Color, String>,
	state: State,
	/// Items drawn by the last paint, for statistics.
	drawn: u32,
}

impl Painter {
	/// A painter for `canvas`, with an opaque 2D context.
	pub(crate) fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
		let attributes = ContextAttributes2d::new();
		attributes.set_alpha(false);
		let context = canvas
			.get_context_with_context_options("2d", &attributes)?
			.ok_or_else(|| JsValue::from_str("the canvas has no 2D context"))?
			.dyn_into::<CanvasRenderingContext2d>()?;

		let mut painter = Self {
			canvas,
			context,
			surface: Surface {
				width: 0.0,
				height: 0.0,
				scale: 1.0,
				pixel_width: 0,
				pixel_height: 0,
			},
			fonts: Vec::new(),
			hidden_families: Vec::new(),
			hidden: Vec::new(),
			grid: PixelGrid::default(),
			colors: rustc_hash::FxHashMap::default(),
			state: State::default(),
			drawn: 0,
		};
		painter.reset_context();
		Ok(painter)
	}

	pub(crate) fn surface(&self) -> Surface {
		self.surface
	}

	/// Lay out but do not draw text in `families`. Returns whether the set
	/// changed, in which case the canvas needs a full repaint.
	pub(crate) fn hide_families(&mut self, families: Vec<String>) -> bool {
		if families == self.hidden_families {
			return false;
		}

		self.hidden_families = families;
		self.fonts.clear();
		self.hidden.clear();
		true
	}

	/// Forget every font string, as when a web font loads and a family list
	/// may resolve to another face.
	pub(crate) fn forget_fonts(&mut self) {
		self.fonts.clear();
		self.hidden.clear();
		self.state.font = None;
	}

	/// Snap to `grid`. Returns whether that changed, in which case the
	/// canvas needs a full repaint.
	pub(crate) fn set_grid(&mut self, grid: PixelGrid) -> bool {
		let changed = grid != self.grid;
		self.grid = grid;
		changed
	}

	/// Items drawn by the last paint.
	pub(crate) fn drawn(&self) -> u32 {
		self.drawn
	}

	/// Resize the backing store. Returns whether anything changed; resizing
	/// clears the canvas, so the next paint must cover everything.
	pub(crate) fn resize(&mut self, surface: Surface) -> bool {
		if surface == self.surface {
			return false;
		}

		if (surface.pixel_width, surface.pixel_height)
			!= (self.surface.pixel_width, self.surface.pixel_height)
		{
			self.canvas.set_width(surface.pixel_width);
			self.canvas.set_height(surface.pixel_height);
		}

		self.surface = surface;
		// Resizing the backing store resets every context property.
		self.reset_context();
		true
	}

	/// Set the properties that never change between paints.
	fn reset_context(&mut self) {
		let context = &self.context;
		let text: &TextContext = context.unchecked_ref();
		text.set_font_kerning("normal");
		text.set_font_variant_caps("normal");
		text.set_text_rendering("auto");
		text.set_word_spacing("0px");
		context.set_text_align("left");
		context.set_text_baseline("alphabetic");

		self.state = State::default();
	}

	/// Paint `region` of `list`, whose document coordinates are shifted up by
	/// `top` so the viewport starts at the canvas's top edge.
	///
	/// `font` gives the canvas `font` string of each font id the painter has
	/// not drawn with yet.
	pub(crate) fn paint(
		&mut self,
		list: &DisplayList,
		top: f32,
		background: Color,
		region: &Region<'_>,
		font: &dyn Fn(FontId) -> String,
	) {
		self.sync_fonts(list, font);
		// A second handle to the same JavaScript object, so drawing helpers
		// can borrow the painter mutably.
		let context = self.context.clone();
		let scale = f64::from(self.surface.scale);
		let _ = context.set_transform(scale, 0.0, 0.0, scale, 0.0, 0.0);
		self.state = State::default();
		self.drawn = 0;

		match region {
			Region::All => self.paint_all(list, top, background),
			Region::Rects(rects) => self.paint_rects(list, top, background, rects),
		}
	}

	/// Clear the whole canvas to `background` and draw every item.
	fn paint_all(&mut self, list: &DisplayList, top: f32, background: Color) {
		self.fill_surface(background);

		for item in &list.items {
			self.item(item, top);
		}
	}

	/// Clip to `rects`, grown by the list's overhang so glyphs that spill
	/// past their boxes repaint whole, then clear and redraw only the items
	/// that touch them.
	fn paint_rects(&mut self, list: &DisplayList, top: f32, background: Color, rects: &[Rect]) {
		let overhang = overhang(list);
		let rects: Vec<Rect> = rects.iter().map(|rect| inflate(*rect, overhang)).collect();
		let context = self.context.clone();

		context.save();
		self.clip_to(&rects, top);
		self.fill_surface(background);

		for item in &list.items {
			let bounds = inflate(item.bounds(), overhang);

			if rects.iter().any(|rect| rect.intersects(&bounds)) {
				self.item(item, top);
			}
		}

		context.restore();
		self.state = State::default();
	}

	/// Clip the context to `rects`, given in document coordinates, each
	/// snapped outwards to whole device pixels.
	fn clip_to(&self, rects: &[Rect], top: f32) {
		let context = &self.context;
		context.begin_path();

		for rect in rects {
			let snapped = self.snap_out(Rect::new(rect.x, rect.y - top, rect.width, rect.height));
			context.rect(
				f64::from(snapped.x),
				f64::from(snapped.y),
				f64::from(snapped.width),
				f64::from(snapped.height),
			);
		}

		context.clip();
	}

	/// Fill the whole backing store, within any clip, with `background`.
	/// It is the CSS size rounded to whole device pixels, so its last row
	/// or column can reach past the CSS size, and an opaque canvas shows
	/// black wherever nothing was drawn.
	fn fill_surface(&mut self, background: Color) {
		let scale = f64::from(self.surface.scale);
		self.fill_color(background);
		self.context.fill_rect(
			0.0,
			0.0,
			f64::from(self.surface.width).max(f64::from(self.surface.pixel_width) / scale),
			f64::from(self.surface.height).max(f64::from(self.surface.pixel_height) / scale),
		);
	}

	fn item(&mut self, item: &DisplayItem, top: f32) {
		self.drawn += 1;

		match item {
			DisplayItem::Rect {
				rect,
				color,
				radius,
				..
			} => {
				let rect = self.snap(Rect::new(rect.x, rect.y - top, rect.width, rect.height));
				self.fill_color(*color);
				self.fill_rect(rect, *radius);
			}
			DisplayItem::Text(run) => self.text(run, top),
			DisplayItem::Decoration {
				kind: DecorationKind::Squiggle,
				x,
				y,
				width,
				thickness,
				color,
				..
			} => self.squiggle((*x, *y - top), *width, *thickness, *color),
			DisplayItem::Decoration {
				x,
				y,
				width,
				thickness,
				color,
				..
			} => {
				let line = self.snap(Rect::new(*x, y - top - thickness / 2.0, *width, *thickness));
				self.fill_color(*color);
				self.fill_rect(line, 0.0);
			}
			DisplayItem::Slot {
				kind: SlotKind::Checkbox { checked },
				rect,
				..
			} => {
				self.checkbox(
					Rect::new(rect.x, rect.y - top, rect.width, rect.height),
					*checked,
				);
			}
			// Images and other node views are host elements laid over the
			// canvas; the slot only reserves their space.
			DisplayItem::Slot { .. } => {}
		}
	}

	/// Draw one text run, skipping fonts that are hidden or unknown. Each
	/// context property is set only when it differs from what the last run
	/// left behind, since setting the `font` string re-parses it even when it
	/// is unchanged.
	fn text(&mut self, run: &TextRun, top: f32) {
		let index = run.font.index();

		if index >= self.fonts.len() || self.hidden[index] {
			return;
		}

		let context = self.context.clone();
		let text: &TextContext = context.unchecked_ref();

		if self.state.font != Some(run.font) {
			context.set_font(&self.fonts[index]);
			self.state.font = Some(run.font);
		}

		let spacing = run.letter_spacing.to_bits();

		if self.state.letter_spacing != Some(spacing) {
			text.set_letter_spacing(&format!("{}px", canvist_layout::css_number(run.letter_spacing)));
			self.state.letter_spacing = Some(spacing);
		}

		if self.state.rtl != Some(run.rtl) {
			text.set_direction(if run.rtl { "rtl" } else { "ltr" });
			self.state.rtl = Some(run.rtl);
		}

		self.fill_color(run.color);
		let baseline = self.round(run.baseline - top);
		let _ = context.fill_text(run.text(), f64::from(run.x), f64::from(baseline));
	}

	fn squiggle(&mut self, (x, y): (f32, f32), width: f32, thickness: f32, color: Color) {
		let amplitude = thickness * SQUIGGLE_AMPLITUDE_PER_THICKNESS;
		let wave = amplitude * SQUIGGLE_WAVELENGTH_PER_AMPLITUDE;
		let context = self.context.clone();
		context.begin_path();
		context.move_to(f64::from(x), f64::from(y));
		let mut position = 0.0;
		let mut up = true;

		while position < width {
			let step = (wave / 2.0).min(width - position);
			let peak = if up { -amplitude } else { amplitude };
			context.quadratic_curve_to(
				f64::from(x + position + step / 2.0),
				f64::from(y + peak),
				f64::from(x + position + step),
				f64::from(y),
			);
			position += step;
			up = !up;
		}

		let color = self.color(color);
		context.set_stroke_style_str(&color);
		context.set_line_width(f64::from(thickness));
		context.stroke();
	}

	fn checkbox(&mut self, rect: Rect, checked: bool) {
		let rect = self.snap(rect);
		self.state.fill = None;

		if checked {
			self.context.set_fill_style_str(CHECKBOX_FILL);
			self.fill_rect(rect, CHECKBOX_RADIUS as f32);
		}

		// A 1 px border inside the box, as `box-sizing: border-box` draws it.
		let inner = Rect::new(
			rect.x + 0.5,
			rect.y + 0.5,
			rect.width - 1.0,
			rect.height - 1.0,
		);
		self.context.set_stroke_style_str(CHECKBOX_BORDER);
		self.context.set_line_width(1.0);
		self.rounded_path(inner, CHECKBOX_RADIUS as f32 - 0.5);
		self.context.stroke();
	}

	fn fill_rect(&self, rect: Rect, radius: f32) {
		if radius <= 0.0 {
			self.context.fill_rect(
				f64::from(rect.x),
				f64::from(rect.y),
				f64::from(rect.width),
				f64::from(rect.height),
			);

			return;
		}

		self.rounded_path(rect, radius);
		self.context.fill();
	}

	fn rounded_path(&self, rect: Rect, radius: f32) {
		let radius = f64::from(radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0));
		let (x, y) = (f64::from(rect.x), f64::from(rect.y));
		let (right, bottom) = (f64::from(rect.right()), f64::from(rect.bottom()));
		let context = &self.context;

		context.begin_path();
		context.move_to(x + radius, y);
		let _ = context.arc_to(right, y, right, bottom, radius);
		let _ = context.arc_to(right, bottom, x, bottom, radius);
		let _ = context.arc_to(x, bottom, x, y, radius);
		let _ = context.arc_to(x, y, right, y, radius);
		context.close_path();
	}

	fn fill_color(&mut self, color: Color) {
		if self.state.fill == Some(color) {
			return;
		}

		let css = self.color(color);
		self.context.set_fill_style_str(&css);
		self.state.fill = Some(color);
	}

	fn color(&mut self, color: Color) -> String {
		self.colors
			.entry(color)
			.or_insert_with(|| color.to_string())
			.clone()
	}

	/// Build the canvas font string of every font id not seen yet.
	fn sync_fonts(&mut self, list: &DisplayList, css: &dyn Fn(FontId) -> String) {
		for (index, font) in list.fonts.iter().enumerate().skip(self.fonts.len()) {
			self.fonts.push(css(FontId::from_index(index)));
			let hidden = self
				.hidden_families
				.iter()
				.any(|family| font.uses_family(family));
			self.hidden.push(hidden);
		}
	}

	/// Grid lines per CSS pixel.
	fn grid_scale(&self) -> f32 {
		match self.grid {
			PixelGrid::Device => self.surface.scale,
			PixelGrid::Css => 1.0,
		}
	}

	/// `value` rounded to the pixel grid.
	fn round(&self, value: f32) -> f32 {
		let scale = self.grid_scale();
		(value * scale).round() / scale
	}

	/// `rect` with each edge rounded to the pixel grid, as browsers snap
	/// backgrounds and borders.
	fn snap(&self, rect: Rect) -> Rect {
		let (left, top) = (self.round(rect.x), self.round(rect.y));
		Rect::new(
			left,
			top,
			self.round(rect.right()) - left,
			self.round(rect.bottom()) - top,
		)
	}

	/// `rect` grown outwards to whole device pixels.
	fn snap_out(&self, rect: Rect) -> Rect {
		let scale = self.surface.scale;
		let (left, top) = (
			(rect.x * scale).floor() / scale,
			(rect.y * scale).floor() / scale,
		);
		let right = (rect.right() * scale).ceil() / scale;
		let bottom = (rect.bottom() * scale).ceil() / scale;
		Rect::new(left, top, right - left, bottom - top)
	}
}

/// The overhang allowance for `list`: half its largest font size.
fn overhang(list: &DisplayList) -> f32 {
	list.fonts.iter().map(|font| font.size).fold(0.0, f32::max) * OVERHANG_PER_FONT_SIZE
}

fn inflate(rect: Rect, by: f32) -> Rect {
	Rect::new(
		rect.x - by,
		rect.y - by,
		rect.width + 2.0 * by,
		rect.height + 2.0 * by,
	)
}
