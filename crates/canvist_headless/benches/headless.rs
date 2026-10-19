//! Headless hot paths, through the public API a server uses.
//!
//! - `load_markdown`, `load_json`: parse a document of N paragraphs.
//! - `export`: write it as JSON, Markdown, or HTML.
//! - `create`: an empty StarterKit document, which builds its editor.
//! - `run_command`: toggle bold at a caret, through a typed handle.
//! - `apply_update`: integrate a peer's keystroke (typed, then deleted, so
//!   the paragraph keeps its length).
//!
//! Run with `cargo bench -p canvist_headless --all-features`.

use std::fmt::Write;

use canvist_extensions::StarterKitHandles;
use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::Position;
use divan::Bencher;
use divan::black_box;

const SIZES: [u32; 2] = [100, 10_000];
const PARAGRAPH: &str =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";

fn main() {
	divan::main();
}

fn markdown(paragraphs: u32) -> String {
	(0..paragraphs).fold(String::new(), |mut markdown, index| {
		writeln!(markdown, "{PARAGRAPH} {index}\n").expect("writing to a String cannot fail");
		markdown
	})
}

fn doc(engine: &Engine<StarterKitHandles>, paragraphs: u32) -> Doc {
	engine
		.parse(Format::Markdown, &markdown(paragraphs))
		.expect("parses")
}

fn caret(paragraphs: u32) -> Position {
	Position::new([paragraphs / 2], 10)
}

#[divan::bench(args = SIZES, sample_count = 20)]
fn load_markdown(bencher: Bencher<'_, '_>, paragraphs: u32) {
	let engine = Engine::starter_kit();
	let input = markdown(paragraphs);
	bencher.bench(|| engine.parse(Format::Markdown, black_box(&input)));
}

#[divan::bench(args = SIZES, sample_count = 20)]
fn load_json(bencher: Bencher<'_, '_>, paragraphs: u32) {
	let engine = Engine::starter_kit();
	let input = doc(&engine, paragraphs).export(Format::Json);
	bencher.bench(|| engine.parse(Format::Json, black_box(&input)));
}

#[divan::bench(args = [Format::Json, Format::Markdown, Format::Html], sample_count = 20)]
fn export(bencher: Bencher<'_, '_>, format: Format) {
	let doc = doc(&Engine::starter_kit(), 10_000);
	bencher.bench_local(|| doc.export(black_box(format)));
}

#[divan::bench]
fn create(bencher: Bencher<'_, '_>) {
	let engine = Engine::starter_kit();
	bencher.bench(|| engine.create());
}

#[divan::bench(args = SIZES)]
fn run_command(bencher: Bencher<'_, '_>, paragraphs: u32) {
	let engine = Engine::starter_kit();
	let toggle = engine.handles().bold.toggle();
	let mut doc = doc(&engine, paragraphs);
	doc.select_range(&caret(paragraphs), &caret(paragraphs))
		.expect("selects");
	bencher.bench_local(|| doc.run(black_box(&toggle)));
}

#[cfg(feature = "collab")]
#[divan::bench(args = SIZES)]
fn apply_update(bencher: Bencher<'_, '_>, paragraphs: u32) {
	let engine = Engine::starter_kit();
	let mut server = doc(&engine, paragraphs);
	server.share(1).expect("shares");
	let mut peer = engine
		.clone()
		.with_client(2)
		.from_yjs(&server.encode_state().expect("encodes"), 2)
		.expect("joins");
	let server = std::cell::RefCell::new(server);
	let mut pending = Vec::new();

	bencher
		.with_inputs(|| {
			if pending.is_empty() {
				peer.select_range(&caret(paragraphs), &caret(paragraphs))
					.expect("selects");
				peer.insert_text("y").expect("types");
				peer.press("Backspace").expect("deletes");
				pending = peer.take_updates().expect("shared");
				pending.reverse();
			}

			pending.pop().expect("an update")
		})
		.bench_local_values(|update| server.borrow_mut().apply_update(&update));
}
