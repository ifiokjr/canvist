//! Interaction scenarios through the real input path: tools, handles,
//! snapping, commands, the inspector, history, and the camera.

mod support;

use canvist_canvas::Handle;
use canvist_canvas::Input;
use canvist_canvas::KeyEvent;
use canvist_canvas::Modifiers;
use canvist_canvas::OverlayItem;
use canvist_canvas::Phase;
use canvist_canvas::PointerEvent;
use canvist_canvas::PointerKind;
use canvist_canvas::ToolId;
use canvist_canvas::WheelEvent;
use canvist_scene::NodeKind;
use canvist_scene::Shape;
use canvist_scene::TextSizing;
use kurbo::Point;
use kurbo::Vec2;
use serde_json::json;

const SHIFT: Modifiers = Modifiers {
	shift: true,
	alt: false,
	primary: false,
};
const NONE: Modifiers = Modifiers {
	shift: false,
	alt: false,
	primary: false,
};

/// The side of the small squares most scenarios place.
const SIDE: f64 = 50.0;
/// Half of `SIDE`: the centre of a square at the origin.
const HALF: f64 = SIDE / 2.0;
/// Where the second square of a pair starts: one side plus a side-wide gap.
const PITCH: f64 = 2.0 * SIDE;
/// A position far from the squares near the origin.
const FAR: f64 = 300.0;
/// The size of the wide rectangle the resize and handle scenarios use.
const WIDE: Vec2 = Vec2::new(100.0, 50.0);
/// Seconds between pointer samples: one frame at 60 Hz.
const FRAME: f64 = 0.016;
/// The centre of the default 800 × 600 viewport.
const VIEWPORT_CENTER: Point = Point::new(400.0, 300.0);
/// How close computed coordinates must be to count as equal.
const EPSILON: f64 = 1e-9;
/// How close flattened boolean geometry must be to the exact shape.
const BOOLEAN_EPSILON: f64 = 1e-6;

fn key(canvas: &mut canvist_canvas::Canvas, name: &str, modifiers: Modifiers) {
	canvas
		.input(&Input::Key(KeyEvent {
			key: name.to_owned(),
			modifiers,
			down: true,
		}))
		.expect("key");
}

/// The point halfway between `from` and `to`.
fn midway(from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
	(f64::midpoint(from.0, to.0), f64::midpoint(from.1, to.1))
}

#[test]
fn clicking_selects_and_shift_click_toggles() {
	let (mut canvas, _) = support::canvas();
	let a = support::rect(&mut canvas, 0.0, 0.0, SIDE, SIDE);
	let b = support::rect(&mut canvas, PITCH, 0.0, SIDE, SIDE);

	support::click(&mut canvas, HALF, HALF, NONE, 0.0);
	assert_eq!(canvas.state().selection().nodes(), &[a]);

	support::click(&mut canvas, PITCH + HALF, HALF, SHIFT, 1.0);
	assert_eq!(canvas.state().selection().nodes(), &[a, b]);

	support::click(&mut canvas, PITCH + HALF, HALF, SHIFT, 2.0);
	assert_eq!(canvas.state().selection().nodes(), &[a]);

	support::click(&mut canvas, FAR, FAR, NONE, 3.0);
	assert!(canvas.state().selection().is_empty());
}

#[test]
fn a_marquee_selects_what_it_touches() {
	/// Above and left of every square.
	const START: (f64, f64) = (-10.0, -10.0);
	/// Inside the second square, so the marquee touches both of the pair.
	const END: (f64, f64) = (120.0, 20.0);

	let (mut canvas, _) = support::canvas();
	let a = support::rect(&mut canvas, 0.0, 0.0, SIDE, SIDE);
	let b = support::rect(&mut canvas, PITCH, 0.0, SIDE, SIDE);
	support::rect(&mut canvas, FAR, FAR, SIDE, SIDE);

	support::drag(&mut canvas, &[START, midway(START, END), END], NONE);
	assert_eq!(canvas.state().selection().nodes(), &[a, b]);
}

#[test]
fn dragging_moves_and_snaps_with_guides() {
	/// The side of the big square at the origin.
	const BIG: f64 = 100.0;

	let (mut canvas, _) = support::canvas();
	support::rect(&mut canvas, 0.0, 0.0, BIG, BIG);
	let moving = support::rect(&mut canvas, FAR, FAR, SIDE, SIDE);

	// Drag the small square so its left edge lands 3px right of the big
	// square's right edge: it snaps to 100.
	let grab = Point::new(FAR + HALF, FAR + HALF);
	let drop = Point::new(BIG + 3.0 + HALF, BIG + HALF);
	let mut time = 0.0;

	for (phase, position) in [
		(Phase::Down, grab),
		(Phase::Move, grab.midpoint(drop)),
		(Phase::Move, drop),
	] {
		support::feed(&mut canvas, PointerEvent::mouse(phase, position, time));
		time += FRAME;
	}

	let guides = canvas
		.overlay()
		.into_iter()
		.filter(|item| matches!(item, OverlayItem::Guide { .. }))
		.count();
	assert!(guides >= 1, "a guide shows while snapped");
	support::feed(&mut canvas, PointerEvent::mouse(Phase::Up, drop, time));

	let bounds = canvas.state().index().get(moving).expect("moved").bounds;
	assert!(
		(bounds.x0 - BIG).abs() < EPSILON,
		"snapped to the edge, got {bounds:?}"
	);
	assert!((bounds.y0 - BIG).abs() < EPSILON);

	// The whole drag is one undo step.
	assert!(canvas.state_mut().undo().expect("undo"));

	let left = canvas
		.state()
		.index()
		.get(moving)
		.map(|geometry| geometry.bounds.x0);
	assert_eq!(left, Some(FAR));
}

#[test]
fn resizing_changes_size_not_scale() {
	let (mut canvas, _) = support::canvas();
	let rect = support::rect(&mut canvas, 0.0, 0.0, WIDE.x, WIDE.y);
	let corner = (WIDE.x, WIDE.y);
	let dragged = (WIDE.x + SIDE, WIDE.y + SIDE);

	support::click(&mut canvas, WIDE.x / 2.0, WIDE.y / 2.0, NONE, 0.0);
	support::drag(
		&mut canvas,
		&[corner, midway(corner, dragged), dragged],
		NONE,
	);

	let node = canvas.state().scene().node(rect).expect("rect");
	assert_eq!(node.props().size, Vec2::new(dragged.0, dragged.1));
	assert!((node.props().transform.determinant() - 1.0).abs() < EPSILON);
}

#[test]
fn rotating_snaps_with_shift() {
	/// The rotated square's side.
	const BIG: f64 = 100.0;
	/// Just outside the bottom-right corner: the rotation zone.
	const GRIP: (f64, f64) = (110.0, 110.0);
	/// Not quite a quarter turn from the grip about the square's centre.
	const TURNED: (f64, f64) = (-12.0, 115.0);
	/// The rotation snap step with Shift.
	const SNAP_DEGREES: f64 = 15.0;

	let (mut canvas, _) = support::canvas();
	let rect = support::rect(&mut canvas, 0.0, 0.0, BIG, BIG);
	support::click(&mut canvas, BIG / 2.0, BIG / 2.0, NONE, 0.0);
	support::drag(&mut canvas, &[GRIP, midway(GRIP, TURNED), TURNED], SHIFT);

	let transform = canvas
		.state()
		.scene()
		.node(rect)
		.expect("rect")
		.props()
		.transform;
	let [a, b, ..] = transform.as_coeffs();
	let degrees = b.atan2(a).to_degrees();
	let steps = degrees / SNAP_DEGREES;
	assert!(
		(steps - steps.round()).abs() < EPSILON,
		"snapped to 15°: {degrees}"
	);
	assert!(degrees.abs() > 1.0);
}

#[test]
fn shape_tools_draw_and_return_to_select() {
	/// Where the drag starts.
	const START: (f64, f64) = (10.0, 10.0);
	/// 100 across and 50 down from the start; Shift squares it to 100 × 100.
	const END: (f64, f64) = (110.0, 60.0);

	let (mut canvas, kit) = support::canvas();
	canvas.set_tool(kit.shapes.ellipse);
	support::drag(&mut canvas, &[START, midway(START, END), END], SHIFT);

	let ellipse = canvas.state().selection().nodes()[0];
	assert_eq!(
		canvas.state().scene().kind(ellipse),
		Some(NodeKind::Ellipse)
	);

	let size = canvas
		.state()
		.index()
		.get(ellipse)
		.map(|geometry| geometry.size);
	assert_eq!(size, Some(Vec2::new(100.0, 100.0)));
	assert_eq!(canvas.tool(), ToolId::SELECT);
}

#[test]
fn shapes_drawn_inside_a_frame_become_its_children() {
	let (mut canvas, kit) = support::canvas();
	canvas.set_tool(kit.frames.frame);
	support::drag(&mut canvas, &[(0.0, 0.0), (200.0, 200.0)], NONE);
	let frame = canvas.state().selection().nodes()[0];
	canvas.set_tool(kit.shapes.rectangle);
	support::drag(&mut canvas, &[(50.0, 50.0), (80.0, 90.0)], NONE);
	let rect = canvas.state().selection().nodes()[0];

	assert_eq!(
		canvas
			.state()
			.scene()
			.node(rect)
			.map(canvist_scene::Node::parent),
		Some(frame)
	);
	let bounds = canvas.state().index().get(rect).expect("rect").bounds;
	assert!((bounds.x0 - 50.0).abs() < 1e-9 && (bounds.y1 - 90.0).abs() < 1e-9);
}

#[test]
fn the_pen_draws_curves_and_closes_paths() {
	/// The third anchor, a plain corner below the first two.
	const LAST: (f64, f64) = (50.0, 80.0);

	let (mut canvas, kit) = support::canvas();
	canvas.set_tool(kit.shapes.pen);
	support::click(&mut canvas, 0.0, 0.0, NONE, 0.0);
	support::drag(&mut canvas, &[(100.0, 0.0), (130.0, 30.0)], NONE);
	support::click(&mut canvas, LAST.0, LAST.1, NONE, 2.0);
	assert!(
		canvas
			.overlay()
			.iter()
			.any(|item| matches!(item, OverlayItem::Path { .. }))
	);
	support::click(&mut canvas, 1.0, 1.0, NONE, 3.0);

	let path = canvas.state().selection().nodes()[0];
	let node = canvas.state().scene().node(path).expect("path");
	let is_curve = |element: &kurbo::PathEl| matches!(element, kurbo::PathEl::CurveTo(..));
	let curved = matches!(
		&node.props().shape,
		Shape::Path { path, .. } if path.elements().iter().any(is_curve)
	);
	assert!(curved);
	assert!(!node.props().fills.is_empty(), "closed paths are filled");
	assert_eq!(canvas.tool(), ToolId::SELECT);
}

#[test]
fn the_text_tool_opens_the_editor() {
	/// Where the text frame is placed.
	const PLACE: Point = Point::new(40.0, 40.0);

	let (mut canvas, kit) = support::canvas();
	canvas.set_tool(kit.texts.text);
	support::feed(&mut canvas, PointerEvent::mouse(Phase::Down, PLACE, 0.0));
	let outcome = canvas
		.input(&Input::Pointer(PointerEvent::mouse(Phase::Up, PLACE, 0.0)))
		.expect("up");

	let canvist_canvas::Outcome::EditText(text) = outcome else {
		panic!("expected to edit text, got {outcome:?}");
	};
	assert_eq!(
		canvas
			.state()
			.scene()
			.node(text)
			.map(|node| node.props().shape.clone()),
		Some(Shape::Text(TextSizing::AutoWidth))
	);
}

#[test]
fn keys_nudge_delete_and_undo() {
	/// The nudged square's side.
	const SMALL: f64 = 10.0;

	let (mut canvas, _) = support::canvas();
	let rect = support::rect(&mut canvas, 0.0, 0.0, SMALL, SMALL);
	support::click(&mut canvas, SMALL / 2.0, SMALL / 2.0, NONE, 0.0);
	key(&mut canvas, "ArrowRight", SHIFT);
	assert_eq!(
		canvas
			.state()
			.index()
			.get(rect)
			.map(|geometry| geometry.bounds.x0),
		Some(10.0)
	);

	key(&mut canvas, "Backspace", NONE);
	assert!(!canvas.state().scene().contains(rect));

	let primary = Modifiers {
		primary: true,
		..NONE
	};
	key(&mut canvas, "z", primary);
	assert!(canvas.state().scene().contains(rect));
	key(
		&mut canvas,
		"z",
		Modifiers {
			shift: true,
			..primary
		},
	);
	assert!(!canvas.state().scene().contains(rect));
}

#[test]
fn group_ungroup_and_boolean_commands() {
	let (mut canvas, _) = support::canvas();
	let a = support::rect(&mut canvas, 0.0, 0.0, SIDE, SIDE);
	let b = support::rect(&mut canvas, HALF, 0.0, SIDE, SIDE);
	canvas.state_mut().selection_mut().set([a, b]);
	canvas.run("group", &json!({})).expect("group");
	let group = canvas.state().selection().nodes()[0];
	assert_eq!(canvas.state().scene().children(group), &[a, b]);

	// Clicking a member now selects the group.
	support::click(&mut canvas, 10.0, 10.0, NONE, 0.0);
	assert_eq!(canvas.state().selection().nodes(), &[group]);

	canvas.run("ungroup", &json!({})).expect("ungroup");
	assert_eq!(canvas.state().selection().nodes(), &[a, b]);
	assert!(!canvas.state().scene().contains(group));

	canvas
		.run("boolean", &json!({ "op": "intersect" }))
		.expect("boolean");
	let boolean = canvas.state().selection().nodes()[0];
	let bounds = canvas.state().index().get(boolean).expect("boolean").bounds;

	// The overlap runs from the second square's left edge to the first's right.
	assert!((bounds.x0 - HALF).abs() < BOOLEAN_EPSILON, "{bounds:?}");
	assert!((bounds.x1 - SIDE).abs() < BOOLEAN_EPSILON, "{bounds:?}");

	assert!(canvas.run("boolean", &json!({ "op": "melt" })).is_err());
	assert!(canvas.run("explode", &json!({})).is_err());
}

#[test]
fn the_inspector_describes_and_edits_the_selection() {
	let (mut canvas, _) = support::canvas();
	let a = support::rect(&mut canvas, 0.0, 0.0, SIDE, SIDE);
	let b = support::rect(&mut canvas, 0.0, PITCH, SIDE, SIDE);
	canvas.state_mut().selection_mut().set([a, b]);
	let inspector = canvas.inspect();
	let field = |inspector: &serde_json::Value, key: &str| {
		inspector["sections"]
			.as_array()
			.into_iter()
			.flatten()
			.flat_map(|section| section["fields"].as_array().into_iter().flatten())
			.find(|field| field["key"] == key)
			.cloned()
			.unwrap_or_default()
	};

	assert_eq!(field(&inspector, "x")["value"], json!(0.0));
	assert_eq!(field(&inspector, "y")["mixed"], json!(true));
	assert_eq!(
		field(&inspector, "opacity")["control"]["type"],
		json!("number")
	);

	canvas.set_field("opacity", &json!(0.5)).expect("opacity");
	canvas.set_field("rotation", &json!(90)).expect("rotation");
	let after = canvas.inspect();
	assert_eq!(field(&after, "opacity")["value"], json!(0.5));
	let rotation = field(&after, "rotation")["value"].as_f64().expect("number");
	assert!((rotation - 90.0).abs() < 1e-9);
	assert!(canvas.set_field("opacity", &json!("loud")).is_err());
	assert!(canvas.set_field("sparkle", &json!(1)).is_err());
}

#[test]
fn metadata_lists_extensions_tools_and_commands() {
	let (canvas, _) = support::canvas();
	let metadata = canvas.metadata();
	let names = |key: &str| -> Vec<String> {
		metadata[key]
			.as_array()
			.into_iter()
			.flatten()
			.filter_map(|item| {
				item.as_str()
					.map(str::to_owned)
					.or_else(|| item["name"].as_str().map(str::to_owned))
			})
			.collect()
	};

	assert_eq!(names("extensions"), ["shapes", "frames", "texts", "images"]);
	assert!(names("tools").contains(&"pen".to_owned()));
	assert!(names("commands").contains(&"boolean".to_owned()));
	assert!(names("nodes").contains(&"instance".to_owned()));
}

#[test]
fn the_wheel_pans_and_zooms_about_the_pointer() {
	/// A two-finger scroll: pans the camera by this many pixels.
	const PAN: Vec2 = Vec2::new(30.0, 40.0);
	/// Pixels of pinch scroll; zooms in by about 1.65×.
	const PINCH_SCROLL: f64 = 50.0;

	let (mut canvas, _) = support::canvas();
	let wheel = |delta: Vec2, pinch: bool| {
		Input::Wheel(WheelEvent {
			delta,
			position: VIEWPORT_CENTER,
			modifiers: NONE,
			pinch,
		})
	};

	canvas.input(&wheel(PAN, false)).expect("pan");
	assert_eq!(canvas.state().camera().origin, PAN.to_point());

	let under = canvas.state().camera().to_world(VIEWPORT_CENTER);
	canvas
		.input(&wheel(Vec2::new(0.0, -PINCH_SCROLL), true))
		.expect("zoom");
	assert!(canvas.state().camera().zoom > 1.5);

	let drift = canvas
		.state()
		.camera()
		.to_world(VIEWPORT_CENTER)
		.distance(under);
	assert!(drift < EPSILON);
}

#[test]
fn two_touches_pinch_zoom() {
	/// Where the first finger rests.
	const FIRST: f64 = 100.0;
	/// Where the second finger lands.
	const SECOND: f64 = 200.0;
	/// Where the second finger moves: twice as far from the first.
	const SPREAD: f64 = 300.0;

	let (mut canvas, _) = support::canvas();
	let touch = |id: u64, phase: Phase, x: f64| {
		Input::Pointer(PointerEvent {
			id,
			kind: PointerKind::Touch,
			phase,
			position: Point::new(x, 100.0),
			buttons: 1,
			modifiers: NONE,
			time: 0.0,
		})
	};
	let zoom = canvas.state().camera().zoom;

	for input in [
		touch(1, Phase::Down, FIRST),
		touch(2, Phase::Down, SECOND),
		touch(2, Phase::Move, SPREAD),
	] {
		canvas.input(&input).expect("touch");
	}

	assert!((canvas.state().camera().zoom - zoom * 2.0).abs() < EPSILON);
}

#[test]
fn the_hand_tool_coasts_after_release() {
	/// Where the drag starts.
	const START: f64 = 100.0;
	/// Screen pixels the pointer moves per sample.
	const STEP: f64 = 50.0;
	/// Seconds between the hand drag's samples.
	const INTERVAL: f64 = 0.02;

	let (mut canvas, _) = support::canvas();
	canvas.set_tool(ToolId::HAND);
	let end = START + 2.0 * STEP;
	let mut time = 0.0;

	for (phase, x) in [
		(Phase::Down, START),
		(Phase::Move, START + STEP),
		(Phase::Move, end),
		(Phase::Up, end),
	] {
		support::feed(
			&mut canvas,
			PointerEvent::mouse(phase, Point::new(x, 100.0), time),
		);
		time += INTERVAL;
	}

	let before = canvas.state().camera().origin;
	assert!(canvas.frame(1.0 / 60.0));
	assert!(
		canvas.state().camera().origin.x < before.x,
		"momentum keeps panning"
	);
}

#[test]
fn handles_show_for_the_selection() {
	let (mut canvas, _) = support::canvas();
	support::rect(&mut canvas, 0.0, 0.0, WIDE.x, WIDE.y);
	support::click(&mut canvas, WIDE.x / 2.0, WIDE.y / 2.0, NONE, 0.0);
	let handles: Vec<Handle> = canvas
		.overlay()
		.into_iter()
		.filter_map(|item| {
			match item {
				OverlayItem::Handle { handle, .. } => Some(handle),
				_ => None,
			}
		})
		.collect();

	assert_eq!(handles.len(), 8);
	assert!(
		canvas
			.overlay()
			.iter()
			.any(|item| matches!(item, OverlayItem::Label { text, .. } if text == "100 × 50"))
	);
}
