//! Performance budgets for keystrokes through the whole StarterKit editor:
//! keymaps, input rules, history, and extension fix-ups on top of the
//! model.
//!
//! Each measurement is a pair of keystrokes that leaves the document as it
//! found it, halved to give the cost of one. Budgets come from
//! `docs/src/guides/performance.md` and the M1 roadmap: under 5 µs of editor
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
use canvist_extensions::StarterKitHandles;
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

/// A StarterKit editor over `paragraphs`, with the caret in the middle of
/// the middle one, and the kit's typed handles.
fn editor_over(
	paragraphs: impl FnOnce(&canvist_model::Schema) -> Vec<InlineText>,
) -> (Editor, StarterKitHandles) {
	let mut builder = Editor::builder();
	let kit = StarterKit::add_all(&mut builder);
	let mut editor = builder
		.build_with(|schema, client| {
			let paragraph = schema.default_text_block();
			let texts = paragraphs(&schema);
			let mut doc = Document::empty(schema, client);
			let trees = texts
				.into_iter()
				.enumerate()
				.map(|(index, text)| {
					let id = BlockId::new(2, u32::try_from(index).expect("fits"));
					BlockTree::text(id, paragraph, text)
				})
				.collect();
			doc.transaction().insert_blocks(None, 0, trees)?;
			Ok(doc)
		})
		.expect("the editor builds");
	let blocks = editor.state().doc().children(None);
	let middle = blocks[blocks.len() / 2];
	let offset = editor
		.state()
		.doc()
		.inline(middle)
		.map_or(0, |text| text.len() / 2);
	let caret = Selection::caret(Point::new(middle, offset - offset % 10));
	editor
		.select(caret, &mut Env::headless(Duration::ZERO))
		.expect("select");

	(editor, kit)
}

/// 100,000 paragraphs.
fn large_editor() -> Editor {
	editor_over(|_| {
		(0..LARGE_DOC)
			.map(|_| InlineText::plain(PARAGRAPH, MarkSet::empty()))
			.collect()
	})
	.0
}

/// One paragraph of about 100 KB.
fn long_paragraph() -> Editor {
	editor_over(|_| {
		vec![InlineText::plain(
			&PARAGRAPH.repeat(1_200),
			MarkSet::empty(),
		)]
	})
	.0
}

/// One paragraph of 2,000 runs cycling through bold, italic, code, and
/// plain text.
fn heavy_formatting() -> Editor {
	editor_over(|schema| {
		let marks: Vec<MarkSet> = ["bold", "italic", "code"]
			.iter()
			.filter_map(|name| schema.mark_type(name))
			.map(|kind| MarkSet::from_marks([canvist_model::Mark::new(kind)]))
			.chain([MarkSet::empty()])
			.collect();
		let mut text = InlineText::new();

		for index in 0..2_000 {
			text.append(InlineText::plain(
				"formatted ",
				marks[index % marks.len()].clone(),
			));
		}

		vec![text]
	})
	.0
}

/// Measure a pair of keystrokes, returning the cost of one.
fn per_keystroke(batch: u32, pair: impl FnMut(&mut Editor, &mut Env<'_>)) -> Duration {
	per_keystroke_in(large_editor(), batch, pair)
}

/// Measure a pair of keystrokes in `editor`, returning the cost of one.
fn per_keystroke_in(
	mut editor: Editor,
	batch: u32,
	mut pair: impl FnMut(&mut Editor, &mut Env<'_>),
) -> Duration {
	let mut millis = 0;

	median_per_call(batch, || {
		millis += 1;
		let mut env = Env::headless(Duration::from_millis(millis));
		pair(&mut editor, &mut env);
	}) / 2
}

fn press(editor: &mut Editor, key: &str, env: &mut Env<'_>) {
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

fn type_and_delete(text: &'static str) -> impl FnMut(&mut Editor, &mut Env<'_>) {
	move |editor, env| {
		std::hint::black_box(editor.insert_text(text, env).expect("type"));
		press(editor, "Backspace", env);
	}
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn typing_in_a_100kb_paragraph() {
	let letter = per_keystroke_in(long_paragraph(), 1_000, type_and_delete("a"));
	let space = per_keystroke_in(long_paragraph(), 1_000, type_and_delete(" "));
	let star = per_keystroke_in(long_paragraph(), 1_000, type_and_delete("*"));

	assert_within(
		"editor keystroke in a 100 KB paragraph",
		letter,
		Duration::from_micros(5),
	);
	assert_within(
		"editor space in a 100 KB paragraph",
		space,
		Duration::from_micros(5),
	);
	assert_within(
		"editor `*` (mark rules) in a 100 KB paragraph",
		star,
		Duration::from_micros(5),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn typing_in_heavy_formatting() {
	let measured = per_keystroke_in(heavy_formatting(), 1_000, type_and_delete("a"));

	assert_within(
		"editor keystroke among 2,000 formatting runs",
		measured,
		Duration::from_micros(5),
	);
}

/// A dry run applies the command and rolls it back exactly, so it costs
/// what the command costs: microseconds for marks and block types, and
/// `O(siblings)` for list wrapping at the root, like the edit itself.
fn per_can_check(commands: &[canvist_editor::Command]) -> Duration {
	let (mut editor, _) = editor_over(|_| {
		(0..LARGE_DOC)
			.map(|_| InlineText::plain(PARAGRAPH, MarkSet::empty()))
			.collect()
	});
	let mut env = Env::headless(Duration::ZERO);

	median_per_call(50, || {
		for command in commands {
			std::hint::black_box(editor.can(command, &mut env).expect("can"));
		}
	}) / commands.len() as u32
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn can_checks_for_a_toolbar_in_a_100k_block_document() {
	let mut builder = Editor::builder();
	let kit = StarterKit::add_all(&mut builder);
	drop(builder);
	let inline = [
		kit.bold.toggle(),
		kit.italic.toggle(),
		kit.heading.toggle(canvist_extensions::HeadingLevel::H2),
		kit.code_block.toggle(None),
	];
	let structural = [kit.bullet_list.toggle(), kit.blockquote.toggle()];

	assert_within(
		"can() for marks and block types @100k blocks",
		per_can_check(&inline),
		Duration::from_micros(5),
	);
	assert_within(
		"can() for wrapping @100k root blocks",
		per_can_check(&structural),
		Duration::from_micros(200),
	);
}
