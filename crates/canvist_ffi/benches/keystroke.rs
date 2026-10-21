//! The Rust half of keystroke-to-paint on iOS and Android: everything
//! between the platform delivering a key and painting the frame, through
//! the same `Editor` calls the views make, with a fixed-width measurer in
//! place of CoreText and `Paint`. The on-device tests (Swift and Kotlin)
//! measure the whole path, painting included.
//!
//! - `keystroke`: type a character mid-document, then the frame that paints
//!   it (the display list of a phone-sized viewport and its damage), then
//!   Backspace and its frame, so the document keeps its size. Two
//!   keystrokes per iteration.
//! - `composition`: an IME update of the marked text, then its frame.
//! - `text_input_queries`: what `UITextInput` asks after every keystroke:
//!   the selection, the marked range, the caret rectangle, and the text
//!   around the caret.
//! - `scroll`: a frame after scrolling a few points.
//! - `remote_keystroke`: a collaborator's keystroke (typing and deleting
//!   in turn) as a Yjs update, then its frame.
//!
//! Run with `cargo bench -p canvist_ffi`.

use std::fmt::Write;
use std::sync::Arc;

use canvist_ffi::DocFormat;
use canvist_ffi::Editor;
use canvist_ffi::EditorOptions;
use canvist_ffi::OffsetRange;
use canvist_ffi::testing::FixedPlatformMeasurer;
use divan::Bencher;
use divan::black_box;

const SIZES: [u32; 3] = [100, 10_000, 100_000];
const PARAGRAPH: &str =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";
const SAMPLES: u32 = 50;
/// How much text around the caret an input method reads, in UTF-16 units.
const CONTEXT: u32 = 64;
/// A phone in portrait, in points.
const VIEWPORT: (f32, f32) = (360.0, 720.0);

fn main() {
	divan::main();
}

fn markdown(paragraphs: u32) -> String {
	(0..paragraphs).fold(String::new(), |mut markdown, index| {
		writeln!(markdown, "{PARAGRAPH} {index}\n").expect("writing to a String cannot fail");
		markdown
	})
}

/// An editor showing the middle of a document of `paragraphs`, with the
/// caret ten characters into the paragraph at the top of the viewport.
fn editor(paragraphs: u32) -> Arc<Editor> {
	let editor = Editor::new(
		EditorOptions::starter_kit(),
		Arc::new(FixedPlatformMeasurer::default()),
	)
	.expect("valid options");
	editor
		.load(DocFormat::Markdown, &markdown(paragraphs))
		.expect("parses");
	editor.set_viewport(VIEWPORT.0, VIEWPORT.1);
	editor.set_focused(true);
	editor.frame().expect("lays out");

	let middle = editor.text_length() / 2;
	let block = editor.block_range(middle);
	let top = editor.caret_rect(block.start).expect("laid out").y;
	editor.set_scroll(top);
	editor.frame().expect("lays out");
	editor
		.select(block.start + 10, block.start + 10)
		.expect("selects");
	editor.frame().expect("paints");
	editor
}

#[divan::bench(args = SIZES, sample_count = SAMPLES)]
fn keystroke(bencher: Bencher<'_, '_>, paragraphs: u32) {
	let editor = editor(paragraphs);
	bencher.bench_local(|| {
		editor.insert_text(black_box("x")).expect("types");
		black_box(editor.frame().expect("paints"));
		editor.delete_backward().expect("deletes");
		black_box(editor.frame().expect("paints"));
	});
}

#[divan::bench(args = SIZES, sample_count = SAMPLES)]
fn composition(bencher: Bencher<'_, '_>, paragraphs: u32) {
	let editor = editor(paragraphs);
	let mut long = false;
	bencher.bench_local(|| {
		long = !long;
		let marked = if long { "にほんご" } else { "に" };
		let end = marked.encode_utf16().count() as u32;
		editor
			.set_marked_text(marked, OffsetRange { start: end, end })
			.expect("composes");
		black_box(editor.frame().expect("paints"));
	});
}

#[divan::bench(args = SIZES, sample_count = SAMPLES)]
fn text_input_queries(bencher: Bencher<'_, '_>, paragraphs: u32) {
	let editor = editor(paragraphs);
	bencher.bench_local(|| {
		let selection = editor.selection();
		black_box(editor.marked_range());
		black_box(editor.caret_rect(selection.head));
		black_box(editor.text_in(OffsetRange {
			start: selection.head.saturating_sub(CONTEXT),
			end: selection.head + CONTEXT,
		}));
	});
}

#[divan::bench(args = SIZES, sample_count = SAMPLES)]
fn scroll(bencher: Bencher<'_, '_>, paragraphs: u32) {
	let editor = editor(paragraphs);
	let top = editor
		.caret_rect(editor.selection().head)
		.expect("laid out")
		.y;
	let mut down = false;
	bencher.bench_local(|| {
		down = !down;
		editor.set_scroll(if down { top + 4.0 } else { top });
		black_box(editor.frame().expect("paints"));
	});
}

#[divan::bench(args = SIZES, sample_count = SAMPLES)]
fn remote_keystroke(bencher: Bencher<'_, '_>, paragraphs: u32) {
	let local = editor(paragraphs);
	local.share(1).expect("shares");
	let remote = editor(1);
	remote
		.join(local.encode_state().expect("encodes"), 2)
		.expect("joins");
	let caret = local.selection().head;
	remote.select(caret, caret).expect("selects");
	let mut typing = false;

	bencher
		.with_inputs(|| {
			typing = !typing;
			if typing {
				remote.insert_text("x").expect("types");
			} else {
				remote.delete_backward().expect("deletes");
			}

			remote.take_updates().expect("updates")
		})
		.bench_local_values(|updates| {
			for update in updates {
				local.apply_update(black_box(update)).expect("applies");
			}

			black_box(local.frame().expect("paints"));
		});
}
