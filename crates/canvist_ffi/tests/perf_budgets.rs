//! Performance budgets for the Rust half of keystroke-to-paint on iOS and
//! Android, recorded in `docs/src/guides/performance.md`. The on-device
//! tests hold the whole path, painting included, to its own budget. These
//! only mean something in release mode:
//!
//! ```sh
//! cargo test --release -p canvist_ffi --test perf_budgets -- --ignored
//! ```
//!
//! `CANVIST_PERF_SLACK` multiplies every budget. CI sets it for noisy shared
//! runners; locally it defaults to 1.

#![cfg(not(debug_assertions))]

use std::fmt::Write;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use canvist_ffi::DocFormat;
use canvist_ffi::Editor;
use canvist_ffi::EditorOptions;
use canvist_ffi::OffsetRange;
use canvist_ffi::testing::FixedPlatformMeasurer;

const PARAGRAPH: &str =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";
const PARAGRAPHS: u32 = 10_000;
const SAMPLES: usize = 31;
/// How much text around the caret an input method reads, in UTF-16 units.
const CONTEXT: u32 = 64;

fn slack() -> f64 {
	let configured = std::env::var("CANVIST_PERF_SLACK").ok();
	configured
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

/// The median of `SAMPLES` timings of `batch` calls to `run`, per call,
/// after one warm-up batch.
fn median_per_call(batch: u32, mut run: impl FnMut()) -> Duration {
	for _ in 0..batch {
		run();
	}

	let mut timings: Vec<Duration> = (0..SAMPLES)
		.map(|_| {
			let start = Instant::now();
			for _ in 0..batch {
				run();
			}

			start.elapsed() / batch
		})
		.collect();
	timings.sort_unstable();
	timings[SAMPLES / 2]
}

/// A phone-sized editor showing the middle of 10k paragraphs, with the
/// caret in the paragraph at the top.
fn editor() -> Arc<Editor> {
	let markdown = (0..PARAGRAPHS).fold(String::new(), |mut markdown, index| {
		writeln!(markdown, "{PARAGRAPH} {index}\n").expect("writing to a String cannot fail");
		markdown
	});
	let editor = Editor::new(
		EditorOptions::starter_kit(),
		Arc::new(FixedPlatformMeasurer::default()),
	)
	.expect("valid options");
	editor.load(DocFormat::Markdown, &markdown).expect("parses");
	editor.set_viewport(360.0, 720.0);
	editor.set_focused(true);
	editor.frame().expect("lays out");

	let block = editor.block_range(editor.text_length() / 2);
	let top = editor.caret_rect(block.start).expect("laid out").y;
	editor.set_scroll(top);
	editor.frame().expect("lays out");
	editor
		.select(block.start + 10, block.start + 10)
		.expect("selects");
	editor.frame().expect("paints");
	editor
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn keystroke_to_frame_at_10k_paragraphs() {
	let editor = editor();
	let mut typing = true;
	let keystroke = median_per_call(200, || {
		typing = !typing;
		let applied = if typing {
			editor.insert_text("x")
		} else {
			editor.delete_backward()
		};

		assert!(applied.expect("edits"));
		let frame = editor.frame().expect("paints");
		assert!(frame.items.is_some(), "the keystroke shows");
	});

	assert_within(
		"type or delete a character and build its frame @10k paragraphs",
		keystroke,
		Duration::from_micros(200),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn composition_to_frame_at_10k_paragraphs() {
	let editor = editor();
	let mut long = false;
	let update = median_per_call(200, || {
		long = !long;
		let marked = if long { "にほんご" } else { "に" };
		let end = marked.encode_utf16().count() as u32;
		editor
			.set_marked_text(marked, OffsetRange { start: end, end })
			.expect("composes");
		editor.frame().expect("paints");
	});

	assert_within(
		"update marked text and build its frame @10k paragraphs",
		update,
		Duration::from_micros(200),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn text_input_queries_at_10k_paragraphs() {
	let editor = editor();
	let queries = median_per_call(1_000, || {
		let selection = editor.selection();
		std::hint::black_box(editor.marked_range());
		std::hint::black_box(editor.caret_rect(selection.head));
		std::hint::black_box(editor.text_in(OffsetRange {
			start: selection.head.saturating_sub(CONTEXT),
			end: selection.head + CONTEXT,
		}));
	});

	assert_within(
		"answer a keystroke's text input queries @10k paragraphs",
		queries,
		Duration::from_micros(10),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn scrolled_frame_at_10k_paragraphs() {
	let editor = editor();
	let top = editor
		.caret_rect(editor.selection().head)
		.expect("laid out")
		.y;
	let mut down = false;
	let frame = median_per_call(200, || {
		down = !down;
		editor.set_scroll(if down { top + 4.0 } else { top });
		editor.frame().expect("paints");
	});

	assert_within(
		"build the frame after scrolling @10k paragraphs",
		frame,
		Duration::from_micros(50),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn remote_keystroke_to_frame_at_10k_paragraphs() {
	let local = editor();
	local.share(1).expect("shares");
	let remote = Editor::new(
		EditorOptions::starter_kit(),
		Arc::new(FixedPlatformMeasurer::default()),
	)
	.expect("valid options");
	remote
		.join(local.encode_state().expect("encodes"), 2)
		.expect("joins");
	let caret = local.selection().head;
	remote.select(caret, caret).expect("selects");
	let mut typing = false;
	let mut next_update = || {
		typing = !typing;

		if typing {
			remote.insert_text("x").expect("types");
		} else {
			remote.delete_backward().expect("deletes");
		}

		remote.take_updates().expect("updates")
	};
	let updates: Vec<Vec<u8>> = (0..(SAMPLES + 1) * 200)
		.flat_map(|_| next_update())
		.collect();
	let mut updates = updates.into_iter();

	let applied = median_per_call(200, || {
		let update = updates.next().expect("one update per call");
		assert!(local.apply_update(update).expect("applies"));
		local.frame().expect("paints");
	});

	assert_within(
		"apply a collaborator's keystroke and build its frame @10k paragraphs",
		applied,
		Duration::from_micros(200),
	);
}
