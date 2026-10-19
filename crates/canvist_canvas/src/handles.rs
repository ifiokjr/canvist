//! Transform handles: where they are on screen, which one a pointer is on,
//! and how dragging one changes the selection's frame.

use std::f64::consts::PI;

use kurbo::Affine;
use kurbo::Line;
use kurbo::ParamCurveNearest;
use kurbo::Point;
use kurbo::Rect;

use crate::Camera;
use crate::Frame;

/// The centre of the frame's unit square.
const UNIT_CENTER: Point = Point::new(0.5, 0.5);
/// How precisely, in screen pixels, to find the nearest point on an edge.
const NEAREST_ACCURACY: f64 = 1e-6;

/// One of the frame's interactive parts. Corners and edges are numbered
/// clockwise from the frame's `(0, 0)` corner and its top edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Handle {
	/// A corner: resize both axes.
	Corner(u8),
	/// An edge midpoint: resize one axis, or skew with a modifier.
	Edge(u8),
	/// Just outside a corner: rotate.
	Rotate(u8),
	/// Inside the frame: move.
	Body,
}

impl Handle {
	/// The handle's position in the frame's unit square.
	#[must_use]
	pub fn unit(self) -> Point {
		const CORNERS: [Point; 4] = [
			Point::new(0.0, 0.0),
			Point::new(1.0, 0.0),
			Point::new(1.0, 1.0),
			Point::new(0.0, 1.0),
		];
		const EDGES: [Point; 4] = [
			Point::new(0.5, 0.0),
			Point::new(1.0, 0.5),
			Point::new(0.5, 1.0),
			Point::new(0.0, 0.5),
		];

		match self {
			Self::Corner(corner) | Self::Rotate(corner) => CORNERS[usize::from(corner % 4)],
			Self::Edge(edge) => EDGES[usize::from(edge % 4)],
			Self::Body => UNIT_CENTER,
		}
	}
}

/// Handle sizes, in screen pixels, so handles look the same at any zoom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleStyle {
	/// The side of a corner or edge square.
	pub size: f64,
	/// How far outside a corner the rotation zone reaches.
	pub rotate_reach: f64,
	/// How close to an edge counts as on it.
	pub edge_reach: f64,
}

impl Default for HandleStyle {
	fn default() -> Self {
		Self {
			size: 8.0,
			rotate_reach: 18.0,
			edge_reach: 4.0,
		}
	}
}

/// The eight resize squares in screen space: corners first, then edges.
#[must_use]
pub fn squares(frame: &Frame, camera: &Camera, style: &HandleStyle) -> Vec<(Handle, Rect)> {
	(0..4)
		.map(Handle::Corner)
		.chain((0..4).map(Handle::Edge))
		.map(|handle| {
			let unit = handle.unit();
			let center = camera.to_screen(frame.point(unit.x, unit.y));
			(
				handle,
				Rect::from_center_size(center, (style.size, style.size)),
			)
		})
		.collect()
}

/// The handle under screen point `point`, if any: squares first, then edges,
/// then rotation zones outside the corners, then the body.
#[must_use]
pub fn handle_at(
	frame: &Frame,
	camera: &Camera,
	point: Point,
	style: &HandleStyle,
) -> Option<Handle> {
	let squares = squares(frame, camera, style);

	if let Some((handle, _)) = squares
		.iter()
		.find(|(_, rect)| rect.inflate(1.0, 1.0).contains(point))
	{
		return Some(*handle);
	}

	let corners = frame.corners().map(|corner| camera.to_screen(corner));
	let edge = (0..4u8).find(|&edge| {
		let line = Line::new(
			corners[usize::from(edge)],
			corners[usize::from((edge + 1) % 4)],
		);
		line.nearest(point, NEAREST_ACCURACY).distance_sq.sqrt() <= style.edge_reach
	});

	if let Some(edge) = edge {
		return Some(Handle::Edge(edge));
	}

	let unit = frame.transform.inverse();
	let inside = |screen: Point| {
		let local = unit * camera.to_world(screen);
		(0.0..=1.0).contains(&local.x) && (0.0..=1.0).contains(&local.y)
	};

	if inside(point) {
		return Some(Handle::Body);
	}

	(0..4u8)
		.find(|&corner| corners[usize::from(corner)].distance(point) <= style.rotate_reach)
		.map(Handle::Rotate)
}

/// Modifiers that change a resize.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResizeOptions {
	/// Keep the aspect ratio (usually Shift).
	pub keep_aspect: bool,
	/// Resize around the centre (usually Alt).
	pub from_center: bool,
}

/// The frame after dragging `handle` from world point `start` to `current`.
///
/// The drag is measured in the frame's own unit square, so rotated and
/// skewed frames resize along their own axes. Dragging past the opposite
/// side flips the frame.
#[must_use]
pub fn resize(
	frame: &Frame,
	handle: Handle,
	start: Point,
	current: Point,
	options: ResizeOptions,
) -> Frame {
	let to_unit = frame.transform.inverse();
	let delta = (to_unit * current) - (to_unit * start);
	let grip = handle.unit();
	let anchor = if options.from_center {
		UNIT_CENTER
	} else {
		Point::new(1.0 - grip.x, 1.0 - grip.y)
	};

	let (moves_x, moves_y) = match handle {
		Handle::Edge(edge) => (edge % 2 == 1, edge % 2 == 0),
		_ => (true, true),
	};

	let mut scale_x = axis_scale(moves_x, grip.x, anchor.x, delta.x);
	let mut scale_y = axis_scale(moves_y, grip.y, anchor.y, delta.y);

	if options.keep_aspect {
		let scale = aspect_scale(scale_x, scale_y, moves_x, moves_y);
		scale_x = scale.copysign(scale_x);
		scale_y = scale.copysign(scale_y);
	}

	let around = Affine::translate(anchor.to_vec2());
	let scale = around * Affine::scale_non_uniform(scale_x, scale_y) * around.inverse();

	Frame {
		transform: frame.transform * scale,
	}
}

/// The scale along one unit-square axis when the grip at `grip` moves by
/// `delta` while `anchor` stays put. An axis the handle does not move, or
/// a grip on the anchor, keeps its scale.
fn axis_scale(moves: bool, grip: f64, anchor: f64, delta: f64) -> f64 {
	let span = grip - anchor;

	if moves && span.abs() > f64::EPSILON {
		(span + delta) / span
	} else {
		1.0
	}
}

/// The common scale magnitude that keeps the aspect ratio: an edge handle
/// follows the axis it moves, and a corner follows the larger change.
fn aspect_scale(scale_x: f64, scale_y: f64, moves_x: bool, moves_y: bool) -> f64 {
	if !moves_x {
		scale_y.abs()
	} else if !moves_y {
		scale_x.abs()
	} else {
		scale_x.abs().max(scale_y.abs())
	}
}

/// The frame after rotating with a drag from world point `start` to
/// `current` around its centre, and the angle applied. With `snap`, the
/// angle rounds to multiples of that many radians.
#[must_use]
pub fn rotate(frame: &Frame, start: Point, current: Point, snap: Option<f64>) -> (Frame, f64) {
	let center = frame.center();
	let angle = (current - center).atan2() - (start - center).atan2();
	let angle = snap
		.filter(|step| *step > 0.0)
		.map_or(angle, |step| (angle / step).round() * step);
	let angle = normalize_angle(angle);
	let rotation = Affine::rotate_about(angle, center);

	(
		Frame {
			transform: rotation * frame.transform,
		},
		angle,
	)
}

/// The frame after skewing it by dragging edge `edge` from world point
/// `start` to `current`: the dragged edge slides along itself while the
/// opposite edge stays put.
#[must_use]
pub fn skew(frame: &Frame, edge: u8, start: Point, current: Point) -> Frame {
	let to_unit = frame.transform.inverse();
	let delta = (to_unit * current) - (to_unit * start);
	let grip = Handle::Edge(edge).unit();
	let shear = if edge.is_multiple_of(2) {
		Affine::skew(delta.x * side(grip.y), 0.0)
	} else {
		Affine::skew(0.0, delta.y * side(grip.x))
	};

	let around = Affine::translate(UNIT_CENTER.to_vec2());

	Frame {
		transform: frame.transform * around * shear * around.inverse(),
	}
}

/// Which side of the unit square's centre line `position` is on: 1 past
/// it, -1 before it.
fn side(position: f64) -> f64 {
	if position > UNIT_CENTER.x { 1.0 } else { -1.0 }
}

/// The default rotation snap: 15 degrees.
pub const ROTATION_SNAP: f64 = PI / 12.0;

fn normalize_angle(angle: f64) -> f64 {
	let turn = 2.0 * PI;
	let wrapped = angle.rem_euclid(turn);

	if wrapped > PI {
		wrapped - turn
	} else {
		wrapped
	}
}

#[cfg(test)]
mod tests {
	use kurbo::Vec2;

	use super::*;

	/// The test frame's width.
	const WIDTH: f64 = 100.0;
	/// The test frame's height.
	const HEIGHT: f64 = 50.0;
	/// How far drags travel.
	const DRAG: f64 = 50.0;
	/// An x position left of the frame, past its left edge.
	const PAST_LEFT: f64 = -20.0;
	/// A screen point far from every handle.
	const FAR: Point = Point::new(300.0, 300.0);
	/// How close two points must be to count as the same.
	const EPSILON: f64 = 1e-9;

	fn frame() -> Frame {
		Frame::from_rect(Rect::new(0.0, 0.0, WIDTH, HEIGHT))
	}

	fn near(a: Point, b: Point) -> bool {
		a.distance(b) < EPSILON
	}

	#[test]
	fn dragging_a_corner_keeps_the_opposite_one() {
		let corner = Point::new(WIDTH, HEIGHT);
		let dragged = corner + Vec2::new(DRAG, DRAG);
		let resized = resize(
			&frame(),
			Handle::Corner(2),
			corner,
			dragged,
			ResizeOptions::default(),
		);

		assert!(near(resized.point(0.0, 0.0), Point::ORIGIN));
		assert!(near(resized.point(1.0, 1.0), dragged));
	}

	#[test]
	fn aspect_lock_and_centre_resizing() {
		let options = ResizeOptions {
			keep_aspect: true,
			from_center: true,
		};
		let corner = Point::new(WIDTH, HEIGHT);
		let center = Point::new(WIDTH / 2.0, HEIGHT / 2.0);
		// Half the width again from the centre doubles the frame on both axes.
		let resized = resize(
			&frame(),
			Handle::Corner(2),
			corner,
			corner + Vec2::new(WIDTH / 2.0, 0.0),
			options,
		);

		assert!(near(resized.center(), center));
		assert!(near(
			resized.point(1.0, 1.0),
			center + Vec2::new(WIDTH, HEIGHT)
		));
	}

	#[test]
	fn dragging_past_the_anchor_flips() {
		let resized = resize(
			&frame(),
			Handle::Edge(1),
			Point::new(WIDTH, HEIGHT / 2.0),
			Point::new(PAST_LEFT, HEIGHT / 2.0),
			ResizeOptions::default(),
		);

		assert!(near(resized.point(1.0, 0.0), Point::new(PAST_LEFT, 0.0)));
		assert!(resized.transform.determinant() < 0.0);
	}

	#[test]
	fn rotated_frames_resize_along_their_own_axes() {
		// A quarter turn points the frame's x axis down the screen.
		let rotated = Frame {
			transform: Affine::rotate(PI / 2.0) * frame().transform,
		};
		let start = rotated.point(1.0, 0.5);
		let resized = resize(
			&rotated,
			Handle::Edge(1),
			start,
			start + Vec2::new(0.0, DRAG),
			ResizeOptions::default(),
		);

		assert!(near(
			resized.point(1.0, 0.5),
			Point::new(-HEIGHT / 2.0, WIDTH + DRAG)
		));
	}

	#[test]
	fn rotation_snaps() {
		let center = Point::new(WIDTH / 2.0, HEIGHT / 2.0);
		let (rotated, angle) = rotate(
			&frame(),
			Point::new(WIDTH, center.y),
			Point::new(center.x, HEIGHT + DRAG),
			Some(ROTATION_SNAP),
		);

		assert!((angle - PI / 2.0).abs() < EPSILON);
		assert!(near(rotated.center(), center));
	}

	#[test]
	fn finds_handles_by_screen_position() {
		let camera = Camera::default();
		let style = HandleStyle::default();
		let probes = [
			(
				Point::new(WIDTH + 1.0, HEIGHT + 1.0),
				Some(Handle::Corner(2)),
			),
			(Point::new(WIDTH / 2.0, 1.0), Some(Handle::Edge(0))),
			(Point::new(WIDTH / 2.0, HEIGHT / 2.0), Some(Handle::Body)),
			(
				Point::new(WIDTH + 10.0, HEIGHT + 10.0),
				Some(Handle::Rotate(2)),
			),
			(FAR, None),
		];

		for (point, expected) in probes {
			assert_eq!(
				handle_at(&frame(), &camera, point, &style),
				expected,
				"at {point:?}"
			);
		}
	}
}
