//! Input events hosts feed to the canvas, in screen coordinates.

use kurbo::Point;
use kurbo::Vec2;
use serde_json::Value as Json;

/// What produced a pointer event.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PointerKind {
	/// A mouse or trackpad.
	#[default]
	Mouse,
	/// A stylus.
	Pen {
		/// Pressure from 0 to 1.
		pressure: f32,
		/// Tilt in degrees on each axis.
		tilt: Vec2,
	},
	/// A finger.
	Touch,
}

/// Where a pointer event falls in a press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
	/// The pointer went down.
	Down,
	/// The pointer moved, pressed or not.
	Move,
	/// The pointer went up.
	Up,
	/// The platform cancelled the press.
	Cancel,
}

/// Modifier keys. `primary` is Cmd on Apple platforms and Ctrl elsewhere;
/// the host decides.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
	/// Shift.
	pub shift: bool,
	/// Alt or Option.
	pub alt: bool,
	/// Cmd on Apple platforms, Ctrl elsewhere.
	pub primary: bool,
}

/// A pointer event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
	/// Identifies one pointer across its events, for multi-touch.
	pub id: u64,
	/// What produced it.
	pub kind: PointerKind,
	/// Where it is in the press.
	pub phase: Phase,
	/// The position in screen pixels.
	pub position: Point,
	/// Pressed buttons as a bitmask: 1 primary, 2 secondary, 4 middle.
	pub buttons: u8,
	/// Modifier keys.
	pub modifiers: Modifiers,
	/// The time in seconds, from any fixed origin.
	pub time: f64,
}

impl PointerEvent {
	/// A primary-button mouse event at `position`.
	#[must_use]
	pub fn mouse(phase: Phase, position: Point, time: f64) -> Self {
		Self {
			id: 0,
			kind: PointerKind::Mouse,
			phase,
			position,
			buttons: u8::from(phase != Phase::Up),
			modifiers: Modifiers::default(),
			time,
		}
	}

	/// The same event with `modifiers`.
	#[must_use]
	pub fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
		self.modifiers = modifiers;
		self
	}
}

/// A wheel or trackpad scroll.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelEvent {
	/// The scroll delta in screen pixels.
	pub delta: Vec2,
	/// The pointer position in screen pixels.
	pub position: Point,
	/// Modifier keys.
	pub modifiers: Modifiers,
	/// Whether this is a trackpad pinch (browsers report pinches as wheel
	/// events with Ctrl held).
	pub pinch: bool,
}

/// A key press or release. `key` uses DOM key names such as `"Escape"`,
/// `"Backspace"`, `"ArrowLeft"`, or `"a"`.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyEvent {
	/// The key.
	pub key: String,
	/// Modifier keys.
	pub modifiers: Modifiers,
	/// Whether the key went down (or up).
	pub down: bool,
}

/// Anything a host can feed the canvas.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
	/// A pointer event.
	Pointer(PointerEvent),
	/// A wheel event.
	Wheel(WheelEvent),
	/// A key event.
	Key(KeyEvent),
	/// Anything else a tool understands: device motion, MIDI, a gamepad. The
	/// host names it with `kind` and passes its data as JSON.
	Custom {
		/// What kind of input this is, such as `"deviceorientation"`.
		kind: String,
		/// Its data.
		value: Json,
	},
}
