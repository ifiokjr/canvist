//! Layout hot-path benchmarks, measured with the deterministic
//! [`FixedMeasurer`] so they track layout cost rather than font shaping.
//!
//! Budgets (see `docs/src/guides/performance.md`):
//!
//! - `relayout_keystroke`: under 100 µs.
//! - `open_10k_paragraphs`: under 50 ms to the first laid-out viewport.
//!
//! Run with `cargo bench -p canvist_layout`.

#[path = "../tests/support/mod.rs"]
mod support;

use std::sync::Arc;

use canvist_layout::DisplayOptions;
use canvist_layout::DocumentLayout;
use canvist_layout::FixedMeasurer;
use canvist_layout::LayoutOptions;
use canvist_layout::Theme;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Point;
use divan::Bencher;
use divan::black_box;

/// A paragraph that wraps to four or five lines at 600 px.
///
/// Built with `concat!` so the literal keeps one indentation style.
const PARAGRAPH: &str = concat!(
	"The quick brown fox jumps over the lazy dog, again and again, until the ",
	"line wraps. Typing in the middle of a long paragraph must only relayout ",
	"that paragraph, never the document. Every word here is a cached segment ",
	"after the first viewport is measured.",
);
/// Paragraphs in the benchmark document.
const PARAGRAPH_COUNT: usize = 10_000;
/// Document sizes for the benches whose cost must not grow with the document.
const SIZES: [usize; 3] = [100, 10_000, 100_000];
/// The paragraph edited by the relayout benches; it sits on the first screen.
const EDITED_PARAGRAPH: usize = 3;
/// The byte offset, inside the ASCII paragraph, where the relayout benches
/// type or split.
const EDIT_OFFSET: u32 = 40;
const WIDTH: f32 = 600.0;
const SCREEN: f32 = 900.0;
/// A prime step, so successive hit-test points scatter across the screen
/// instead of walking a regular grid.
const HIT_TEST_STEP: u32 = 7919;

fn main() {
	divan::main();
}

fn options() -> LayoutOptions {
	LayoutOptions::new(WIDTH).overscan(SCREEN)
}

/// A 10,000-paragraph document with its first screen laid out, and the id
/// of a paragraph on that screen.
fn opened() -> (Document, DocumentLayout, FixedMeasurer, BlockId) {
	opened_with(PARAGRAPH_COUNT)
}

/// A document of `paragraphs` paragraphs with its first screen laid out,
/// and the id of a paragraph on that screen.
fn opened_with(paragraphs: usize) -> (Document, DocumentLayout, FixedMeasurer, BlockId) {
	let doc = support::large(paragraphs, PARAGRAPH);
	let mut measurer = FixedMeasurer::new();
	let mut layout = DocumentLayout::new(&doc, Arc::new(Theme::starter_kit()), options());
	layout
		.ensure(&doc, &mut measurer, 0.0, SCREEN)
		.expect("measured");
	let block = doc.children(None)[EDITED_PARAGRAPH];
	(doc, layout, measurer, block)
}

/// Type or delete one character in a visible paragraph, then relayout.
#[divan::bench(args = SIZES)]
fn relayout_keystroke(bencher: Bencher<'_, '_>, paragraphs: usize) {
	let (mut doc, mut layout, mut measurer, block) = opened_with(paragraphs);
	let mut typed = false;

	bencher.bench_local(|| {
		let mut transaction = doc.transaction();

		if typed {
			transaction
				.delete_text(block, EDIT_OFFSET..EDIT_OFFSET + 1)
				.expect("valid range");
		} else {
			transaction
				.insert_text(Point::new(block, EDIT_OFFSET), "x", MarkSet::empty())
				.expect("valid point");
		}

		typed = !typed;
		let commit = transaction.commit();
		layout.apply(&doc, &commit);
		black_box(
			layout
				.ensure(&doc, &mut measurer, 0.0, SCREEN)
				.expect("measured"),
		);
	});
}

/// Press Enter in a visible paragraph (a structural commit), then relayout.
/// The paragraph is near the top, so nearly every row shifts.
#[divan::bench(args = SIZES)]
fn relayout_enter(bencher: Bencher<'_, '_>, paragraphs: usize) {
	let (mut doc, mut layout, mut measurer, block) = opened_with(paragraphs);
	let paragraph = doc.schema().default_text_block();
	let mut split: Option<BlockId> = None;

	bencher.bench_local(|| {
		let mut transaction = doc.transaction();

		if let Some(next) = split.take() {
			transaction.join(block, next).expect("joinable");
		} else {
			let start = transaction
				.split(Point::new(block, EDIT_OFFSET), paragraph, Attrs::new())
				.expect("valid split");
			split = Some(start.block);
		}

		let commit = transaction.commit();
		layout.apply(&doc, &commit);
		black_box(
			layout
				.ensure(&doc, &mut measurer, 0.0, SCREEN)
				.expect("measured"),
		);
	});
}

/// Open a document: flatten it and lay out the first screen plus overscan,
/// with a cold measurement cache.
#[divan::bench(args = SIZES)]
fn open_document(bencher: Bencher<'_, '_>, paragraphs: usize) {
	let doc = support::large(paragraphs, PARAGRAPH);
	let theme = Arc::new(Theme::starter_kit());

	bencher.bench_local(|| {
		let mut measurer = FixedMeasurer::new();
		let mut layout = DocumentLayout::new(&doc, theme.clone(), options());
		black_box(
			layout
				.ensure(&doc, &mut measurer, 0.0, SCREEN)
				.expect("measured"),
		);
		layout
	});
}

/// Scroll down by one screen into unmeasured blocks.
#[divan::bench]
fn scroll_one_screen(bencher: Bencher<'_, '_>) {
	let (doc, layout, ..) = opened();

	bencher
		.with_inputs(|| (layout.clone(), FixedMeasurer::new()))
		.bench_local_values(|(mut layout, mut measurer)| {
			black_box(
				layout
					.ensure(&doc, &mut measurer, SCREEN, 2.0 * SCREEN)
					.expect("measured"),
			);
			layout
		});
}

/// Hit-test points across the first screen.
#[divan::bench]
fn hit_test(bencher: Bencher<'_, '_>) {
	let (_, layout, ..) = opened();
	let mut step = 0u32;

	bencher.bench_local(|| {
		step = step.wrapping_add(HIT_TEST_STEP);
		let x = (step % WIDTH as u32) as f32;
		let y = (step % SCREEN as u32) as f32;
		black_box(layout.hit_test(x, y))
	});
}

/// Build the display list for one screen.
#[divan::bench]
fn display_list_viewport(bencher: Bencher<'_, '_>) {
	let (_, layout, ..) = opened();
	let options = DisplayOptions::viewport(0.0, SCREEN);

	bencher.bench_local(|| black_box(layout.display_list(&options)));
}

/// Paragraph lengths for the long-paragraph bench, in repeats of
/// [`PARAGRAPH`] (about 250 bytes each): one, about 10 KB, and about 100 KB.
const REPEATS: [usize; 3] = [1, 40, 400];

/// Type or delete one character in the middle of one long paragraph.
#[divan::bench(args = REPEATS)]
fn relayout_keystroke_long_paragraph(bencher: Bencher<'_, '_>, repeats: usize) {
	let doc = support::large(1, &PARAGRAPH.repeat(repeats));
	let block = doc.children(None)[0];
	let mut measurer = FixedMeasurer::new();
	let mut layout = DocumentLayout::new(&doc, Arc::new(Theme::starter_kit()), options());
	let mut doc = doc;
	let offset = (PARAGRAPH.len() * repeats / 2) as u32;
	let mut typed = false;

	layout
		.ensure(&doc, &mut measurer, 0.0, SCREEN)
		.expect("measured");

	bencher.bench_local(|| {
		let mut transaction = doc.transaction();

		if typed {
			transaction
				.delete_text(block, offset..offset + 1)
				.expect("valid range");
		} else {
			transaction
				.insert_text(Point::new(block, offset), "x", MarkSet::empty())
				.expect("valid point");
		}

		typed = !typed;
		let commit = transaction.commit();
		layout.apply(&doc, &commit);
		black_box(
			layout
				.ensure(&doc, &mut measurer, 0.0, SCREEN)
				.expect("measured"),
		);
	});
}

/// Alternating formatting runs in the heavily formatted paragraph.
const FORMATTED_RUNS: usize = 500;
/// Bytes per formatting run.
const RUN_LENGTH: usize = 6;

/// Type or delete one character in a paragraph of 500 alternating bold
/// runs, the worst case for style resolution and itemizing.
#[divan::bench]
fn relayout_keystroke_heavy_formatting(bencher: Bencher<'_, '_>) {
	let mut doc = support::large(1, &"word, ".repeat(FORMATTED_RUNS));
	let block = doc.children(None)[0];
	let bold = Mark::new(
		doc.schema()
			.mark_type("bold")
			.expect("the support schema has bold"),
	);
	let mut transaction = doc.transaction();

	for run in (0..FORMATTED_RUNS).step_by(2) {
		let start = (run * RUN_LENGTH) as u32;
		transaction
			.add_mark(block, start..start + RUN_LENGTH as u32, bold.clone())
			.expect("valid range");
	}

	drop(transaction.commit());

	let mut measurer = FixedMeasurer::new();
	let mut layout = DocumentLayout::new(&doc, Arc::new(Theme::starter_kit()), options());
	let middle = (FORMATTED_RUNS * RUN_LENGTH / 2) as u32;
	let mut typed = false;

	layout
		.ensure(&doc, &mut measurer, 0.0, SCREEN)
		.expect("measured");

	bencher.bench_local(|| {
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
		black_box(
			layout
				.ensure(&doc, &mut measurer, 0.0, SCREEN)
				.expect("measured"),
		);
	});
}
