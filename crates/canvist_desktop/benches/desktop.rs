//! Desktop editor benchmarks: a keystroke's whole frame (the edit, layout,
//! the display list, and painting), opening a document, scrolling, and the
//! accessibility tree, at 1k and 10k paragraphs. Run with
//! `cargo bench -p canvist_desktop`.

use std::time::Duration;

use canvist_desktop::Context;
use canvist_desktop::DesktopEditor;
use canvist_desktop::EditorOptions;
use canvist_desktop::FontSource;
use canvist_desktop::InputEvent;
use canvist_desktop::KeyInput;
use canvist_desktop::MemoryClipboard;
use canvist_desktop::Modifiers;
use canvist_desktop::ScrollDelta;
use canvist_desktop::Viewport;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::canvist_editor::Platform;
use canvist_paint::Recorder;
use divan::Bencher;
use divan::black_box;
use kurbo::Affine;

/// Paragraph counts.
const SIZES: [usize; 2] = [1_000, 10_000];

fn main() {
	divan::main();
}

fn markdown(paragraphs: usize) -> String {
	(0..paragraphs)
		.map(|index| {
			format!(
				"Paragraph {index} with **bold** and *italic* words that wrap in a narrow window."
			)
		})
		.collect::<Vec<_>>()
		.join("\n\n")
}

/// A focused editor holding `paragraphs`, with its first frame drawn.
fn editor(paragraphs: usize) -> DesktopEditor {
	let options = EditorOptions::new()
		.fonts(FontSource::Bundled)
		.caret_blink(Duration::ZERO)
		.platform(Platform::Other);
	let mut editor = DesktopEditor::new(Engine::starter_kit(), options).expect("editor");
	editor
		.load(Format::Markdown, &markdown(paragraphs))
		.expect("markdown");
	editor.resize(Viewport {
		width: 900.0,
		height: 700.0,
		scale: 2.0,
	});
	send(&mut editor, &InputEvent::Focus(true));
	editor.frame(Duration::ZERO).expect("frame");
	editor
}

fn send(editor: &mut DesktopEditor, event: &InputEvent) {
	let mut clipboard = MemoryClipboard::new();
	let mut cx = Context::new(Duration::ZERO, &mut clipboard);
	let _ = editor.handle(event, &mut cx).expect("event");
}

/// One typed key and its frame, painted into a recorder: everything but the
/// backend's own encoding.
#[divan::bench(args = SIZES)]
fn keystroke_to_frame(bencher: Bencher<'_, '_>, paragraphs: usize) {
	let mut editor = editor(paragraphs);
	let key = InputEvent::Key(KeyInput::text("a", Modifiers::NONE));

	bencher.bench_local(|| {
		send(&mut editor, &key);
		editor.frame(Duration::ZERO).expect("frame");
		let mut recorder = Recorder::new();
		editor.paint(&mut recorder, Affine::IDENTITY);
		black_box(recorder.ops.len())
	});
}

/// Opening a document until its first frame is ready.
#[divan::bench(args = SIZES)]
fn open_to_first_frame(bencher: Bencher<'_, '_>, paragraphs: usize) {
	let source = markdown(paragraphs);
	let mut editor = editor(1);

	bencher.bench_local(|| {
		editor.load(Format::Markdown, &source).expect("markdown");
		black_box(editor.frame(Duration::ZERO).expect("frame"))
	});
}

/// Scrolling a screen down and drawing the frame, starting from the top
/// each time.
#[divan::bench(args = SIZES)]
fn scroll_a_screen(bencher: Bencher<'_, '_>, paragraphs: usize) {
	let mut editor = editor(paragraphs);
	let down = InputEvent::Scroll(ScrollDelta::Pixels { x: 0.0, y: -700.0 });

	bencher.bench_local(|| {
		editor.scroll_to(0.0);
		send(&mut editor, &down);
		black_box(editor.frame(Duration::ZERO).expect("frame"))
	});
}

/// The accessibility tree of one screen.
#[divan::bench(args = SIZES)]
fn accessibility_tree(bencher: Bencher<'_, '_>, paragraphs: usize) {
	let mut editor = editor(paragraphs);

	bencher.bench_local(|| {
		black_box(
			editor
				.accessibility_tree("Notes", Affine::IDENTITY)
				.nodes
				.len(),
		)
	});
}

/// A frame rasterised on the CPU at 2x: screenshots and thumbnails.
#[divan::bench(sample_count = 20)]
fn render_png(bencher: Bencher<'_, '_>) {
	let editor = editor(100);

	bencher.bench_local(|| black_box(editor.render_rgba8().expect("pixels").2.len()));
}
