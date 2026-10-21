//! Plain records and enums that cross the boundary by value.
//!
//! Names avoid the platform types apps import next to them: `Bounds`, not
//! `Rect` (`android.graphics.Rect`); `Rgba`, not `Color` (`SwiftUI`); and
//! `OffsetRange`, not `TextRange` (Jetpack Compose).

use canvist_headless::Format;
use canvist_headless::Position;
use canvist_headless::canvist_editor::Platform;
use canvist_layout::Color;
use canvist_layout::Rect;

/// A rectangle in document coordinates: points on iOS, density-independent
/// pixels on Android, and CSS pixels in the theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, uniffi::Record)]
pub struct Bounds {
	/// The left edge.
	pub x: f32,
	/// The top edge.
	pub y: f32,
	/// The width.
	pub width: f32,
	/// The height.
	pub height: f32,
}

impl From<Rect> for Bounds {
	fn from(rect: Rect) -> Self {
		Self {
			x: rect.x,
			y: rect.y,
			width: rect.width,
			height: rect.height,
		}
	}
}

/// A colour with 8-bit channels, not premultiplied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, uniffi::Record)]
pub struct Rgba {
	/// Red.
	pub r: u8,
	/// Green.
	pub g: u8,
	/// Blue.
	pub b: u8,
	/// Alpha, where 255 is opaque.
	pub a: u8,
}

impl From<Color> for Rgba {
	fn from(color: Color) -> Self {
		Self {
			r: color.r,
			g: color.g,
			b: color.b,
			a: color.a,
		}
	}
}

/// A format documents are read from and written to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum DocFormat {
	/// ProseMirror and TipTap JSON.
	Json,
	/// Semantic HTML.
	Html,
	/// Markdown, with GitHub task lists and strikethrough.
	Markdown,
	/// Plain text: one paragraph per line.
	Text,
}

impl From<DocFormat> for Format {
	fn from(format: DocFormat) -> Self {
		match format {
			DocFormat::Json => Self::Json,
			DocFormat::Html => Self::Html,
			DocFormat::Markdown => Self::Markdown,
			DocFormat::Text => Self::Text,
		}
	}
}

/// A point in a text block, addressed as in every other binding: child
/// indexes from the root to the block, and UTF-16 code units into its text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, uniffi::Record)]
pub struct DocPosition {
	/// Child indexes from the root to the text block.
	pub path: Vec<u32>,
	/// UTF-16 code units into the block's text.
	pub offset: u32,
}

impl From<Position> for DocPosition {
	fn from(position: Position) -> Self {
		Self {
			path: position.path,
			offset: position.offset,
		}
	}
}

impl From<DocPosition> for Position {
	fn from(position: DocPosition) -> Self {
		Self::new(position.path, position.offset)
	}
}

/// A range of the editor's text, in UTF-16 code units: what `UITextInput`
/// and Android's `InputConnection` address (see [`Editor`](crate::Editor)'s
/// text input methods).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, uniffi::Record)]
pub struct OffsetRange {
	/// The start, inclusive.
	pub start: u32,
	/// The end, exclusive; never before `start`.
	pub end: u32,
}

impl OffsetRange {
	/// The range between two offsets in either order.
	#[must_use]
	pub fn between(a: u32, b: u32) -> Self {
		Self {
			start: a.min(b),
			end: a.max(b),
		}
	}
}

/// The selection as text offsets. `anchor` stays put while `head` moves,
/// so a selection made backwards has `head < anchor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, uniffi::Record)]
pub struct TextSelection {
	/// The fixed end.
	pub anchor: u32,
	/// The moving end, where the caret is.
	pub head: u32,
}

impl TextSelection {
	/// The selected range, start first.
	#[must_use]
	pub fn range(self) -> OffsetRange {
		OffsetRange::between(self.anchor, self.head)
	}
}

/// The modifier keys held during a key press.
#[allow(
	clippy::struct_excessive_bools,
	reason = "one flag per physical modifier key"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, uniffi::Record)]
pub struct Modifiers {
	/// Shift.
	pub shift: bool,
	/// Control.
	pub ctrl: bool,
	/// Option on Apple keyboards, Alt elsewhere.
	pub alt: bool,
	/// Command on Apple keyboards, Meta elsewhere.
	pub meta: bool,
}

/// Which modifier `Mod` means in key bindings such as `Mod-b`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum KeyPlatform {
	/// iOS, iPadOS, and macOS: `Mod` is Command.
	Apple,
	/// Android and everything else: `Mod` is Control.
	#[default]
	Other,
}

impl From<KeyPlatform> for Platform {
	fn from(platform: KeyPlatform) -> Self {
		match platform {
			KeyPlatform::Apple => Self::Apple,
			KeyPlatform::Other => Self::Other,
		}
	}
}

/// A command and its arguments, from a typed extension handle such as
/// `kit.heading.toggleHeading(level: 2)`.
///
/// The generated handles build these; `args` is a JSON object (or `null`)
/// checked against the command's declaration when it runs.
#[derive(Clone, Debug, PartialEq, Eq, Hash, uniffi::Record)]
pub struct Command {
	/// The command's name in the extension metadata, such as `toggleBold`.
	pub name: String,
	/// Its arguments as a JSON object, or `null`.
	pub args: String,
}

/// How the StarterKit theme looks. Every size is in points on iOS and
/// density-independent pixels on Android.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ThemeOptions {
	/// Light text on a dark page instead of dark text on white.
	pub dark: bool,
	/// The font family list for body text and headings, as CSS writes it.
	/// The platform measurer resolves the names; `system-ui` is the
	/// platform's UI font.
	pub body_family: String,
	/// The font family list for code.
	pub code_family: String,
	/// The body font size. Hosts scale it with the user's text size
	/// setting (Dynamic Type, Android font scale).
	pub font_size: f32,
	/// The body line height, as a multiple of the font size.
	pub line_height: f32,
}

impl Default for ThemeOptions {
	fn default() -> Self {
		Self {
			dark: false,
			body_family: "system-ui, sans-serif".to_owned(),
			code_family: "ui-monospace, monospace".to_owned(),
			font_size: 17.0,
			line_height: 1.4,
		}
	}
}

/// What an editor is made of.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct EditorOptions {
	/// The extensions to register, by name, such as `bold` or `heading`.
	/// Paragraphs and the base keymap are always there.
	pub extensions: Vec<String>,
	/// The id this editor allocates blocks as. Collaborating editors must
	/// not share one.
	pub client: u32,
	/// Which modifier key bindings call `Mod`.
	pub platform: KeyPlatform,
	/// The look.
	pub theme: ThemeOptions,
	/// Whether the display list includes the caret and the selection
	/// highlight. iOS leaves them to `UITextInteraction`, which draws the
	/// system's own; Android draws them from the display list.
	pub draws_selection: bool,
	/// How far past the viewport to lay out, so scrolling a little never
	/// waits for measurement.
	pub overscan: f32,
}

/// What changed in one call, for [`EditorObserver`](crate::EditorObserver)s.
#[allow(
	clippy::struct_excessive_bools,
	reason = "independent facts about one change"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, uniffi::Record)]
pub struct EditorChange {
	/// The content changed.
	pub document: bool,
	/// The selection moved.
	pub selection: bool,
	/// IME composition started, changed, or ended.
	pub composition: bool,
	/// The document is shared and has Yjs updates waiting in
	/// [`Editor::take_updates`](crate::Editor::take_updates).
	pub updates: bool,
}

impl EditorChange {
	/// Whether anything changed at all.
	#[must_use]
	pub fn is_empty(self) -> bool {
		self == Self::default()
	}

	/// Both changes as one.
	#[must_use]
	pub fn merge(self, other: Self) -> Self {
		Self {
			document: self.document || other.document,
			selection: self.selection || other.selection,
			composition: self.composition || other.composition,
			updates: self.updates || other.updates,
		}
	}
}

/// What to send after [`Editor::receive_sync`](crate::Editor::receive_sync)
/// handled a y-sync message.
#[derive(Clone, Debug, Default, PartialEq, Eq, uniffi::Record)]
pub struct SyncReply {
	/// Messages for the connection the message came from.
	pub reply: Vec<Vec<u8>>,
	/// Messages for every other connection, when this editor relays.
	pub broadcast: Vec<Vec<u8>>,
	/// Whether the document changed.
	pub changed: bool,
}

/// What a text block is, for accessibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum BlockRole {
	/// A paragraph.
	Paragraph,
	/// A heading.
	Heading {
		/// The level, from 1 to 6.
		level: u8,
	},
	/// A list item's text.
	ListItem {
		/// Whether the list is numbered.
		ordered: bool,
		/// Whether the item is a task, and whether it is done.
		checked: Option<bool>,
	},
	/// Text in a block quote.
	Quote,
	/// A code block.
	Code,
}

/// One text block laid out on screen.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct BlockInfo {
	/// Its text in the editor's text, without the line break after it.
	pub range: OffsetRange,
	/// Its border box.
	pub bounds: Bounds,
	/// What it is.
	pub role: BlockRole,
}

/// One visual line of text.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct LineInfo {
	/// The line's text in the editor's text.
	pub range: OffsetRange,
	/// The line's box.
	pub bounds: Bounds,
}
