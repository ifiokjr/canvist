//! Performance budgets for the model's hot paths.
//!
//! These tests fail when an operation exceeds the budget recorded in
//! `docs/src/v2/architecture.md`, so a regression breaks CI instead of
//! hiding in a benchmark report. They only mean something in release mode:
//!
//! ```sh
//! cargo test --release -p canvist_model --test perf_budgets -- --ignored
//! ```
//!
//! `CANVIST_PERF_SLACK` multiplies every budget. CI sets it for noisy shared
//! runners; locally it defaults to 1.
//!
//! The file compiles to nothing in debug builds, where timings are meaningless.

#![cfg(not(debug_assertions))]

mod support;

use std::time::Duration;
use std::time::Instant;

use canvist_model::Attrs;
use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::MarkSet;
use canvist_model::Point;

/// Samples per measurement; the median is reported.
const SAMPLES: usize = 31;
const PARAGRAPH: &str =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";
const LARGE_DOC: usize = 100_000;

fn slack() -> f64 {
	let configured = std::env::var("CANVIST_PERF_SLACK").ok();
	configured
		.and_then(|value| value.parse().ok())
		.unwrap_or(1.0)
}

/// The median time of one call to `run`, measured over [`SAMPLES`] batches of
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

fn large_doc() -> (Document, BlockId) {
	let doc = support::paragraphs(LARGE_DOC, PARAGRAPH);
	let middle = doc.children(None)[LARGE_DOC / 2];
	(doc, middle)
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn keystroke_in_a_100k_block_document() {
	let (mut doc, block) = large_doc();
	let caret = Point::new(block, 10);

	let measured = median_per_call(1_000, || {
		let mut transaction = doc.transaction();
		transaction
			.insert_text(caret, "a", MarkSet::empty())
			.expect("valid caret");
		std::hint::black_box(transaction.commit());
	});

	assert_within("keystroke @100k blocks", measured, Duration::from_micros(2));
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn enter_and_backspace_in_a_100k_block_document() {
	let (mut doc, block) = large_doc();
	let paragraph = doc.schema().default_text_block();

	let measured = median_per_call(50, || {
		let mut transaction = doc.transaction();
		let start = transaction
			.split(Point::new(block, 20), paragraph, Attrs::new())
			.expect("enter");
		transaction.join(block, start.block).expect("backspace");
		std::hint::black_box(transaction.commit());
	});

	assert_within(
		"enter + backspace @100k blocks",
		measured,
		Duration::from_micros(100),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn keystroke_in_a_100kb_paragraph() {
	let mut doc = support::paragraphs(1, &PARAGRAPH.repeat(1_200));
	let block = doc.children(None)[0];

	let measured = median_per_call(1_000, || {
		let mut transaction = doc.transaction();
		transaction
			.insert_text(Point::new(block, 50_000), "a", MarkSet::empty())
			.expect("valid caret");
		std::hint::black_box(transaction.commit());
	});

	assert_within(
		"keystroke in 100 KB paragraph",
		measured,
		Duration::from_micros(5),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn full_walk_of_a_100k_block_document() {
	let (doc, _) = large_doc();

	let measured = median_per_call(3, || {
		std::hint::black_box(doc.text_blocks().map(Block::rev).sum::<u32>());
	});

	assert_within("walk 100k blocks", measured, Duration::from_millis(5));
}
