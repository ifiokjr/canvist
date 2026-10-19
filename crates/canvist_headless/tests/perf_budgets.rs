//! Performance budgets for the headless API's hot paths, recorded in
//! `docs/src/v2/architecture.md`. They only mean something in release mode:
//!
//! ```sh
//! cargo test --release -p canvist_headless --all-features --test perf_budgets -- --ignored
//! ```
//!
//! `CANVIST_PERF_SLACK` multiplies every budget. CI sets it for noisy shared
//! runners; locally it defaults to 1.

#![cfg(all(not(debug_assertions), feature = "starter-kit"))]

use std::fmt::Write;
use std::time::Duration;
use std::time::Instant;

use canvist_extensions::StarterKitHandles;
use canvist_headless::Doc;
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::Position;

const PARAGRAPH: &str =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";
const PARAGRAPHS: u32 = 10_000;
/// Samples for operations measured in microseconds; the median is reported.
const SAMPLES: usize = 31;
/// Samples for whole-document operations, which take milliseconds.
const LARGE_SAMPLES: usize = 7;

fn slack() -> f64 {
	let configured = std::env::var("CANVIST_PERF_SLACK").ok();
	configured
		.and_then(|value| value.parse().ok())
		.unwrap_or(1.0)
}

fn assert_within(name: &str, measured: Duration, budget: Duration) {
	let allowed = budget.mul_f64(slack());
	eprintln!("{name}: {measured:?} (budget {budget:?}, allowed {allowed:?})");
	assert!(
		measured <= allowed,
		"{name} took {measured:?}, over its {allowed:?} budget"
	);
}

/// The median of `samples` timings of `batch` calls to `run`, per call,
/// after one warm-up batch.
fn median_per_call(samples: usize, batch: u32, mut run: impl FnMut()) -> Duration {
	for _ in 0..batch {
		run();
	}

	let mut timings: Vec<Duration> = (0..samples)
		.map(|_| {
			let start = Instant::now();
			for _ in 0..batch {
				run();
			}

			start.elapsed() / batch
		})
		.collect();
	timings.sort_unstable();
	timings[samples / 2]
}

fn markdown() -> String {
	(0..PARAGRAPHS).fold(String::new(), |mut markdown, index| {
		writeln!(markdown, "{PARAGRAPH} {index}\n").expect("writing to a String cannot fail");
		markdown
	})
}

fn large_doc(engine: &Engine<StarterKitHandles>) -> Doc {
	engine
		.parse(Format::Markdown, &markdown())
		.expect("the document parses")
}

/// A caret in the middle of the middle paragraph.
fn caret() -> Position {
	Position::new([PARAGRAPHS / 2], 10)
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn load_10k_paragraphs() {
	let engine = Engine::starter_kit();
	let markdown = markdown();
	let json = large_doc(&engine).export(Format::Json);

	let from_markdown = median_per_call(LARGE_SAMPLES, 1, || {
		std::hint::black_box(engine.parse(Format::Markdown, &markdown).expect("parses"));
	});
	let from_json = median_per_call(LARGE_SAMPLES, 1, || {
		std::hint::black_box(engine.parse(Format::Json, &json).expect("parses"));
	});

	assert_within(
		"load 10k paragraphs from Markdown",
		from_markdown,
		Duration::from_millis(50),
	);
	assert_within(
		"load 10k paragraphs from JSON",
		from_json,
		Duration::from_millis(50),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn export_10k_paragraphs() {
	let doc = large_doc(&Engine::starter_kit());
	let budgets = [
		(Format::Json, Duration::from_millis(50)),
		(Format::Markdown, Duration::from_millis(25)),
		(Format::Html, Duration::from_millis(25)),
	];

	for (format, budget) in budgets {
		let measured = median_per_call(LARGE_SAMPLES, 1, || {
			std::hint::black_box(doc.export(format));
		});
		assert_within(
			&format!("export 10k paragraphs as {format}"),
			measured,
			budget,
		);
	}
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn create_a_document() {
	let engine = Engine::starter_kit();
	let measured = median_per_call(SAMPLES, 100, || {
		std::hint::black_box(engine.create().expect("creates"));
	});

	assert_within(
		"create an empty document (StarterKit)",
		measured,
		Duration::from_micros(250),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn command_overhead_at_10k_paragraphs() {
	let engine = Engine::starter_kit();
	let kit = engine.handles();
	let mut doc = large_doc(&engine);
	doc.select_range(&caret(), &caret()).expect("selects");
	let toggle = kit.bold.toggle();

	let typed = median_per_call(SAMPLES, 1_000, || {
		doc.run(&toggle).expect("runs");
	});
	let named = median_per_call(SAMPLES, 1_000, || {
		doc.run_named("toggleBold", &serde_json::Value::Null)
			.expect("runs");
	});
	let mut typing = true;
	let keystroke = median_per_call(SAMPLES, 1_000, || {
		typing = !typing;
		let applied = if typing {
			doc.insert_text("x")
		} else {
			doc.press("Backspace")
		};
		assert!(applied.expect("edits"));
	});

	assert_within(
		"run a typed command @10k paragraphs",
		typed,
		Duration::from_micros(2),
	);
	assert_within(
		"run a named command @10k paragraphs",
		named,
		Duration::from_micros(2),
	);
	assert_within(
		"type or delete a character @10k paragraphs",
		keystroke,
		Duration::from_micros(2),
	);
}

#[cfg(feature = "collab")]
#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn yjs_updates_at_10k_paragraphs() {
	let engine = Engine::starter_kit();
	let mut server = large_doc(&engine);
	server.share(1).expect("shares");
	let state = server.encode_state().expect("encodes");
	let mut peer = engine
		.clone()
		.with_client(2)
		.from_yjs(&state, 2)
		.expect("joins");

	// A typed character and its deletion, so the paragraph keeps its length.
	let mut updates = Vec::new();

	for _ in 0..(SAMPLES as u32 + 1) * 100 {
		peer.select_range(&caret(), &caret()).expect("selects");
		peer.insert_text("y").expect("types");
		peer.press("Backspace").expect("deletes");
		updates.extend(peer.take_updates().expect("shared"));
	}

	let mut incoming = updates.into_iter();
	let apply = median_per_call(SAMPLES, 100, || {
		let update = incoming.next().expect("enough updates");
		assert!(server.apply_update(&update).expect("applies"));
	});

	let mut typing = true;
	server.select_range(&caret(), &caret()).expect("selects");
	let local = median_per_call(SAMPLES, 1_000, || {
		typing = !typing;
		let applied = if typing {
			server.insert_text("x")
		} else {
			server.press("Backspace")
		};
		assert!(applied.expect("edits"));
		std::hint::black_box(server.take_updates().expect("shared"));
	});

	assert_within(
		"apply one Yjs keystroke update @10k paragraphs",
		apply,
		Duration::from_micros(20),
	);
	assert_within(
		"type into a shared document @10k paragraphs",
		local,
		Duration::from_micros(20),
	);
}
