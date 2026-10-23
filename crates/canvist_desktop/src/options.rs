//! How an editor looks and behaves.

use std::sync::Arc;
use std::time::Duration;

use canvist_headless::canvist_core::Platform;
use canvist_layout::Theme;

use crate::FontSource;

/// How long the caret stays on, and then off: Windows' default, close to
/// every desktop's.
const CARET_BLINK: Duration = Duration::from_millis(530);

/// Options for a [`DesktopEditor`](crate::DesktopEditor).
///
/// ```
/// use canvist_desktop::EditorOptions;
/// use canvist_desktop::FontSource;
///
/// let options = EditorOptions::new()
/// 	.fonts(FontSource::Bundled)
/// 	.padding(32.0)
/// 	.max_width(720.0);
///
/// assert_eq!(options.max_width, Some(720.0));
/// ```
#[derive(Clone, Debug)]
pub struct EditorOptions {
	/// The theme: block and mark styles, colours, the caret, and the
	/// selection.
	pub theme: Arc<Theme>,
	/// Which fonts to use.
	pub fonts: FontSource,
	/// Which modifier `Mod-` key bindings mean: Cmd on [`Platform::Apple`],
	/// Ctrl elsewhere. Defaults to the platform this was compiled for.
	pub platform: Platform,
	/// The collaboration client id that seeds new block ids. Peers must not
	/// share one.
	pub client: u32,
	/// Space around the document, in logical pixels.
	pub padding: f32,
	/// The widest the document column gets, centred in wider editors, or
	/// `None` to fill the editor.
	pub max_width: Option<f32>,
	/// How far above and below the viewport to lay out ahead of scrolling,
	/// in logical pixels.
	pub overscan: f32,
	/// How long the caret stays on, and then off, while it blinks. Zero keeps
	/// it on.
	pub caret_blink: Duration,
	/// The accessible name of the document, read by screen readers.
	pub label: String,
}

impl Default for EditorOptions {
	fn default() -> Self {
		Self {
			theme: Arc::new(Theme::starter_kit()),
			fonts: FontSource::System,
			platform: current_platform(),
			client: 1,
			padding: 24.0,
			max_width: None,
			overscan: 600.0,
			caret_blink: CARET_BLINK,
			label: "Document".to_owned(),
		}
	}
}

impl EditorOptions {
	/// The defaults: the StarterKit theme, system fonts, this platform's
	/// shortcuts, and 24 px of padding.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Use `theme`.
	#[must_use]
	pub fn theme(mut self, theme: Arc<Theme>) -> Self {
		self.theme = theme;
		self
	}

	/// Use fonts from `source`.
	#[must_use]
	pub fn fonts(mut self, source: FontSource) -> Self {
		self.fonts = source;
		self
	}

	/// Use `platform`'s shortcuts.
	#[must_use]
	pub fn platform(mut self, platform: Platform) -> Self {
		self.platform = platform;
		self
	}

	/// Seed block ids as collaboration client `client`.
	#[must_use]
	pub fn client(mut self, client: u32) -> Self {
		self.client = client;
		self
	}

	/// Leave `padding` logical pixels around the document.
	#[must_use]
	pub fn padding(mut self, padding: f32) -> Self {
		self.padding = padding;
		self
	}

	/// Keep the document column at most `width` logical pixels wide.
	#[must_use]
	pub fn max_width(mut self, width: f32) -> Self {
		self.max_width = Some(width);
		self
	}

	/// Blink the caret every `period`, or keep it on with zero.
	#[must_use]
	pub fn caret_blink(mut self, period: Duration) -> Self {
		self.caret_blink = period;
		self
	}

	/// Name the document `label` for screen readers.
	#[must_use]
	pub fn label(mut self, label: &str) -> Self {
		label.clone_into(&mut self.label);
		self
	}
}

/// The platform this crate was compiled for.
#[must_use]
pub fn current_platform() -> Platform {
	if cfg!(target_vendor = "apple") {
		Platform::Apple
	} else {
		Platform::Other
	}
}
