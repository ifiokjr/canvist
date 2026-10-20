//! Fixtures for the paint tests and benchmarks.

#![allow(dead_code)]

use std::sync::Arc;

use canvist_canvas::Canvas;
use canvist_canvas::DesignKit;
use canvist_canvas::TextLayouts;
use canvist_layout::ParleyMeasurer;
use canvist_layout::Theme;
use canvist_model::BlockSpec;
use canvist_model::MarkSet;
use canvist_model::Point as TextPoint;
use canvist_model::Schema;
use canvist_paint::Assets;
use canvist_paint::Quality;
use canvist_paint::Sources;
use canvist_paint::View;
use canvist_scene::Color;
use canvist_scene::FracKey;
use canvist_scene::NodeId;
use canvist_scene::NodeKind;
use canvist_scene::NodeRecord;
use canvist_scene::Paint;
use canvist_scene::Props;
use canvist_scene::Scene;
use canvist_scene::Shape;
use canvist_scene::Step;
use canvist_scene::TextSizing;
use kurbo::Affine;
use kurbo::Size;
use kurbo::Vec2;

/// canvist's paint blue.
pub const BLUE: Color = Color::rgb(59, 44, 245);
/// A warm red for contrast.
pub const RED: Color = Color::rgb(240, 82, 82);

pub fn text_schema() -> Arc<Schema> {
	Arc::new(
		Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.build()
			.expect("text schema"),
	)
}

/// Text layouts measured with parley and the bundled font, as the canvas
/// does on every platform.
pub fn texts() -> TextLayouts {
	let mut measurer = ParleyMeasurer::new();
	measurer.register_font(canvist_paint::BUNDLED_FONT.to_vec());
	TextLayouts::new(Arc::new(Theme::starter_kit()), measurer)
}

/// A canvas over `scene` with every built-in extension.
pub fn canvas(scene: Scene) -> Canvas {
	let mut builder = Canvas::builder().text_schema(text_schema());
	DesignKit::register(&mut builder);
	builder.build(scene, texts()).expect("canvas")
}

/// An empty scene with the built-in node types.
pub fn scene() -> Scene {
	let mut builder = Canvas::builder().text_schema(text_schema());
	DesignKit::register(&mut builder);
	Scene::new(builder.schema().expect("schema"), 1).expect("scene")
}

pub fn solid(x: f64, y: f64, size: f64, color: Color) -> Props {
	Props {
		transform: Affine::translate((x, y)),
		size: Vec2::new(size, size),
		fills: Arc::new([Paint::solid(color)]),
		..Props::default()
	}
}

/// Insert a text frame holding `text` at `(x, y)`, `width` wide.
pub fn text_frame(
	scene: &mut Scene,
	parent: NodeId,
	(x, y, width): (f64, f64, f64),
	text: &str,
) -> NodeId {
	let mut transaction = scene.transaction();
	let node = transaction
		.insert(
			parent,
			None,
			"text",
			Props {
				transform: Affine::translate((x, y)),
				size: Vec2::new(width, 0.0),
				shape: Shape::Text(TextSizing::AutoHeight),
				..Props::default()
			},
		)
		.expect("text");
	transaction
		.text(node, |inner| {
			let block = inner.doc().first_text_block().expect("paragraph");
			inner.insert_text(TextPoint::new(block, 0), text, MarkSet::empty())
		})
		.expect("type");
	let _ = transaction.commit();
	node
}

pub fn view(canvas: &Canvas, width: f64, height: f64) -> View {
	View {
		camera: *canvas.state().camera(),
		viewport: Size::new(width, height),
		quality: Quality::Full,
	}
}

pub fn sources<'a>(canvas: &'a Canvas, assets: &'a Assets) -> Sources<'a> {
	let state = canvas.state();
	Sources {
		scene: state.scene(),
		index: state.index(),
		texts: state.texts(),
		assets,
	}
}

/// The grid pitch as a multiple of a square's side.
const PITCH: f64 = 1.25;

/// `count` coloured squares on a grid in one page, built in one step.
pub fn grid(count: usize, side: f64) -> Scene {
	let mut scene = scene();
	let page = scene.pages()[0];
	let rect = scene.schema().node_type("rectangle").expect("rectangle");
	let keys = FracKey::n_between(None, None, count).expect("keys");
	let columns = (count as f64).sqrt().ceil() as usize;
	let records = keys
		.into_iter()
		.map(|key| {
			let id = scene.allocate_id().expect("id");
			let index = id.seq() as usize;
			let props = Props {
				shape: Shape::Rect,
				..solid(
					(index % columns) as f64 * side * PITCH,
					(index / columns) as f64 * side * PITCH,
					side,
					BLUE,
				)
			};
			NodeRecord::new(id, rect, page, key).with_props(props)
		})
		.collect();
	scene
		.apply_steps([Step::InsertNodes { nodes: records }])
		.expect("grid");
	scene
}

/// Whether a kind paints its own body.
pub fn paints_body(kind: NodeKind) -> bool {
	!matches!(kind, NodeKind::Group | NodeKind::Page)
}
