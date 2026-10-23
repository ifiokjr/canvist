//! Input events, in the editor's own terms so tests and other windowing
//! libraries can drive it. The `winit` feature translates winit's events
//! into these.

use std::time::Duration;

pub use canvist_headless::canvist_core::Modifiers;
use kurbo::Point;

use crate::Clipboard;
use crate::DesktopEditor;
use crate::DesktopResult;

/// One input event.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
	/// A key was pressed.
	Key(KeyInput),
	/// An input method event.
	Ime(Ime),
	/// A pointer button was pressed at `position`, in the editor's logical
	/// coordinates.
	PointerDown {
		/// Where, relative to the editor's top-left corner.
		position: Point,
		/// Which button.
		button: PointerButton,
		/// The modifiers held: Shift extends the selection.
		modifiers: Modifiers,
	},
	/// The pointer moved to `position`.
	PointerMove {
		/// Where, relative to the editor's top-left corner.
		position: Point,
	},
	/// A pointer button was released.
	PointerUp {
		/// Which button.
		button: PointerButton,
	},
	/// The wheel turned or the trackpad scrolled.
	Scroll(ScrollDelta),
	/// The editor gained or lost keyboard focus.
	Focus(bool),
}

/// A key press.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyInput {
	/// The key, as DOM `KeyboardEvent.key` names it: `a`, `Enter`,
	/// `ArrowLeft`, `Backspace`.
	pub key: String,
	/// The text the key types, if any.
	pub text: Option<String>,
	/// The modifiers held.
	pub modifiers: Modifiers,
}

impl KeyInput {
	/// A key that types nothing, such as `ArrowLeft`.
	#[must_use]
	pub fn named(key: &str, modifiers: Modifiers) -> Self {
		Self {
			key: key.to_owned(),
			text: None,
			modifiers,
		}
	}

	/// A key that types `text` when no binding takes it.
	#[must_use]
	pub fn text(text: &str, modifiers: Modifiers) -> Self {
		Self {
			key: text.to_owned(),
			text: Some(text.to_owned()),
			modifiers,
		}
	}
}

/// An input method event, as platforms report composition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ime {
	/// The input method is active.
	Enabled,
	/// The text being composed changed. Empty text ends the composition
	/// without committing it.
	Preedit(String),
	/// The composition finished with `text`.
	Commit(String),
	/// The input method is no longer active.
	Disabled,
}

/// A pointer button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
	/// The primary (usually left) button, or a touch.
	Primary,
	/// The secondary (usually right) button.
	Secondary,
	/// The middle button.
	Middle,
	/// Any other button.
	Other,
}

/// How far to scroll.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollDelta {
	/// Wheel notches. Positive `y` scrolls the content down (toward the
	/// top of the document), as winit reports it.
	Lines {
		/// Horizontal notches.
		x: f32,
		/// Vertical notches.
		y: f32,
	},
	/// Logical pixels, from trackpads, with the same sign as `Lines`.
	Pixels {
		/// Horizontal pixels.
		x: f32,
		/// Vertical pixels.
		y: f32,
	},
}

/// What an input event needs from the host.
pub struct Context<'a> {
	/// The time of the event, from any monotonic clock: it groups typing
	/// into undo steps, counts double clicks, and blinks the caret.
	pub now: Duration,
	/// The clipboard copy, cut, and paste use.
	pub clipboard: &'a mut dyn Clipboard,
}

impl std::fmt::Debug for Context<'_> {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("Context")
			.field("now", &self.now)
			.finish_non_exhaustive()
	}
}

impl<'a> Context<'a> {
	/// A context at time `now` with `clipboard`.
	pub fn new(now: Duration, clipboard: &'a mut dyn Clipboard) -> Self {
		Self { now, clipboard }
	}
}

/// What handling an input event did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct Response {
	/// Whether the editor used the event. When it did not, the host may
	/// handle it, such as a key its own shortcuts bind.
	pub consumed: bool,
	/// Whether the editor should draw a new frame.
	pub repaint: bool,
}

impl Response {
	/// The event was used and the view changed.
	pub const CHANGED: Self = Self {
		consumed: true,
		repaint: true,
	};
	/// Nothing happened.
	pub const IGNORED: Self = Self {
		consumed: false,
		repaint: false,
	};

	/// Used if `changed`, repainting then.
	pub(crate) const fn changed(changed: bool) -> Self {
		Self {
			consumed: changed,
			repaint: changed,
		}
	}
}

impl<Handles: Copy> DesktopEditor<Handles> {
	/// Handle one input event.
	///
	/// Every edit goes through the editor (so key bindings, input rules, and
	/// history apply) and then through layout, so only what changed lays out
	/// again. Call [`DesktopEditor::frame`] and paint when the response asks
	/// for a repaint.
	pub fn handle(&mut self, event: &InputEvent, cx: &mut Context<'_>) -> DesktopResult<Response> {
		self.now = self.now.max(cx.now);

		let response = match event {
			InputEvent::Key(key) => self.key(key, cx)?,
			InputEvent::Ime(ime) => self.ime(ime)?,
			InputEvent::PointerDown {
				position,
				button,
				modifiers,
			} => self.pointer_down(*position, *button, *modifiers)?,
			InputEvent::PointerMove { position } => self.pointer_move(*position)?,
			InputEvent::PointerUp { button } => self.pointer_up(*button),
			InputEvent::Scroll(delta) => self.scroll(*delta),
			InputEvent::Focus(focused) => self.focus(*focused),
		};

		self.dirty |= response.repaint;
		Ok(response)
	}

	/// Whether the editor has keyboard focus.
	#[must_use]
	pub fn is_focused(&self) -> bool {
		self.focused
	}

	fn focus(&mut self, focused: bool) -> Response {
		if focused == self.focused {
			return Response::IGNORED;
		}

		self.focused = focused;
		self.blink_from = self.now;
		self.pointer.release();
		Response::CHANGED
	}
}
