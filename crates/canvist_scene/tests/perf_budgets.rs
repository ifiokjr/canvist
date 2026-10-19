//! Performance budgets for the scene model's hot paths.
//!
//! These tests fail when an operation exceeds the budget recorded in
//! `docs/internal/canvas.md`. They only mean something in release
//! mode:
//!
//! ```sh
//! cargo test --release -p canvist_scene --test perf_budgets -- --ignored
//! ```
//!
//! `CANVIST_PERF_SLACK` multiplies every budget. CI sets it for noisy shared
//! runners; locally it defaults to 1.

#![cfg(not(debug_assertions))]

mod support;

use std::time::Duration;
use std::time::Instant;

use canvist_scene::Prop;
use canvist_scene::Scene;
use kurbo::Affine;

/// Samples per measurement; the median is reported.
const SAMPLES: usize = 31;

fn slack() -> f64 {
	std::env::var("CANVIST_PERF_SLACK")
		.ok()
		.and_then(|value| value.parse().ok())
		.unwrap_or(1.0)
}

/// The median time of one call to `run`, over [`SAMPLES`] batches of
/// `batch` calls after one warm-up batch.
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
fn set_prop_at_100k_nodes() {
	let (mut scene, frames) = support::framed(1_000, 100);
	let node = scene.children(frames[500])[50];
	let mut x = 0.0;

	let measured = median_per_call(2_000, || {
		x += 1.0;
		let mut transaction = scene.transaction();
		transaction
			.set(node, Prop::Transform(Affine::translate((x, 0.0))))
			.expect("valid node");
		std::hint::black_box(transaction.commit());
	});

	assert_within(
		"set one prop @100k nodes",
		measured,
		Duration::from_micros(2),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn move_in_frame_at_100k_nodes() {
	let (mut scene, frames) = support::framed(1_000, 100);
	let frame = frames[500];
	let node = scene.children(frame)[10];
	let mut toggle = false;

	let measured = median_per_call(2_000, || {
		toggle = !toggle;
		let mut transaction = scene.transaction();
		transaction
			.move_to(node, frame, Some(if toggle { 80 } else { 10 }))
			.expect("valid move");
		std::hint::black_box(transaction.commit());
	});

	assert_within(
		"reorder in a frame @100k nodes",
		measured,
		Duration::from_micros(2),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn insert_10k_nodes() {
	let measured = median_per_call(1, || {
		std::hint::black_box(support::framed(100, 99));
	});

	assert_within(
		"insert 10k nodes in one step",
		measured,
		Duration::from_millis(20),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn json_round_trip_10k_nodes() {
	let (scene, _) = support::framed(100, 99);
	let schema = support::schema();
	let json = scene.to_json();

	let export = median_per_call(1, || {
		std::hint::black_box(scene.to_json());
	});
	let import = median_per_call(1, || {
		std::hint::black_box(Scene::from_json(schema.clone(), 2, &json).expect("loads"));
	});

	assert_within(
		"export 10k nodes to JSON",
		export,
		Duration::from_millis(50),
	);
	assert_within(
		"load 10k nodes from JSON",
		import,
		Duration::from_millis(50),
	);
}
