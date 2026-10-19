//! Performance budgets for painting, from `docs/src/canvas/architecture.md`.
//! Release mode only:
//!
//! ```sh
//! cargo test --release -p canvist_paint --features gpu,cpu --test perf_budgets -- --ignored
//! ```

#![cfg(not(debug_assertions))]

mod support;

use std::time::Duration;
use std::time::Instant;

use canvist_paint::Assets;

/// Samples per measurement; the median is reported.
const SAMPLES: usize = 21;
/// Nodes in the budget scene.
const NODES: usize = 10_000;
/// The side of each square, in world units.
const SIDE: f64 = 20.0;
/// A 1080p viewport.
const WIDTH: f64 = 1_920.0;
/// A 1080p viewport.
const HEIGHT: f64 = 1_080.0;
/// The GPU frame budget: 120 fps leaves about 8 ms.
const GPU_FRAME: Duration = Duration::from_millis(8);
/// The CPU fallback budget: 30 fps.
const CPU_FRAME: Duration = Duration::from_millis(33);
/// Zoomed out far enough that every node is on screen.
const ZOOM: f64 = 0.4;

fn slack() -> f64 {
	std::env::var("CANVIST_PERF_SLACK")
		.ok()
		.and_then(|value| value.parse().ok())
		.unwrap_or(1.0)
}

fn median(mut run: impl FnMut()) -> Duration {
	run();
	let mut samples: Vec<Duration> = (0..SAMPLES)
		.map(|_| {
			let start = Instant::now();
			run();
			start.elapsed()
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

fn budget_canvas() -> canvist_canvas::Canvas {
	let mut canvas = support::canvas(support::grid(NODES, SIDE));
	canvas.state_mut().camera_mut().zoom = ZOOM;
	canvas
}

/// One pan/zoom frame with every node on screen: walk the scene and encode
/// it for the GPU. The GPU's own rasterisation runs in parallel on the
/// device and is measured by the browser frame-time test.
#[cfg(feature = "gpu")]
#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn encode_a_frame_of_10k_nodes() {
	let canvas = budget_canvas();
	let assets = Assets::new();
	let sources = support::sources(&canvas, &assets);
	let view = support::view(&canvas, WIDTH, HEIGHT);
	let page = canvas.state().scene().pages()[0];
	let mut painter = canvist_paint::VelloPainter::new();

	let measured = median(|| {
		painter.reset();
		canvist_paint::paint_page(&mut painter, &sources, page, &view);
	});

	assert_within(
		"encode a frame @10k nodes",
		measured,
		Duration::from_millis(8),
	);
}

/// The CPU fallback rasterising the same frame at 1080p.
#[cfg(feature = "cpu")]
#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn rasterise_a_frame_of_10k_nodes_on_the_cpu() {
	let canvas = budget_canvas();
	let assets = Assets::new();
	let sources = support::sources(&canvas, &assets);
	let view = support::view(&canvas, WIDTH, HEIGHT);
	let page = canvas.state().scene().pages()[0];
	let mut painter = canvist_paint::CpuPainter::new(WIDTH as u32, HEIGHT as u32).expect("painter");

	let measured = median(|| {
		painter.reset(WIDTH as u32, HEIGHT as u32).expect("reset");
		canvist_paint::paint_page(&mut painter, &sources, page, &view);
		std::hint::black_box(painter.render());
	});

	assert_within(
		"CPU frame @10k nodes, 1080p",
		measured,
		Duration::from_millis(33),
	);
}
