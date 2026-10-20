//! The scene index: derived geometry for every node, kept up to date from
//! commits.
//!
//! For each node it holds the world transform, the effective size (text
//! frames take their text's size, hugging frames their content's), the
//! outline, and world bounds. An R-tree over the bounds of hit-testable
//! nodes answers point and rectangle queries in `O(log n + hits)`.
//!
//! A commit recomputes only the subtrees it touched. A node whose geometry
//! depends on its children (a group, a boolean, an auto-layout frame) is
//! recomputed with them.

use std::sync::Arc;

use canvist_layout::LayoutError;
use canvist_scene::NodeId;
use canvist_scene::NodeKind;
use canvist_scene::Scene;
use canvist_scene::SceneCommit;
use canvist_scene::Sizing;
use canvist_scene::Step;
use canvist_scene::StrokeAlign;
use kurbo::Affine;
use kurbo::BezPath;
use kurbo::Point;
use kurbo::Rect;
use kurbo::Shape as _;
use kurbo::Vec2;
use rstar::AABB;
use rstar::RTree;
use rstar::primitives::GeomWithData;
use rstar::primitives::Rectangle;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use crate::TextLayouts;
use crate::geometry;
use crate::layout::FlowItem;
use crate::layout::flow;

type TreeEntry = GeomWithData<Rectangle<[f64; 2]>, NodeId>;

/// Derived geometry of one node.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeGeometry {
	/// Maps the node's box (origin at its top-left corner) to the world.
	pub world: Affine,
	/// The effective size: auto-sized text and hugging frames differ from
	/// the `size` prop.
	pub size: Vec2,
	/// The outline in the node's own coordinates, for nodes with a shape.
	pub outline: Option<Arc<BezPath>>,
	/// The world bounds of what the node itself paints, strokes included.
	/// Groups, which paint nothing themselves, take the union of their
	/// children's bounds. So do pages, but a page's bounds, like `extent`,
	/// only grow between rebuilds.
	pub bounds: Rect,
	/// The world bounds of the node and everything inside it. It may be
	/// larger than needed after children shrink: it is only used to skip
	/// subtrees outside the viewport.
	pub extent: Rect,
	/// Whether hit testing considers the node.
	pub hittable: bool,
}

/// Derived geometry for a whole scene.
#[derive(Debug, Default)]
pub struct SceneIndex {
	nodes: FxHashMap<NodeId, NodeGeometry>,
	tree: RTree<TreeEntry>,
}

impl SceneIndex {
	/// Index every node of `scene`, laying out its text frames.
	pub fn build(scene: &Scene, texts: &mut TextLayouts) -> Result<Self, LayoutError> {
		let mut index = Self::default();
		let mut entries = Vec::with_capacity(scene.len());
		let mut builder = Builder {
			scene,
			texts,
			nodes: &mut index.nodes,
			entries: &mut entries,
		};

		for &page in scene.pages() {
			builder.place(page, Affine::IDENTITY, None)?;
		}

		index.tree = RTree::bulk_load(entries);
		Ok(index)
	}

	/// Update the index after `commit` was applied to `scene`.
	pub fn apply(
		&mut self,
		scene: &Scene,
		commit: &SceneCommit,
		texts: &mut TextLayouts,
	) -> Result<(), LayoutError> {
		for record in removed(commit) {
			self.forget(record);
			texts.remove(record);
		}

		let changed = changed(scene, commit);
		self.refresh(scene, &changed, texts)
	}

	/// Recompute the geometry of `ids` (and whatever depends on them), for
	/// changes that did not arrive as commits, such as a lent text frame's
	/// edits.
	pub fn refresh(
		&mut self,
		scene: &Scene,
		ids: &[NodeId],
		texts: &mut TextLayouts,
	) -> Result<(), LayoutError> {
		let roots = roots(scene, ids);

		for &root in &roots {
			for id in scene.subtree(root) {
				self.forget(id);
			}
		}

		let mut entries = Vec::new();

		for &root in &roots {
			let parent_world = self.parent_world(scene, root);
			let mut builder = Builder {
				scene,
				texts,
				nodes: &mut self.nodes,
				entries: &mut entries,
			};
			builder.place(root, parent_world, None)?;
		}

		for entry in entries {
			self.tree.insert(entry);
		}

		for &root in &roots {
			self.grow_ancestors(scene, root);
		}

		Ok(())
	}

	/// The geometry of node `id`.
	#[must_use]
	pub fn get(&self, id: NodeId) -> Option<&NodeGeometry> {
		self.nodes.get(&id)
	}

	/// How many nodes the index holds.
	#[must_use]
	pub fn len(&self) -> usize {
		self.nodes.len()
	}

	/// Whether the index is empty.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.nodes.is_empty()
	}

	/// Hit-testable nodes whose bounds intersect `rect`, in no particular
	/// order.
	pub fn query(&self, rect: Rect) -> impl Iterator<Item = NodeId> + '_ {
		let envelope = AABB::from_corners([rect.x0, rect.y0], [rect.x1, rect.y1]);
		self.tree
			.locate_in_envelope_intersecting(envelope)
			.map(|entry| entry.data)
	}

	/// The union of the bounds of `ids`.
	#[must_use]
	pub fn bounds_of(&self, ids: &[NodeId]) -> Option<Rect> {
		ids.iter()
			.filter_map(|id| self.nodes.get(id))
			.map(|geometry| geometry.bounds)
			.reduce(|a, b| a.union(b))
	}

	fn forget(&mut self, id: NodeId) {
		if let Some(old) = self.nodes.remove(&id)
			&& old.hittable
		{
			self.tree.remove(&entry(old.bounds, id));
		}
	}

	fn parent_world(&self, scene: &Scene, id: NodeId) -> Affine {
		scene
			.node(id)
			.and_then(|node| self.nodes.get(&node.parent()))
			.map_or(Affine::IDENTITY, |parent| parent.world)
	}

	/// Widen the extents of `id`'s ancestors to cover its new extent. Pages
	/// take their bounds from their children, so a page's bounds widen too.
	/// (Groups are never above a recomputed root without being part of it:
	/// a change inside a group climbs to the group.)
	fn grow_ancestors(&mut self, scene: &Scene, id: NodeId) {
		let Some(geometry) = self.nodes.get(&id) else {
			return;
		};
		let (bounds, extent) = (geometry.bounds, geometry.extent);

		for ancestor in scene.ancestors(id) {
			let is_page = scene.kind(ancestor) == Some(NodeKind::Page);

			let Some(geometry) = self.nodes.get_mut(&ancestor) else {
				continue;
			};

			geometry.extent = geometry.extent.union(extent);

			if is_page {
				geometry.bounds = geometry.bounds.union(bounds);
			}
		}
	}
}

/// Every node a commit removed: its `RemoveNodes` inverses list them all.
fn removed(commit: &SceneCommit) -> impl Iterator<Item = NodeId> + '_ {
	commit
		.steps
		.iter()
		.zip(&commit.inverses)
		.filter(|(step, _)| matches!(step, Step::RemoveNodes { .. }))
		.flat_map(|(_, inverse)| {
			match inverse {
				Step::InsertNodes { nodes } => nodes.iter().map(|record| record.id).collect(),
				_ => Vec::new(),
			}
		})
}

/// The nodes a commit changed directly, plus the old parents of moved and
/// removed nodes when those parents take their geometry from children.
/// Other parents keep their geometry: only their extent may need to grow.
fn changed(scene: &Scene, commit: &SceneCommit) -> Vec<NodeId> {
	let mut out = Vec::new();

	for (step, inverse) in commit.steps.iter().zip(&commit.inverses) {
		match (step, inverse) {
			(Step::RemoveNodes { .. }, Step::InsertNodes { nodes }) => {
				out.extend(nodes.iter().map(|record| record.parent));
			}
			(Step::Move { node, .. }, Step::Move { parent, .. }) => out.extend([*node, *parent]),
			_ => out.extend(step.nodes()),
		}
	}

	out.retain(|&id| {
		scene.contains(id) && (!is_parent_only(commit, id) || depends_on_children(scene, id))
	});
	out
}

/// Whether `id` appears in the commit only as a parent whose child list
/// changed.
fn is_parent_only(commit: &SceneCommit, id: NodeId) -> bool {
	!commit.steps.iter().any(|step| step.nodes().contains(&id))
}

/// The subtrees to recompute for changes to `ids`: each id climbs to the
/// highest ancestor whose geometry depends on it, and nested roots merge.
fn roots(scene: &Scene, ids: &[NodeId]) -> Vec<NodeId> {
	let climbed: FxHashSet<NodeId> = ids
		.iter()
		.filter(|id| scene.contains(**id))
		.map(|&id| climb(scene, id))
		.collect();

	climbed
		.iter()
		.copied()
		.filter(|&id| {
			!scene
				.ancestors(id)
				.any(|ancestor| climbed.contains(&ancestor))
		})
		.collect()
}

fn climb(scene: &Scene, mut id: NodeId) -> NodeId {
	while let Some(parent) = scene.node(id).map(canvist_scene::Node::parent) {
		if parent.is_root() || !depends_on_children(scene, parent) {
			break;
		}

		id = parent;
	}

	id
}

/// Whether a node's geometry follows its children: groups and booleans take
/// their extent from them, and auto-layout frames place them.
fn depends_on_children(scene: &Scene, id: NodeId) -> bool {
	let Some(node) = scene.node(id) else {
		return false;
	};

	match scene.kind(id) {
		Some(NodeKind::Group | NodeKind::Boolean) => true,
		Some(NodeKind::Frame | NodeKind::Component) => node.props().layout.is_some(),
		_ => false,
	}
}

fn entry(bounds: Rect, id: NodeId) -> TreeEntry {
	GeomWithData::new(
		Rectangle::from_corners([bounds.x0, bounds.y0], [bounds.x1, bounds.y1]),
		id,
	)
}

/// Computes geometry for subtrees.
struct Builder<'a> {
	scene: &'a Scene,
	texts: &'a mut TextLayouts,
	nodes: &'a mut FxHashMap<NodeId, NodeGeometry>,
	entries: &'a mut Vec<TreeEntry>,
}

impl Builder<'_> {
	/// Place node `id` under a parent at `parent_world`. `slot` is the
	/// local transform and size an auto-layout parent assigned.
	fn place(
		&mut self,
		id: NodeId,
		parent_world: Affine,
		slot: Option<(Affine, Vec2)>,
	) -> Result<(), LayoutError> {
		let Some(node) = self.scene.node(id) else {
			return Ok(());
		};

		let (local, size) = match slot {
			Some(slot) => slot,
			None => (node.props().transform, self.intrinsic_size(id)?),
		};

		let world = parent_world * local;
		self.place_children(id, world, size)?;

		let spec = self.scene.schema().spec(node.kind());

		let outline = match spec.kind() {
			NodeKind::Boolean => Some(self.boolean_outline(id, world)),
			_ => geometry::outline(node, spec, size),
		};

		let own = outline
			.as_ref()
			.map(|path| world_bounds(world, path, stroke_reach(node)));
		let (bounds, extent) = self.bounds_and_extent(id, world, own);
		let hittable = own.is_some() && !self.inside_boolean(id);

		if hittable {
			self.entries.push(entry(bounds, id));
		}

		self.nodes.insert(
			id,
			NodeGeometry {
				world,
				size,
				outline: outline.map(Arc::new),
				bounds,
				extent,
				hittable,
			},
		);
		Ok(())
	}

	/// A placed node's bounds (its own, else its children's, else an empty
	/// rectangle at its origin) and its extent (the bounds plus its
	/// children's extents).
	fn bounds_and_extent(&self, id: NodeId, world: Affine, own: Option<Rect>) -> (Rect, Rect) {
		let empty = || Rect::from_origin_size(world * Point::ORIGIN, (0.0, 0.0));
		let bounds = own
			.or_else(|| self.children_union(id, |child| child.bounds))
			.unwrap_or_else(empty);
		let extent = self
			.children_union(id, |child| child.extent)
			.map_or(bounds, |children| bounds.union(children));

		(bounds, extent)
	}

	fn place_children(&mut self, id: NodeId, world: Affine, size: Vec2) -> Result<(), LayoutError> {
		let children = self.scene.children(id).to_vec();
		let slots = self.layout_slots(id, size, &children)?;

		for (child, slot) in children.into_iter().zip(slots) {
			self.place(child, world, slot)?;
		}

		Ok(())
	}

	/// Auto-layout slots for `children`, or `None` for each when the node
	/// does not lay them out.
	fn layout_slots(
		&mut self,
		id: NodeId,
		size: Vec2,
		children: &[NodeId],
	) -> Result<Vec<Option<(Affine, Vec2)>>, LayoutError> {
		let Some(layout) = self.layout_of(id) else {
			return Ok(vec![None; children.len()]);
		};
		let items = self.flow_items(children)?;
		let result = flow(&layout, size, (false, false), &items);

		Ok(result
			.placed
			.into_iter()
			.zip(children)
			.map(|(placed, &child)| {
				let placed = placed?;
				let rotation = self
					.scene
					.node(child)?
					.props()
					.transform
					.with_translation(Vec2::ZERO);
				Some((
					Affine::translate(placed.origin.to_vec2()) * rotation,
					placed.size,
				))
			})
			.collect())
	}

	fn flow_items(&mut self, children: &[NodeId]) -> Result<Vec<FlowItem>, LayoutError> {
		children
			.iter()
			.map(|&child| {
				let props = self.scene.node(child).map(canvist_scene::Node::props);
				Ok(FlowItem {
					size: self.intrinsic_size(child)?,
					child: props.map_or_else(Default::default, |props| {
						let mut layout_child = props.layout_child;
						layout_child.absolute |= !props.visible;
						layout_child
					}),
				})
			})
			.collect()
	}

	fn layout_of(&self, id: NodeId) -> Option<canvist_scene::AutoLayout> {
		match self.scene.kind(id)? {
			NodeKind::Frame | NodeKind::Component => self.scene.node(id)?.props().layout,
			_ => None,
		}
	}

	/// The size a node takes before any parent stretches it: text frames
	/// fit their text, and hugging auto-layout frames fit their children.
	fn intrinsic_size(&mut self, id: NodeId) -> Result<Vec2, LayoutError> {
		let Some(node) = self.scene.node(id) else {
			return Ok(Vec2::ZERO);
		};
		let size = node.props().size;
		let hug = (
			node.props().layout_child.width == Sizing::Hug,
			node.props().layout_child.height == Sizing::Hug,
		);

		if let Some(text) = self.texts.ensure(self.scene, id)? {
			return Ok(text);
		}

		match self.layout_of(id) {
			Some(layout) if hug.0 || hug.1 => {
				let children = self.scene.children(id).to_vec();
				let items = self.flow_items(&children)?;
				Ok(flow(&layout, size, hug, &items).frame)
			}

			_ => Ok(size),
		}
	}

	/// The union of one rectangle of each child.
	fn children_union(&self, id: NodeId, rect: impl Fn(&NodeGeometry) -> Rect) -> Option<Rect> {
		self.scene
			.children(id)
			.iter()
			.filter_map(|child| self.nodes.get(child))
			.map(rect)
			.reduce(|a, b| a.union(b))
	}

	/// A boolean's outline from its children's, in the boolean's coordinates.
	fn boolean_outline(&self, id: NodeId, world: Affine) -> BezPath {
		let Some(node) = self.scene.node(id) else {
			return BezPath::new();
		};
		let canvist_scene::Shape::Boolean(op) = node.props().shape else {
			return BezPath::new();
		};
		let to_local = world.inverse();
		let children: Vec<_> = self
			.scene
			.children(id)
			.iter()
			.filter_map(|&child| {
				let geometry = self.nodes.get(&child)?;
				let outline = geometry.outline.as_ref()?;
				let path = (to_local * geometry.world) * (**outline).clone();
				Some((path, geometry::fill_rule(self.scene.node(child)?)))
			})
			.collect();

		geometry::boolean(op, &children)
	}

	/// Children of a boolean paint and hit as part of it.
	fn inside_boolean(&self, id: NodeId) -> bool {
		self.scene
			.ancestors(id)
			.any(|ancestor| self.scene.kind(ancestor) == Some(NodeKind::Boolean))
	}
}

/// How far a node's stroke reaches outside its outline.
fn stroke_reach(node: &canvist_scene::Node) -> f64 {
	let props = node.props();

	if !props.strokes.iter().any(|paint| paint.visible) {
		return 0.0;
	}

	match props.stroke.align {
		StrokeAlign::Inside => 0.0,
		StrokeAlign::Center => props.stroke.width / 2.0,
		StrokeAlign::Outside => props.stroke.width,
	}
}

/// The world bounds of `path` under `world`, widened by `reach`.
fn world_bounds(world: Affine, path: &BezPath, reach: f64) -> Rect {
	let local = path.bounding_box();
	let corners = [
		Point::new(local.x0, local.y0),
		Point::new(local.x1, local.y0),
		Point::new(local.x1, local.y1),
		Point::new(local.x0, local.y1),
	]
	.map(|corner| world * corner);
	let rect = corners.iter().skip(1).fold(
		Rect::from_points(corners[0], corners[0]),
		|rect, &corner| rect.union_pt(corner),
	);

	rect.inflate(reach, reach)
}
