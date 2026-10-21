//! The browser app's core, without a browser: two peers editing shapes and
//! text frames together.

use std::time::Duration;

use canvist_canvas::Input;
use canvist_canvas::KeyEvent;
use canvist_canvas::Modifiers;
use canvist_canvas::Phase;
use canvist_canvas::PointerEvent;
use canvist_canvas_wasm::App;
use canvist_canvas_wasm::AppError;
use canvist_paint::PaintOp;
use canvist_paint::Quality;
use canvist_paint::Recorder;
use kurbo::Point;
use kurbo::Size;
use kurbo::Vec2;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The viewport every test app has, in CSS pixels.
const VIEWPORT: Size = Size::new(1200.0, 800.0);
/// A point on the demo's title text frame at the default camera.
const ON_TITLE: Point = Point::new(100.0, 70.0);
/// A point on the demo's star.
const ON_STAR: Point = Point::new(640.0, 290.0);
/// A point on empty canvas.
const EMPTY: Point = Point::new(700.0, 40.0);
/// Seconds between input events: one frame at 60 Hz.
const FRAME: f64 = 0.016;
/// How far each step of a test drag moves.
const DRAG_STEP: Vec2 = Vec2::new(12.0, 12.0);
/// Steps in a test drag.
const DRAG_STEPS: u32 = 10;
/// The size of a rectangle drawn with the rectangle tool.
const DRAWN: Vec2 = Vec2::new(80.0, 60.0);
/// Device pixels per CSS pixel when painting.
const SCALE: f64 = 2.0;
/// The caret is narrower than this, in device pixels...
const CARET_MAX_WIDTH: f64 = 6.0;
/// ...and taller than this: a line of text.
const CARET_MIN_HEIGHT: f64 = 20.0;

/// Input timestamps, a frame apart.
#[derive(Default)]
struct Clock(f64);

impl Clock {
	fn tick(&mut self) -> f64 {
		self.0 += FRAME;
		self.0
	}
}

fn app(client: u32) -> Result<App, AppError> {
	let mut app = App::new(client)?;
	app.resize(VIEWPORT);
	Ok(app)
}

fn pointer(
	app: &mut App,
	clock: &mut Clock,
	phase: Phase,
	position: Point,
) -> Result<bool, AppError> {
	let time = clock.tick();
	app.input(
		&Input::Pointer(PointerEvent::mouse(phase, position, time)),
		Duration::from_secs_f64(time),
	)
}

fn click(app: &mut App, clock: &mut Clock, position: Point) -> Result<(), AppError> {
	pointer(app, clock, Phase::Down, position)?;
	pointer(app, clock, Phase::Up, position)?;
	Ok(())
}

fn double_click(app: &mut App, clock: &mut Clock, position: Point) -> Result<(), AppError> {
	click(app, clock, position)?;
	click(app, clock, position)
}

fn key(
	app: &mut App,
	clock: &mut Clock,
	key: &str,
	modifiers: Modifiers,
) -> Result<bool, AppError> {
	let time = clock.tick();
	app.input(
		&Input::Key(KeyEvent {
			key: key.to_owned(),
			modifiers,
			down: true,
		}),
		Duration::from_secs_f64(time),
	)
}

fn type_keys(app: &mut App, clock: &mut Clock, text: &str) -> Result<(), AppError> {
	for character in text.chars() {
		key(app, clock, &character.to_string(), Modifiers::default())?;
	}

	Ok(())
}

/// Deliver every update `from` made to `to`.
fn sync(from: &mut App, to: &mut App) -> Result<(), AppError> {
	for update in from.take_updates() {
		to.apply_update(&update)?;
	}

	Ok(())
}

fn scene_json(app: &App) -> String {
	app.canvas().state().scene().to_json().to_string()
}

fn title_text(app: &App) -> String {
	let scene = app.canvas().state().scene();
	scene
		.nodes()
		.find_map(|node| scene.text(node.id()))
		.map(canvist_model::Document::plain_text)
		.unwrap_or_default()
}

#[test]
fn every_peer_starts_from_the_same_shared_state() -> TestResult {
	let mut ada = app(2)?;
	let mut grace = app(3)?;

	assert_eq!(ada.state_vector(), grace.state_vector());
	assert_eq!(scene_json(&ada), scene_json(&grace));

	// Each lacks nothing the other has, so the first exchange changes
	// nothing.
	let diff = grace.encode_diff(&ada.state_vector())?;
	assert!(!ada.apply_update(&diff)?);
	assert!(ada.take_updates().is_empty() && grace.take_updates().is_empty());
	Ok(())
}

#[test]
fn the_seed_client_id_is_reserved() {
	let seed = canvist_canvas_wasm::demo::SEED_CLIENT;

	assert!(matches!(App::new(seed), Err(AppError::ReservedClient(_))));
	assert!(matches!(
		App::with_scene(
			seed,
			&serde_json::json!({ "type": "canvist-scene", "version": 1, "nodes": [] })
		),
		Err(AppError::ReservedClient(_))
	));
}

#[test]
fn peers_given_the_same_scene_share_its_state() -> TestResult {
	let scene = serde_json::json!({
		"type": "canvist-scene",
		"version": 1,
		"nodes": [{
			"type": "page",
			"children": [{
				"type": "rectangle",
				"props": { "name": "Card", "size": [120, 80], "fills": [{ "type": "solid", "color": "#3b2cf5" }] },
			}],
		}],
	});
	let mut ada = App::with_scene(2, &scene)?;
	let grace = App::with_scene(3, &scene)?;

	assert!(scene_json(&ada).contains("Card"));
	// Merging a peer that started from the same scene changes nothing. The
	// encoded bytes may still differ: yrs writes map entries in hash order.
	assert!(!ada.apply_update(&grace.encode_state())?);
	assert_eq!(scene_json(&ada), scene_json(&grace));
	Ok(())
}

#[test]
fn a_scene_that_is_not_scene_json_is_an_error() {
	assert!(App::with_scene(2, &serde_json::json!({ "type": "doc" })).is_err());
}

#[test]
fn a_drag_reaches_the_other_peer() -> TestResult {
	let mut ada = app(2)?;
	let mut grace = app(3)?;
	let mut clock = Clock::default();
	let before = scene_json(&grace);

	pointer(&mut ada, &mut clock, Phase::Down, ON_STAR)?;
	let mut at = ON_STAR;

	for _ in 0..DRAG_STEPS {
		at += DRAG_STEP;
		pointer(&mut ada, &mut clock, Phase::Move, at)?;
	}

	pointer(&mut ada, &mut clock, Phase::Up, at)?;
	sync(&mut ada, &mut grace)?;

	assert_ne!(scene_json(&grace), before);
	assert_eq!(scene_json(&grace), scene_json(&ada));
	Ok(())
}

#[test]
fn undo_reverts_only_this_peers_change() -> TestResult {
	let mut ada = app(2)?;
	let mut grace = app(3)?;
	let original = scene_json(&ada);
	let (ada_adds, grace_adds) = (3, 2);

	ada.add_stress(ada_adds)?;
	sync(&mut ada, &mut grace)?;
	grace.add_stress(grace_adds)?;
	sync(&mut grace, &mut ada)?;
	let nodes = ada.canvas().state().scene().len();

	let primary = Modifiers {
		primary: true,
		..Modifiers::default()
	};
	assert!(key(&mut ada, &mut Clock::default(), "z", primary)?);
	sync(&mut ada, &mut grace)?;

	assert_eq!(ada.canvas().state().scene().len(), nodes - ada_adds);
	assert_eq!(scene_json(&ada), scene_json(&grace));
	assert_ne!(scene_json(&ada), original, "grace's rectangles stay");
	Ok(())
}

#[test]
fn double_clicking_a_text_frame_edits_it_for_everyone() -> TestResult {
	let mut ada = app(2)?;
	let mut grace = app(3)?;
	let mut clock = Clock::default();
	let original = title_text(&ada);

	double_click(&mut ada, &mut clock, ON_TITLE)?;
	assert!(ada.session().is_some(), "a double click opens the editor");

	key(&mut ada, &mut clock, "End", Modifiers::default())?;
	type_keys(&mut ada, &mut clock, " Hi!")?;
	sync(&mut ada, &mut grace)?;
	let seen = title_text(&grace);
	assert!(seen.ends_with("Hi!"), "got {seen:?}");

	// Clicking empty canvas ends editing and hands the text back.
	click(&mut ada, &mut clock, EMPTY)?;
	assert!(ada.session().is_none());
	assert_eq!(title_text(&ada), title_text(&grace));
	assert_ne!(title_text(&ada), original);
	Ok(())
}

#[test]
fn a_collaborators_typing_reaches_an_open_text_frame() -> TestResult {
	let mut ada = app(2)?;
	let mut grace = app(3)?;
	let mut clock = Clock::default();

	// Both peers have the title open.
	for peer in [&mut ada, &mut grace] {
		double_click(peer, &mut clock, ON_TITLE)?;
		assert!(peer.session().is_some());
	}

	assert!(ada.type_text("!", Duration::from_secs_f64(clock.tick()))?);
	sync(&mut ada, &mut grace)?;
	assert!(grace.type_text("?", Duration::from_secs_f64(clock.tick()))?);
	sync(&mut grace, &mut ada)?;

	let text = |app: &App| {
		app.session()
			.map(|session| session.editor().state().doc().plain_text())
			.unwrap_or_default()
	};
	assert_eq!(text(&ada), text(&grace));
	assert!(text(&ada).contains('!') && text(&ada).contains('?'));

	key(&mut ada, &mut clock, "Escape", Modifiers::default())?;
	key(&mut grace, &mut clock, "Escape", Modifiers::default())?;
	assert_eq!(scene_json(&ada), scene_json(&grace));
	Ok(())
}

#[test]
fn painting_draws_every_kind_of_node_and_the_caret() -> TestResult {
	let mut ada = app(2)?;
	double_click(&mut ada, &mut Clock::default(), ON_TITLE)?;

	let mut recorder = Recorder::new();
	ada.paint(&mut recorder, SCALE, Quality::Full);

	let fills = |test: &dyn Fn(kurbo::Rect) -> bool| {
		recorder
			.ops
			.iter()
			.any(|op| matches!(op, PaintOp::Fill { bounds, .. } if test(*bounds)))
	};
	let has = |test: fn(&PaintOp) -> bool| recorder.ops.iter().any(test);
	assert!(has(|op| matches!(op, PaintOp::Image { .. })));
	assert!(has(|op| matches!(op, PaintOp::Text { .. })));
	assert!(has(|op| matches!(op, PaintOp::Shadow(_))));
	assert!(has(|op| matches!(op, PaintOp::Stroke { .. })));
	assert!(
		fills(&|bounds| bounds.width() < CARET_MAX_WIDTH && bounds.height() > CARET_MIN_HEIGHT),
		"the caret is drawn"
	);
	// Device-pixel scaling reaches every call: the page background covers
	// the viewport at `SCALE` times its size.
	let device = VIEWPORT * SCALE;
	assert!(fills(&|bounds| {
		bounds.width().ceil() >= device.width && bounds.height().ceil() >= device.height
	}));
	Ok(())
}

#[test]
fn tools_switch_by_name() -> TestResult {
	let mut ada = app(2)?;
	let mut clock = Clock::default();
	assert!(ada.set_tool("rectangle")?);
	assert!(!ada.set_tool("teleport")?);

	pointer(&mut ada, &mut clock, Phase::Down, EMPTY)?;
	pointer(&mut ada, &mut clock, Phase::Move, EMPTY + DRAWN)?;
	pointer(&mut ada, &mut clock, Phase::Up, EMPTY + DRAWN)?;

	assert_eq!(ada.take_updates().len(), 1, "one rectangle, one update");
	Ok(())
}
