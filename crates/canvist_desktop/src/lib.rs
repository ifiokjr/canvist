#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_desktop`
//!
//! The canvist rich-text editor for macOS, Windows, and Linux. A
//! [`DesktopEditor`] lays a [`canvist_headless::Doc`] out with
//! `canvist_layout` and parley, paints it with `canvist_paint` (vello on the
//! GPU, `vello_cpu` for PNGs), and takes input as [`InputEvent`]s:
//!
//! - keys, with `Mod` as Cmd on macOS and Ctrl elsewhere;
//! - input methods, with preedit text underlined in place and the candidate
//!   window placed by [`DesktopEditor::ime_area`];
//! - the clipboard, writing HTML and plain text and preferring HTML on
//!   paste;
//! - mouse and trackpad selection: click, double-click for a word,
//!   triple-click for a block, Shift-click and drag to extend, with
//!   autoscroll past the edges;
//! - scrolling and high-DPI;
//! - accessibility, as an AccessKit tree of text runs.
//!
//! The core never touches a window, so tests drive it headlessly. The
//! `winit` feature (on by default) adds [`winit::EditorWindow`], which embeds
//! the editor in any winit app: it translates winit events, renders with
//! vello on wgpu, talks to the system clipboard, and runs the AccessKit
//! platform adapter.
//!
//! # Example
//!
//! ```
//! use std::time::Duration;
//!
//! use canvist_desktop::Context;
//! use canvist_desktop::DesktopEditor;
//! use canvist_desktop::EditorOptions;
//! use canvist_desktop::FontSource;
//! use canvist_desktop::InputEvent;
//! use canvist_desktop::KeyInput;
//! use canvist_desktop::MemoryClipboard;
//! use canvist_desktop::Modifiers;
//! use canvist_headless::Engine;
//! use canvist_headless::Format;
//!
//! let options = EditorOptions::new().fonts(FontSource::Bundled);
//! let mut editor = DesktopEditor::new(Engine::starter_kit(), options)?;
//! let mut clipboard = MemoryClipboard::new();
//!
//! for key in ["#", " ", "H", "i"] {
//! 	let event = InputEvent::Key(KeyInput::text(key, Modifiers::NONE));
//! 	let mut cx = Context::new(Duration::ZERO, &mut clipboard);
//! 	let _ = editor.handle(&event, &mut cx)?;
//! }
//!
//! editor.frame(Duration::ZERO)?;
//! assert_eq!(editor.export(Format::Markdown), "# Hi");
//! assert!(editor.render_png()?.starts_with(b"\x89PNG"));
//! # Ok::<(), canvist_desktop::DesktopError>(())
//! ```
mod accessibility;
mod clipboard;
mod editor;
mod error;
mod fonts;
mod ime;
mod input;
mod keys;
mod lines;
mod options;
mod paint;
mod pointer;

#[cfg(feature = "collab")]
mod collab;
#[cfg(feature = "winit")]
pub mod winit;

pub use accessibility::EDITOR_ID;
pub use accessibility::WINDOW_ID;
pub use accesskit;
pub use canvist_headless;
pub use canvist_layout;
pub use canvist_paint;
pub use clipboard::Clipboard;
pub use clipboard::ClipboardContent;
pub use clipboard::MemoryClipboard;
pub use editor::DesktopEditor;
pub use editor::FrameInfo;
pub use editor::Viewport;
pub use error::DesktopError;
pub use error::DesktopResult;
pub use fonts::FontSource;
pub use fonts::Fonts;
pub use input::Context;
pub use input::Ime;
pub use input::InputEvent;
pub use input::KeyInput;
pub use input::Modifiers;
pub use input::PointerButton;
pub use input::Response;
pub use input::ScrollDelta;
pub use kurbo;
pub use options::EditorOptions;
pub use options::current_platform;
