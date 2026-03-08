//! Viewport and scroll position management.
//!
//! The [`Viewport`] tracks the visible area of the document and the current
//! scroll offset. The renderer uses it to determine which parts of the document
//! need to be drawn and to cull off-screen content.

use serde::Deserialize;
use serde::Serialize;

use crate::Rect;
use crate::Size;

/// The visible area of the document.
///
/// The viewport defines what portion of the infinite document canvas is
/// currently visible to the user, taking into account scroll position and
/// zoom level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
	/// Width and height of the visible area in logical pixels.
	pub size: Size,
	/// Horizontal scroll offset.
	pub scroll_x: f32,
	/// Vertical scroll offset.
	pub scroll_y: f32,
	/// Zoom level (1.0 = 100%).
	pub zoom: f32,
}

impl Viewport {
	/// Create a new viewport with the given dimensions.
	#[must_use]
	pub fn new(width: f32, height: f32) -> Self {
		Self {
			size: Size::new(width, height),
			scroll_x: 0.0,
			scroll_y: 0.0,
			zoom: 1.0,
		}
	}

	/// Return the visible rectangle in document coordinates.
	#[must_use]
	pub fn visible_rect(&self) -> Rect {
		Rect::new(
			self.scroll_x,
			self.scroll_y,
			self.size.width / self.zoom,
			self.size.height / self.zoom,
		)
	}

	/// Scroll by a delta in logical pixels.
	pub fn scroll_by(&mut self, dx: f32, dy: f32) {
		self.scroll_x += dx;
		self.scroll_y += dy;
		// Clamp to non-negative.
		self.scroll_x = self.scroll_x.max(0.0);
		self.scroll_y = self.scroll_y.max(0.0);
	}

	/// Set the zoom level, clamped to [0.1, 10.0].
	pub fn set_zoom(&mut self, zoom: f32) {
		self.zoom = zoom.clamp(0.1, 10.0);
	}

	/// Zoom in by a factor.
	pub fn zoom_in(&mut self, factor: f32) {
		self.set_zoom(self.zoom * factor);
	}

	/// Zoom out by a factor.
	pub fn zoom_out(&mut self, factor: f32) {
		self.set_zoom(self.zoom / factor);
	}

	/// Resize the viewport (e.g. when the window is resized).
	pub fn resize(&mut self, width: f32, height: f32) {
		self.size = Size::new(width, height);
	}

	/// Convert a point from screen coordinates to document coordinates.
	#[must_use]
	pub fn screen_to_document(&self, screen_x: f32, screen_y: f32) -> (f32, f32) {
		let doc_x = screen_x / self.zoom + self.scroll_x;
		let doc_y = screen_y / self.zoom + self.scroll_y;
		(doc_x, doc_y)
	}

	/// Convert a point from document coordinates to screen coordinates.
	#[must_use]
	pub fn document_to_screen(&self, doc_x: f32, doc_y: f32) -> (f32, f32) {
		let screen_x = (doc_x - self.scroll_x) * self.zoom;
		let screen_y = (doc_y - self.scroll_y) * self.zoom;
		(screen_x, screen_y)
	}
}

impl Default for Viewport {
	fn default() -> Self {
		Self::new(1024.0, 768.0)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn visible_rect_at_default() {
		let vp = Viewport::new(800.0, 600.0);
		let rect = vp.visible_rect();
		assert!((rect.x).abs() < f32::EPSILON);
		assert!((rect.y).abs() < f32::EPSILON);
		assert!((rect.width - 800.0).abs() < f32::EPSILON);
		assert!((rect.height - 600.0).abs() < f32::EPSILON);
	}

	#[test]
	fn scroll_clamps_to_zero() {
		let mut vp = Viewport::new(800.0, 600.0);
		vp.scroll_by(-100.0, -100.0);
		assert!((vp.scroll_x).abs() < f32::EPSILON);
		assert!((vp.scroll_y).abs() < f32::EPSILON);
	}

	#[test]
	fn zoom_clamps() {
		let mut vp = Viewport::default();
		vp.set_zoom(0.01);
		assert!((vp.zoom - 0.1).abs() < f32::EPSILON);

		vp.set_zoom(20.0);
		assert!((vp.zoom - 10.0).abs() < f32::EPSILON);
	}

	#[test]
	fn coordinate_roundtrip() {
		let mut vp = Viewport::new(800.0, 600.0);
		vp.scroll_by(100.0, 50.0);
		vp.set_zoom(2.0);

		let (doc_x, doc_y) = vp.screen_to_document(400.0, 300.0);
		let (screen_x, screen_y) = vp.document_to_screen(doc_x, doc_y);

		assert!((screen_x - 400.0).abs() < 0.01);
		assert!((screen_y - 300.0).abs() < 0.01);
	}
}
