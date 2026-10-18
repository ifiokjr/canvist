//! Performance budgets for keystrokes through the whole StarterKit editor:
//! keymaps, input rules, history, and extension fix-ups on top of the
//! model.
//!
//! Each measurement is a pair of keystrokes that leaves the document as it
//! found it, halved to give the cost of one. Budgets come from
//! `docs/src/v2/architecture.md` and the M1 roadmap: under 5 µs of editor
//! work per typed character and under 100 µs per structural edit, at
//! 100,000 blocks. They only mean something in release mode:
//!
//! ```sh
//! cargo test --release -p canvist_extensions --test perf_budgets -- --ignored
//! ```
//!
//! `CANVIST_PERF_SLACK` multiplies every budget; CI sets it for noisy shared
//! runners. The file compiles to nothing in debug builds.

#![cfg(not(debug_assertions))]

use std::time::Duration;
use std::time::Instant;

use canvist_editor::Editor;
use canvist_editor::Env;
use canvist_editor::KeyPress;
use canvist_extensions::StarterKit;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Selection;

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

/// The median time of one call to `run`, over [`SAMPLES`] batches of `batch`
/// calls after one warm-up batch.
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

/// A StarterKit editor over 100,000 paragraphs with the caret inside the
/// middle one.
fn large_editor() -> Editor {
	let mut editor = Editor::builder()
		.extensions(StarterKit::default().extensions())
		.build_with(|schema, client| {
			let paragraph = schema.default_text_block();
			let mut doc = Document::empty(schema, client);
			let trees = (0..LARGE_DOC)
				.map(|index| {
					let id = BlockId::new(2, u32::try_from(index).expect("fits"));
					BlockTree::text(
						id,
						paragraph,
						InlineText::plain(PARAGRAPH, MarkSet::empty()),
					)
				})
				.collect();
			doc.transaction().insert_blocks(None, 0, trees)?;
			Ok(doc)
		})
		.expect("the editor builds");
	let middle = editor.state().doc().children(None)[LARGE_DOC / 2];
	let caret = Selection::caret(Point::new(middle, 10));
	editor
		.select(caret, &Env::headless(Duration::ZERO))
		.expect("select");
	editor
}

/// Measure a pair of keystrokes, returning the cost of one.
fn per_keystroke(batch: u32, mut pair: impl FnMut(&mut Editor, &Env<'_>)) -> Duration {
	let mut editor = large_editor();
	let mut millis = 0;

	median_per_call(batch, || {
		millis += 1;
		let env = Env::headless(Duration::from_millis(millis));
		pair(&mut editor, &env);
	}) / 2
}

fn press(editor: &mut Editor, key: &str, env: &Env<'_>) {
	std::hint::black_box(editor.handle_key(KeyPress::plain(key), env).expect("key"));
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn typing_a_character_in_a_100k_block_document() {
	let measured = per_keystroke(1_000, |editor, env| {
		std::hint::black_box(editor.insert_text("a", env).expect("type"));
		press(editor, "Backspace", env);
	});

	assert_within(
		"editor keystroke @100k blocks",
		measured,
		Duration::from_micros(5),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn typing_a_space_runs_every_input_rule_within_budget() {
	let measured = per_keystroke(1_000, |editor, env| {
		std::hint::black_box(editor.insert_text(" ", env).expect("type"));
		press(editor, "Backspace", env);
	});

	assert_within(
		"editor space with input rules @100k blocks",
		measured,
		Duration::from_micros(5),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn moving_the_caret_in_a_100k_block_document() {
	let measured = per_keystroke(1_000, |editor, env| {
		press(editor, "ArrowLeft", env);
		press(editor, "ArrowRight", env);
	});

	assert_within(
		"editor arrow key @100k blocks",
		measured,
		Duration::from_micros(5),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn enter_and_backspace_in_a_100k_block_document() {
	let measured = per_keystroke(50, |editor, env| {
		press(editor, "Enter", env);
		press(editor, "Backspace", env);
	});

	assert_within(
		"editor enter or backspace @100k blocks",
		measured,
		Duration::from_micros(100),
	);
}
