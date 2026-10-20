//! Shared fixtures for the integration tests and benchmarks.

#![allow(dead_code)]

use std::sync::Arc;

use canvist_model::BlockSpec;
use canvist_model::MarkSpec;
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
use kurbo::Vec2;

/// A text schema with paragraphs and bold.
pub fn text_schema() -> Arc<Schema> {
	Arc::new(
		Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.mark(MarkSpec::new("bold"))
			.build()
			.expect("valid text schema"),
	)
}

/// The standard scene schema.
pub fn schema() -> Arc<SceneSchema> {
	Arc::new(SceneSchema::standard(text_schema()).expect("valid scene schema"))
}

/// A scene with one page holding `count` rectangles in a grid, returning
/// the page.
pub fn rectangles(count: usize) -> (Scene, NodeId) {
	let mut scene = Scene::new(schema(), 1).expect("new scene");
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();

	for index in 0..count {
		transaction
			.insert(page, None, "rectangle", rectangle_props(index))
			.expect("insert rectangle");
	}

	let _ = transaction.commit();
	(scene, page)
}

/// Props for the `index`th rectangle of a 100-wide grid.
pub fn rectangle_props(index: usize) -> Props {
	let column = (index % 100) as f64;
	let row = (index / 100) as f64;

	Props {
		transform: Affine::translate((column * 24.0, row * 24.0)),
		size: Vec2::new(20.0, 20.0),
		fills: Arc::new([Paint::solid(Color::rgb(59, 44, 245))]),
		..Props::default()
	}
}

/// A scene of `frames` frames on one page, each holding `per_frame`
/// rectangles: the shape of a real design file. Built with one
/// `InsertNodes` step. Returns the scene and the frames.
pub fn framed(frames: usize, per_frame: usize) -> (Scene, Vec<NodeId>) {
	let mut scene = Scene::new(schema(), 1).expect("new scene");
	let page = scene.pages()[0];
	let schema = scene.schema().clone();
	let frame_type = schema.node_type("frame").expect("standard type");
	let rect_type = schema.node_type("rectangle").expect("standard type");
	let frame_keys = FracKey::n_between(None, None, frames).expect("keys");
	let child_keys = FracKey::n_between(None, None, per_frame).expect("keys");
	let mut records = Vec::with_capacity(frames * (per_frame + 1));
	let mut frame_ids = Vec::with_capacity(frames);

	for (column, frame_key) in frame_keys.into_iter().enumerate() {
		let frame = scene.allocate_id().expect("id");
		frame_ids.push(frame);
		let props = Props {
			transform: Affine::translate((column as f64 * 1_000.0, 0.0)),
			size: Vec2::new(960.0, 960.0),
			..Props::for_kind(NodeKind::Frame)
		};
		records.push(NodeRecord::new(frame, frame_type, page, frame_key).with_props(props));

		for (index, key) in child_keys.iter().enumerate() {
			let id = scene.allocate_id().expect("id");
			let props = Props {
				shape: Shape::Rect,
				..rectangle_props(index)
			};
			records.push(NodeRecord::new(id, rect_type, frame, key.clone()).with_props(props));
		}
	}

	scene
		.apply_steps([Step::InsertNodes { nodes: records }])
		.expect("insert the fixture");
	(scene, frame_ids)
}
