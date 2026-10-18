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

use canvist_editor::Editor;
use canvist_editor::Env;
use canvist_editor::KeyPress;
use canvist_editor::commands;
use canvist_extensions::StarterKit;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::Point;
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
	let mut editor = Editor::builder()
		.extensions(StarterKit::default().extensions())
		.build_with(|schema, client| {
			let paragraph = schema.default_text_block();
			let mut doc = Document::empty(schema, client);
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
	let caret = Selection::caret(Point::new(middle, 10));
	editor.select(caret, &env(0)).expect("select");
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
		black_box(editor.insert_text(black_box("a"), &env(now)).expect("type"));
		black_box(
			editor
				.handle_key(KeyPress::plain("Backspace"), &env(now))
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
		black_box(editor.insert_text(black_box(" "), &env(now)).expect("type"));
		black_box(
			editor
				.handle_key(KeyPress::plain("Backspace"), &env(now))
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
				.handle_key(KeyPress::plain("ArrowLeft"), &env(0))
				.expect("left"),
		);
		black_box(
			editor
				.handle_key(KeyPress::plain("ArrowRight"), &env(0))
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
				.handle_key(KeyPress::plain("Enter"), &env(now))
				.expect("enter"),
		);
		black_box(
			editor
				.handle_key(KeyPress::plain("Backspace"), &env(now))
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
	let lines = canvist_editor::ParagraphNavigator;
	bencher.bench_local(|| {
		black_box(commands::insert_text(&mut state, black_box("a")).expect("type"));
		black_box(
			commands::delete_backward(&mut state, canvist_editor::Unit::Grapheme, &lines)
				.expect("delete"),
		);
	});
}
