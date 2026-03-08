//! The core rendering trait.
//!
//! A [`Renderer`] is a backend that can draw a document to some surface. All
//! renderers manage a [`Viewport`] and implement the [`Canvas`] trait for
//! drawing primitives.

use crate::Canvas;
use crate::Viewport;

/// A rendering backend that can draw a canvist document.
///
/// Implementors provide the connection between the abstract canvas API and
/// a concrete graphics system (HTML `Canvas2D`, `WebGL`, Skia, etc.).
pub trait Renderer: Canvas {
	/// Return a reference to the current viewport.
	fn viewport(&self) -> &Viewport;

	/// Replace the current viewport.
	fn set_viewport(&mut self, viewport: Viewport);
}
