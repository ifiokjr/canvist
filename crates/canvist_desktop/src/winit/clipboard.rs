//! The system clipboard, through arboard.

use crate::Clipboard;
use crate::ClipboardContent;
use crate::MemoryClipboard;

/// The system clipboard: HTML and plain text, as other apps offer and read
/// them. Where the system clipboard cannot be opened (a headless Linux
/// session without X11 or Wayland), it falls back to one in memory, so copy
/// and paste still work inside the app.
pub struct SystemClipboard {
	system: Option<arboard::Clipboard>,
	fallback: MemoryClipboard,
	/// Whether the last write failed, so the fallback holds the newest copy.
	failed: bool,
}

impl std::fmt::Debug for SystemClipboard {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("SystemClipboard")
			.field("system", &self.system.is_some())
			.finish_non_exhaustive()
	}
}

impl Default for SystemClipboard {
	fn default() -> Self {
		Self::new()
	}
}

impl SystemClipboard {
	/// Open the system clipboard.
	#[must_use]
	pub fn new() -> Self {
		Self {
			system: arboard::Clipboard::new().ok(),
			fallback: MemoryClipboard::new(),
			failed: false,
		}
	}

	/// Whether the system clipboard is open, rather than the in-memory
	/// fallback.
	#[must_use]
	pub fn is_system(&self) -> bool {
		self.system.is_some()
	}
}

impl Clipboard for SystemClipboard {
	fn read(&mut self) -> Option<ClipboardContent> {
		let Some(system) = self.system.as_mut().filter(|_| !self.failed) else {
			return self.fallback.read();
		};
		// Apps that offer HTML offer plain text beside it; an empty or missing
		// HTML flavour falls back to the text alone.
		let html = system.get().html().ok().filter(|html| !html.is_empty());
		let text = system.get_text().ok();

		match (html, text) {
			(None, None) => None,
			(html, text) => {
				Some(ClipboardContent {
					html,
					text: text.unwrap_or_default(),
				})
			}
		}
	}

	fn write(&mut self, content: ClipboardContent) {
		let Some(system) = self.system.as_mut() else {
			self.fallback.write(content);

			return;
		};

		let written = match &content.html {
			Some(html) => system.set_html(html.as_str(), Some(content.text.as_str())),
			None => system.set_text(content.text.as_str()),
		};

		// The write fails when the platform refuses it (on X11, when no
		// clipboard manager answers); keep the copy in memory so pasting inside
		// the app still works until a write succeeds again.
		self.failed = written.is_err();

		if self.failed {
			self.fallback.write(content);
		}
	}
}
