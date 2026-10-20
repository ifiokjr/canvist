//! GPU painting with vello on wgpu: WebGPU in browsers, Metal, Vulkan, and
//! DX12 natively. This module encodes a `vello::Scene`; the host owns the
//! wgpu device and surface and renders the scene with `vello::Renderer`.

use std::sync::Arc;

use kurbo::Affine;
use kurbo::Shape as _;
use peniko::Blob;
use peniko::Fill;
use peniko::ImageBrush;
use rustc_hash::FxHashMap;
use vello::Scene;

use crate::painter::BoxShadow;
use crate::painter::Draw;
use crate::painter::Image;
use crate::painter::Layer;
use crate::painter::Painter;
use crate::painter::Text;
use crate::text::Shaper;

/// Half the side of the clip an unclipped layer uses, in screen pixels.
const UNBOUNDED: f64 = 1e9;
/// Flattening tolerance for that clip.
const TOLERANCE: f64 = 0.1;

/// Encodes painter calls into a `vello::Scene`.
pub struct VelloPainter {
	scene: Scene,
	shaper: Shaper,
	images: FxHashMap<String, peniko::ImageData>,
}

impl std::fmt::Debug for VelloPainter {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("VelloPainter").finish_non_exhaustive()
	}
}

impl Default for VelloPainter {
	fn default() -> Self {
		Self::new()
	}
}

impl VelloPainter {
	/// An empty painter.
	#[must_use]
	pub fn new() -> Self {
		Self {
			scene: Scene::new(),
			shaper: Shaper::new(),
			images: FxHashMap::default(),
		}
	}

	/// The encoded scene, for `vello::Renderer::render_to_texture`.
	#[must_use]
	pub fn scene(&self) -> &Scene {
		&self.scene
	}

	/// Clear the scene for a new frame, keeping caches.
	pub fn reset(&mut self) {
		self.scene.reset();
	}

	/// The shaper, to register fonts.
	pub fn shaper_mut(&mut self) -> &mut Shaper {
		&mut self.shaper
	}
}

impl Painter for VelloPainter {
	fn fill(&mut self, draw: &Draw<'_>, rule: Fill) {
		self.scene.fill(
			rule,
			draw.transform,
			draw.brush,
			draw.brush_transform,
			draw.path,
		);
	}

	fn stroke(&mut self, draw: &Draw<'_>, style: &kurbo::Stroke) {
		self.scene.stroke(
			style,
			draw.transform,
			draw.brush,
			draw.brush_transform,
			draw.path,
		);
	}

	fn push_layer(&mut self, layer: &Layer<'_>) {
		// vello layers always clip; an unclipped layer clips to everything.
		let everything;
		let (clip, transform) = if let Some(clip) = layer.clip {
			(clip, layer.transform)
		} else {
			everything =
				kurbo::Rect::new(-UNBOUNDED, -UNBOUNDED, UNBOUNDED, UNBOUNDED).to_path(TOLERANCE);
			(&everything, Affine::IDENTITY)
		};

		self.scene
			.push_layer(Fill::NonZero, layer.blend, layer.alpha, transform, clip);
	}

	fn pop_layer(&mut self) {
		self.scene.pop_layer();
	}

	fn image(&mut self, image: &Image<'_>) {
		// vello keys its image atlas by blob id, so reuse one blob per asset.
		let data = self
			.images
			.entry(image.asset.to_owned())
			.or_insert_with(|| {
				let mut data = image.data.clone();
				data.data = Blob::new(Arc::new(image.data.data.data().to_vec()));
				data
			})
			.clone();
		let brush = ImageBrush::new(data).with_alpha(image.alpha);
		self.scene.draw_image(&brush, image.transform);
	}

	fn text(&mut self, text: &Text<'_>) {
		let run = text.run;
		let shaped = self.shaper.shape(text.font, run.text(), run.letter_spacing);
		let origin = Affine::translate((f64::from(run.x), f64::from(run.baseline)));
		let mut offset = 0.0;

		for part in shaped.iter() {
			let glyphs = part.glyphs.iter().map(|glyph| {
				vello::Glyph {
					id: glyph.id,
					x: glyph.x + offset,
					y: glyph.y,
				}
			});
			self.scene
				.draw_glyphs(&part.font)
				.font_size(part.size)
				.transform(text.transform * origin)
				.brush(text.color)
				.draw(Fill::NonZero, glyphs);
			offset += part.glyphs.last().map_or(0.0, |glyph| glyph.x);
		}
	}

	fn shadow(&mut self, shadow: &BoxShadow) {
		self.scene.draw_blurred_rounded_rect(
			shadow.transform,
			shadow.rect,
			shadow.color,
			shadow.radius,
			shadow.blur / 2.0,
		);
	}
}
