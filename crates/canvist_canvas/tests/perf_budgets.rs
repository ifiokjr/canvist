//! Performance budgets for the canvas's hot paths, from
//! `docs/internal/canvas.md`. Release mode only:
//!
//! ```sh
//! cargo test --release -p canvist_canvas --test perf_budgets -- --ignored
//! ```
//!
//! `CANVIST_PERF_SLACK` multiplies every budget.

#![cfg(not(debug_assertions))]

mod support;

use std::time::Duration;
use std::time::Instant;

use canvist_canvas::Canvas;
use canvist_canvas::MarqueeMode;
use canvist_canvas::Phase;
use canvist_canvas::PointerEvent;
use canvist_canvas::SceneIndex;
use kurbo::Point;
use kurbo::Rect;

const SAMPLES: usize = 31;

/// Frames in the 100k-node scene.
const FRAMES_100K: usize = 1_000;
/// Frames in the 10k-node scene.
const FRAMES_10K: usize = 100;
/// A hit-test point in the middle of the 100k-node scene's grid of frames.
const MID_GRID: Point = Point::new(55_123.0, 4_321.0);
/// A hit-test point near the far corner of the 100k-node scene's grid.
const FAR_CORNER: Point = Point::new(108_950.0, 9_950.0);
/// Hit-test points spread across the 100k-node scene.
const PROBES: [Point; 3] = [support::ON_FIRST_RECT, MID_GRID, FAR_CORNER];
/// A marquee from outside the first frame's corner across every frame of
/// the 10k-node scene.
const ALL_FRAMES: Rect = Rect::new(-10.0, -10.0, 110_000.0, 2_000.0);
/// Marquee calls per timed sample.
const MARQUEE_BATCH: u32 = 5;
/// Drag steps per timed sample.
const DRAG_BATCH: u32 = 500;

/// The budget for building the index of 100k nodes.
const BUILD_BUDGET: Duration = Duration::from_millis(100);
/// The budget for one hit test among 100k nodes.
const HIT_TEST_BUDGET: Duration = Duration::from_millis(1);
/// The budget for one marquee over 10k nodes.
const MARQUEE_BUDGET: Duration = Duration::from_millis(2);
/// The budget for one drag step among 100k nodes.
const DRAG_BUDGET: Duration = Duration::from_micros(500);

fn slack() -> f64 {
	std::env::var("CANVIST_PERF_SLACK")
		.ok()
		.and_then(|value| value.parse().ok())
		.unwrap_or(1.0)
}

fn median_per_call(batch: u32, mut run: impl FnMut()) -> Duration {
	for _ in 0..batch {
		run();
	}

	let mut samples: Vec<Duration> = (0..SAMPLES)
		.map(|_| {
			let start = Instant::now();

			for _ in 0..batch {
				run();
			}

			start.elapsed() / batch
		})
		.collect();
	samples.sort_unstable();
	samples[SAMPLES / 2]
}

fn assert_within(name: &str, measured: Duration, budget: Duration) {
	let allowed = budget.mul_f64(slack());
	eprintln!("{name}: {measured:?} (budget {budget:?}, allowed {allowed:?})");
	assert!(
		measured <= allowed,
		"{name} took {measured:?}, over its {allowed:?} budget"
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn build_the_index_for_100k_nodes() {
	let scene = support::framed(FRAMES_100K, support::RECTS_PER_FRAME);
	let measured = median_per_call(1, || {
		std::hint::black_box(SceneIndex::build(&scene, &mut support::texts()).expect("index"));
	});

	assert_within("build the index @100k nodes", measured, BUILD_BUDGET);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn hit_test_at_100k_nodes() {
	let scene = support::framed(FRAMES_100K, support::RECTS_PER_FRAME);
	let index = SceneIndex::build(&scene, &mut support::texts()).expect("index");
	let mut next = 0;

	let measured = median_per_call(1_000, || {
		next = (next + 1) % PROBES.len();
		std::hint::black_box(canvist_canvas::hit_test(&scene, &index, PROBES[next], 4.0));
	});

	assert_within("hit test @100k nodes", measured, HIT_TEST_BUDGET);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn marquee_over_10k_nodes() {
	let scene = support::framed(FRAMES_10K, support::RECTS_PER_FRAME);
	let index = SceneIndex::build(&scene, &mut support::texts()).expect("index");
	let rect = ALL_FRAMES;
	// The marquee touches all 10k nodes; starting outside the frames, it
	// picks the frames, whose rectangles come with them.
	assert_eq!(index.query(rect).count(), 10_000);
	let picked = canvist_canvas::marquee(&scene, &index, rect, MarqueeMode::Intersect);
	assert_eq!(picked.len(), 100);

	let measured = median_per_call(MARQUEE_BATCH, || {
		std::hint::black_box(canvist_canvas::marquee(
			&scene,
			&index,
			rect,
			MarqueeMode::Intersect,
		));
	});

	assert_within("marquee over 10k nodes", measured, MARQUEE_BUDGET);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn drag_step_at_100k_nodes() {
	let scene = support::framed(FRAMES_100K, support::RECTS_PER_FRAME);
	let mut builder = Canvas::builder().text_schema(support::text_schema());
	canvist_canvas::DesignKit::register(&mut builder);
	let mut canvas = builder.build(scene, support::texts()).expect("canvas");
	// Press on a rectangle inside the first frame and start dragging it.
	support::feed(
		&mut canvas,
		PointerEvent::mouse(Phase::Down, support::ON_FIRST_RECT, 0.0),
	);
	support::feed(
		&mut canvas,
		PointerEvent::mouse(Phase::Move, support::DRAG_START, 0.01),
	);
	let dragged = canvas.state().selection().nodes()[0];

	let before = canvas.state().index().get(dragged).expect("dragged").bounds;
	let mut step = 0.0;

	let measured = median_per_call(DRAG_BATCH, || {
		step += 1.0;
		let position = support::wander(step);
		support::feed(
			&mut canvas,
			PointerEvent::mouse(Phase::Move, position, step),
		);
	});

	let after = canvas.state().index().get(dragged).expect("dragged").bounds;
	assert_ne!(before, after, "the drag moved the node");
	assert_within("drag step @100k nodes", measured, DRAG_BUDGET);
}
