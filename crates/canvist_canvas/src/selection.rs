//! The selection and the frame its transform handles surround.

use canvist_scene::NodeId;
use kurbo::Affine;
use kurbo::Point;
use kurbo::Rect;
use kurbo::Vec2;

use crate::SceneIndex;

/// The selected nodes, in the order they were selected.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
	nodes: Vec<NodeId>,
}

impl Selection {
	/// The selected nodes.
	#[must_use]
	pub fn nodes(&self) -> &[NodeId] {
		&self.nodes
	}

	/// Whether nothing is selected.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.nodes.is_empty()
	}

	/// Whether `id` is selected.
	#[must_use]
	pub fn contains(&self, id: NodeId) -> bool {
		self.nodes.contains(&id)
	}

	/// Select exactly `ids`.
	pub fn set(&mut self, ids: impl IntoIterator<Item = NodeId>) {
		self.nodes.clear();

		for id in ids {
			if !self.nodes.contains(&id) {
				self.nodes.push(id);
			}
		}
	}

	/// Add `id`, or remove it if it is already selected.
	pub fn toggle(&mut self, id: NodeId) {
		match self.nodes.iter().position(|&node| node == id) {
			Some(at) => {
				self.nodes.remove(at);
			}
			None => self.nodes.push(id),
		}
	}

	/// Select nothing.
	pub fn clear(&mut self) {
		self.nodes.clear();
	}

	/// Drop nodes that no longer exist.
	pub fn retain(&mut self, mut keep: impl FnMut(NodeId) -> bool) {
		self.nodes.retain(|&id| keep(id));
	}

	/// The frame transform handles surround: a single node's own rotated
	/// box, or the world bounds of several.
	#[must_use]
	pub fn frame(&self, index: &SceneIndex) -> Option<Frame> {
		match self.nodes.as_slice() {
			[] => None,
			[single] => {
				let geometry = index.get(*single)?;
				Some(Frame::from_box(geometry.world, geometry.size))
			}
			many => index.bounds_of(many).map(Frame::from_rect),
		}
	}
}

/// A parallelogram in the world: the unit square mapped by `transform`.
/// Handles sit on its corners and edge midpoints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
	/// Maps the unit square to the world.
	pub transform: Affine,
}

impl Frame {
	/// The frame of a box of `size` placed by `world`. A zero-size axis is
	/// kept at a tiny extent so the frame stays invertible.
	#[must_use]
	pub fn from_box(world: Affine, size: Vec2) -> Self {
		let size = Vec2::new(size.x.max(f64::EPSILON), size.y.max(f64::EPSILON));

		Self {
			transform: world * Affine::scale_non_uniform(size.x, size.y),
		}
	}

	/// The frame of an axis-aligned world rectangle.
	#[must_use]
	pub fn from_rect(rect: Rect) -> Self {
		Self::from_box(
			Affine::translate(rect.origin().to_vec2()),
			rect.size().to_vec2(),
		)
	}

	/// The world point at unit coordinates `(u, v)`: `(0, 0)` is the first
	/// corner, `(1, 1)` the opposite one.
	#[must_use]
	pub fn point(&self, u: f64, v: f64) -> Point {
		self.transform * Point::new(u, v)
	}

	/// The four corners, clockwise from `(0, 0)`.
	#[must_use]
	pub fn corners(&self) -> [Point; 4] {
		[
			self.point(0.0, 0.0),
			self.point(1.0, 0.0),
			self.point(1.0, 1.0),
			self.point(0.0, 1.0),
		]
	}

	/// The centre.
	#[must_use]
	pub fn center(&self) -> Point {
		self.point(0.5, 0.5)
	}
}
