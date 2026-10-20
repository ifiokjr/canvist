//! Hit testing: which node is under a point, and which nodes a marquee
//! covers.

use std::cmp::Ordering;

use canvist_scene::NodeId;
use canvist_scene::NodeKind;
use canvist_scene::Scene;
use kurbo::Point;
use kurbo::Rect;
use rustc_hash::FxHashSet;

use crate::SceneIndex;
use crate::geometry;

/// How a marquee picks nodes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarqueeMode {
	/// Nodes the marquee touches.
	#[default]
	Intersect,
	/// Only nodes entirely inside the marquee.
	Contain,
}

/// The topmost visible, unlocked node under world point `point`, within
/// `tolerance` world units of its stroke or outline.
///
/// Candidates come from the index's R-tree; each is then tested exactly:
/// inside its fill (by its fill rule), on its stroke, or near the outline of
/// an unfilled shape. Frames that clip hide what lies outside them.
#[must_use]
pub fn hit_test(scene: &Scene, index: &SceneIndex, point: Point, tolerance: f64) -> Option<NodeId> {
	let probe = Rect::from_center_size(point, (2.0 * tolerance, 2.0 * tolerance));

	index
		.query(probe)
		.filter(|&id| interactive(scene, id) && hits(scene, index, id, point, tolerance))
		.filter(|&id| unclipped(scene, index, id, point))
		.max_by(|&a, &b| paint_order(scene, a, b))
}

/// What clicking `hit` selects: the hit node itself when `deep`, otherwise
/// the outermost group or boolean around it. Frames do not capture clicks,
/// so their children are selected directly.
#[must_use]
pub fn selection_target(scene: &Scene, hit: NodeId, deep: bool) -> NodeId {
	if deep {
		return hit;
	}

	let mut target = hit;

	for ancestor in scene.ancestors(hit) {
		match scene.kind(ancestor) {
			Some(NodeKind::Group | NodeKind::Boolean | NodeKind::Instance) => target = ancestor,
			Some(NodeKind::Page) => break,
			_ => {}
		}
	}

	target
}

/// The selection targets a marquee over world rectangle `rect` picks, in
/// paint order, without nodes whose ancestor is also picked.
#[must_use]
pub fn marquee(scene: &Scene, index: &SceneIndex, rect: Rect, mode: MarqueeMode) -> Vec<NodeId> {
	let targets: FxHashSet<NodeId> = index
		.query(rect)
		.filter(|&id| interactive(scene, id))
		.filter(|&id| {
			let Some(bounds) = index.get(id).map(|geometry| geometry.bounds) else {
				return false;
			};

			match mode {
				MarqueeMode::Intersect => true,
				MarqueeMode::Contain => {
					rect.contains(bounds.origin())
						&& rect.contains(Point::new(bounds.x1, bounds.y1))
				}
			}
		})
		.map(|id| selection_target(scene, id, false))
		.collect();
	// Drop targets inside other targets before sorting, so the sort only
	// sees what is picked.
	let mut picked: Vec<NodeId> = targets
		.iter()
		.copied()
		.filter(|&id| {
			!scene
				.ancestors(id)
				.any(|ancestor| targets.contains(&ancestor))
		})
		.collect();
	picked.sort_by_cached_key(|&id| order_key(scene, id));
	picked
}

/// Whether the node and every ancestor are visible and unlocked.
fn interactive(scene: &Scene, id: NodeId) -> bool {
	std::iter::once(id)
		.chain(scene.ancestors(id))
		.filter_map(|node| scene.node(node))
		.all(|node| node.props().visible && !node.props().locked)
}

/// The exact test against the node's outline.
fn hits(scene: &Scene, index: &SceneIndex, id: NodeId, point: Point, tolerance: f64) -> bool {
	let (Some(node), Some(found), Some(kind)) = (scene.node(id), index.get(id), scene.kind(id))
	else {
		return false;
	};
	let Some(outline) = &found.outline else {
		return false;
	};
	let local = found.world.inverse() * point;
	let props = node.props();
	let solid = matches!(
		kind,
		NodeKind::Frame
			| NodeKind::Text
			| NodeKind::Image
			| NodeKind::Instance
			| NodeKind::Component
	);

	let filled = solid || props.fills.iter().any(|paint| paint.visible);

	if filled && geometry::contains(outline, geometry::fill_rule(node), local) {
		return true;
	}

	let stroked = props.strokes.iter().any(|paint| paint.visible);
	let reach = if stroked {
		props.stroke.width / 2.0
	} else {
		0.0
	};

	geometry::distance_to_outline(outline, local) <= reach + tolerance
}

/// Whether every clipping ancestor contains `point`.
fn unclipped(scene: &Scene, index: &SceneIndex, id: NodeId, point: Point) -> bool {
	scene.ancestors(id).all(|ancestor| {
		let clips = scene.node(ancestor).is_some_and(|node| node.props().clip);

		if !clips {
			return true;
		}

		index.get(ancestor).is_none_or(|found| {
			found.outline.as_ref().is_none_or(|outline| {
				let local = found.world.inverse() * point;
				geometry::contains(outline, canvist_scene::FillRule::NonZero, local)
			})
		})
	})
}

/// A key that sorts nodes into paint order: the index path from the root.
#[must_use]
pub fn order_key(scene: &Scene, id: NodeId) -> Vec<usize> {
	let mut path: Vec<usize> = std::iter::once(id)
		.chain(scene.ancestors(id))
		.map(|node| scene.index_in_parent(node).unwrap_or(0))
		.collect();
	path.reverse();
	path
}

/// Paint order: ancestors paint before descendants, and earlier siblings
/// before later ones.
#[must_use]
pub fn paint_order(scene: &Scene, a: NodeId, b: NodeId) -> Ordering {
	let chain = |id: NodeId| {
		let mut chain: Vec<NodeId> = scene.ancestors(id).collect();
		chain.reverse();
		chain.push(id);
		chain
	};
	let (left, right) = (chain(a), chain(b));

	for (x, y) in left.iter().zip(&right) {
		if x != y {
			let index = |id: NodeId| scene.index_in_parent(id);

			return index(*x).cmp(&index(*y));
		}
	}

	left.len().cmp(&right.len())
}
