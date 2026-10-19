//! The pen tool: click to place corners, drag to pull out smooth curve
//! handles, click the first point to close the path, and press Enter or
//! Escape to finish an open one.

use std::sync::Arc;

use canvist_scene::Color;
use canvist_scene::FillRule;
use canvist_scene::Paint;
use canvist_scene::Props;
use canvist_scene::Shape;
use canvist_scene::StrokeStyle;
use kurbo::Affine;
use kurbo::BezPath;
use kurbo::Point;
use kurbo::Shape as _;
use kurbo::Vec2;

use super::Outcome;
use super::Tool;
use super::is_primary_down;
use super::pointer;
use super::shape::SHAPE_FILL;
use crate::CanvasError;
use crate::CanvasState;
use crate::Cursor;
use crate::Input;
use crate::OverlayItem;
use crate::input::KeyEvent;
use crate::input::Phase;
use crate::overlay::PointKind;

/// Screen pixels within which a click closes the path on its first point.
const CLOSE_PIXELS: f64 = 8.0;
/// The stroke new paths start with.
const PEN_STROKE: Color = Color::rgb(23, 20, 31);
/// The stroke width, in world units, new paths start with.
const PEN_STROKE_WIDTH: f64 = 2.0;

/// One placed point: an anchor and its outgoing handle (the incoming one
/// mirrors it, so curves stay smooth).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Anchor {
	point: Point,
	out: Vec2,
}

/// The pen tool.
#[derive(Clone, Debug, Default)]
pub struct PenTool {
	anchors: Vec<Anchor>,
	dragging: bool,
	hover: Option<Point>,
}

impl PenTool {
	/// A pen tool.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	fn down(&mut self, canvas: &mut CanvasState, position: Point) -> Result<Outcome, CanvasError> {
		let camera = canvas.camera();
		let closes = self.anchors.len() >= 2
			&& self.anchors.first().is_some_and(|first| {
				camera.to_screen(first.point).distance(position) <= CLOSE_PIXELS
			});

		if closes {
			return self.finish(canvas, true);
		}

		self.anchors.push(Anchor {
			point: camera.to_world(position),
			out: Vec2::ZERO,
		});
		self.dragging = true;
		Ok(Outcome::Redraw)
	}

	/// Enter or Escape finishes the path open; other keys do nothing.
	fn key(&mut self, canvas: &mut CanvasState, key: &KeyEvent) -> Result<Outcome, CanvasError> {
		if key.down && matches!(key.key.as_str(), "Enter" | "Escape") {
			return self.finish(canvas, false);
		}

		Ok(Outcome::Idle)
	}

	/// Pull the last anchor's handle out towards world point `world`.
	fn pull_handle(&mut self, world: Point) {
		if let Some(last) = self.anchors.last_mut() {
			last.out = world - last.point;
		}
	}

	/// Create the path node from the placed points.
	fn finish(&mut self, canvas: &mut CanvasState, closed: bool) -> Result<Outcome, CanvasError> {
		let anchors = std::mem::take(&mut self.anchors);
		self.dragging = false;

		if anchors.len() < 2 {
			return Ok(Outcome::Redraw);
		}

		let world = path(&anchors, closed);
		let bounds = world.bounding_box();
		let local = Affine::translate(-bounds.origin().to_vec2()) * world;
		let (parent, to_parent) = super::shape::target_parent(canvas, bounds.center());

		let fills: Arc<[Paint]> = if closed {
			Arc::new([Paint::solid(SHAPE_FILL)])
		} else {
			Arc::new([])
		};

		let props = Props {
			transform: to_parent * Affine::translate(bounds.origin().to_vec2()),
			size: Vec2::new(bounds.width().max(1.0), bounds.height().max(1.0)),
			fills,
			strokes: Arc::new([Paint::solid(PEN_STROKE)]),
			stroke: StrokeStyle {
				width: PEN_STROKE_WIDTH,
				..StrokeStyle::default()
			},
			shape: Shape::Path {
				path: Arc::new(local),
				rule: FillRule::NonZero,
			},
			..Props::default()
		};

		let (node, _) = canvas.edit(false, |transaction| {
			transaction.insert(parent, None, "path", props)
		})?;
		canvas.selection_mut().set([node]);
		Ok(Outcome::Done)
	}
}

impl Tool for PenTool {
	fn name(&self) -> &'static str {
		"pen"
	}

	fn input(&mut self, canvas: &mut CanvasState, input: &Input) -> Result<Outcome, CanvasError> {
		if let Input::Key(key) = input {
			return self.key(canvas, key);
		}

		let Some(event) = pointer(input) else {
			return Ok(Outcome::Idle);
		};
		let world = canvas.camera().to_world(event.position);

		match event.phase {
			_ if is_primary_down(event) => self.down(canvas, event.position),

			Phase::Move if self.dragging => {
				self.pull_handle(world);
				Ok(Outcome::Redraw)
			}

			Phase::Move => {
				self.hover = Some(world);
				Ok(Outcome::Redraw)
			}
			Phase::Up | Phase::Cancel => {
				self.dragging = false;
				Ok(Outcome::Idle)
			}
			Phase::Down => Ok(Outcome::Idle),
		}
	}

	fn overlay(&self, canvas: &CanvasState, out: &mut Vec<OverlayItem>) {
		let to_screen = canvas.camera().world_to_screen();
		let mut preview = self.anchors.clone();

		if let Some(hover) = self.hover.filter(|_| !self.dragging) {
			preview.push(Anchor {
				point: hover,
				out: Vec2::ZERO,
			});
		}

		if preview.len() >= 2 {
			out.push(OverlayItem::Path {
				path: to_screen * path(&preview, false),
			});
		}

		for anchor in &self.anchors {
			out.push(OverlayItem::Point {
				at: to_screen * anchor.point,
				kind: PointKind::Anchor,
			});

			if anchor.out != Vec2::ZERO {
				out.extend(
					[anchor.point + anchor.out, anchor.point - anchor.out].map(|control| {
						OverlayItem::Point {
							at: to_screen * control,
							kind: PointKind::Control,
						}
					}),
				);
			}
		}
	}

	fn cursor(&self, _canvas: &CanvasState) -> Cursor {
		Cursor::Crosshair
	}

	fn cancel(&mut self, _canvas: &mut CanvasState) {
		self.anchors.clear();
		self.dragging = false;
	}
}

/// The path through `anchors`: straight where neither end has a handle,
/// cubic otherwise.
fn path(anchors: &[Anchor], closed: bool) -> BezPath {
	let mut path = BezPath::new();
	let Some(first) = anchors.first() else {
		return path;
	};
	path.move_to(first.point);
	let segment = |path: &mut BezPath, from: &Anchor, to: &Anchor| {
		if from.out == Vec2::ZERO && to.out == Vec2::ZERO {
			path.line_to(to.point);
		} else {
			path.curve_to(from.point + from.out, to.point - to.out, to.point);
		}
	};

	for pair in anchors.windows(2) {
		segment(&mut path, &pair[0], &pair[1]);
	}

	if closed {
		if let (Some(last), Some(first)) = (anchors.last(), anchors.first()) {
			segment(&mut path, last, first);
		}

		path.close_path();
	}

	path
}
