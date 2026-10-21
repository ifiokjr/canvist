#![doc(
	html_logo_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/icon.svg",
	html_favicon_url = "https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/web/favicon.svg"
)]
//! # `canvist_ffi`
//!
//! The canvist editor for iOS and Android, exported through
//! [UniFFI](https://mozilla.github.io/uniffi-rs/) to Swift (the `Canvist`
//! package) and Kotlin (`io.github.ifiokjr:canvist-android`).
//!
//! An [`Editor`] is the whole engine without a UI: a
//! [`canvist_headless::Doc`] with the StarterKit's commands, laid out by
//! [`canvist_layout`] with text measured by the platform
//! ([`PlatformMeasurer`]), and painted by the platform from the
//! [`DisplayItem`]s each [`Frame`] reports. The `CanvistView`s on both
//! platforms are thin: they forward input, paint frames, and expose the
//! editor; React Native and Lynx embed the same views.
//!
//! - **Typed commands.** Generated Swift and Kotlin handles build
//!   [`Command`]s (`kit.bold.toggleBold()`), checked against the same
//!   metadata as Rust, TypeScript, and Dart.
//! - **Platform text input.** `UITextInput` and `InputConnection` address
//!   text by UTF-16 offsets into one string; the editor converts them to
//!   model points and runs every change through its key bindings, input
//!   rules, and history.
//! - **Collaboration.** Documents are Yjs peers, through Yjs updates or
//!   the y-sync protocol.
//!
//! ```
//! use std::sync::Arc;
//!
//! use canvist_ffi::DocFormat;
//! use canvist_ffi::Editor;
//! use canvist_ffi::EditorOptions;
//! use canvist_ffi::testing::FixedPlatformMeasurer;
//!
//! let editor = Editor::new(
//! 	EditorOptions::starter_kit(),
//! 	Arc::new(FixedPlatformMeasurer::default()),
//! )?;
//! editor.load(DocFormat::Markdown, "Hello")?;
//! editor.select(5, 5)?;
//! editor.insert_text(" world")?;
//!
//! assert_eq!(editor.export(DocFormat::Markdown), "Hello world");
//! # Ok::<(), canvist_ffi::CanvistError>(())
//! ```
mod access;
mod collab;
mod display;
mod editor;
mod error;
mod input;
mod lines;
mod measure;
mod session;
pub mod testing;
mod text;
mod types;

pub use display::DisplayItem;
pub use display::FillKind;
pub use display::Frame;
pub use display::LineKind;
pub use editor::Editor;
pub use editor::EditorObserver;
pub use editor::EditorStats;
pub use editor::core_version;
pub use editor::default_editor_options;
pub use editor::default_theme_options;
pub use editor::extension_metadata;
pub use error::CanvistError;
pub use measure::FontFace;
pub use measure::MeasureRequest;
pub use measure::MeasureResponse;
pub use measure::PlatformMeasurer;
pub use measure::TextSegment;
pub use measure::VerticalMetrics;
pub use types::*;

uniffi::setup_scaffolding!();

/// Every StarterKit extension, by name.
const STARTER_KIT: [&str; 17] = [
	"heading",
	"blockquote",
	"codeBlock",
	"hardBreak",
	"horizontalRule",
	"bulletList",
	"orderedList",
	"listItem",
	"taskList",
	"bold",
	"italic",
	"underline",
	"strike",
	"code",
	"link",
	"history",
	"trailingNode",
];

impl EditorOptions {
	/// Every StarterKit extension, client 1, the default theme, and the
	/// caret and selection drawn from the display list.
	#[must_use]
	pub fn starter_kit() -> Self {
		Self {
			extensions: STARTER_KIT.iter().map(|name| (*name).to_owned()).collect(),
			client: 1,
			platform: KeyPlatform::Other,
			theme: ThemeOptions::default(),
			draws_selection: true,
			overscan: 0.0,
		}
	}
}
