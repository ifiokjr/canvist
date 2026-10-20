//! Shared fixtures for the canvas tests and benchmarks.

#![allow(dead_code)]

use std::sync::Arc;

use canvist_canvas::Canvas;
use canvist_canvas::DesignKit;
use canvist_canvas::DesignKitHandle;
use canvist_canvas::Input;
use canvist_canvas::Modifiers;
use canvist_canvas::Phase;
use canvist_canvas::PointerEvent;
use canvist_canvas::TextLayouts;
use canvist_layout::FixedMeasurer;
use canvist_layout::Theme;
use canvist_model::BlockSpec;
use canvist_model::Schema;
use canvist_scene::Color;
use canvist_scene::FracKey;
use canvist_scene::NodeId;
use canvist_scene::NodeKind;
use canvist_scene::NodeRecord;
use canvist_scene::Paint;
use canvist_scene::Props;
use canvist_scene::Scene;
use canvist_scene::SceneSchema;
use canvist_scene::Shape;
use canvist_scene::Step;
use kurbo::Affine;
use kurbo::Point;
use kurbo::Vec2;

/// The fill test rectangles get.
const RECT_FILL: Color = Color::rgb(59, 44, 245);
/// Seconds between the samples of a drag: one frame at 60 Hz.
const SAMPLE_INTERVAL: f64 = 0.016;
/// How many frames [`framed`] puts in each row.
const FRAMES_PER_ROW: usize = 100;
/// The side of each square frame [`framed`] builds.
const FRAME_SIDE: f64 = 1_000.0;
/// The distance between neighbouring frames' origins: a side plus a gap.
const FRAME_PITCH: f64 = 1_100.0;
/// The share of its grid cell each rectangle in a frame covers.
const CELL_COVERAGE: f64 = 0.8;

/// Rectangles per frame in the performance scenes: with the frame itself,
/// 100 nodes each.
pub const RECTS_PER_FRAME: usize = 99;
/// A screen point on a rectangle inside the first frame of a [`framed`]
/// scene, at the default camera.
pub const ON_FIRST_RECT: Point = Point::new(50.0, 50.0);
/// Where a measured drag first moves to, past the drag threshold.
pub const DRAG_START: Point = Point::new(60.0, 60.0);
/// How far a measured drag wanders from [`DRAG_START`] on each axis before
/// wrapping back.
const WANDER: Vec2 = Vec2::new(200.0, 150.0);

/// Where a measured drag is at step `step`: wandering away from
/// [`DRAG_START`] and wrapping so it stays near the first frame.
pub fn wander(step: f64) -> Point {
	Point::new(
		DRAG_START.x + step % WANDER.x,
		DRAG_START.y + step % WANDER.y,
	)
}

pub fn text_schema() -> Arc<Schema> {
	Arc::new(
		Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.build()
			.expect("text schema"),
	)
}

pub fn texts() -> TextLayouts {
	TextLayouts::new(Arc::new(Theme::starter_kit()), FixedMeasurer::new())
}

/// A canvas with every built-in extension over a scene with one page.
pub fn canvas() -> (Canvas, DesignKitHandle) {
	let mut builder = Canvas::builder().text_schema(text_schema());
	let kit = DesignKit::register(&mut builder);
	let scene = Scene::new(builder.schema().expect("schema"), 1).expect("scene");
	(builder.build(scene, texts()).expect("canvas"), kit)
}

/// The schema [`canvas`] uses.
pub fn schema() -> Arc<SceneSchema> {
	let mut builder = Canvas::builder().text_schema(text_schema());
	DesignKit::register(&mut builder);
	builder.schema().expect("schema")
}

pub fn page(canvas: &Canvas) -> NodeId {
	canvas.state().scene().pages()[0]
}

/// Props for a filled rectangle at `(x, y)` of `size`.
pub fn rect_props(x: f64, y: f64, w: f64, h: f64) -> Props {
	Props {
		transform: Affine::translate((x, y)),
		size: Vec2::new(w, h),
		fills: Arc::new([Paint::solid(RECT_FILL)]),
		shape: Shape::Rect,
		..Props::default()
	}
}

/// Insert a rectangle on the first page.
pub fn rect(canvas: &mut Canvas, x: f64, y: f64, w: f64, h: f64) -> NodeId {
	let page = page(canvas);
	let props = rect_props(x, y, w, h);
	canvas
		.state_mut()
		.edit(false, |transaction| {
			transaction.insert(page, None, "rectangle", props)
		})
		.expect("insert")
		.0
}

/// Feed a mouse gesture: down at the first point, moves through the rest,
/// up at the last.
pub fn drag(canvas: &mut Canvas, points: &[(f64, f64)], modifiers: Modifiers) {
	let mut time = 0.0;
	let last = points.len() - 1;

	for (index, &(x, y)) in points.iter().enumerate() {
		let phase = if index == 0 { Phase::Down } else { Phase::Move };
		feed(
			canvas,
			PointerEvent::mouse(phase, Point::new(x, y), time).with_modifiers(modifiers),
		);
		time += SAMPLE_INTERVAL;

		if index == last {
			feed(
				canvas,
				PointerEvent::mouse(Phase::Up, Point::new(x, y), time).with_modifiers(modifiers),
			);
		}
	}
}

/// Click at `(x, y)`.
pub fn click(canvas: &mut Canvas, x: f64, y: f64, modifiers: Modifiers, time: f64) {
	feed(
		canvas,
		PointerEvent::mouse(Phase::Down, Point::new(x, y), time).with_modifiers(modifiers),
	);
	feed(
		canvas,
		PointerEvent::mouse(Phase::Up, Point::new(x, y), time).with_modifiers(modifiers),
	);
}

pub fn feed(canvas: &mut Canvas, event: PointerEvent) {
	canvas.input(&Input::Pointer(event)).expect("input");
}

/// A scene of `frames` frames on one page, each with `per_frame`
/// rectangles, built in one step.
pub fn framed(frames: usize, per_frame: usize) -> Scene {
	let mut scene = Scene::new(schema(), 1).expect("scene");
	let page = scene.pages()[0];
	let schema = scene.schema().clone();
	let frame_type = schema.node_type("frame").expect("frame");
	let rect_type = schema.node_type("rectangle").expect("rectangle");
	let frame_keys = FracKey::n_between(None, None, frames).expect("keys");
	let child_keys = FracKey::n_between(None, None, per_frame).expect("keys");
	let mut records = Vec::with_capacity(frames * (per_frame + 1));

	let side = (per_frame as f64).sqrt().ceil() as usize;

	for (column, frame_key) in frame_keys.into_iter().enumerate() {
		let frame = scene.allocate_id().expect("id");
		let x = (column % FRAMES_PER_ROW) as f64 * FRAME_PITCH;
		let y = (column / FRAMES_PER_ROW) as f64 * FRAME_PITCH;
		let props = Props {
			transform: Affine::translate((x, y)),
			size: Vec2::new(FRAME_SIDE, FRAME_SIDE),
			..Props::for_kind(NodeKind::Frame)
		};
		records.push(NodeRecord::new(frame, frame_type, page, frame_key).with_props(props));

		for (index, key) in child_keys.iter().enumerate() {
			let id = scene.allocate_id().expect("id");
			let cell = FRAME_SIDE / side as f64;
			let props = rect_props(
				(index % side) as f64 * cell,
				(index / side) as f64 * cell,
				cell * CELL_COVERAGE,
				cell * CELL_COVERAGE,
			);
			records.push(NodeRecord::new(id, rect_type, frame, key.clone()).with_props(props));
		}
	}

	scene
		.apply_steps([Step::InsertNodes { nodes: records }])
		.expect("fixture");
	scene
}
