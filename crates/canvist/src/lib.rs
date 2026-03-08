//! # canvist
//!
//! A canvas-based text editor written in Rust — build your own Google Docs.
//!
//! **canvist** gives you full control over the editing experience by rendering
//! text through a custom canvas engine, just like Figma and Google Docs do.
//! Instead of relying on `contenteditable` or platform text views, canvist owns
//! every pixel.
//!
//! ## Overview
//!
//! This is the umbrella crate that re-exports the public API from the
//! individual canvist crates:
//!
//! - [`canvist_core`] — Document model, operations, selections, and
//!   collaboration
//! - [`canvist_render`] — Platform-agnostic rendering traits and text layout
//! - [`canvist_wasm`] (behind the `wasm` feature) — WebAssembly + `Canvas2D`
//!   backend
//!
//! ## Quick start
//!
//! ```
//! use canvist::prelude::*;
//!
//! // Create a new document
//! let mut doc = Document::new();
//!
//! // Insert text at the beginning
//! let cursor = doc.cursor();
//! doc.insert_text(cursor.position(), "Hello, canvist!");
//!
//! // Style it
//! let style = Style::new().bold().font_size(24.0);
//! doc.apply_style(Selection::all(&doc), &style);
//! ```

/// Re-export of [`canvist_core`].
pub use canvist_core as core;
/// Re-export of [`canvist_render`].
pub use canvist_render as render;
#[cfg(feature = "wasm")]
/// Re-export of [`canvist_wasm`] (requires the `wasm` feature).
pub use canvist_wasm as wasm;

/// Convenience prelude that imports the most commonly used types.
///
/// ```
/// use canvist::prelude::*;
/// ```
pub mod prelude {
	pub use canvist_core::Document;
	pub use canvist_core::NodeId;
	pub use canvist_core::Position;
	pub use canvist_core::Selection;
	pub use canvist_core::Style;
	pub use canvist_render::Canvas;
	pub use canvist_render::Renderer;
	pub use canvist_render::Viewport;
}
