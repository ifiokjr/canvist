//! Per-keystroke cost with every extension registered, on top of the
//! StarterKit: the extra marks, their input rules, and text style
//! attributes on many runs.
//!
//! Each benchmark runs a pair of keystrokes that leaves the document as it
//! found it, so divide by two for one keystroke. The budget is the
//! StarterKit's: under 5 µs per typed character at 100,000 blocks. Run with
//! `cargo bench -p canvist_extensions --features full --bench extensions`.

use std::sync::Arc;
use std::time::Duration;

use canvist_editor::Editor;
use canvist_editor::Env;
use canvist_editor::Extension;
use canvist_editor::KeyPress;
use canvist_extensions::StarterKit;
use canvist_model::Attrs;
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

/// The StarterKit plus every other extension.
fn extensions() -> Vec<Arc<dyn Extension>> {
	let mut extensions = StarterKit::default().extensions();
	extensions.extend(canvist_extensions::extras());
	extensions
}

/// An editor with every extension over the given paragraphs, with the
/// caret inside the middle one.
fn editor_over(paragraphs: impl FnOnce(&Schema) -> Vec<InlineText>) -> Editor {
	let mut editor = Editor::builder()
		.extensions(extensions())
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

fn fixture(blocks: usize) -> Editor {
	editor_over(|_| {
		(0..blocks)
			.map(|_| InlineText::plain(PARAGRAPH, MarkSet::empty()))
			.collect()
	})
}

/// One paragraph of 2,000 runs alternating highlights and coloured,
/// resized text styles.
fn styled_runs() -> Editor {
	editor_over(|schema| {
		let style = schema.mark_type("textStyle").expect("textStyle");
		let highlight = schema.mark_type("highlight").expect("highlight");
		let attrs = Attrs::new()
			.with("color", "#e8590c")
			.with("fontSize", "18px");
		let marks = [
			MarkSet::from_marks([Mark::with_attrs(style, attrs)]),
			MarkSet::from_marks([Mark::new(highlight)]),
		];
		let mut text = InlineText::new();

		for index in 0..2_000 {
			text.append(InlineText::plain("styled ", marks[index % 2].clone()));
		}

		vec![text]
	})
}

fn env(millis: u64) -> Env<'static> {
	Env::headless(Duration::from_millis(millis))
}

/// Type `text`, then delete it with Backspace, once per iteration.
fn type_and_delete(bencher: Bencher<'_, '_>, mut editor: Editor, text: &'static str) {
	let mut millis = 0;

	bencher.bench_local(move || {
		millis += 1;
		let mut env = env(millis);
		black_box(editor.insert_text(text, &mut env).expect("type"));
		black_box(
			editor
				.handle_key(KeyPress::plain("Backspace"), &mut env)
				.expect("delete"),
		);
	});
}

/// Type a character, then delete it.
#[divan::bench(args = SIZES)]
fn type_character(bencher: Bencher<'_, '_>, blocks: usize) {
	type_and_delete(bencher, fixture(blocks), "a");
}

/// Type `=`, which the highlight rule looks at, then delete it.
#[divan::bench(args = SIZES)]
fn type_equals(bencher: Bencher<'_, '_>, blocks: usize) {
	type_and_delete(bencher, fixture(blocks), "=");
}

/// Type a character among 2,000 styled runs, then delete it.
#[divan::bench]
fn type_among_styled_runs(bencher: Bencher<'_, '_>) {
	type_and_delete(bencher, styled_runs(), "a");
}
