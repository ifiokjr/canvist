//! Snapping and smart guides.
//!
//! While a selection moves, its left, centre, and right lines (and top,
//! middle, bottom) snap to the same lines of nearby nodes. Candidate lines
//! are sorted once when the drag starts, so each pointer move costs
//! `O(log n)` binary searches.

use canvist_scene::NodeId;
use canvist_scene::Scene;
use kurbo::Rect;
use kurbo::Vec2;
use rustc_hash::FxHashSet;

use crate::SceneIndex;

/// World units within which two candidate lines count as the same line.
const SAME_LINE: f64 = 1e-9;

/// Which way a guide runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
	/// A vertical line at an x position.
	X,
	/// A horizontal line at a y position.
	Y,
}

/// A line to draw while snapped, in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
	/// Which way the line runs.
	pub axis: Axis,
	/// Its x (for [`Axis::X`]) or y position.
	pub position: f64,
	/// Where it starts along the other axis.
	pub from: f64,
	/// Where it ends along the other axis.
	pub to: f64,
}

/// The result of snapping a moving rectangle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snap {
	/// The world offset to add to the move.
	pub offset: Vec2,
	/// The guides to show.
	pub guides: Vec<Guide>,
}

/// Snap candidates gathered when a drag starts.
#[derive(Clone, Debug, Default)]
pub struct Snapper {
	x_lines: Vec<(f64, Rect)>,
	y_lines: Vec<(f64, Rect)>,
	grid: Option<f64>,
}

impl Snapper {
	/// Candidates from nodes intersecting `area` (usually the viewport),
	/// leaving out `moving` and everything inside them.
	#[must_use]
	pub fn new(scene: &Scene, index: &SceneIndex, moving: &[NodeId], area: Rect) -> Self {
		let excluded: FxHashSet<NodeId> = moving.iter().flat_map(|&id| scene.subtree(id)).collect();
		let mut x_lines = Vec::new();
		let mut y_lines = Vec::new();

		for id in index.query(area) {
			if excluded.contains(&id)
				|| scene
					.ancestors(id)
					.any(|ancestor| excluded.contains(&ancestor))
			{
				continue;
			}

			let Some(bounds) = index.get(id).map(|geometry| geometry.bounds) else {
				continue;
			};

			x_lines.extend([bounds.x0, bounds.center().x, bounds.x1].map(|x| (x, bounds)));
			y_lines.extend([bounds.y0, bounds.center().y, bounds.y1].map(|y| (y, bounds)));
		}

		x_lines.sort_by(|a, b| a.0.total_cmp(&b.0));
		y_lines.sort_by(|a, b| a.0.total_cmp(&b.0));

		Self {
			x_lines,
			y_lines,
			grid: None,
		}
	}

	/// Also snap to a grid of `step` world units when no node is close.
	#[must_use]
	pub fn with_grid(mut self, step: f64) -> Self {
		self.grid = (step > 0.0).then_some(step);
		self
	}

	/// Snap `moving` (its bounds after the unsnapped move) within
	/// `tolerance` world units.
	#[must_use]
	pub fn snap(&self, moving: Rect, tolerance: f64) -> Snap {
		let x = best(
			&self.x_lines,
			[moving.x0, moving.center().x, moving.x1],
			tolerance,
		);
		let y = best(
			&self.y_lines,
			[moving.y0, moving.center().y, moving.y1],
			tolerance,
		);
		let offset = Vec2::new(
			x.as_ref()
				.map_or_else(|| self.grid_offset(moving.x0), |found| found.delta),
			y.as_ref()
				.map_or_else(|| self.grid_offset(moving.y0), |found| found.delta),
		);
		let moved = moving + offset;
		let mut guides = Vec::new();

		if let Some(found) = x {
			guides.push(guide(Axis::X, found.line, moved, &found.rects));
		}

		if let Some(found) = y {
			guides.push(guide(Axis::Y, found.line, moved, &found.rects));
		}

		Snap { offset, guides }
	}

	fn grid_offset(&self, value: f64) -> f64 {
		self.grid
			.map_or(0.0, |step| (value / step).round() * step - value)
	}
}

/// The closest candidate line to any of `edges`.
struct Found {
	line: f64,
	delta: f64,
	rects: Vec<Rect>,
}

/// The closest candidate line within `tolerance` of any of `edges`, with
/// every candidate rectangle on that line. `candidates` is sorted by line,
/// so each edge binary-searches to the start of its window.
fn best(candidates: &[(f64, Rect)], edges: [f64; 3], tolerance: f64) -> Option<Found> {
	let mut found: Option<Found> = None;

	for edge in edges {
		let start = candidates.partition_point(|(line, _)| *line < edge - tolerance);
		let window = candidates[start..]
			.iter()
			.take_while(|(line, _)| *line <= edge + tolerance);

		for &(line, rect) in window {
			consider(&mut found, line, line - edge, rect);
		}
	}

	found
}

/// Fold one candidate into the best so far: another rectangle on the same
/// line joins it, and a strictly closer line replaces it.
fn consider(found: &mut Option<Found>, line: f64, delta: f64, rect: Rect) {
	match found {
		Some(current) if (current.line - line).abs() < SAME_LINE => current.rects.push(rect),

		Some(current) if delta.abs() >= current.delta.abs() => {}
		_ => {
			*found = Some(Found {
				line,
				delta,
				rects: vec![rect],
			});
		}
	}
}

/// A guide through `line` spanning the moving rectangle and every snapped
/// candidate.
fn guide(axis: Axis, line: f64, moved: Rect, rects: &[Rect]) -> Guide {
	let span = |rect: &Rect| {
		match axis {
			Axis::X => (rect.y0, rect.y1),
			Axis::Y => (rect.x0, rect.x1),
		}
	};
	let (from, to) = rects
		.iter()
		.map(span)
		.fold(span(&moved), |(from, to), (a, b)| (from.min(a), to.max(b)));

	Guide {
		axis,
		position: line,
		from,
		to,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn snapper(rects: &[Rect]) -> Snapper {
		let mut x_lines = Vec::new();
		let mut y_lines = Vec::new();

		for &bounds in rects {
			x_lines.extend([bounds.x0, bounds.center().x, bounds.x1].map(|x| (x, bounds)));
			y_lines.extend([bounds.y0, bounds.center().y, bounds.y1].map(|y| (y, bounds)));
		}

		x_lines.sort_by(|a, b| a.0.total_cmp(&b.0));
		y_lines.sort_by(|a, b| a.0.total_cmp(&b.0));
		Snapper {
			x_lines,
			y_lines,
			grid: None,
		}
	}

	/// The one candidate node every test snaps against.
	const TARGET: Rect = Rect::new(100.0, 0.0, 200.0, 50.0);
	/// World units within which edges snap.
	const TOLERANCE: f64 = 5.0;
	/// Three units right of `TARGET`'s right edge, and below it.
	const NEAR_RIGHT: Rect = Rect::new(203.0, 80.0, 253.0, 130.0);
	/// Thirteen units right of `TARGET`, beyond the tolerance, and three units
	/// past a 10-unit grid line on both axes.
	const BEYOND: Rect = Rect::new(213.0, 83.0, 233.0, 103.0);
	/// The grid step for the grid-snapping test.
	const GRID: f64 = 10.0;
	/// How close two computed positions must be to count as equal.
	const EPSILON: f64 = 1e-9;

	#[test]
	fn edges_snap_to_nearby_edges_and_centres() {
		let snapper = snapper(&[TARGET]);
		let snap = snapper.snap(NEAR_RIGHT, TOLERANCE);

		assert_eq!(snap.offset, Vec2::new(TARGET.x1 - NEAR_RIGHT.x0, 0.0));
		assert_eq!(snap.guides.len(), 1);

		let guide = snap.guides[0];
		assert_eq!(guide.axis, Axis::X);
		assert!((guide.position - TARGET.x1).abs() < EPSILON);
		assert!((guide.from - TARGET.y0).abs() < EPSILON);
		assert!((guide.to - NEAR_RIGHT.y1).abs() < EPSILON);
	}

	#[test]
	fn nothing_snaps_beyond_the_tolerance() {
		let snapper = snapper(&[TARGET]).with_grid(GRID);
		let snap = snapper.snap(BEYOND, TOLERANCE);

		assert_eq!(snap.offset, Vec2::new(-3.0, -3.0));
		assert!(snap.guides.is_empty());
	}
}
