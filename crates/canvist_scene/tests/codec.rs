//! Every property's JSON form decodes back to the same value, and malformed
//! values are rejected.

use std::sync::Arc;

use canvist_scene::*;
use kurbo::Affine;
use kurbo::BezPath;
use kurbo::Point;
use kurbo::Vec2;
use serde_json::json;

const HALF_ALPHA: u8 = 128;
const QUARTER_ALPHA: u8 = 64;

fn assert_round_trips(props: &[Prop]) {
	for prop in props {
		let key = prop.key();
		let decoded = Prop::from_json(&key, &prop.to_json()).expect("decodes");

		assert_eq!(&decoded, prop, "{key}");
	}
}

fn stops() -> Arc<[GradientStop]> {
	Arc::new([
		GradientStop {
			offset: 0.0,
			color: Color::rgb(255, 0, 0),
		},
		GradientStop {
			offset: 1.0,
			color: Color::rgba(0, 0, 255, HALF_ALPHA),
		},
	])
}

#[test]
fn scalar_and_geometry_props_round_trip() {
	assert_round_trips(&[
		Prop::Name("Hero".into()),
		Prop::Visible(false),
		Prop::Locked(true),
		Prop::Clip(true),
		Prop::Transform(Affine::rotate(0.5).then_translate(Vec2::new(10.0, 20.0))),
		Prop::Size(Vec2::new(120.0, 48.5)),
		Prop::Opacity(0.25),
		Prop::Blend(BlendMode::ColorDodge),
		Prop::Radius([1.0, 2.0, 3.0, 4.0]),
	]);
}

#[test]
fn paints_round_trip() {
	let linear = Paint {
		opacity: 0.5,
		visible: false,
		blend: BlendMode::Multiply,
		..Paint::new(PaintKind::Linear {
			from: Point::new(0.0, 0.0),
			to: Point::new(1.0, 1.0),
			stops: stops(),
		})
	};
	let radial = Paint::new(PaintKind::Radial {
		center: Point::new(0.5, 0.5),
		radius: Vec2::new(0.5, 0.25),
		stops: stops(),
	});
	let sweep = Paint::new(PaintKind::Sweep {
		center: Point::new(0.5, 0.5),
		angle: 90.0,
		stops: stops(),
	});
	let image = Paint::new(PaintKind::Image {
		asset: "sha256:abc".into(),
		fit: ImageFit::Tile,
	});

	assert_round_trips(&[
		Prop::Fills(Arc::new([
			Paint::solid(Color::rgb(59, 44, 245)),
			linear,
			radial,
			sweep,
			image,
		])),
		Prop::Strokes(Arc::new([Paint::solid(Color::BLACK)])),
		Prop::Stroke(StrokeStyle {
			width: 2.0,
			align: StrokeAlign::Inside,
			cap: StrokeCap::Round,
			join: StrokeJoin::Bevel,
			miter_limit: 8.0,
			dashes: Arc::new([4.0, 2.0]),
		}),
	]);
}

#[test]
fn effects_round_trip() {
	let shadow = Shadow {
		color: Color::rgba(0, 0, 0, QUARTER_ALPHA),
		offset: Vec2::new(0.0, 4.0),
		blur: 12.0,
		spread: -2.0,
	};
	let effect = |kind, visible| Effect { kind, visible };

	assert_round_trips(&[Prop::Effects(Arc::new([
		effect(EffectKind::DropShadow(shadow), true),
		effect(EffectKind::InnerShadow(shadow), false),
		effect(EffectKind::LayerBlur { radius: 4.0 }, true),
		effect(EffectKind::BackgroundBlur { radius: 20.0 }, true),
	]))]);
}

#[test]
fn layout_props_round_trip() {
	let padding = Padding {
		top: 1.0,
		right: 2.0,
		bottom: 3.0,
		left: 4.0,
	};

	assert_round_trips(&[
		Prop::Constraints(Constraints {
			horizontal: Constraint::Both,
			vertical: Constraint::Scale,
		}),
		Prop::Layout(None),
		Prop::Layout(Some(AutoLayout {
			direction: Direction::Column,
			gap: 8.0,
			padding,
			align: Align::SpaceBetween,
			cross_align: Align::Stretch,
			wrap: true,
		})),
		Prop::LayoutChild(LayoutChild {
			width: Sizing::Fill,
			height: Sizing::Hug,
			absolute: true,
		}),
	]);
}

#[test]
fn shapes_round_trip() {
	let mut path = BezPath::new();
	path.move_to((0.0, 0.0));
	path.curve_to((10.0, 0.0), (20.0, 10.0), (20.0, 20.0));
	path.close_path();

	assert_round_trips(&[
		Prop::Shape(Shape::None),
		Prop::Shape(Shape::Rect),
		Prop::Shape(Shape::Ellipse {
			start: 10.0,
			sweep: 270.0,
			inner: 0.5,
		}),
		Prop::Shape(Shape::Polygon {
			points: 5,
			ratio: 0.4,
		}),
		Prop::Shape(Shape::Path {
			path: Arc::new(path),
			rule: FillRule::EvenOdd,
		}),
		Prop::Shape(Shape::Boolean(BoolOp::Exclude)),
		Prop::Shape(Shape::Text(TextSizing::AutoWidth)),
		Prop::Shape(Shape::Instance {
			component: NodeId::new(2, 5),
		}),
	]);
}

#[test]
fn custom_values_and_overrides_round_trip() {
	let target = NodeId::new(2, 5);

	assert_round_trips(&[
		Prop::Custom("seed".into(), Some(json!({ "value": [1, 2] }))),
		Prop::Custom("seed".into(), None),
		Prop::Override(target, Box::new(Prop::Opacity(0.5))),
		Prop::ClearOverride(target, Box::new(PropKey::Fills)),
	]);
}

#[test]
fn rejects_bad_values() {
	let cases = [
		(PropKey::Size, json!([1, "2"])),
		(PropKey::Opacity, json!("half")),
		(PropKey::Blend, json!("glow")),
		(PropKey::Fills, json!([{ "type": "plaid" }])),
		(PropKey::Shape, json!({ "type": "path", "d": "M 0 0 Q" })),
		(PropKey::Transform, json!([1, 0, 0, 1, 0])),
	];

	for (key, value) in cases {
		assert!(Prop::from_json(&key, &value).is_err(), "{key}: {value}");
	}
}
