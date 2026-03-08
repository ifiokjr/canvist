//! Abstract canvas drawing primitives.
//!
//! The [`Canvas`] trait defines the minimal set of drawing operations that
//! every rendering backend must support. The editor's rendering pipeline only
//! talks through this trait, making it straightforward to swap backends.

use canvist_core::Color;
use canvist_core::Style;
use serde::Deserialize;
use serde::Serialize;

/// A rectangle defined by its top-left corner, width, and height.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
	/// X coordinate of the top-left corner.
	pub x: f32,
	/// Y coordinate of the top-left corner.
	pub y: f32,
	/// Width of the rectangle.
	pub width: f32,
	/// Height of the rectangle.
	pub height: f32,
}

impl Rect {
	/// Create a new rectangle.
	#[must_use]
	pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
		Self {
			x,
			y,
			width,
			height,
		}
	}

	/// The right edge (x + width).
	#[must_use]
	pub fn right(&self) -> f32 {
		self.x + self.width
	}

	/// The bottom edge (y + height).
	#[must_use]
	pub fn bottom(&self) -> f32 {
		self.y + self.height
	}

	/// Whether this rectangle contains a point.
	#[must_use]
	pub fn contains(&self, x: f32, y: f32) -> bool {
		x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
	}

	/// Whether this rectangle intersects another.
	#[must_use]
	pub fn intersects(&self, other: &Self) -> bool {
		self.x < other.right()
			&& self.right() > other.x
			&& self.y < other.bottom()
			&& self.bottom() > other.y
	}
}

/// A size in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Size {
	/// Width.
	pub width: f32,
	/// Height.
	pub height: f32,
}

impl Size {
	/// Create a new size.
	#[must_use]
	pub const fn new(width: f32, height: f32) -> Self {
		Self { width, height }
	}
}

/// The abstract canvas that rendering backends implement.
///
/// Each method corresponds to a basic drawing primitive. The editor calls these
/// methods during its render pass; the backend translates them to actual
/// graphics API calls (`Canvas2D`, `WebGL`, Skia, etc.).
pub trait Canvas {
	/// Fill a rectangle with a solid color.
	fn fill_rect(&mut self, rect: Rect, color: Color);

	/// Draw a string of text at the given position.
	///
	/// The `(x, y)` coordinates refer to the top-left of the text's bounding
	/// box. The style dictates font, size, color, and decorations.
	fn draw_text(&mut self, x: f32, y: f32, text: &str, style: &Style);

	/// Clear the entire canvas to a solid color.
	fn clear(&mut self, color: Color);

	/// Draw a horizontal line (e.g. for underlines or strikethroughs).
	fn draw_line(&mut self, _x1: f32, _y1: f32, _x2: f32, _y2: f32, _color: Color) {
		// Default no-op — backends override as needed.
	}

	/// Save the current canvas state (transform, clip, etc.).
	fn save(&mut self) {
		// Default no-op.
	}

	/// Restore the previously saved canvas state.
	fn restore(&mut self) {
		// Default no-op.
	}

	/// Translate the canvas origin by `(dx, dy)`.
	fn translate(&mut self, _dx: f32, _dy: f32) {
		// Default no-op.
	}

	/// Clip drawing to the given rectangle.
	fn clip(&mut self, _rect: Rect) {
		// Default no-op.
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn rect_contains() {
		let r = Rect::new(10.0, 20.0, 100.0, 50.0);
		assert!(r.contains(50.0, 40.0));
		assert!(!r.contains(5.0, 40.0));
		assert!(!r.contains(50.0, 80.0));
	}

	#[test]
	fn rect_intersects() {
		let a = Rect::new(0.0, 0.0, 100.0, 100.0);
		let b = Rect::new(50.0, 50.0, 100.0, 100.0);
		let c = Rect::new(200.0, 200.0, 10.0, 10.0);

		assert!(a.intersects(&b));
		assert!(b.intersects(&a));
		assert!(!a.intersects(&c));
	}
}
