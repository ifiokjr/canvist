//! Canvas2D rendering backend.
//!
//! Implements the [`canvist_render::Canvas`] and [`canvist_render::Renderer`]
//! traits for the browser's `CanvasRenderingContext2d`.

use canvist_core::Color;
use canvist_core::Style;
use canvist_render::Canvas;
use canvist_render::Rect;
use canvist_render::Renderer;
use canvist_render::Viewport;
use web_sys::CanvasRenderingContext2d;

/// A rendering backend that draws to an HTML Canvas 2D context.
pub struct Canvas2dRenderer {
	/// The 2D rendering context.
	ctx: CanvasRenderingContext2d,
	/// Current viewport state.
	viewport: Viewport,
}

#[allow(dead_code)]
impl Canvas2dRenderer {
	/// Wrap an existing `CanvasRenderingContext2d`.
	#[must_use]
	pub fn new(ctx: CanvasRenderingContext2d, width: f32, height: f32) -> Self {
		Self {
			ctx,
			viewport: Viewport::new(width, height),
		}
	}
}

impl Canvas for Canvas2dRenderer {
	fn fill_rect(&mut self, rect: Rect, color: Color) {
		self.ctx.set_fill_style_str(&color.to_css());
		self.ctx.fill_rect(
			f64::from(rect.x),
			f64::from(rect.y),
			f64::from(rect.width),
			f64::from(rect.height),
		);
	}

	fn draw_text(&mut self, x: f32, y: f32, text: &str, style: &Style) {
		let resolved = style.resolve();

		let weight = if resolved.font_weight.as_u16() >= 700 {
			"bold "
		} else {
			""
		};
		let italic = if resolved.italic { "italic " } else { "" };
		let font = format!(
			"{italic}{weight}{}px {}",
			resolved.font_size, resolved.font_family
		);

		self.ctx.set_font(&font);
		self.ctx.set_fill_style_str(&resolved.color.to_css());
		let _ = self.ctx.fill_text(text, f64::from(x), f64::from(y));
	}

	fn clear(&mut self, color: Color) {
		let w = f64::from(self.viewport.size.width);
		let h = f64::from(self.viewport.size.height);
		self.ctx.set_fill_style_str(&color.to_css());
		self.ctx.fill_rect(0.0, 0.0, w, h);
	}

	fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color) {
		self.ctx.begin_path();
		self.ctx.set_stroke_style_str(&color.to_css());
		self.ctx.move_to(f64::from(x1), f64::from(y1));
		self.ctx.line_to(f64::from(x2), f64::from(y2));
		self.ctx.stroke();
	}

	fn save(&mut self) {
		self.ctx.save();
	}

	fn restore(&mut self) {
		self.ctx.restore();
	}

	fn translate(&mut self, dx: f32, dy: f32) {
		let _ = self.ctx.translate(f64::from(dx), f64::from(dy));
	}
}

impl Renderer for Canvas2dRenderer {
	fn viewport(&self) -> &Viewport {
		&self.viewport
	}

	fn set_viewport(&mut self, viewport: Viewport) {
		self.viewport = viewport;
	}
}
