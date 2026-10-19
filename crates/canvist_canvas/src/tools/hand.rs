//! The hand tool: drag to pan, with momentum after release.

use kurbo::Point;

use super::Outcome;
use super::Tool;
use super::pointer;
use crate::CanvasError;
use crate::CanvasState;
use crate::Cursor;
use crate::Input;
use crate::input::Phase;
use crate::input::PointerEvent;

/// The hand tool.
#[derive(Clone, Debug, Default)]
pub struct HandTool {
	last: Option<Point>,
}

impl HandTool {
	/// A hand tool.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Pan by how far the pointer moved since the last event.
	fn drag(&mut self, canvas: &mut CanvasState, event: &PointerEvent) -> Outcome {
		let Some(last) = self.last else {
			return Outcome::Idle;
		};

		canvas.camera_mut().pan(event.position - last);
		canvas.inertia.track(event.time, event.position);
		self.last = Some(event.position);
		Outcome::Redraw
	}

	/// End the drag; only a real release (not a cancel) keeps the momentum.
	fn release(&mut self, canvas: &mut CanvasState, phase: Phase) -> Outcome {
		if self.last.take().is_some() && phase == Phase::Up {
			canvas.inertia.release();
		}

		Outcome::Redraw
	}
}

impl Tool for HandTool {
	fn name(&self) -> &'static str {
		"hand"
	}

	fn input(&mut self, canvas: &mut CanvasState, input: &Input) -> Result<Outcome, CanvasError> {
		let Some(event) = pointer(input) else {
			return Ok(Outcome::Idle);
		};

		match event.phase {
			Phase::Down => {
				canvas.inertia.stop();
				canvas.inertia.track(event.time, event.position);
				self.last = Some(event.position);
				Ok(Outcome::Idle)
			}
			Phase::Move => Ok(self.drag(canvas, event)),
			Phase::Up | Phase::Cancel => Ok(self.release(canvas, event.phase)),
		}
	}

	fn cursor(&self, _canvas: &CanvasState) -> Cursor {
		if self.last.is_some() {
			Cursor::Grabbing
		} else {
			Cursor::Grab
		}
	}

	fn cancel(&mut self, _canvas: &mut CanvasState) {
		self.last = None;
	}
}
