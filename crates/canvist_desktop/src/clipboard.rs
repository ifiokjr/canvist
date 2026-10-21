//! The clipboard: copy and cut write HTML and plain text, and paste
//! prefers HTML, so formatting survives a round trip through other apps.

use canvist_headless::canvist_editor::Slice;
use canvist_headless::canvist_editor::command;
use canvist_headless::canvist_editor::commands;
use canvist_headless::canvist_editor::position;
use canvist_headless::canvist_extensions::clipboard;

use crate::Context;
use crate::DesktopEditor;
use crate::DesktopResult;
use crate::Response;

/// What is on the clipboard, in the formats canvist reads and writes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClipboardContent {
	/// Rich content as HTML, when the source offered it.
	pub html: Option<String>,
	/// Plain text, which every source offers.
	pub text: String,
}

/// A clipboard the editor copies to and pastes from: the system clipboard
/// in a window, or [`MemoryClipboard`] in tests.
pub trait Clipboard {
	/// What the clipboard holds, or `None` when it holds no text.
	fn read(&mut self) -> Option<ClipboardContent>;

	/// Replace the clipboard's contents.
	fn write(&mut self, content: ClipboardContent);
}

/// A clipboard in memory, for tests and for hosts without a system
/// clipboard.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryClipboard {
	/// What it holds.
	pub content: Option<ClipboardContent>,
}

impl MemoryClipboard {
	/// An empty clipboard.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}
}

impl Clipboard for MemoryClipboard {
	fn read(&mut self) -> Option<ClipboardContent> {
		self.content.clone()
	}

	fn write(&mut self, content: ClipboardContent) {
		self.content = Some(content);
	}
}

/// A clipboard operation a shortcut asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClipboardAction {
	Copy,
	Cut,
	Paste,
	/// Paste only the plain text, dropping formatting.
	PastePlain,
}

impl<Handles: Copy> DesktopEditor<Handles> {
	/// The selection in clipboard formats, or `None` when nothing is
	/// selected.
	pub fn copy(&self) -> DesktopResult<Option<ClipboardContent>> {
		if self.selection_is_collapsed() {
			return Ok(None);
		}

		let slice = self
			.doc
			.editor()
			.copy()
			.map_err(canvist_headless::HeadlessError::from)?;
		Ok(Some(self.clipboard_content(&slice)))
	}

	/// Copy the selection and delete it.
	pub fn cut(&mut self) -> DesktopResult<Option<ClipboardContent>> {
		let copied = self.copy()?;

		if copied.is_some() {
			self.edit(|editor, env| editor.run(&command(commands::delete_selection), env))?;
		}

		Ok(copied)
	}

	/// Paste `content` over the selection: its HTML when it has some,
	/// otherwise its text. Paste rules apply, and text pasted into code
	/// stays literal.
	pub fn paste(&mut self, content: &ClipboardContent) -> DesktopResult<bool> {
		match &content.html {
			Some(html) => {
				let slice = clipboard::from_html(self.doc.document().schema(), html);
				self.edit(|editor, env| editor.paste(slice, env))
			}
			None => self.paste_text(&content.text),
		}
	}

	/// Paste plain text over the selection.
	pub fn paste_text(&mut self, text: &str) -> DesktopResult<bool> {
		self.edit(|editor, env| editor.paste_text(text, env))
	}

	/// Run a clipboard shortcut against `cx`'s clipboard. Copying with
	/// nothing selected leaves the clipboard alone and lets the host have
	/// the key.
	pub(crate) fn clipboard(
		&mut self,
		action: ClipboardAction,
		cx: &mut Context<'_>,
	) -> DesktopResult<Response> {
		match action {
			ClipboardAction::Copy => {
				let copied = write_to(cx, self.copy()?);
				Ok(Response {
					consumed: copied,
					repaint: false,
				})
			}
			ClipboardAction::Cut => Ok(Response::changed(write_to(cx, self.cut()?))),
			ClipboardAction::Paste | ClipboardAction::PastePlain => {
				let Some(content) = cx.clipboard.read() else {
					return Ok(Response::IGNORED);
				};
				let pasted = if action == ClipboardAction::Paste {
					self.paste(&content)?
				} else {
					self.paste_text(&content.text)?
				};

				Ok(Response::changed(pasted))
			}
		}
	}

	pub(crate) fn selection_is_collapsed(&self) -> bool {
		position::text_range(self.doc.document(), self.doc.selection())
			.is_none_or(|range| range.is_empty())
	}

	fn clipboard_content(&self, slice: &Slice) -> ClipboardContent {
		let schema = self.doc.document().schema();
		ClipboardContent {
			html: Some(clipboard::to_html(schema, slice)),
			text: clipboard::to_plain_text(schema, slice),
		}
	}
}

/// Put `content` on `cx`'s clipboard, if there is any. Returns whether
/// there was.
fn write_to(cx: &mut Context<'_>, content: Option<ClipboardContent>) -> bool {
	let Some(content) = content else {
		return false;
	};

	cx.clipboard.write(content);
	true
}
