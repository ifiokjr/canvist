//! Node outlines and exact hit tests.
//!
//! Outlines are in a node's own coordinates: its box spans `(0, 0)` to its
//! size, and its transform places it in its parent.

use std::f64::consts::PI;

use canvist_scene::BoolOp;
use canvist_scene::FillRule;
use canvist_scene::Node;
use canvist_scene::NodeKind;
use canvist_scene::NodeSpec;
use canvist_scene::Shape;
use i_overlay::core::fill_rule::FillRule as OverlayFill;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use kurbo::Arc;
use kurbo::BezPath;
use kurbo::Ellipse;
use kurbo::ParamCurveNearest;
use kurbo::PathEl;
use kurbo::Point;
use kurbo::Rect;
use kurbo::RoundedRect;
use kurbo::RoundedRectRadii;
use kurbo::Shape as _;
use kurbo::Vec2;

/// How closely curves are approximated when flattened, in world units.
pub const TOLERANCE: f64 = 0.1;
/// Degrees in a full turn: an ellipse sweep this large is a whole ellipse.
const FULL_TURN_DEGREES: f64 = 360.0;
/// How precisely, in world units, to find the nearest point on an outline.
const NEAREST_ACCURACY: f64 = 1e-3;

/// The outline of a node with an own shape, in its own coordinates, or
/// `None` for nodes whose extent comes from their children (pages, groups)
/// and for booleans, whose outline [`boolean`] computes from the children.
#[must_use]
pub fn outline(node: &Node, spec: &NodeSpec, size: Vec2) -> Option<BezPath> {
	let props = node.props();
	let rect = Rect::from_origin_size(Point::ORIGIN, size.to_size());

	match (&props.shape, spec.kind()) {
		(_, NodeKind::Custom) => spec.geometry().map(|geometry| geometry(node)),
		(_, NodeKind::Page | NodeKind::Group | NodeKind::Boolean) => None,
		(
			Shape::Ellipse {
				start,
				sweep,
				inner,
			},
			_,
		) => Some(ellipse(rect, *start, *sweep, *inner)),
		(Shape::Polygon { points, ratio }, _) => Some(polygon(rect, *points, *ratio)),
		(Shape::Path { path, .. }, _) => Some((**path).clone()),
		_ => Some(rounded(rect, props.radius)),
	}
}

/// The fill rule of a node's outline.
#[must_use]
pub fn fill_rule(node: &Node) -> FillRule {
	match &node.props().shape {
		Shape::Path { rule, .. } => *rule,
		_ => FillRule::NonZero,
	}
}

fn rounded(rect: Rect, radius: [f64; 4]) -> BezPath {
	let limit = rect.width().min(rect.height()) / 2.0;
	let [top_left, top_right, bottom_right, bottom_left] = radius.map(|value| value.min(limit));

	if top_left + top_right + bottom_right + bottom_left == 0.0 {
		return rect.to_path(TOLERANCE);
	}

	let radii = RoundedRectRadii::new(top_left, top_right, bottom_right, bottom_left);
	RoundedRect::from_rect(rect, radii).to_path(TOLERANCE)
}

/// An ellipse inscribed in `rect`, cut to an arc of `sweep` degrees from
/// `start`, with an inner hole of `inner` times the radius.
fn ellipse(rect: Rect, start: f64, sweep: f64, inner: f64) -> BezPath {
	let full = sweep.abs() >= FULL_TURN_DEGREES;

	if full && inner <= 0.0 {
		return Ellipse::from_rect(rect).to_path(TOLERANCE);
	}

	let center = rect.center();
	let radii = Vec2::new(rect.width() / 2.0, rect.height() / 2.0);
	let (start, sweep) = (
		start.to_radians(),
		sweep
			.clamp(-FULL_TURN_DEGREES, FULL_TURN_DEGREES)
			.to_radians(),
	);
	let outer = Arc::new(center, radii, start, sweep, 0.0);
	let mut path = BezPath::new();
	path.move_to(point_on(center, radii, start));
	path.extend(outer.append_iter(TOLERANCE));

	if inner > 0.0 {
		let hole = radii * inner;
		let end = start + sweep;
		let back = Arc::new(center, hole, end, -sweep, 0.0);

		if full {
			path.close_path();
			path.move_to(point_on(center, hole, end));
		} else {
			path.line_to(point_on(center, hole, end));
		}

		path.extend(back.append_iter(TOLERANCE));
	} else {
		path.line_to(center);
	}

	path.close_path();
	path
}

fn point_on(center: Point, radii: Vec2, angle: f64) -> Point {
	center + Vec2::new(radii.x * angle.cos(), radii.y * angle.sin())
}

/// A regular polygon (or star, when `ratio` < 1) with `points` outer
/// corners, the first at the top, inscribed in `rect`.
fn polygon(rect: Rect, points: u32, ratio: f64) -> BezPath {
	let center = rect.center();
	let radii = Vec2::new(rect.width() / 2.0, rect.height() / 2.0);
	let star = ratio < 1.0;
	let corners = if star { points * 2 } else { points };

	closed_polygon((0..corners).map(|corner| {
		let angle = -PI / 2.0 + f64::from(corner) * 2.0 * PI / f64::from(corners);
		let inner = star && corner % 2 == 1;
		let scale = if inner { ratio } else { 1.0 };
		point_on(center, radii * scale, angle)
	}))
}

/// A closed path through `points` in order.
pub(crate) fn closed_polygon(points: impl IntoIterator<Item = Point>) -> BezPath {
	let mut path = BezPath::new();
	let mut points = points.into_iter();

	if let Some(first) = points.next() {
		path.move_to(first);
		points.for_each(|point| path.line_to(point));
	}

	path.close_path();
	path
}

/// Combine child outlines (already in the boolean node's coordinates) with
/// `op`. Curves are flattened; the result is polygons.
#[must_use]
pub fn boolean(op: BoolOp, children: &[(BezPath, FillRule)]) -> BezPath {
	let mut shapes = children.iter().map(|(path, rule)| normalize(path, *rule));
	let Some(first) = shapes.next() else {
		return BezPath::new();
	};

	let (rule, rest): (OverlayRule, Vec<Vec<Vec<[f64; 2]>>>) = match op {
		BoolOp::Union => (OverlayRule::Union, shapes.collect()),
		BoolOp::Intersect => (OverlayRule::Intersect, shapes.collect()),
		BoolOp::Exclude => (OverlayRule::Xor, shapes.collect()),
		BoolOp::Subtract => {
			let others = shapes.reduce(|a, b| overlay(&a, &b, OverlayRule::Union));
			(OverlayRule::Difference, others.into_iter().collect())
		}
	};

	let result = rest
		.iter()
		.fold(first, |acc, next| overlay(&acc, next, rule));

	from_contours(&result)
}

/// One shape's contours with its fill rule applied, so later overlays can
/// all use non-zero.
fn normalize(path: &BezPath, rule: FillRule) -> Vec<Vec<[f64; 2]>> {
	let contours = contours(path);

	let fill = match rule {
		FillRule::NonZero => OverlayFill::NonZero,
		FillRule::EvenOdd => OverlayFill::EvenOdd,
	};

	contours
		.overlay(&Vec::<Vec<[f64; 2]>>::new(), OverlayRule::Subject, fill)
		.into_iter()
		.flatten()
		.collect()
}

fn overlay(a: &[Vec<[f64; 2]>], b: &[Vec<[f64; 2]>], rule: OverlayRule) -> Vec<Vec<[f64; 2]>> {
	a.to_vec()
		.overlay(&b.to_vec(), rule, OverlayFill::NonZero)
		.into_iter()
		.flatten()
		.collect()
}

/// A path flattened into closed polygon contours.
fn contours(path: &BezPath) -> Vec<Vec<[f64; 2]>> {
	let mut out: Vec<Vec<[f64; 2]>> = Vec::new();

	kurbo::flatten(path.iter(), TOLERANCE, |element| {
		match element {
			PathEl::MoveTo(point) => out.push(vec![[point.x, point.y]]),
			PathEl::LineTo(point) => extend_last(&mut out, point),
			_ => {}
		}
	});

	out.retain(|contour| contour.len() >= 3);
	out
}

/// Add `point` to the last contour, if there is one.
fn extend_last(contours: &mut [Vec<[f64; 2]>], point: Point) {
	if let Some(contour) = contours.last_mut() {
		contour.push([point.x, point.y]);
	}
}

fn from_contours(contours: &[Vec<[f64; 2]>]) -> BezPath {
	let mut path = BezPath::new();

	for contour in contours {
		let mut points = contour.iter().map(|&[x, y]| Point::new(x, y));

		if let Some(first) = points.next() {
			path.move_to(first);
			points.for_each(|point| path.line_to(point));
			path.close_path();
		}
	}

	path
}

/// Whether `point` is inside `path` under `rule`.
#[must_use]
pub fn contains(path: &BezPath, rule: FillRule, point: Point) -> bool {
	let winding = path.winding(point);

	match rule {
		FillRule::NonZero => winding != 0,
		FillRule::EvenOdd => winding % 2 != 0,
	}
}

/// The distance from `point` to the nearest point of `path`'s outline.
#[must_use]
pub fn distance_to_outline(path: &BezPath, point: Point) -> f64 {
	path.segments()
		.map(|segment| segment.nearest(point, NEAREST_ACCURACY).distance_sq)
		.fold(f64::INFINITY, f64::min)
		.sqrt()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn square(x: f64, y: f64, side: f64) -> BezPath {
		Rect::new(x, y, x + side, y + side).to_path(TOLERANCE)
	}

	/// The box the shape tests inscribe into.
	const BOX: Rect = Rect::new(0.0, 0.0, 100.0, 100.0);
	/// A point just inside `BOX`'s corner, outside any inscribed shape.
	const NEAR_CORNER: Point = Point::new(2.0, 2.0);
	/// The side of the boolean and distance tests' squares.
	const SIDE: f64 = 10.0;
	/// How far the second boolean square is shifted, so the two overlap.
	const SHIFT: f64 = 5.0;

	#[test]
	fn polygons_and_stars_have_the_right_corners() {
		/// A five-pointed star has ten corners.
		const STAR_POINTS: u32 = 5;

		let triangle = polygon(BOX, 3, 1.0);
		let star = polygon(BOX, STAR_POINTS, 0.5);

		assert_eq!(triangle.segments().count(), 3);
		assert_eq!(star.segments().count(), 10);
		assert!(contains(&star, FillRule::NonZero, BOX.center()));
		assert!(!contains(&star, FillRule::NonZero, NEAR_CORNER));
	}

	#[test]
	fn rings_have_holes() {
		/// On the ring: halfway down, near the left edge.
		const ON_RING: Point = Point::new(10.0, 50.0);

		let ring = ellipse(BOX, 0.0, FULL_TURN_DEGREES, 0.5);

		assert!(contains(&ring, FillRule::EvenOdd, ON_RING));
		assert!(!contains(&ring, FillRule::EvenOdd, BOX.center()));
	}

	#[test]
	fn booleans_combine_children() {
		/// Inside both squares.
		const OVERLAP: Point = Point::new(7.0, 7.0);

		let first = (square(0.0, 0.0, SIDE), FillRule::NonZero);
		let second = (square(SHIFT, SHIFT, SIDE), FillRule::NonZero);
		let check = |op, first_only: bool, both: bool| {
			let result = boolean(op, &[first.clone(), second.clone()]);
			assert_eq!(
				contains(&result, FillRule::NonZero, NEAR_CORNER),
				first_only,
				"{op:?}"
			);
			assert_eq!(
				contains(&result, FillRule::NonZero, OVERLAP),
				both,
				"{op:?}"
			);
		};

		check(BoolOp::Union, true, true);
		check(BoolOp::Subtract, true, false);
		check(BoolOp::Intersect, false, true);
		check(BoolOp::Exclude, true, false);
	}

	#[test]
	fn distance_measures_to_the_outline() {
		/// How close a measured distance must be to the expected one.
		const EPSILON: f64 = 1e-6;

		let path = square(0.0, 0.0, SIDE);
		let middle = SIDE / 2.0;

		assert!((distance_to_outline(&path, Point::new(middle, -3.0)) - 3.0).abs() < EPSILON);
		assert!((distance_to_outline(&path, Point::new(middle, 4.0)) - 4.0).abs() < EPSILON);
	}
}
