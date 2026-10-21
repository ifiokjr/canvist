//! Per-keystroke cost of the full editor: StarterKit keymaps, input rules,
//! history, and extension fix-ups on top of the model.
//!
//! Each benchmark runs a pair of keystrokes that leaves the document as it
//! found it (type then Backspace, Enter then Backspace, left then right), so
//! the numbers stay comparable however many iterations run. Divide by two
//! for one keystroke. The budget is under 5 µs per typed character and under
//! 100 µs per structural edit at 100,000 blocks. Run with
//! `cargo bench -p canvist_extensions`.

use std::time::Duration;

use canvist_core::Editor;
use canvist_core::Env;
use canvist_core::KeyPress;
use canvist_core::commands;
use canvist_extensions::StarterKit;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Selection;
use divan::Bencher;
use divan::black_box;

const SIZES: [usize; 3] = [100, 10_000, 100_000];
const PARAGRAPH: &str =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";

fn main() {
	divan::main();
}

/// A StarterKit editor over `blocks` paragraphs, with the caret inside the
/// middle one.
fn fixture(blocks: usize) -> Editor {
	editor_over(|_| {
		(0..blocks)
			.map(|_| InlineText::plain(PARAGRAPH, MarkSet::empty()))
			.collect()
	})
}

/// One paragraph of about 100 KB.
fn long_paragraph() -> Editor {
	editor_over(|_| {
		vec![InlineText::plain(
			&PARAGRAPH.repeat(1_200),
			MarkSet::empty(),
		)]
	})
}

/// One paragraph of 2,000 short runs cycling through bold, italic, code,
/// and plain text.
fn heavy_formatting() -> Editor {
	editor_over(|schema| {
		let names = ["bold", "italic", "code"];
		let marks: Vec<MarkSet> = names
			.iter()
			.filter_map(|name| schema.mark_type(name))
			.map(|kind| MarkSet::from_marks([Mark::new(kind)]))
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
}

/// A StarterKit editor over the given paragraphs, with the caret inside
/// the middle one.
fn editor_over(paragraphs: impl FnOnce(&Schema) -> Vec<InlineText>) -> Editor {
	let mut editor = Editor::builder()
		.extensions(StarterKit::default().extensions())
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
		.expect("fixture builds");
	let blocks = editor.state().doc().children(None);
	let middle = blocks[blocks.len() / 2];
	let offset = editor
		.state()
		.doc()
		.inline(middle)
		.map_or(0, |text| text.len() / 2);
	let caret = Selection::caret(Point::new(middle, offset - offset % 10));
	editor.select(caret, &mut env(0)).expect("select");
	editor
}

fn env(millis: u64) -> Env<'static> {
	Env::headless(Duration::from_millis(millis))
}

/// Type a character, then delete it with Backspace.
#[divan::bench(args = SIZES)]
fn type_and_backspace(bencher: Bencher<'_, '_>, blocks: usize) {
	let mut editor = fixture(blocks);
	let mut now = 0;
	bencher.bench_local(|| {
		now += 1;
		black_box(
			editor
				.insert_text(black_box("a"), &mut env(now))
				.expect("type"),
		);
		black_box(
			editor
				.handle_key(KeyPress::plain("Backspace"), &mut env(now))
				.expect("backspace"),
		);
	});
}

/// Type a space, which runs every input rule, then delete it.
#[divan::bench(args = SIZES)]
fn space_and_backspace(bencher: Bencher<'_, '_>, blocks: usize) {
	let mut editor = fixture(blocks);
	let mut now = 0;
	bencher.bench_local(|| {
		now += 1;
		black_box(
			editor
				.insert_text(black_box(" "), &mut env(now))
				.expect("type"),
		);
		black_box(
			editor
				.handle_key(KeyPress::plain("Backspace"), &mut env(now))
				.expect("backspace"),
		);
	});
}

/// Move the caret left, then right.
#[divan::bench(args = SIZES)]
fn arrow_left_and_right(bencher: Bencher<'_, '_>, blocks: usize) {
	let mut editor = fixture(blocks);
	bencher.bench_local(|| {
		black_box(
			editor
				.handle_key(KeyPress::plain("ArrowLeft"), &mut env(0))
				.expect("left"),
		);
		black_box(
			editor
				.handle_key(KeyPress::plain("ArrowRight"), &mut env(0))
				.expect("right"),
		);
	});
}

/// Split the paragraph with Enter, then join it back with Backspace.
#[divan::bench(args = SIZES)]
fn enter_and_backspace(bencher: Bencher<'_, '_>, blocks: usize) {
	let mut editor = fixture(blocks);
	let mut now = 0;
	bencher.bench_local(|| {
		now += 1;
		black_box(
			editor
				.handle_key(KeyPress::plain("Enter"), &mut env(now))
				.expect("enter"),
		);
		black_box(
			editor
				.handle_key(KeyPress::plain("Backspace"), &mut env(now))
				.expect("backspace"),
		);
	});
}

/// The same type-and-delete pair through the bare state commands, without
/// keymaps, rules, history, or fix-ups, to show what the editor layer adds.
#[divan::bench(args = SIZES)]
fn state_commands_only(bencher: Bencher<'_, '_>, blocks: usize) {
	let editor = fixture(blocks);
	let mut state = editor.state().clone();
	let mut lines = canvist_core::ParagraphNavigator;
	bencher.bench_local(|| {
		black_box(commands::insert_text(&mut state, black_box("a")).expect("type"));
		black_box(
			commands::delete_backward(&mut state, canvist_core::Unit::Grapheme, &mut lines)
				.expect("delete"),
		);
	});
}

/// Type and delete a character in the middle of a 100 KB paragraph.
#[divan::bench]
fn type_in_a_long_paragraph(bencher: Bencher<'_, '_>) {
	let mut editor = long_paragraph();
	let mut now = 0;
	bencher.bench_local(|| {
		now += 1;
		black_box(
			editor
				.insert_text(black_box("a"), &mut env(now))
				.expect("type"),
		);
		black_box(
			editor
				.handle_key(KeyPress::plain("Backspace"), &mut env(now))
				.expect("backspace"),
		);
	});
}

/// Type and delete a space, which runs the input rules over the 500 bytes
/// before the caret, in the middle of a 100 KB paragraph.
#[divan::bench]
fn space_in_a_long_paragraph(bencher: Bencher<'_, '_>) {
	let mut editor = long_paragraph();
	let mut now = 0;
	bencher.bench_local(|| {
		now += 1;
		black_box(
			editor
				.insert_text(black_box(" "), &mut env(now))
				.expect("type"),
		);
		black_box(
			editor
				.handle_key(KeyPress::plain("Backspace"), &mut env(now))
				.expect("backspace"),
		);
	});
}

/// Type and delete a character in a paragraph of 2,000 formatting runs.
#[divan::bench]
fn type_in_heavy_formatting(bencher: Bencher<'_, '_>) {
	let mut editor = heavy_formatting();
	let mut now = 0;
	bencher.bench_local(|| {
		now += 1;
		black_box(
			editor
				.insert_text(black_box("a"), &mut env(now))
				.expect("type"),
		);
		black_box(
			editor
				.handle_key(KeyPress::plain("Backspace"), &mut env(now))
				.expect("backspace"),
		);
	});
}

/// Ask whether bold can be toggled over a selected word, as a toolbar
/// does on every selection change. Nothing changes.
#[divan::bench(args = SIZES)]
fn can_toggle_bold(bencher: Bencher<'_, '_>, blocks: usize) {
	let mut builder = Editor::builder();
	let kit = StarterKit::add_all(&mut builder);
	let mut editor = builder
		.build_with(|schema, client| {
			let mut doc = Document::empty(schema.clone(), client);
			let paragraph = schema.default_text_block();
			let trees = (0..blocks)
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
		.expect("fixture builds");
	let middle = editor.state().doc().children(None)[blocks / 2];
	let word = Selection::text(Point::new(middle, 4), Point::new(middle, 9));
	editor.select(word, &mut env(0)).expect("select");
	let toggle = kit.bold.toggle();
	bencher.bench_local(|| black_box(editor.can(&toggle, &mut env(0)).expect("can")));
}
