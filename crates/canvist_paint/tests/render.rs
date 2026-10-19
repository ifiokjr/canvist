//! Painting scenes: what the walker draws, culling and level of detail, and
//! real pixels from the CPU backend and markup from the SVG backend.

mod support;

use std::sync::Arc;

use canvist_paint::Assets;
use canvist_paint::PaintOp;
use canvist_paint::Recorder;
use canvist_paint::SvgPainter;
use canvist_scene::BlendMode;
use canvist_scene::Color;
use canvist_scene::Effect;
use canvist_scene::EffectKind;
use canvist_scene::GradientStop;
use canvist_scene::ImageFit;
use canvist_scene::Paint;
use canvist_scene::PaintKind;
use canvist_scene::Prop;
use canvist_scene::PropKey;
use canvist_scene::Props;
use canvist_scene::Shadow;
use canvist_scene::Shape;
use canvist_scene::StrokeAlign;
use canvist_scene::StrokeStyle;
use kurbo::Point;
use kurbo::Size;
use kurbo::Vec2;
use support::BLUE;
use support::RED;

const VIEW: f64 = 400.0;

fn record(canvas: &canvist_canvas::Canvas, assets: &Assets) -> Vec<PaintOp> {
	let mut recorder = Recorder::new();
	let page = canvas.state().scene().pages()[0];
	canvist_paint::paint_page(
		&mut recorder,
		&support::sources(canvas, assets),
		page,
		&support::view(canvas, VIEW, VIEW),
	);
	recorder.ops
}

fn fills(ops: &[PaintOp]) -> usize {
	ops.iter()
		.filter(|op| matches!(op, PaintOp::Fill { .. }))
		.count()
}

#[test]
fn culls_what_is_outside_the_viewport() {
	let mut scene = support::scene();
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	transaction
		.insert(
			page,
			None,
			"rectangle",
			support::solid(10.0, 10.0, 50.0, BLUE),
		)
		.expect("inside");
	transaction
		.insert(
			page,
			None,
			"rectangle",
			support::solid(5_000.0, 10.0, 50.0, RED),
		)
		.expect("outside");
	let _ = transaction.commit();
	let canvas = support::canvas(scene);

	assert_eq!(fills(&record(&canvas, &Assets::new())), 1);
}

#[test]
fn tiny_nodes_draw_as_boxes_and_tiny_text_as_bars() {
	let mut scene = support::scene();
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	transaction
		.insert(
			page,
			None,
			"ellipse",
			Props {
				strokes: Arc::new([Paint::solid(RED)]),
				..support::solid(0.0, 0.0, 40.0, BLUE)
			},
		)
		.expect("ellipse");
	let _ = transaction.commit();
	support::text_frame(&mut scene, page, (0.0, 100.0, 400.0), "Small print");
	let mut canvas = support::canvas(scene);
	canvas.state_mut().camera_mut().zoom = 0.05;

	let ops = record(&canvas, &Assets::new());

	// The ellipse is two pixels across: one box fill, no stroke.
	assert!(ops.iter().all(|op| !matches!(op, PaintOp::Stroke { .. })));
	// The text is under a pixel tall: bars, not glyph runs.
	assert!(ops.iter().all(|op| !matches!(op, PaintOp::Text { .. })));
	assert!(fills(&ops) >= 2);
}

#[test]
fn layers_wrap_faded_blended_and_clipping_nodes() {
	let mut scene = support::scene();
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	let frame = transaction
		.insert(
			page,
			None,
			"frame",
			Props {
				opacity: 0.5,
				clip: true,
				..support::solid(0.0, 0.0, 200.0, Color::WHITE)
			},
		)
		.expect("frame");
	transaction
		.insert(
			frame,
			None,
			"rectangle",
			Props {
				blend: BlendMode::Multiply,
				..support::solid(150.0, 150.0, 100.0, RED)
			},
		)
		.expect("rect");
	let _ = transaction.commit();
	let canvas = support::canvas(scene);
	let ops = record(&canvas, &Assets::new());

	let pushes: Vec<(bool, f32)> = ops
		.iter()
		.filter_map(|op| {
			match op {
				PaintOp::PushLayer { clips, alpha } => Some((*clips, *alpha)),
				_ => None,
			}
		})
		.collect();

	assert_eq!(pushes, [(true, 0.5), (false, 1.0)]);
	assert_eq!(
		ops.iter()
			.filter(|op| matches!(op, PaintOp::PopLayer))
			.count(),
		2
	);
}

#[test]
fn strokes_shadows_gradients_and_images() {
	let mut scene = support::scene();
	let page = scene.pages()[0];
	let stops: Arc<[GradientStop]> = Arc::new([
		GradientStop {
			offset: 0.0,
			color: BLUE,
		},
		GradientStop {
			offset: 1.0,
			color: RED,
		},
	]);
	let mut transaction = scene.transaction();
	transaction
		.insert(
			page,
			None,
			"rectangle",
			Props {
				strokes: Arc::new([Paint::solid(Color::BLACK)]),
				stroke: StrokeStyle {
					width: 4.0,
					align: StrokeAlign::Inside,
					..StrokeStyle::default()
				},
				effects: Arc::new([Effect {
					kind: EffectKind::DropShadow(Shadow {
						color: Color::rgba(0, 0, 0, 80),
						offset: Vec2::new(0.0, 4.0),
						blur: 8.0,
						spread: 0.0,
					}),
					visible: true,
				}]),
				fills: Arc::new([Paint::new(PaintKind::Linear {
					from: Point::new(0.0, 0.0),
					to: Point::new(1.0, 0.0),
					stops,
				})]),
				..support::solid(20.0, 20.0, 100.0, BLUE)
			},
		)
		.expect("rect");
	transaction
		.insert(
			page,
			None,
			"image",
			Props {
				fills: Arc::new([Paint::new(PaintKind::Image {
					asset: "dot".into(),
					fit: ImageFit::Fill,
				})]),
				..support::solid(200.0, 20.0, 100.0, BLUE)
			},
		)
		.expect("image");
	let _ = transaction.commit();
	let canvas = support::canvas(scene);

	let mut assets = Assets::new();
	assets.insert_rgba8("dot", 2, 2, vec![255; 16]);
	let ops = record(&canvas, &assets);

	assert!(ops.iter().any(|op| matches!(op, PaintOp::Shadow(_))));
	assert!(
		ops.iter()
			.any(|op| matches!(op, PaintOp::Stroke { width, .. } if (*width - 8.0).abs() < 1e-9))
	);
	assert!(ops.iter().any(|op| {
		matches!(
			op,
			PaintOp::Fill {
				brush: peniko::Brush::Gradient(_),
				..
			}
		)
	}));
	assert!(
		ops.iter()
			.any(|op| matches!(op, PaintOp::Image { asset, .. } if asset == "dot"))
	);
}

#[test]
fn instances_draw_their_component_with_overrides() {
	let mut scene = support::scene();
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	let component = transaction
		.insert(
			page,
			None,
			"component",
			support::solid(0.0, 0.0, 100.0, Color::WHITE),
		)
		.expect("component");
	let dot = transaction
		.insert(
			component,
			None,
			"ellipse",
			support::solid(25.0, 25.0, 50.0, BLUE),
		)
		.expect("dot");
	let instance = transaction
		.insert(
			page,
			None,
			"instance",
			Props {
				shape: Shape::Instance { component },
				..support::solid(200.0, 0.0, 100.0, Color::WHITE)
			},
		)
		.expect("instance");
	transaction
		.set(
			instance,
			Prop::Override(dot, Box::new(Prop::Fills(Arc::new([Paint::solid(RED)])))),
		)
		.expect("override");
	let _ = transaction.commit();

	let canvas = support::canvas(scene);
	let ops = record(&canvas, &Assets::new());
	let red_dot = ops.iter().find_map(|op| {
		match op {
			PaintOp::Fill {
				bounds,
				brush: peniko::Brush::Solid(color),
				..
			} if *color == canvist_paint::convert::color(RED) => Some(*bounds),
			_ => None,
		}
	});

	let bounds = red_dot.expect("the instance's dot is red");
	assert!(
		(bounds.x0 - 225.0).abs() < 1e-6 && (bounds.y0 - 25.0).abs() < 1e-6,
		"{bounds:?}"
	);
	assert!(canvas.state().scene().node(instance).is_some_and(|node| {
		node.props()
			.get(&PropKey::Override(dot, Box::new(PropKey::Fills)))
			!= Prop::ClearOverride(dot, Box::new(PropKey::Fills))
	}));
}

#[test]
fn text_frames_draw_their_runs() {
	let mut scene = support::scene();
	let page = scene.pages()[0];
	support::text_frame(&mut scene, page, (10.0, 10.0, 300.0), "Hello, canvas");
	let canvas = support::canvas(scene);
	let ops = record(&canvas, &Assets::new());
	let text: String = ops
		.iter()
		.filter_map(|op| {
			match op {
				PaintOp::Text { text, .. } => Some(text.as_str()),
				_ => None,
			}
		})
		.collect();

	assert_eq!(text.trim(), "Hello, canvas");
}

#[test]
fn svg_keeps_shapes_gradients_and_text() {
	let mut scene = support::scene();
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	transaction
		.insert(
			page,
			None,
			"rectangle",
			support::solid(10.0, 10.0, 50.0, BLUE),
		)
		.expect("rect");
	let _ = transaction.commit();
	support::text_frame(&mut scene, page, (10.0, 100.0, 300.0), "Tom & Jerry <3");
	let canvas = support::canvas(scene);
	let assets = Assets::new();

	let page = canvas.state().scene().pages()[0];
	let mut svg = SvgPainter::new(Size::new(VIEW, VIEW));
	canvist_paint::paint_page(
		&mut svg,
		&support::sources(&canvas, &assets),
		page,
		&support::view(&canvas, VIEW, VIEW),
	);
	let markup = svg.finish();

	assert!(markup.starts_with("<svg"));
	assert!(markup.contains("fill=\"#3b2cf5ff\""));
	// Runs split at words; each piece is escaped.
	assert!(markup.contains("&amp;") && markup.contains("&lt;3"));
	assert!(!markup.contains(">&<") && !markup.contains(" <3"));
}

#[cfg(feature = "cpu")]
#[test]
fn the_cpu_backend_paints_real_pixels() {
	let mut scene = support::scene();
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	transaction
		.insert(
			page,
			None,
			"rectangle",
			support::solid(10.0, 10.0, 50.0, BLUE),
		)
		.expect("rect");
	let _ = transaction.commit();
	support::text_frame(&mut scene, page, (10.0, 100.0, 300.0), "Wet paint");
	let canvas = support::canvas(scene);
	let assets = Assets::new();

	let page = canvas.state().scene().pages()[0];
	let mut painter = canvist_paint::CpuPainter::new(200, 160).expect("painter");
	canvist_paint::paint_page(
		&mut painter,
		&support::sources(&canvas, &assets),
		page,
		&support::view(&canvas, 200.0, 160.0),
	);
	let pixels = painter.render_rgba8();
	let pixel = |x: usize, y: usize| {
		let at = (y * 200 + x) * 4;
		[pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
	};

	assert_eq!(pixel(35, 35), [59, 44, 245, 255]);
	assert_eq!(pixel(150, 40)[3], 0, "empty canvas is transparent");
	let inked = (100..130)
		.flat_map(|y| (10..150).map(move |x| (x, y)))
		.filter(|&(x, y)| pixel(x, y)[3] > 128)
		.count();
	assert!(inked > 50, "the text drew glyphs ({inked} inked pixels)");

	let png = painter.render_png().expect("png");
	assert_eq!(&png[1..4], b"PNG");
}

#[test]
fn drop_shadow_is_skipped_while_interacting() {
	let mut scene = support::scene();
	let page = scene.pages()[0];
	let mut transaction = scene.transaction();
	transaction
		.insert(
			page,
			None,
			"rectangle",
			Props {
				effects: Arc::new([
					Effect {
						kind: EffectKind::LayerBlur { radius: 4.0 },
						visible: true,
					},
					Effect {
						kind: EffectKind::DropShadow(Shadow {
							color: Color::BLACK,
							offset: Vec2::ZERO,
							blur: 4.0,
							spread: 0.0,
						}),
						visible: true,
					},
				]),
				..support::solid(10.0, 10.0, 50.0, BLUE)
			},
		)
		.expect("rect");
	let _ = transaction.commit();
	let canvas = support::canvas(scene);
	let assets = Assets::new();
	let mut view = support::view(&canvas, VIEW, VIEW);

	view.quality = canvist_paint::Quality::Interactive;
	let mut recorder = Recorder::new();
	canvist_paint::paint_page(
		&mut recorder,
		&support::sources(&canvas, &assets),
		canvas.state().scene().pages()[0],
		&view,
	);

	assert!(
		recorder
			.ops
			.iter()
			.all(|op| !matches!(op, PaintOp::Shadow(_)))
	);
}
