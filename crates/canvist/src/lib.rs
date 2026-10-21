#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist`
//!
//! The canvist rich-text editor engine in one crate. Add it once and reach
//! every part of the engine through a module, turning on the parts you need
//! with Cargo features:
//!
//! ```sh
//! cargo add canvist
//! cargo add canvist --features collab,ssr
//! ```
//!
//! | Module         | Crate                | Feature          | What it is                                                     |
//! | -------------- | -------------------- | ---------------- | -------------------------------------------------------------- |
//! | [`model`]      | `canvist_model`      | always           | The document model: schema, blocks, marks, steps, transactions |
//! | [`core`]       | `canvist_core`       | always           | The editing engine: state, commands, keymaps, history          |
//! | [`extensions`] | `canvist_extensions` | always           | The extensions; `starter-kit` (default) turns on TipTap's kit  |
//! | `headless`     | `canvist_headless`   | `headless`       | Load, edit, and export documents with no UI (default)          |
//! | `collab`       | `canvist_collab`     | `collab`         | Yjs collaboration: bindings, y-sync, awareness, shared undo    |
//! | `layout`       | `canvist_layout`     | `layout`         | Text and block layout, hit testing, display lists, SVG/HTML    |
//! | `scene`        | `canvist_scene`      | `canvas`         | The design canvas's scene model                                |
//! | `canvas`       | `canvist_canvas`     | `canvas`         | The canvas's camera, hit testing, selection, handles, tools    |
//! | `paint`        | `canvist_paint`      | `paint`          | Draw scenes as SVG, PNG (`png`), or with vello (`gpu`)         |
//!
//! The default features, `headless` and `starter-kit`, are what a server, a
//! script, or a test needs. The other features add to them:
//!
//! - `extras` and `full`: every extension beyond the StarterKit, with the
//!   emoji table loaded at runtime (`extras`) or compiled in (`full`).
//! - `collab`: Yjs collaboration. With `headless`, a document is a Yjs peer
//!   and a y-sync endpoint.
//! - `journal`: `Doc::record_commits`, for hosts that keep a layout of a
//!   document other code changes too.
//! - `ssr`: `headless::Renderer`, which renders the HTML and SVG the canvas
//!   editor would paint.
//! - `layout`, and `parley` to measure text with real fonts.
//! - `canvas`, `paint`, `png`, and `gpu`: the design canvas and its
//!   renderers.
//! - `headless-scene`: `headless::SceneEngine`, which loads, edits, and
//!   exports canvas scenes as SVG and PNG on a server.
//!
//! Each module is the crate it names, so its documentation, and every path
//! in it, apply unchanged: `canvist_headless::Engine` is
//! `canvist::headless::Engine`.
//!
//! # Example
//!
//! Parse Markdown, select a word, make it bold, and export the result:
//!
//! ```
//! use canvist::headless::Engine;
//! use canvist::headless::Format;
//! use canvist::headless::Position;
//!
//! // An engine holds the extensions. Build one and share it.
//! let engine = Engine::starter_kit();
//! let kit = engine.handles();
//! let mut doc = engine.parse(Format::Markdown, "# Plan\n\nShip the docs")?;
//!
//! // Select "docs": block 1, characters 9 to 13.
//! doc.select_range(&Position::new([1], 9), &Position::new([1], 13))?;
//! doc.run(&kit.bold.toggle())?;
//!
//! assert_eq!(doc.export(Format::Markdown), "# Plan\n\nShip the **docs**");
//! assert_eq!(
//! 	doc.export(Format::Html),
//! 	"<h1>Plan</h1><p>Ship the <strong>docs</strong></p>"
//! );
//! # Ok::<(), canvist::headless::HeadlessError>(())
//! ```

#[cfg(feature = "canvas")]
#[doc(inline)]
pub use canvist_canvas as canvas;
#[cfg(feature = "collab")]
#[doc(inline)]
pub use canvist_collab as collab;
#[doc(inline)]
pub use canvist_core as core;
#[doc(inline)]
pub use canvist_extensions as extensions;
#[cfg(feature = "headless")]
#[doc(inline)]
pub use canvist_headless as headless;
#[cfg(feature = "layout")]
#[doc(inline)]
pub use canvist_layout as layout;
#[doc(inline)]
pub use canvist_model as model;
#[cfg(feature = "paint")]
#[doc(inline)]
pub use canvist_paint as paint;
#[cfg(feature = "canvas")]
#[doc(inline)]
pub use canvist_scene as scene;
