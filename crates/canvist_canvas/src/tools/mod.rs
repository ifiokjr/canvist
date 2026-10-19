//! Tools: state machines that turn input into scene changes and overlay.
//!
//! A tool sees [`Input`] in screen coordinates and changes the scene only
//! through [`CanvasState`], so every change reaches history, the index, and
//! collaboration the same way. Extensions add tools; the built-in ones are
//! [`SelectTool`], [`HandTool`], [`ShapeTool`], [`PenTool`], and
//! [`TextTool`].

mod hand;
mod pen;
pub(crate) mod select;
mod shape;
mod text;

pub use hand::HandTool;
use kurbo::Point;
pub use pen::PenTool;
pub use select::SelectTool;
pub use shape::ShapeTool;
pub use text::TextTool;

use crate::CanvasError;
use crate::CanvasState;
use crate::Cursor;
use crate::Input;
use crate::OverlayItem;
use crate::input::Phase;
use crate::input::PointerEvent;

/// Screen pixels a press must travel before it counts as a drag.
pub const DRAG_THRESHOLD: f64 = 3.0;
/// Screen pixels within which a click hits a node.
pub const HIT_TOLERANCE: f64 = 4.0;

/// What handling an input asks of the host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Outcome {
	/// Nothing visible changed.
	#[default]
	Idle,
	/// Repaint.
	Redraw,
	/// Open the text editor on this text frame.
	EditText(canvist_scene::NodeId),
	/// The tool finished a creation; switch back to the default tool.
	Done,
}

impl Outcome {
	/// The more demanding of two outcomes.
	#[must_use]
	pub fn and(self, other: Self) -> Self {
		match (self, other) {
			(Self::Idle, other) => other,
			(this, Self::Idle | Self::Redraw) => this,
			(_, other) => other,
		}
	}
}

/// A tool.
pub trait Tool: Send {
	/// The tool's name, used in metadata and by bindings.
	fn name(&self) -> &str;

	/// Handle one input.
	fn input(&mut self, canvas: &mut CanvasState, input: &Input) -> Result<Outcome, CanvasError>;

	/// Add the tool's overlay items (screen space) to `out`.
	fn overlay(&self, canvas: &CanvasState, out: &mut Vec<OverlayItem>) {
		let _ = (canvas, out);
	}

	/// The cursor the tool wants.
	fn cursor(&self, canvas: &CanvasState) -> Cursor {
		let _ = canvas;
		Cursor::Default
	}

	/// Abandon any gesture in progress, for example when the tool changes.
	fn cancel(&mut self, canvas: &mut CanvasState) {
		let _ = canvas;
	}
}

/// A press being tracked: where it started and whether it became a drag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Press {
	pub(crate) start: Point,
	pub(crate) dragging: bool,
}

impl Press {
	pub(crate) fn new(start: Point) -> Self {
		Self {
			start,
			dragging: false,
		}
	}

	/// Note a move to `position`; returns whether the press is now a drag.
	pub(crate) fn moved(&mut self, position: Point) -> bool {
		self.dragging |= position.distance(self.start) > DRAG_THRESHOLD;
		self.dragging
	}
}

/// The pointer event in `input`, if it is one.
pub(crate) fn pointer(input: &Input) -> Option<&PointerEvent> {
	match input {
		Input::Pointer(event) => Some(event),
		_ => None,
	}
}

/// Whether a pointer event is a press of the primary button.
pub(crate) fn is_primary_down(event: &PointerEvent) -> bool {
	event.phase == Phase::Down && event.buttons & 1 == 1
}
