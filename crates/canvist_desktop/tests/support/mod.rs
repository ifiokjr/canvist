//! A headless driver: an editor with bundled fonts, an in-memory
//! clipboard, and a clock that advances by a few milliseconds per event, as
//! a fast typist's would.

#![allow(dead_code, reason = "each test file uses a different part")]

use std::time::Duration;

use canvist_desktop::Context;
use canvist_desktop::DesktopEditor;
use canvist_desktop::EditorOptions;
use canvist_desktop::FontSource;
use canvist_desktop::Ime;
use canvist_desktop::InputEvent;
use canvist_desktop::KeyInput;
use canvist_desktop::MemoryClipboard;
use canvist_desktop::Modifiers;
use canvist_desktop::PointerButton;
use canvist_desktop::Response;
use canvist_desktop::ScrollDelta;
use canvist_desktop::Viewport;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::canvist_core::Platform;
use kurbo::Point;

/// The editor's size when a test does not care: a small laptop window.
pub const WIDTH: f32 = 600.0;
pub const HEIGHT: f32 = 400.0;
/// Narrow enough that a long paragraph wraps onto many lines.
pub const NARROW: f32 = 200.0;
/// Short enough that a long document scrolls.
pub const SHORT: f32 = 300.0;

/// Time between driven events.
pub const TICK: Duration = Duration::from_millis(8);

/// No modifiers.
pub const NONE: Modifiers = Modifiers::NONE;
/// Shift.
pub const SHIFT: Modifiers = Modifiers {
	shift: true,
	..Modifiers::NONE
};
/// Ctrl, the `Mod` key on [`Platform::Other`].
pub const CTRL: Modifiers = Modifiers {
	ctrl: true,
	..Modifiers::NONE
};
/// Shift and Ctrl.
pub const SHIFT_CTRL: Modifiers = Modifiers {
	shift: true,
	ctrl: true,
	..Modifiers::NONE
};
/// Cmd, the `Mod` key on [`Platform::Apple`].
pub const META: Modifiers = Modifiers {
	meta: true,
	..Modifiers::NONE
};

/// Options every test uses: bundled fonts, so results are the same on every
/// machine, a steady caret, and Ctrl as `Mod`.
pub fn options() -> EditorOptions {
	EditorOptions::new()
		.fonts(FontSource::Bundled)
		.caret_blink(Duration::ZERO)
		.platform(Platform::Other)
}

/// Drives one editor with events.
pub struct Driver {
	pub editor: DesktopEditor,
	pub clipboard: MemoryClipboard,
	pub now: Duration,
}

impl Driver {
	/// A focused editor of the default size holding `markdown`, laid out.
	pub fn new(markdown: &str) -> Self {
		Self::sized(markdown, WIDTH, HEIGHT)
	}

	/// A focused editor `width` by `height` holding `markdown`, laid out.
	pub fn sized(markdown: &str, width: f32, height: f32) -> Self {
		Self::with_options(markdown, width, height, options())
	}

	pub fn with_options(markdown: &str, width: f32, height: f32, options: EditorOptions) -> Self {
		let mut editor =
			DesktopEditor::new(Engine::starter_kit(), options).expect("the StarterKit editor");
		editor.load(Format::Markdown, markdown).expect("markdown");
		editor.resize(Viewport {
			width,
			height,
			scale: 1.0,
		});
		let mut driver = Self {
			editor,
			clipboard: MemoryClipboard::new(),
			now: Duration::ZERO,
		};
		let _ = driver.send(&InputEvent::Focus(true));
		driver.frame();
		driver
	}

	/// Send one event, a tick after the last.
	pub fn send(&mut self, event: &InputEvent) -> Response {
		self.now += TICK;
		let mut cx = Context::new(self.now, &mut self.clipboard);
		self.editor.handle(event, &mut cx).expect("event")
	}

	/// Lay out and build a display list, as a frame does.
	pub fn frame(&mut self) {
		self.editor.frame(self.now).expect("frame");
	}

	/// Press a key that types nothing.
	pub fn press(&mut self, key: &str, modifiers: Modifiers) -> Response {
		self.send(&InputEvent::Key(KeyInput::named(key, modifiers)))
	}

	/// Press a key that types its own text, such as `a` or `Mod-b`'s `b`.
	pub fn key(&mut self, key: &str, modifiers: Modifiers) -> Response {
		self.send(&InputEvent::Key(KeyInput::text(key, modifiers)))
	}

	/// Type `text` one key per character, with a frame after each key.
	pub fn type_text(&mut self, text: &str) {
		for character in text.chars() {
			let _ = self.key(&character.to_string(), NONE);
			self.frame();
		}
	}

	pub fn ime(&mut self, ime: Ime) -> Response {
		self.send(&InputEvent::Ime(ime))
	}

	pub fn down(&mut self, x: f64, y: f64, modifiers: Modifiers) -> Response {
		self.send(&InputEvent::PointerDown {
			position: Point::new(x, y),
			button: PointerButton::Primary,
			modifiers,
		})
	}

	pub fn move_to(&mut self, x: f64, y: f64) -> Response {
		self.send(&InputEvent::PointerMove {
			position: Point::new(x, y),
		})
	}

	pub fn up(&mut self) -> Response {
		self.send(&InputEvent::PointerUp {
			button: PointerButton::Primary,
		})
	}

	/// Click at `(x, y)` `count` times in quick succession.
	pub fn click(&mut self, x: f64, y: f64, count: u32) {
		for _ in 0..count {
			let _ = self.down(x, y, NONE);
			let _ = self.up();
		}
	}

	pub fn scroll(&mut self, y: f32) -> Response {
		self.send(&InputEvent::Scroll(ScrollDelta::Pixels { x: 0.0, y }))
	}

	pub fn markdown(&self) -> String {
		self.editor.export(Format::Markdown)
	}

	/// The selected text as the clipboard would hold it.
	pub fn selected_text(&self) -> String {
		self.editor
			.copy()
			.expect("copy")
			.map(|content| content.text)
			.unwrap_or_default()
	}

	/// Where byte `offset` of top-level block `index` is drawn, in the
	/// editor's coordinates: its caret's left edge and vertical middle.
	pub fn point_of(&self, index: usize, offset: u32) -> (f64, f64) {
		let block = self.editor.doc().document().children(None)[index];
		let caret =
			canvist_layout::Caret::new(canvist_headless::canvist_model::Point::new(block, offset));
		let rect = self.editor.layout().caret_rect(caret).expect("laid out");
		let padding = self.editor.options().padding;
		(
			f64::from(rect.x + padding),
			f64::from(rect.y + rect.height / 2.0 + padding - self.editor.scroll_top()),
		)
	}

	/// Where the caret is drawn, in the editor's coordinates.
	pub fn caret(&self) -> (f64, f64) {
		let area = self.editor.caret_area().expect("a caret");
		(f64::from(area.x), f64::from(area.y + area.height / 2.0))
	}
}
