//! Performance budgets for layout's hot paths.
//!
//! These tests fail when an operation exceeds the budget recorded in
//! `docs/src/v2/architecture.md`, so a regression breaks CI instead of
//! hiding in a benchmark report. They only mean something in release mode:
//!
//! ```sh
//! cargo test --release -p canvist_layout --test perf_budgets -- --ignored
//! ```
//!
//! `CANVIST_PERF_SLACK` multiplies every budget. CI sets it for noisy shared
//! runners; locally it defaults to 1.
//!
//! Layout is measured with the deterministic [`FixedMeasurer`], so budgets
//! track layout's own cost, not font shaping. The file compiles to nothing
//! in debug builds, where timings are meaningless.

#![cfg(not(debug_assertions))]

mod support;

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use canvist_layout::Caret;
use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::FixedMeasurer;
use canvist_layout::LayoutOptions;
use canvist_layout::Theme;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::MarkSet;
use canvist_model::Point;

/// Samples per measurement; the median is reported.
const SAMPLES: usize = 31;
/// A paragraph that wraps to four or five lines at [`WIDTH`].
const PARAGRAPH: &str = concat!(
	"The quick brown fox jumps over the lazy dog, again and again, until the ",
	"line wraps. Typing in the middle of a long paragraph must only relayout ",
	"that paragraph, never the document.",
);
const WIDTH: f32 = 600.0;
/// One screen of a desktop window.
const SCREEN: f32 = 900.0;
const MEDIUM_DOC: usize = 10_000;
const LARGE_DOC: usize = 100_000;
/// The paragraph edited by the relayout budgets; it is on the first screen,
/// so a structural edit shifts nearly every row after it.
const EDITED: usize = 3;
const EDIT_OFFSET: u32 = 40;

/// Relaying out after a keystroke, from the architecture's budget table.
const KEYSTROKE_BUDGET: Duration = Duration::from_micros(100);
/// Enter splices one row in and shifts the indices of every row after it:
/// linear memory moves with no hashing, styling, or measuring. These
/// budgets record that cost at 10k and 100k paragraphs.
const ENTER_BUDGET_MEDIUM: Duration = Duration::from_micros(200);
const ENTER_BUDGET_LARGE: Duration = Duration::from_millis(2);
/// Opening a document to its first screen, from the budget table.
const FIRST_VIEWPORT_BUDGET: Duration = Duration::from_millis(50);
/// Scrolling one fresh screen in, a sixteenth of a 60 Hz frame.
const SCROLL_BUDGET: Duration = Duration::from_millis(1);
/// Pointer and caret queries run several times per frame.
const HIT_TEST_BUDGET: Duration = Duration::from_micros(5);
const CARET_RECT_BUDGET: Duration = Duration::from_micros(2);
/// Building a screen's display list, well under a millisecond so painting
/// keeps the frame.
const DISPLAY_LIST_BUDGET: Duration = Duration::from_micros(100);

/// Repeats of [`PARAGRAPH`] that make one paragraph of about 100 KB.
const LONG_PARAGRAPH_REPEATS: usize = 600;

/// Calls per sample, so every sample runs long enough to time.
const KEYSTROKE_BATCH: u32 = 200;
const ENTER_BATCH: u32 = 20;
const OPEN_BATCH: u32 = 3;
const SCROLL_BATCH: u32 = 10;
const QUERY_BATCH: u32 = 10_000;
const DISPLAY_BATCH: u32 = 1_000;
/// A prime step, so successive hit-test points scatter across the screen.
const HIT_TEST_STEP: u32 = 7919;

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

fn options() -> LayoutOptions {
	LayoutOptions::new(WIDTH).overscan(SCREEN)
}

/// A document of `count` paragraphs with its first screen laid out, and the
/// paragraph the budgets edit.
fn opened(count: usize) -> (Document, DocumentLayout, FixedMeasurer, BlockId) {
	let doc = support::large(count, PARAGRAPH);
	let mut measurer = FixedMeasurer::new();
	let mut layout = DocumentLayout::new(&doc, Arc::new(Theme::starter_kit()), options());
	layout
		.ensure(&doc, &mut measurer, 0.0, SCREEN)
		.expect("measured");
	let block = doc.children(None)[EDITED];

	(doc, layout, measurer, block)
}

/// Type a character, or delete it again, and relay out the screen.
fn keystroke(count: usize) -> Duration {
	let (mut doc, mut layout, mut measurer, block) = opened(count);
	let mut typed = false;

	median_per_call(KEYSTROKE_BATCH, || {
		let mut transaction = doc.transaction();

		if typed {
			let range = EDIT_OFFSET..EDIT_OFFSET + 1;
			transaction.delete_text(block, range).expect("valid range");
		} else {
			let point = Point::new(block, EDIT_OFFSET);
			transaction
				.insert_text(point, "x", MarkSet::empty())
				.expect("valid point");
		}

		typed = !typed;
		let commit = transaction.commit();
		layout.apply(&doc, &commit);
		let update = layout.ensure(&doc, &mut measurer, 0.0, SCREEN);
		std::hint::black_box(update.expect("measured"));
	})
}

/// Press Enter, or Backspace to undo it, and relay out the screen.
fn enter(count: usize) -> Duration {
	let (mut doc, mut layout, mut measurer, block) = opened(count);
	let paragraph = doc.schema().default_text_block();
	let mut split: Option<BlockId> = None;

	median_per_call(ENTER_BATCH, || {
		let mut transaction = doc.transaction();

		if let Some(next) = split.take() {
			transaction.join(block, next).expect("joinable");
		} else {
			let point = Point::new(block, EDIT_OFFSET);
			let start = transaction
				.split(point, paragraph, Attrs::new())
				.expect("valid split");
			split = Some(start.block);
		}

		let commit = transaction.commit();
		layout.apply(&doc, &commit);
		let update = layout.ensure(&doc, &mut measurer, 0.0, SCREEN);
		std::hint::black_box(update.expect("measured"));
	})
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn keystroke_relayout_is_independent_of_document_size() {
	assert_within(
		"keystroke relayout @10k paragraphs",
		keystroke(MEDIUM_DOC),
		KEYSTROKE_BUDGET,
	);
	assert_within(
		"keystroke relayout @100k paragraphs",
		keystroke(LARGE_DOC),
		KEYSTROKE_BUDGET,
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn enter_relayout() {
	assert_within(
		"enter relayout @10k paragraphs",
		enter(MEDIUM_DOC),
		ENTER_BUDGET_MEDIUM,
	);
	assert_within(
		"enter relayout @100k paragraphs",
		enter(LARGE_DOC),
		ENTER_BUDGET_LARGE,
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn first_viewport_of_a_10k_paragraph_document() {
	let doc = support::large(MEDIUM_DOC, PARAGRAPH);
	let theme = Arc::new(Theme::starter_kit());

	let measured = median_per_call(OPEN_BATCH, || {
		let mut measurer = FixedMeasurer::new();
		let mut layout = DocumentLayout::new(&doc, theme.clone(), options());
		let update = layout.ensure(&doc, &mut measurer, 0.0, SCREEN);
		std::hint::black_box(update.expect("measured"));
	});

	assert_within(
		"first viewport @10k paragraphs",
		measured,
		FIRST_VIEWPORT_BUDGET,
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn scroll_by_one_screen() {
	let (doc, mut layout, mut measurer, _) = opened(MEDIUM_DOC);
	let mut top = 0.0;

	// Each call scrolls one more screen into blocks that have only estimated
	// heights, so every sample lays out a fresh screen.
	let measured = median_per_call(SCROLL_BATCH, || {
		top += SCREEN;
		let update = layout.ensure(&doc, &mut measurer, top, top + SCREEN);
		std::hint::black_box(update.expect("measured"));
	});

	assert_within("scroll one screen @10k", measured, SCROLL_BUDGET);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn hit_test_and_caret_rect() {
	let (_, layout, _, block) = opened(MEDIUM_DOC);
	let mut step = 0u32;

	let hit = median_per_call(QUERY_BATCH, || {
		step = step.wrapping_add(HIT_TEST_STEP);
		let point = ((step % WIDTH as u32) as f32, (step % SCREEN as u32) as f32);
		std::hint::black_box(layout.hit_test(point.0, point.1));
	});
	let caret = Caret::new(Point::new(block, EDIT_OFFSET));
	let rect = median_per_call(QUERY_BATCH, || {
		std::hint::black_box(layout.caret_rect(caret));
	});

	assert_within("hit test", hit, HIT_TEST_BUDGET);
	assert_within("caret rect", rect, CARET_RECT_BUDGET);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn display_list_for_one_screen() {
	let (_, layout, ..) = opened(MEDIUM_DOC);
	let options = DisplayOptions::viewport(0.0, SCREEN);

	let measured = median_per_call(DISPLAY_BATCH, || {
		std::hint::black_box(layout.display_list(&options));
	});

	assert_within("display list, one screen", measured, DISPLAY_LIST_BUDGET);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn keystroke_relayout_in_a_100kb_paragraph() {
	let doc = support::large(1, &PARAGRAPH.repeat(LONG_PARAGRAPH_REPEATS));
	let block = doc.children(None)[0];
	let mut measurer = FixedMeasurer::new();
	let mut layout = DocumentLayout::new(&doc, Arc::new(Theme::starter_kit()), options());
	let mut doc = doc;
	let middle = (PARAGRAPH.len() * LONG_PARAGRAPH_REPEATS / 2) as u32;
	let mut typed = false;

	layout
		.ensure(&doc, &mut measurer, 0.0, SCREEN)
		.expect("measured");

	let measured = median_per_call(KEYSTROKE_BATCH, || {
		let mut transaction = doc.transaction();

		if typed {
			transaction
				.delete_text(block, middle..middle + 1)
				.expect("valid range");
		} else {
			transaction
				.insert_text(Point::new(block, middle), "x", MarkSet::empty())
				.expect("valid point");
		}

		typed = !typed;
		let commit = transaction.commit();
		layout.apply(&doc, &commit);
		let update = layout.ensure(&doc, &mut measurer, 0.0, SCREEN);
		std::hint::black_box(update.expect("measured"));
	});

	assert_within(
		"keystroke relayout in a 100 KB paragraph",
		measured,
		KEYSTROKE_BUDGET,
	);
}
