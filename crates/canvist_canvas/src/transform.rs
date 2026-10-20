//! Turning a change of the selection's frame into scene steps.
//!
//! A drag captures each node's state when it starts ([`Originals`]) and
//! every pointer move computes absolute values from that state, so steps
//! never accumulate rounding drift. Sizes stay separate from transforms:
//! resizing a rotated rectangle changes its `size`, and its `transform`
//! keeps only translation, rotation, skew, and reflection.

use std::sync::Arc;

use canvist_scene::Constraint;
use canvist_scene::NodeId;
use canvist_scene::NodeKind;
use canvist_scene::Prop;
use canvist_scene::Scene;
use canvist_scene::Shape;
use canvist_scene::Step;
use canvist_scene::TextSizing;
use kurbo::Affine;
use kurbo::Vec2;
use rustc_hash::FxHashMap;

use crate::Frame;
use crate::SceneIndex;

/// World units below which a change of size counts as no change.
const SIZE_EPSILON: f64 = 1e-9;

/// One node's state when a gesture started.
#[derive(Clone, Debug, PartialEq)]
pub struct Original {
	/// The node's box-to-world transform.
	pub world: Affine,
	/// Its parent's world transform.
	pub parent_world: Affine,
	/// Its `transform` prop.
	pub transform: Affine,
	/// Its effective size.
	pub size: Vec2,
	/// Its shape.
	pub shape: Shape,
}

/// The state of the selected nodes and their descendants when a gesture
/// started.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Originals {
	nodes: FxHashMap<NodeId, Original>,
	roots: Vec<NodeId>,
}

impl Originals {
	/// Capture `roots` and everything inside them.
	#[must_use]
	pub fn capture(scene: &Scene, index: &SceneIndex, roots: &[NodeId]) -> Self {
		let mut nodes = FxHashMap::default();

		for id in roots.iter().flat_map(|&root| scene.subtree(root)) {
			let (Some(node), Some(geometry)) = (scene.node(id), index.get(id)) else {
				continue;
			};
			let parent_world = index
				.get(node.parent())
				.map_or(Affine::IDENTITY, |parent| parent.world);
			nodes.insert(
				id,
				Original {
					world: geometry.world,
					parent_world,
					transform: node.props().transform,
					size: geometry.size,
					shape: node.props().shape.clone(),
				},
			);
		}

		Self {
			nodes,
			roots: roots.to_vec(),
		}
	}

	/// The captured roots.
	#[must_use]
	pub fn roots(&self) -> &[NodeId] {
		&self.roots
	}

	/// The captured state of `id`.
	#[must_use]
	pub fn get(&self, id: NodeId) -> Option<&Original> {
		self.nodes.get(&id)
	}

	/// The steps that move every root from frame `from` to frame `to`.
	#[must_use]
	pub fn steps(&self, scene: &Scene, from: &Frame, to: &Frame) -> Vec<Step> {
		let delta = to.transform * from.transform.inverse();
		let mut steps = Vec::new();

		for &root in &self.roots {
			self.transform_node(scene, root, delta, &mut steps);
		}

		steps
	}

	/// The steps that move every root by world offset `offset`.
	#[must_use]
	pub fn translate(&self, scene: &Scene, offset: Vec2) -> Vec<Step> {
		let mut steps = Vec::new();

		for &root in &self.roots {
			let (Some(original), Some(node)) = (self.nodes.get(&root), scene.node(root)) else {
				continue;
			};

			if scene.kind(root) == Some(NodeKind::Page) || node.props().locked {
				continue;
			}

			let local =
				original.parent_world.inverse() * Affine::translate(offset) * original.parent_world;
			push_prop(
				&mut steps,
				scene,
				root,
				Prop::Transform(local * original.transform),
			);
		}

		steps
	}

	/// Apply world-space `delta` to node `id`. Groups and booleans pass it to
	/// their children; everything else gets a new transform and size.
	fn transform_node(&self, scene: &Scene, id: NodeId, delta: Affine, steps: &mut Vec<Step>) {
		let (Some(original), Some(kind)) = (self.nodes.get(&id), scene.kind(id)) else {
			return;
		};

		if kind == NodeKind::Page {
			return;
		}

		if matches!(kind, NodeKind::Group | NodeKind::Boolean) {
			for &child in scene.children(id) {
				self.transform_node(scene, child, delta, steps);
			}

			return;
		}

		let boxed =
			delta * original.world * Affine::scale_non_uniform(original.size.x, original.size.y);
		let (transform, size) = decompose(original.parent_world.inverse() * boxed);
		push_prop(steps, scene, id, Prop::Transform(transform));
		self.resize_node(scene, id, original, size, steps);
	}

	/// Set a new size, rescaling path data, switching auto-width text to
	/// a fixed width, and applying constraints to a frame's children.
	fn resize_node(
		&self,
		scene: &Scene,
		id: NodeId,
		original: &Original,
		size: Vec2,
		steps: &mut Vec<Step>,
	) {
		if (size - original.size).hypot() < SIZE_EPSILON {
			return;
		}

		push_prop(steps, scene, id, Prop::Size(size));

		if let Some(shape) = resized_shape(&original.shape, original.size, size) {
			push_prop(steps, scene, id, Prop::Shape(shape));
		}

		let frames_children = matches!(scene.kind(id), Some(NodeKind::Frame | NodeKind::Component))
			&& scene
				.node(id)
				.is_some_and(|node| node.props().layout.is_none());

		if frames_children {
			for &child in scene.children(id) {
				self.constrain(scene, child, original.size, size, steps);
			}
		}
	}

	/// Reposition and resize a frame's child for the frame's resize from
	/// `old` to `new`, by the child's constraints.
	fn constrain(&self, scene: &Scene, id: NodeId, old: Vec2, new: Vec2, steps: &mut Vec<Step>) {
		let (Some(original), Some(node)) = (self.nodes.get(&id), scene.node(id)) else {
			return;
		};
		let constraints = node.props().constraints;
		let position = original.transform.translation();
		let (x, width) = constrain_axis(
			constraints.horizontal,
			position.x,
			original.size.x,
			old.x,
			new.x,
		);
		let (y, height) = constrain_axis(
			constraints.vertical,
			position.y,
			original.size.y,
			old.y,
			new.y,
		);
		let transform = original.transform.with_translation(Vec2::new(x, y));
		push_prop(steps, scene, id, Prop::Transform(transform));

		if (width - original.size.x).abs() > SIZE_EPSILON
			|| (height - original.size.y).abs() > SIZE_EPSILON
		{
			self.resize_node(scene, id, original, Vec2::new(width, height), steps);
		}
	}
}

/// One axis of a constraint: the child's new start and length when its
/// parent's length changes from `old` to `new`.
fn constrain_axis(
	constraint: Constraint,
	start: f64,
	length: f64,
	old: f64,
	new: f64,
) -> (f64, f64) {
	let grow = new - old;

	match constraint {
		Constraint::Start => (start, length),
		Constraint::End => (start + grow, length),
		Constraint::Both => (start, (length + grow).max(0.0)),
		Constraint::Center => (start + grow / 2.0, length),
		Constraint::Scale => {
			let factor = scale_factor(new, old);
			(start * factor, length * factor)
		}
	}
}

/// Split a unit-box mapping into a scale-free transform and a size.
fn decompose(boxed: Affine) -> (Affine, Vec2) {
	let [a, b, c, d, _, _] = boxed.as_coeffs();
	let width = a.hypot(b);
	let height = if width > f64::EPSILON {
		(a * d - b * c).abs() / width
	} else {
		c.hypot(d)
	};

	let scale = Affine::scale_non_uniform(
		1.0 / width.max(f64::EPSILON),
		1.0 / height.max(f64::EPSILON),
	);

	(boxed * scale, Vec2::new(width, height))
}

/// The shape a node needs after resizing from `old` to `new`, or `None`
/// when it keeps its shape: path data rescales with the box, and auto-width
/// text whose width changed becomes fixed-width.
fn resized_shape(shape: &Shape, old: Vec2, new: Vec2) -> Option<Shape> {
	match shape {
		Shape::Path { path, rule } => {
			let scale = ratio(new, old);
			let scaled = Affine::scale_non_uniform(scale.x, scale.y) * (**path).clone();

			Some(Shape::Path {
				path: Arc::new(scaled),
				rule: *rule,
			})
		}

		Shape::Text(TextSizing::AutoWidth) if (new.x - old.x).abs() > SIZE_EPSILON => {
			Some(Shape::Text(TextSizing::AutoHeight))
		}

		_ => None,
	}
}

/// The per-axis scale from size `old` to size `new`.
fn ratio(new: Vec2, old: Vec2) -> Vec2 {
	Vec2::new(scale_factor(new.x, old.x), scale_factor(new.y, old.y))
}

/// How much a length scales going from `old` to `new`; a zero `old` has no
/// meaningful scale, so it stays at 1.
fn scale_factor(new: f64, old: f64) -> f64 {
	if old.abs() > f64::EPSILON {
		new / old
	} else {
		1.0
	}
}

/// Add a `SetProp` step unless the node already has that value.
fn push_prop(steps: &mut Vec<Step>, scene: &Scene, node: NodeId, prop: Prop) {
	let current = scene.node(node).map(|found| found.props().get(&prop.key()));

	if current.as_ref() != Some(&prop) {
		steps.push(Step::SetProp { node, prop });
	}
}

#[cfg(test)]
mod tests {
	use std::f64::consts::PI;

	use super::*;

	/// Where the decomposed box sits.
	const OFFSET: Vec2 = Vec2::new(10.0, 20.0);
	/// The decomposed box's size.
	const SIZE: Vec2 = Vec2::new(40.0, 30.0);
	/// The decomposed box's rotation: 30 degrees.
	const ANGLE: f64 = PI / 6.0;

	/// Where the constrained child starts along the axis.
	const START: f64 = 10.0;
	/// The constrained child's length along the axis.
	const LENGTH: f64 = 20.0;
	/// The parent's length before the resize.
	const OLD: f64 = 100.0;
	/// The parent's length after the resize: half as long again.
	const NEW: f64 = 150.0;

	#[test]
	fn decompose_keeps_rotation_and_moves_scale_into_size() {
		let boxed = Affine::translate(OFFSET)
			* Affine::rotate(ANGLE)
			* Affine::scale_non_uniform(SIZE.x, SIZE.y);
		let (transform, size) = decompose(boxed);

		assert!((size - SIZE).hypot() < SIZE_EPSILON);
		assert!((transform.determinant() - 1.0).abs() < SIZE_EPSILON);

		let rebuilt = transform * Affine::scale_non_uniform(size.x, size.y);
		let matches = rebuilt
			.as_coeffs()
			.iter()
			.zip(boxed.as_coeffs())
			.all(|(rebuilt, original)| (rebuilt - original).abs() < SIZE_EPSILON);
		assert!(matches);
	}

	#[test]
	fn constraints_follow_the_parent() {
		let grow = NEW - OLD;
		let factor = NEW / OLD;
		let expected = [
			(Constraint::Start, (START, LENGTH)),
			(Constraint::End, (START + grow, LENGTH)),
			(Constraint::Both, (START, LENGTH + grow)),
			(Constraint::Center, (START + grow / 2.0, LENGTH)),
			(Constraint::Scale, (START * factor, LENGTH * factor)),
		];

		for (constraint, placed) in expected {
			assert_eq!(
				constrain_axis(constraint, START, LENGTH, OLD, NEW),
				placed,
				"{constraint:?}"
			);
		}
	}
}
