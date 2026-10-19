//! # `canvist_headless`
//!
//! The canvist document model and editing engine with no UI, for servers,
//! scripts, and tests. It runs anywhere Rust runs, including WebAssembly
//! without a DOM, and is what the TypeScript and Dart server packages wrap.
//!
//! - An [`Engine`] holds a set of extensions: their schema, their metadata,
//!   and their typed handles. Build one per process; it is `Send + Sync`.
//! - A [`Doc`] is one document with an editor around it. Load it from JSON,
//!   HTML, Markdown, or plain text ([`Format`]), set a selection, run
//!   commands, and export it again. No layout is involved: selections are
//!   explicit, addressed by model [`Point`](canvist_model::Point)s or by
//!   language-neutral [`Position`]s. A `Doc` is `Send`.
//! - Typed handles make unregistered extensions a compile error: an engine
//!   built without bold has no `bold` handle to call. Bindings use
//!   [`Doc::run_named`] and [`Doc::query`], whose names come from
//!   [`Engine::metadata`].
//! - [`Snapshot`]s capture content and selection as JSON.
//! - With the `collab` feature, a `Doc` is a Yjs peer: it joins from Yjs
//!   state, merges updates, produces updates and y-sync messages, and checks
//!   the merged state against its schema.
//! - With the `ssr` feature, a [`Renderer`] produces the HTML and SVG the
//!   canvas editor would paint, using `canvist_layout` and `parley`. The
//!   default build pulls in neither.
//! - With the `scene` feature, a [`SceneEngine`] loads canvas scenes of
//!   shapes, frames, images, vector paths, and text frames into
//!   [`SceneDoc`]s, which run canvas commands and export SVG and PNG with
//!   the renderer the canvas draws with. With `collab` too, a scene is a Yjs
//!   peer.
//!
//! Every binding runs the same language-neutral fixtures in `conformance/`
//! at the repository root, so a document transformed in Rust, TypeScript, or
//! Dart comes out the same.
//!
//! # Converting Markdown to JSON
//!
//! ```
//! use canvist_headless::Engine;
//! use canvist_headless::Format;
//!
//! let engine = Engine::starter_kit();
//! let doc = engine.parse(Format::Markdown, "# Plan\n\n- [x] Model\n- [ ] Server")?;
//! let json = doc.to_json();
//!
//! assert_eq!(json["content"][0]["type"], "heading");
//! assert_eq!(json["content"][1]["type"], "taskList");
//! assert_eq!(doc.export(Format::Text), "Plan\nModel\nServer");
//! # Ok::<(), canvist_headless::HeadlessError>(())
//! ```
//!
//! # A scripted transform
//!
//! Commands run through the typed handles the engine returns, against an
//! explicit selection. This one turns every paragraph that starts with
//! `TODO:` into an unchecked task:
//!
//! ```
//! use canvist_headless::Engine;
//! use canvist_headless::Format;
//! use canvist_headless::Position;
//!
//! let engine = Engine::starter_kit();
//! let kit = engine.handles();
//! let mut doc = engine.parse(Format::Markdown, "Intro\n\nTODO: write docs\n\nTODO: ship")?;
//!
//! let todos: Vec<u32> = doc
//! 	.document()
//! 	.children(None)
//! 	.iter()
//! 	.enumerate()
//! 	.filter(|&(_, &id)| doc.document().inline(id).is_some_and(|text| text.as_str().starts_with("TODO: ")))
//! 	.map(|(index, _)| index as u32)
//! 	.collect();
//!
//! // Later blocks keep their indexes because turning a paragraph into a task
//! // list keeps one top-level block per paragraph.
//! for index in todos {
//! 	let start = Position::new([index], 0);
//! 	doc.select_range(&start, &Position::new([index], 6))?;
//! 	doc.delete_selection()?;
//! 	doc.run(&kit.task_list.toggle())?;
//! }
//!
//! assert_eq!(doc.export(Format::Markdown), "Intro\n\n- [ ] write docs\n- [ ] ship");
//! # Ok::<(), canvist_headless::HeadlessError>(())
//! ```
//!
//! # Merging Yjs updates on a server
//!
//! See [`Doc::receive_sync`] (feature `collab`) for a y-websocket endpoint,
//! and `examples/` for runnable programs.

mod doc;
mod engine;
mod error;
mod format;
#[cfg(feature = "starter-kit")]
mod named;
mod position;
mod snapshot;

#[cfg(feature = "collab")]
mod collab;
#[cfg(feature = "scene")]
mod scene;
#[cfg(feature = "ssr")]
mod ssr;

#[cfg(feature = "collab")]
pub use canvist_collab;
pub use canvist_editor;
#[cfg(feature = "starter-kit")]
pub use canvist_extensions;
pub use canvist_model;
#[cfg(feature = "scene")]
pub use canvist_scene;
#[cfg(feature = "collab")]
pub use collab::SyncReply;
#[cfg(feature = "collab")]
pub use collab::Validation;
pub use doc::Doc;
pub use engine::Engine;
pub use error::ErrorCode;
pub use error::HeadlessError;
pub use format::Format;
pub use position::Position;
#[cfg(feature = "scene")]
pub use scene::ExportOptions;
#[cfg(feature = "scene")]
pub use scene::SceneDoc;
#[cfg(feature = "scene")]
pub use scene::SceneEngine;
pub use snapshot::Snapshot;
#[cfg(feature = "ssr")]
pub use ssr::Renderer;
