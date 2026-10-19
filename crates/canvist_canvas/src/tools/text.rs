//! The text tool: click to place an auto-width text frame, or drag to set
//! its width, then type into it with the real canvist text editor.

use canvist_scene::Props;
use canvist_scene::Shape;
use canvist_scene::TextSizing;
use kurbo::Affine;
use kurbo::Point;
use kurbo::Rect;
use kurbo::Vec2;

use super::Outcome;
use super::Press;
use super::Tool;
use super::is_primary_down;
use super::pointer;
use super::shape::drag_rect;
use super::shape::target_parent;
use crate::CanvasError;
use crate::CanvasState;
use crate::Cursor;
use crate::Input;
use crate::input::Modifiers;
use crate::input::Phase;

/// The text tool.
#[derive(Clone, Debug, Default)]
pub struct TextTool {
	press: Option<(Press, Point)>,
}

impl TextTool {
	/// A text tool.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	fn place(
		canvas: &mut CanvasState,
		rect: Rect,
		sizing: TextSizing,
	) -> Result<Outcome, CanvasError> {
		let (parent, to_parent) = target_parent(canvas, rect.origin());
		let props = Props {
			transform: to_parent * Affine::translate(rect.origin().to_vec2()),
			size: Vec2::new(rect.width().max(1.0), rect.height()),
			shape: Shape::Text(sizing),
			..Props::default()
		};
		let (node, _) = canvas.edit(false, |transaction| {
			transaction.insert(parent, None, "text", props)
		})?;
		canvas.selection_mut().set([node]);
		Ok(Outcome::EditText(node))
	}

	/// Note a pointer move to `position`, so release can tell a click from a
	/// drag.
	fn track(&mut self, position: Point) {
		if let Some((press, _)) = &mut self.press {
			press.moved(position);
		}
	}

	/// End the gesture at `world`: a drag places a fixed-width frame across
	/// it, a click an auto-width frame at the press.
	fn release(
		&mut self,
		canvas: &mut CanvasState,
		world: Point,
		modifiers: Modifiers,
	) -> Result<Outcome, CanvasError> {
		let Some((press, start)) = self.press.take() else {
			return Ok(Outcome::Idle);
		};

		if press.dragging {
			return Self::place(
				canvas,
				drag_rect(start, world, modifiers),
				TextSizing::AutoHeight,
			);
		}

		Self::place(
			canvas,
			Rect::from_origin_size(start, (1.0, 0.0)),
			TextSizing::AutoWidth,
		)
	}
}

impl Tool for TextTool {
	fn name(&self) -> &'static str {
		"text"
	}

	fn input(&mut self, canvas: &mut CanvasState, input: &Input) -> Result<Outcome, CanvasError> {
		let Some(event) = pointer(input) else {
			return Ok(Outcome::Idle);
		};
		let world = canvas.camera().to_world(event.position);

		match event.phase {
			_ if is_primary_down(event) => {
				self.press = Some((Press::new(event.position), world));
				Ok(Outcome::Idle)
			}

			Phase::Move => {
				self.track(event.position);
				Ok(Outcome::Idle)
			}
			Phase::Up => self.release(canvas, world, event.modifiers),
			Phase::Cancel | Phase::Down => Ok(Outcome::Idle),
		}
	}

	fn cursor(&self, _canvas: &CanvasState) -> Cursor {
		Cursor::Text
	}

	fn cancel(&mut self, _canvas: &mut CanvasState) {
		self.press = None;
	}
}
