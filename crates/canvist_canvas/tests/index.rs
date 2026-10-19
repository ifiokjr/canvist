//! The scene index: derived geometry is right, and updating it from commits
//! always matches rebuilding it from scratch.

mod support;

use std::sync::Arc;

use canvist_canvas::SceneIndex;
use canvist_model::MarkSet;
use canvist_model::Point as TextPoint;
use canvist_scene::Align;
use canvist_scene::AutoLayout;
use canvist_scene::BoolOp;
use canvist_scene::Direction;
use canvist_scene::LayoutChild;
use canvist_scene::NodeId;
use canvist_scene::Padding;
use canvist_scene::Prop;
use canvist_scene::Props;
use canvist_scene::Scene;
use canvist_scene::Shape;
use canvist_scene::Sizing;
use canvist_scene::TextSizing;
use canvist_scene::Transaction;
use kurbo::Affine;
use kurbo::Point;
use kurbo::Rect;
use kurbo::Vec2;
use proptest::prelude::*;

/// How close two computed coordinates must be to count as equal.
const EPSILON: f64 = 1e-6;

fn scene() -> Scene {
	Scene::new(support::schema(), 1).expect("scene")
}

fn build(scene: &Scene) -> SceneIndex {
	SceneIndex::build(scene, &mut support::texts()).expect("index")
}

/// Insert a node of type `kind` under `parent`, panicking if that fails.
fn insert(transaction: &mut Transaction<'_>, parent: NodeId, kind: &str, props: Props) -> NodeId {
	transaction.insert(parent, None, kind, props).expect(kind)
}

/// Props for a filled rectangle covering `rect`.
fn props_at(rect: Rect) -> Props {
	support::rect_props(rect.x0, rect.y0, rect.width(), rect.height())
}

fn near(a: Rect, b: Rect) -> bool {
	(a.x0 - b.x0).abs() < EPSILON
		&& (a.y0 - b.y0).abs() < EPSILON
		&& (a.x1 - b.x1).abs() < EPSILON
		&& (a.y1 - b.y1).abs() < EPSILON
}

#[test]
fn nested_transforms_compose() {
	/// Where the frame sits on the page.
	const OFFSET: Vec2 = Vec2::new(100.0, 50.0);
	/// The frame's size.
	const FRAME_SIZE: Vec2 = Vec2::new(400.0, 300.0);
	/// The rectangle, in the frame's coordinates.
	const CHILD: Rect = Rect::new(10.0, 20.0, 40.0, 60.0);
	/// A page area above and left of the frame.
	const OUTSIDE: Rect = Rect::new(0.0, 0.0, 50.0, 50.0);

	let mut scene = scene();
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();

	let frame_props = Props {
		transform: Affine::translate(OFFSET),
		size: FRAME_SIZE,
		..Props::default()
	};
	let frame = insert(&mut transaction, page, "frame", frame_props);
	let rect = insert(&mut transaction, frame, "rectangle", props_at(CHILD));
	let _ = transaction.commit();
	let index = build(&scene);

	let world = CHILD + OFFSET;
	let probe = Rect::from_origin_size(world.center(), (1.0, 1.0));
	assert!(near(index.get(rect).expect("rect").bounds, world));
	assert!(index.query(probe).any(|id| id == rect));
	assert!(index.query(OUTSIDE).next().is_none());
}

#[test]
fn auto_layout_places_and_hugs_children() {
	/// Padding on every side of the row.
	const PADDING: f64 = 5.0;
	/// The first, short child's size.
	const SHORT: Vec2 = Vec2::new(20.0, 10.0);
	/// The second, tall child's size.
	const TALL: Vec2 = Vec2::new(30.0, 40.0);
	/// Where the first child claims to be; auto-layout ignores it.
	const IGNORED: f64 = 500.0;
	/// The hugged row: padding, both children, and the gap, by padding and
	/// the tall child.
	const HUGGED: Vec2 = Vec2::new(70.0, 50.0);
	/// The short child after the padding, centred on the row's cross axis.
	const SHORT_PLACED: Rect = Rect::new(5.0, 20.0, 25.0, 30.0);
	/// The tall child after the short one and the gap.
	const TALL_PLACED: Rect = Rect::new(35.0, 5.0, 65.0, 45.0);

	let mut scene = scene();
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();

	let row_props = Props {
		layout: Some(AutoLayout {
			direction: Direction::Row,
			gap: 10.0,
			padding: Padding::all(PADDING),
			cross_align: Align::Center,
			..AutoLayout::default()
		}),
		layout_child: LayoutChild {
			width: Sizing::Hug,
			height: Sizing::Hug,
			absolute: false,
		},
		..Props::default()
	};
	let row = insert(&mut transaction, page, "frame", row_props);
	let short = support::rect_props(IGNORED, IGNORED, SHORT.x, SHORT.y);
	let short = insert(&mut transaction, row, "rectangle", short);
	let tall = insert(
		&mut transaction,
		row,
		"rectangle",
		support::rect_props(0.0, 0.0, TALL.x, TALL.y),
	);
	let _ = transaction.commit();
	let index = build(&scene);

	assert_eq!(index.get(row).map(|geometry| geometry.size), Some(HUGGED));
	assert!(near(index.get(short).expect("short").bounds, SHORT_PLACED));
	assert!(near(index.get(tall).expect("tall").bounds, TALL_PLACED));
}

#[test]
fn text_frames_take_their_text_height() {
	/// The text frame's fixed width.
	const WIDTH: f64 = 200.0;
	/// Enough words to wrap onto several lines.
	const WORDS: usize = 40;

	let mut scene = scene();
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();

	let text_props = Props {
		size: Vec2::new(WIDTH, 0.0),
		shape: Shape::Text(TextSizing::AutoHeight),
		..Props::default()
	};
	let text = insert(&mut transaction, page, "text", text_props);
	transaction
		.text(text, |inner| {
			let block = inner.doc().first_text_block().expect("paragraph");
			inner.insert_text(
				TextPoint::new(block, 0),
				&"words ".repeat(WORDS),
				MarkSet::empty(),
			)
		})
		.expect("type");
	let _ = transaction.commit();
	let index = build(&scene);
	let size = index.get(text).expect("text").size;

	assert!((size.x - WIDTH).abs() < EPSILON);
	assert!(size.y > 40.0, "several wrapped lines, got {size:?}");
}

#[test]
fn booleans_take_their_outline_from_children() {
	/// The side of both squares.
	const SIDE: f64 = 100.0;
	/// How far right the subtracted square sits.
	const SHIFT: f64 = 50.0;
	/// In the first square only, so the subtraction keeps it.
	const KEPT: Point = Point::new(25.0, 50.0);
	/// In both squares, so the subtraction cuts it away.
	const CUT: Point = Point::new(75.0, 50.0);

	let mut scene = scene();
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();

	let boolean_props = Props {
		shape: Shape::Boolean(BoolOp::Subtract),
		fills: support::rect_props(0.0, 0.0, 0.0, 0.0).fills,
		..Props::default()
	};
	let boolean = insert(&mut transaction, page, "boolean", boolean_props);
	let first = insert(
		&mut transaction,
		boolean,
		"rectangle",
		support::rect_props(0.0, 0.0, SIDE, SIDE),
	);
	insert(
		&mut transaction,
		boolean,
		"rectangle",
		support::rect_props(SHIFT, 0.0, SIDE, SIDE),
	);
	let _ = transaction.commit();
	let index = build(&scene);

	let geometry = index.get(boolean).expect("boolean");

	assert!(near(geometry.bounds, Rect::new(0.0, 0.0, SHIFT, SIDE)));
	assert!(geometry.hittable);
	assert!(!index.get(first).expect("child").hittable);
	assert_eq!(
		canvist_canvas::hit_test(&scene, &index, KEPT, 1.0),
		Some(boolean)
	);
	assert_eq!(canvist_canvas::hit_test(&scene, &index, CUT, 1.0), None);
}

/// Whether `outer` covers `inner`, allowing for empty rectangles.
fn covers(outer: Rect, inner: Rect) -> bool {
	inner.area() == 0.0 && inner.width() == 0.0 && inner.height() == 0.0
		|| outer.union(inner) == outer
}

/// A random edit, resolved against the current scene.
#[derive(Clone, Debug)]
enum Edit {
	Insert { parent: u8, kind: u8, x: i16 },
	Remove { node: u8 },
	Move { node: u8, parent: u8 },
	Translate { node: u8, x: i16, y: i16 },
	Resize { node: u8, w: u8 },
	Layout { node: u8, on: bool },
}

const KINDS: [&str; 7] = [
	"page",
	"frame",
	"group",
	"rectangle",
	"ellipse",
	"boolean",
	"text",
];

/// How many random edit sequences the property test runs.
const CASES: u32 = 128;
/// Where inserted nodes sit vertically.
const INSERT_Y: f64 = 7.0;
/// The size inserted nodes start with.
const INSERT_SIZE: Vec2 = Vec2::new(30.0, 20.0);
/// The height resized nodes get.
const RESIZE_HEIGHT: f64 = 12.0;

fn edit() -> impl Strategy<Value = Edit> {
	prop_oneof![
		4 => (any::<u8>(), any::<u8>(), any::<i16>()).prop_map(|(parent, kind, x)| Edit::Insert { parent, kind, x }),
		1 => any::<u8>().prop_map(|node| Edit::Remove { node }),
		2 => (any::<u8>(), any::<u8>()).prop_map(|(node, parent)| Edit::Move { node, parent }),
		3 => (any::<u8>(), any::<i16>(), any::<i16>()).prop_map(|(node, x, y)| Edit::Translate { node, x, y }),
		2 => (any::<u8>(), any::<u8>()).prop_map(|(node, w)| Edit::Resize { node, w }),
		1 => (any::<u8>(), any::<bool>()).prop_map(|(node, on)| Edit::Layout { node, on }),
	]
}

/// The root or any node, so pages can be re-created after removal.
fn pick(scene: &Scene, seed: u8) -> NodeId {
	let all: Vec<NodeId> = std::iter::once(NodeId::ROOT)
		.chain(scene.subtree(NodeId::ROOT))
		.collect();
	all[usize::from(seed) % all.len()]
}

/// Props for an inserted node of type `kind` at x position `x`.
fn insert_props(kind: &str, x: i16) -> Props {
	let props = support::rect_props(f64::from(x), INSERT_Y, INSERT_SIZE.x, INSERT_SIZE.y);

	let shape = match kind {
		"boolean" => Shape::Boolean(BoolOp::Union),
		"text" => Shape::Text(TextSizing::AutoHeight),
		_ => Shape::None,
	};

	Props { shape, ..props }
}

fn run(transaction: &mut Transaction<'_>, edit: &Edit) -> Result<(), canvist_scene::SceneError> {
	let scene = transaction.scene();

	match *edit {
		Edit::Insert { parent, kind, x } => {
			let parent = pick(scene, parent);
			let kind = KINDS[usize::from(kind) % KINDS.len()];
			transaction.insert(parent, None, kind, insert_props(kind, x))?;
		}
		Edit::Remove { node } => transaction.remove(&[pick(scene, node)])?,
		Edit::Move { node, parent } => {
			let (node, parent) = (pick(scene, node), pick(scene, parent));
			transaction.move_to(node, parent, None)?;
		}
		Edit::Translate { node, x, y } => {
			let node = pick(scene, node);
			let offset = (f64::from(x), f64::from(y));
			transaction.set(node, Prop::Transform(Affine::translate(offset)))?;
		}
		Edit::Resize { node, w } => {
			let node = pick(scene, node);
			transaction.set(node, Prop::Size(Vec2::new(f64::from(w), RESIZE_HEIGHT)))?;
		}
		Edit::Layout { node, on } => {
			let node = pick(scene, node);
			transaction.set(node, Prop::Layout(on.then(AutoLayout::default)))?;
		}
	}

	Ok(())
}

/// Check that node `id`'s incrementally updated geometry matches a fresh
/// rebuild after `edit`. Pages and extents may only have grown, since
/// updates widen them without shrinking.
fn check_node(
	scene: &Scene,
	index: &SceneIndex,
	fresh: &SceneIndex,
	id: NodeId,
	edit: &Edit,
) -> Result<(), TestCaseError> {
	let (Some(updated), Some(rebuilt)) = (index.get(id), fresh.get(id)) else {
		return Err(TestCaseError::fail(format!("{id} missing after {edit:?}")));
	};
	let is_page = scene.kind(id) == Some(canvist_scene::NodeKind::Page);
	prop_assert_eq!(updated.world, rebuilt.world, "{} after {:?}", id, edit);
	prop_assert_eq!(updated.size, rebuilt.size, "{} after {:?}", id, edit);

	if is_page {
		// Page bounds only grow between rebuilds.
		prop_assert!(
			covers(updated.bounds, rebuilt.bounds),
			"{} after {:?}",
			id,
			edit
		);
	} else {
		prop_assert_eq!(updated.bounds, rebuilt.bounds, "{} after {:?}", id, edit);
	}

	prop_assert!(
		covers(updated.extent, rebuilt.extent),
		"{} extent after {:?}",
		id,
		edit
	);
	let hits_updated: Vec<NodeId> = index
		.query(rebuilt.bounds)
		.filter(|&hit| hit == id)
		.collect();
	prop_assert_eq!(
		hits_updated.is_empty(),
		!rebuilt.hittable,
		"{} after {:?}",
		id,
		edit
	);
	Ok(())
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(CASES))]

	#[test]
	fn incremental_updates_match_a_rebuild(edits in prop::collection::vec(edit(), 1..40)) {
		let mut scene = scene();
		let mut texts = support::texts();
		let mut index = SceneIndex::build(&scene, &mut texts).expect("index");

		for edit in &edits {
			let mut transaction = scene.transaction();

			if run(&mut transaction, edit).is_err() {
				continue;
			}

			let commit = transaction.commit();
			index.apply(&scene, &commit, &mut texts).expect("apply");
			let fresh = build(&scene);
			prop_assert_eq!(index.len(), fresh.len(), "after {:?}", edit);

			for id in scene.subtree(NodeId::ROOT) {
				check_node(&scene, &index, &fresh, id, edit)?;
			}
		}
	}
}

#[test]
fn removing_a_node_removes_its_subtree_from_the_tree() {
	/// Covers the frame and everything in it.
	const AREA: Rect = Rect::new(0.0, 0.0, 200.0, 200.0);

	let mut scene = scene();
	let mut texts = support::texts();
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();

	let frame = insert(
		&mut transaction,
		page,
		"frame",
		support::rect_props(0.0, 0.0, 100.0, 100.0),
	);
	insert(
		&mut transaction,
		frame,
		"rectangle",
		support::rect_props(10.0, 10.0, 10.0, 10.0),
	);
	let _ = transaction.commit();
	let mut index = SceneIndex::build(&scene, &mut texts).expect("index");

	let commit = scene
		.apply_steps([canvist_scene::Step::RemoveNodes { ids: vec![frame] }])
		.expect("remove");
	index.apply(&scene, &commit, &mut texts).expect("apply");

	assert_eq!(index.len(), 1);
	assert!(index.query(AREA).next().is_none());
	let _ = Arc::new(());
}
