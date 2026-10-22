//! Export work as callgrind instruction counts, through gungraun.
//!
//! The wall-time `export` benches this replaces swung on the Linux
//! `x86_64` runner by tens of percent on changes that never touched the
//! export path: the whole workspace links with one codegen unit and LTO,
//! so any code change can move the export hot loop to an address where
//! the same instructions run slower. An instruction count measures the
//! work a revision does, which code layout cannot move, so it is what
//! the bench gate compares. The performance guide
//! (`docs/src/guides/performance.md`) tells the whole story.
//!
//! Each benchmark runs once under callgrind, which needs valgrind and
//! gungraun's runner installed (`cargo install gungraun-runner`). Run
//! them with `cargo bench -p canvist_headless --bench counts`.

use std::fmt::Write;

use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use gungraun::library_benchmark;
use gungraun::library_benchmark_group;
use gungraun::main;

const PARAGRAPHS: u32 = 10_000;
const PARAGRAPH: &str =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";

fn markdown(paragraphs: u32) -> String {
	(0..paragraphs).fold(String::new(), |mut markdown, index| {
		let _ = writeln!(markdown, "{PARAGRAPH} {index}\n");
		markdown
	})
}

/// The document every format exports, parsed outside the measured call.
fn setup(format: Format) -> (Doc, Format) {
	let doc = Engine::starter_kit()
		.parse(Format::Markdown, &markdown(PARAGRAPHS))
		.expect("parses");

	(doc, format)
}

#[library_benchmark]
#[bench::html(args = (Format::Html,), setup = setup)]
#[bench::json(args = (Format::Json,), setup = setup)]
#[bench::markdown(args = (Format::Markdown,), setup = setup)]
fn write((doc, format): (Doc, Format)) -> String {
	doc.export(format)
}

library_benchmark_group!(name = export; benchmarks = write);

main!(library_benchmark_groups = export);
