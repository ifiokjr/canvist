//! Shape tools: drag to draw a rectangle, ellipse, polygon, frame, or any
//! node type with a box. Shift keeps it square; Alt draws from the centre.

use std::sync::Arc;

use canvist_scene::Color;
use canvist_scene::NodeId;
use canvist_scene::NodeKind;
use canvist_scene::Paint;
use canvist_scene::Prop;
use canvist_scene::Props;
use kurbo::Affine;
use kurbo::Point;
use kurbo::Rect;
use kurbo::Vec2;

use super::Outcome;
use super::Press;
use super::Tool;
use super::is_primary_down;
use super::pointer;
use crate::CanvasError;
use crate::CanvasState;
use crate::Cursor;
use crate::Input;
use crate::input::Modifiers;
use crate::input::Phase;

/// The side of a shape made by clicking without dragging.
const CLICK_SIZE: f64 = 100.0;
/// The fill new shapes start with: Figma's familiar light grey.
pub(crate) const SHAPE_FILL: Color = Color::rgb(217, 217, 217);

/// A tool that draws nodes of one type by dragging out their box.
#[derive(Clone, Debug)]
pub struct ShapeTool {
	name: String,
	kind: String,
	press: Option<(Press, Point)>,
	node: Option<NodeId>,
}

impl ShapeTool {
	/// A tool named `name` that draws nodes of type `kind`.
	#[must_use]
	pub fn new(name: &str, kind: &str) -> Self {
		Self {
			name: name.to_owned(),
			kind: kind.to_owned(),
			press: None,
			node: None,
		}
	}

	/// The props a new node of this tool's type starts with, placed at world
	/// rectangle `rect` by `to_parent`.
	fn props(&self, canvas: &CanvasState, rect: Rect, to_parent: Affine) -> Props {
		let behaviour = canvas
			.scene()
			.schema()
			.node_type(&self.kind)
			.map(|kind| canvas.scene().schema().spec(kind).kind());

		let fill = match behaviour {
			Some(NodeKind::Frame | NodeKind::Component) => Color::WHITE,
			_ => SHAPE_FILL,
		};

		Props {
			transform: to_parent * Affine::translate(rect.origin().to_vec2()),
			size: rect.size().to_vec2(),
			fills: Arc::new([Paint::solid(fill)]),
			..behaviour.map_or_else(Props::default, Props::for_kind)
		}
	}

	fn draw(&mut self, canvas: &mut CanvasState, rect: Rect) -> Result<(), CanvasError> {
		if let Some(node) = self.node {
			let to_parent = canvas
				.scene()
				.node(node)
				.and_then(|found| canvas.index().get(found.parent()))
				.map_or(Affine::IDENTITY, |parent| parent.world.inverse());
			let steps = vec![
				canvist_scene::Step::SetProp {
					node,
					prop: Prop::Transform(to_parent * Affine::translate(rect.origin().to_vec2())),
				},
				canvist_scene::Step::SetProp {
					node,
					prop: Prop::Size(rect.size().to_vec2()),
				},
			];
			canvas.apply(steps, true)?;
		} else {
			let (parent, to_parent) = target_parent(canvas, rect.center());
			let props = self.props(canvas, rect, to_parent);
			let kind = self.kind.clone();

			let (node, _) = canvas.edit(false, |transaction| {
				transaction.insert(parent, None, &kind, props)
			})?;
			canvas.selection_mut().set([node]);
			self.node = Some(node);
		}

		Ok(())
	}

	/// Resize the shape being drawn to the drag from the press to `world`,
	/// once the pointer at `position` has moved far enough to count.
	fn drag(
		&mut self,
		canvas: &mut CanvasState,
		position: Point,
		world: Point,
		modifiers: Modifiers,
	) -> Result<Outcome, CanvasError> {
		let Some((press, start)) = &mut self.press else {
			return Ok(Outcome::Idle);
		};

		if !press.moved(position) {
			return Ok(Outcome::Idle);
		}

		let rect = drag_rect(*start, world, modifiers);
		self.draw(canvas, rect)?;
		Ok(Outcome::Redraw)
	}

	/// End the gesture; a click without a drag draws a default-sized shape
	/// centred on the press.
	fn release(&mut self, canvas: &mut CanvasState) -> Result<Outcome, CanvasError> {
		let Some((press, start)) = self.press.take() else {
			return Ok(Outcome::Idle);
		};

		if !press.dragging {
			let half = Vec2::new(CLICK_SIZE, CLICK_SIZE) / 2.0;
			self.draw(canvas, Rect::from_points(start - half, start + half))?;
		}

		self.node = None;
		Ok(Outcome::Done)
	}
}

impl Tool for ShapeTool {
	fn name(&self) -> &str {
		&self.name
	}

	fn input(&mut self, canvas: &mut CanvasState, input: &Input) -> Result<Outcome, CanvasError> {
		let Some(event) = pointer(input) else {
			return Ok(Outcome::Idle);
		};
		let world = canvas.camera().to_world(event.position);

		match event.phase {
			_ if is_primary_down(event) => {
				self.press = Some((Press::new(event.position), world));
				self.node = None;
				Ok(Outcome::Idle)
			}

			Phase::Move => self.drag(canvas, event.position, world, event.modifiers),
			Phase::Up => self.release(canvas),
			Phase::Cancel | Phase::Down => Ok(Outcome::Idle),
		}
	}

	fn cursor(&self, _canvas: &CanvasState) -> Cursor {
		Cursor::Crosshair
	}

	fn cancel(&mut self, _canvas: &mut CanvasState) {
		self.press = None;
		self.node = None;
	}
}

/// The rectangle a drag from `start` to `current` draws.
#[must_use]
pub fn drag_rect(start: Point, current: Point, modifiers: Modifiers) -> Rect {
	let mut delta = current - start;

	if modifiers.shift {
		let side = delta.x.abs().max(delta.y.abs());
		delta = Vec2::new(side.copysign(delta.x), side.copysign(delta.y));
	}

	if modifiers.alt {
		Rect::from_points(start - delta, start + delta)
	} else {
		Rect::from_points(start, start + delta)
	}
}

/// Where a node drawn at world point `point` goes: the topmost visible
/// frame or component whose outline contains the point, or the first page.
/// Returns the parent and the transform from world to its coordinates.
pub(crate) fn target_parent(canvas: &CanvasState, point: Point) -> (NodeId, Affine) {
	let scene = canvas.scene();
	let index = canvas.index();
	let frame = index
		.query(Rect::from_center_size(point, (0.0, 0.0)))
		.filter(|&id| matches!(scene.kind(id), Some(NodeKind::Frame | NodeKind::Component)))
		.filter(|&id| scene.node(id).is_some_and(|node| node.props().visible))
		.filter(|&id| {
			index.get(id).is_some_and(|geometry| {
				geometry.outline.as_ref().is_some_and(|outline| {
					crate::geometry::contains(
						outline,
						canvist_scene::FillRule::NonZero,
						geometry.world.inverse() * point,
					)
				})
			})
		})
		.max_by(|&a, &b| crate::hit::paint_order(scene, a, b));

	match frame.and_then(|id| index.get(id).map(|geometry| (id, geometry.world.inverse()))) {
		Some(found) => found,
		None => {
			(
				scene.pages().first().copied().unwrap_or(NodeId::ROOT),
				Affine::IDENTITY,
			)
		}
	}
}
