//! The canvist canvas in the browser.
//!
//! [`App`] wires a [`canvist_canvas::Canvas`] to a
//! [`canvist_collab::SceneBinding`] and to a [`canvist_canvas::TextSession`]
//! on the real canvist text editor. In the browser, `CanvasApp` puts it on
//! a `<canvas>`: vello on WebGPU where available, `vello_cpu` and Canvas2D
//! everywhere else. The `@canvist/canvas` package runs it as a demo with
//! two-tab multiplayer.

#![doc(
	html_logo_url = "https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/web/favicon.svg"
)]

mod app;
pub mod demo;
#[cfg(target_arch = "wasm32")]
mod render;
#[cfg(target_arch = "wasm32")]
mod web;

pub use app::App;
pub use app::AppError;
pub use app::text_editor;
pub use app::text_layouts;
#[cfg(target_arch = "wasm32")]
pub use web::CanvasApp;
