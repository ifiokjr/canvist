//! CPU rasterisation with `vello_cpu`: PNG export, server thumbnails, and
//! the browser fallback where WebGPU is missing (the host copies the pixels
//! into a Canvas2D `ImageData`).

use kurbo::Affine;
use kurbo::Rect;
use peniko::Brush;
use peniko::Fill;
use peniko::ImageAlphaType;
use rustc_hash::FxHashMap;
use vello_cpu::Image as CpuImage;
use vello_cpu::ImageSource;
use vello_cpu::PaintType;
use vello_cpu::Pixmap;
use vello_cpu::RenderContext;
use vello_cpu::Resources;

use crate::painter::BoxShadow;
use crate::painter::Draw;
use crate::painter::Image;
use crate::painter::Layer;
use crate::painter::Painter;
use crate::painter::Text;
use crate::text::Shaper;

/// Why a CPU render failed.
#[derive(Debug, thiserror::Error)]
pub enum CpuError {
	/// The image is wider or taller than `vello_cpu` supports.
	#[error("{0}×{1} is too large to render; both sides must fit in 16 bits")]
	TooLarge(u32, u32),
	/// PNG encoding failed.
	#[error("PNG encoding failed: {0}")]
	Png(String),
}

/// Paints into a `vello_cpu` render context.
pub struct CpuPainter {
	context: RenderContext,
	resources: Resources,
	shaper: Shaper,
	images: FxHashMap<String, ImageSource>,
}

impl std::fmt::Debug for CpuPainter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CpuPainter")
			.field("width", &self.context.width())
			.field("height", &self.context.height())
			.finish_non_exhaustive()
	}
}

impl CpuPainter {
	/// A painter for an image `width` by `height` pixels.
	pub fn new(width: u32, height: u32) -> Result<Self, CpuError> {
		let (Ok(w), Ok(h)) = (u16::try_from(width), u16::try_from(height)) else {
			return Err(CpuError::TooLarge(width, height));
		};

		Ok(Self {
			context: RenderContext::new(w.max(1), h.max(1)),
			resources: Resources::default(),
			shaper: Shaper::new(),
			images: FxHashMap::default(),
		})
	}

	/// Clear the canvas for a new frame of `width` by `height`, keeping the
	/// shaping and image caches.
	pub fn reset(&mut self, width: u32, height: u32) -> Result<(), CpuError> {
		let (Ok(w), Ok(h)) = (u16::try_from(width), u16::try_from(height)) else {
			return Err(CpuError::TooLarge(width, height));
		};
		self.context.reset_and_resize(w.max(1), h.max(1));
		Ok(())
	}

	/// The shaper, to register fonts.
	pub fn shaper_mut(&mut self) -> &mut Shaper {
		&mut self.shaper
	}

	/// Rasterise what was painted.
	pub fn render(&mut self) -> Pixmap {
		self.context.flush();
		let mut pixmap = Pixmap::new(self.context.width(), self.context.height());
		self.context.render(&mut pixmap, &mut self.resources);
		pixmap
	}

	/// Rasterise as straight-alpha RGBA8, the layout of a Canvas2D
	/// `ImageData`.
	pub fn render_rgba8(&mut self) -> Vec<u8> {
		self.render().take_rgba8(ImageAlphaType::Alpha)
	}

	/// Rasterise as a PNG file.
	pub fn render_png(&mut self) -> Result<Vec<u8>, CpuError> {
		self.render()
			.into_png()
			.map_err(|error| CpuError::Png(error.to_string()))
	}

	fn set_brush(&mut self, brush: &Brush, transform: Affine, brush_transform: Option<Affine>) {
		self.context.set_transform(transform);
		self.context
			.set_paint_transform(brush_transform.unwrap_or(Affine::IDENTITY));

		match brush {
			Brush::Solid(color) => self.context.set_paint(*color),
			Brush::Gradient(gradient) => self.context.set_paint(gradient.clone()),
			// Image paints arrive through `image`, not as brushes.
			Brush::Image(_) => self.context.set_paint(peniko::Color::TRANSPARENT),
		}
	}
}

impl Painter for CpuPainter {
	fn fill(&mut self, draw: &Draw<'_>, rule: Fill) {
		self.set_brush(draw.brush, draw.transform, draw.brush_transform);
		self.context.set_fill_rule(rule);
		self.context.fill_path(draw.path);
	}

	fn stroke(&mut self, draw: &Draw<'_>, style: &kurbo::Stroke) {
		self.set_brush(draw.brush, draw.transform, draw.brush_transform);
		self.context.set_stroke(style.clone());
		self.context.stroke_path(draw.path);
	}

	fn push_layer(&mut self, layer: &Layer<'_>) {
		self.context.set_transform(layer.transform);
		self.context.set_fill_rule(Fill::NonZero);
		self.context
			.push_layer(layer.clip, Some(layer.blend), Some(layer.alpha), None, None);
	}

	fn pop_layer(&mut self) {
		self.context.pop_layer();
	}

	fn image(&mut self, image: &Image<'_>) {
		let source = self
			.images
			.entry(image.asset.to_owned())
			.or_insert_with(|| ImageSource::from_peniko_image_data(image.data))
			.clone();
		let brush = CpuImage {
			image: source,
			sampler: peniko::ImageSampler::default().with_alpha(image.alpha),
		};
		self.context.set_transform(image.transform);
		self.context.set_paint_transform(Affine::IDENTITY);
		self.context.set_paint(PaintType::Image(brush));
		let size = Rect::new(
			0.0,
			0.0,
			f64::from(image.data.width),
			f64::from(image.data.height),
		);
		self.context.fill_rect(&size);
	}

	fn text(&mut self, text: &Text<'_>) {
		let run = text.run;
		let shaped = self.shaper.shape(text.font, run.text(), run.letter_spacing);
		let origin = Affine::translate((f64::from(run.x), f64::from(run.baseline)));
		self.context.set_transform(text.transform * origin);
		self.context.set_paint_transform(Affine::IDENTITY);
		self.context.set_paint(text.color);
		let mut offset = 0.0;

		for part in shaped.iter() {
			let glyphs = part.glyphs.iter().map(|glyph| {
				vello_cpu::Glyph {
					id: glyph.id,
					x: glyph.x + offset,
					y: glyph.y,
				}
			});
			// A glyph the font cannot draw leaves a gap; the rest still render.
			let _ = self
				.context
				.glyph_run(&mut self.resources, &part.font)
				.font_size(part.size)
				.fill_glyphs(glyphs);
			offset += part.glyphs.last().map_or(0.0, |glyph| glyph.x);
		}
	}

	fn shadow(&mut self, shadow: &BoxShadow) {
		self.context.set_transform(shadow.transform);
		self.context.set_paint(shadow.color);
		self.context.fill_blurred_rounded_rect(
			&shadow.rect,
			shadow.radius as f32,
			(shadow.blur / 2.0) as f32,
			false,
		);
	}
}
