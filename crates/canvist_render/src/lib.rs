//! # `canvist_render`
//!
//! Platform-agnostic rendering traits and text layout for the canvist canvas
//! editor.
//!
//! This crate defines the abstract rendering interface that platform backends
//! implement. The core editor logic in [`canvist_core`] produces layout
//! information, and a [`Renderer`] turns that into pixels.
//!
//! # Implementing a backend
//!
//! To add a new rendering backend (e.g. Skia, Metal, `Canvas2D`), implement the
//! [`Canvas`] and [`Renderer`] traits:
//!
//! ```
//! use canvist_render::Canvas;
//! use canvist_render::Rect;
//! use canvist_render::Renderer;
//! use canvist_render::Viewport;
//!
//! struct MyCanvas;
//!
//! impl Canvas for MyCanvas {
//!     fn fill_rect(&mut self, rect: Rect, color: canvist_core::Color) {
//!         // your drawing code here
//!     }
//!
//!     fn draw_text(&mut self, x: f32, y: f32, text: &str, style: &canvist_core::Style) {
//!         // your text rendering here
//!     }
//!
//!     fn clear(&mut self, color: canvist_core::Color) {
//!         // clear the canvas
//!     }
//! }
//!
//! impl Renderer for MyCanvas {
//!     fn viewport(&self) -> &Viewport {
//!         todo!()
//!     }
//!
//!     fn set_viewport(&mut self, viewport: Viewport) {
//!         todo!()
//!     }
//! }
//! ```

mod canvas;
mod font;
mod renderer;
mod viewport;

pub use canvas::Canvas;
pub use canvas::Rect;
pub use canvas::Size;
pub use font::FontCache;
pub use font::FontdueMeasure;
pub use renderer::Renderer;
pub use viewport::Viewport;
