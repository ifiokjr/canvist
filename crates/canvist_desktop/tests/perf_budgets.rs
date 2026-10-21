//! Performance budgets for the desktop editor: from a key event to a frame
//! ready for the GPU. Release mode only:
//!
//! ```sh
//! cargo test --release -p canvist_desktop --test perf_budgets -- --ignored --nocapture
//! ```
//!
//! A keystroke's frame is the editor's whole share of the work: handling
//! the key (the edit, history, and relayout bookkeeping), laying out the
//! viewport, building its display list, and encoding it as a vello scene.
//! The GPU then rasterises in parallel.

#![cfg(all(not(debug_assertions), feature = "winit"))]

mod support;

use std::time::Duration;
use std::time::Instant;

use kurbo::Affine;
use support::Driver;

/// Samples per measurement; the median is reported.
const SAMPLES: usize = 51;
/// Keystroke → frame: 120 fps leaves about 8 ms, and the editor gets half.
const KEYSTROKE_FRAME: Duration = Duration::from_millis(4);
/// Opening a 10k-paragraph document until its first frame is ready.
const FIRST_FRAME: Duration = Duration::from_millis(100);
/// A burst of 1,000 keys, each with its own frame.
const BURST: Duration = Duration::from_millis(1_500);
/// A 1080p-ish editor.
const WIDTH: f32 = 1_200.0;
const HEIGHT: f32 = 900.0;

fn slack() -> f64 {
	std::env::var("CANVIST_PERF_SLACK")
		.ok()
		.and_then(|value| value.parse().ok())
		.unwrap_or(1.0)
}

fn assert_within(name: &str, measured: Duration, budget: Duration) {
	let allowed = budget.mul_f64(slack());
	eprintln!("{name}: {measured:?} (budget {budget:?}, allowed {allowed:?})");
	assert!(
		measured <= allowed,
		"{name} took {measured:?}, over its {allowed:?} budget"
	);
}

/// The median time of `run` over [`SAMPLES`] runs, after one warm-up.
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

fn paragraphs(count: usize) -> String {
	(0..count)
		.map(|index| {
			format!(
				"Paragraph {index} has **bold** text, *italic* text, and enough words to wrap \
				 onto a second line in a wide window."
			)
		})
		.collect::<Vec<_>>()
		.join("\n\n")
}

/// Type one key and produce the frame for it, encoded for the GPU.
fn keystroke_frame(driver: &mut Driver, painter: &mut canvist_paint::VelloPainter, key: &str) {
	let _ = driver.key(key, support::NONE);
	driver.frame();
	painter.reset();
	driver.editor.paint(painter, Affine::IDENTITY);
	std::hint::black_box(painter.scene());
}

fn painter(driver: &Driver) -> canvist_paint::VelloPainter {
	let mut painter = canvist_paint::VelloPainter::new();
	*painter.shaper_mut() = driver.editor.fonts().shaper();
	painter
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn keystroke_to_frame_in_a_10k_paragraph_document() {
	let mut driver = Driver::sized(&paragraphs(10_000), WIDTH, HEIGHT);
	let _ = driver.press("End", support::CTRL);
	driver.frame();
	let mut painter = painter(&driver);

	let measured = median(|| keystroke_frame(&mut driver, &mut painter, "a"));

	assert_within(
		"keystroke → frame @10k paragraphs",
		measured,
		KEYSTROKE_FRAME,
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn keystroke_to_frame_in_a_100_kb_paragraph() {
	let text = "word ".repeat(20_000);
	let mut driver = Driver::sized(&text, WIDTH, HEIGHT);
	let _ = driver.press("End", support::CTRL);
	driver.frame();
	let mut painter = painter(&driver);

	let measured = median(|| keystroke_frame(&mut driver, &mut painter, "a"));

	assert_within(
		"keystroke → frame in a 100 KB paragraph",
		measured,
		KEYSTROKE_FRAME,
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn enter_to_frame_in_a_10k_paragraph_document() {
	let mut driver = Driver::sized(&paragraphs(10_000), WIDTH, HEIGHT);
	let _ = driver.press("End", support::CTRL);
	driver.frame();
	let mut painter = painter(&driver);

	let measured = median(|| {
		let _ = driver.press("Enter", support::NONE);
		driver.frame();
		painter.reset();
		driver.editor.paint(&mut painter, Affine::IDENTITY);
	});

	assert_within("Enter → frame @10k paragraphs", measured, KEYSTROKE_FRAME);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn first_frame_of_a_10k_paragraph_document() {
	let markdown = paragraphs(10_000);

	let measured = median(|| {
		let driver = Driver::sized(&markdown, WIDTH, HEIGHT);
		let mut painter = painter(&driver);
		driver.editor.paint(&mut painter, Affine::IDENTITY);
		std::hint::black_box(painter.scene());
	});

	assert_within("open 10k paragraphs → first frame", measured, FIRST_FRAME);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn a_burst_of_1000_keys_never_falls_behind() {
	let mut driver = Driver::sized(&paragraphs(1_000), WIDTH, HEIGHT);
	let _ = driver.press("End", support::CTRL);
	driver.frame();
	let mut painter = painter(&driver);
	let keys = "The quick brown fox jumps over the lazy dog. ";
	let before = driver.markdown().len();
	let start = Instant::now();

	for character in keys.chars().cycle().take(1_000) {
		keystroke_frame(&mut driver, &mut painter, &character.to_string());
	}

	let measured = start.elapsed();
	assert_eq!(driver.markdown().len(), before + 1_000, "every key landed");
	assert_within("1,000 keys, a frame each", measured, BURST);
}
