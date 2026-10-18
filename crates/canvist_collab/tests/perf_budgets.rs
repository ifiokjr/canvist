//! Performance budgets for the collaboration hot paths.
//!
//! These tests fail when an operation exceeds the budget recorded in
//! `docs/src/v2/architecture.md`. They only mean something in release mode:
//!
//! ```sh
//! cargo test --release -p canvist_collab --test perf_budgets -- --ignored
//! ```
//!
//! `CANVIST_PERF_SLACK` multiplies every budget. CI sets it for noisy shared
//! runners; locally it defaults to 1.
//!
//! The file compiles to nothing in debug builds, where timings are meaningless.

#![cfg(not(debug_assertions))]

mod support;

use std::time::Duration;
use std::time::Instant;

use canvist_collab::Binding;
use canvist_collab::Encoding;
use canvist_collab::Track;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::Point;

/// Samples per measurement; the median is reported.
const SAMPLES: usize = 31;
const PARAGRAPH: &str =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";
const LARGE_DOC: usize = 100_000;
const JOINED_DOC: usize = 10_000;
/// Keystrokes per sample. All land in one paragraph, which grows by about
/// 3,000 characters over a run: long, but a paragraph people write.
const KEYSTROKES: u32 = 100;

fn slack() -> f64 {
	let configured = std::env::var("CANVIST_PERF_SLACK").ok();
	configured
		.and_then(|value| value.parse().ok())
		.unwrap_or(1.0)
}

/// The median time of one call to `run`, measured over [`SAMPLES`] batches of
/// `batch` calls after one warm-up batch.
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

/// A document of `blocks` paragraphs, seeded into a binding.
fn seeded(blocks: usize) -> (Document, Binding) {
	let schema = support::schema();
	let paragraph = schema.default_text_block();
	let mut doc = Document::empty(schema.clone(), 1);
	let trees = (0..blocks)
		.map(|index| {
			let id = BlockId::new(1, u32::try_from(index).expect("fits"));
			let text = InlineText::plain(PARAGRAPH, MarkSet::empty());
			BlockTree::text(id, paragraph, text)
		})
		.collect();
	doc.transaction()
		.insert_blocks(None, 0, trees)
		.expect("paragraphs");
	let mut binding = Binding::with_client_id(schema, 1).expect("binding");
	binding.seed(&doc).expect("seed");
	binding.take_updates();
	(doc, binding)
}

/// A second peer that joined `binding`'s document.
fn follower(binding: &Binding) -> (Document, Binding) {
	let schema = support::schema();
	let mut follower = Binding::with_client_id(schema.clone(), 2).expect("binding");
	let mut doc = Document::empty(schema, 2);
	let state = binding.encode_state(Encoding::V1);
	follower
		.apply_update(&mut doc, &state, Encoding::V1)
		.expect("join");
	(doc, follower)
}

/// Type one character at the end of the middle block, as typing does, and
/// return its update.
fn keystroke(doc: &mut Document, binding: &mut Binding) -> Vec<u8> {
	let block = doc.children(None)[doc.children(None).len() / 2];
	let end = doc.inline(block).map_or(0, InlineText::len);
	let mut transaction = doc.transaction();
	transaction
		.insert_text(Point::new(block, end), "x", MarkSet::empty())
		.expect("insert");
	let commit = transaction.commit();
	binding
		.apply_local(doc, &commit, Track::Merge)
		.expect("local");
	binding.take_updates().concat()
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn encode_a_keystroke_in_a_100k_block_document() {
	let (mut doc, mut binding) = seeded(LARGE_DOC);

	let measured = median_per_call(KEYSTROKES, || {
		std::hint::black_box(keystroke(&mut doc, &mut binding));
	});

	assert_within(
		"encode keystroke @100k blocks",
		measured,
		Duration::from_micros(20),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn apply_a_remote_keystroke_in_a_100k_block_document() {
	let (mut doc, mut binding) = seeded(LARGE_DOC);
	let (mut mirror, mut receiver) = follower(&binding);
	let batch = KEYSTROKES as usize;
	let updates: Vec<Vec<u8>> = (0..batch * (SAMPLES + 1))
		.map(|_| keystroke(&mut doc, &mut binding))
		.collect();
	let mut updates = updates.into_iter();

	let measured = median_per_call(batch as u32, || {
		let update = updates.next().expect("one update per call");
		let commit = receiver
			.apply_update(&mut mirror, &update, Encoding::V1)
			.expect("remote");
		std::hint::black_box(commit);
	});

	assert_within(
		"apply remote keystroke @100k blocks",
		measured,
		Duration::from_micros(20),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn apply_a_remote_enter_in_a_100k_block_document() {
	let (mut doc, mut binding) = seeded(LARGE_DOC);
	let (mut mirror, mut receiver) = follower(&binding);
	let paragraph = doc.schema().default_text_block();
	let batch = 20;
	let updates: Vec<Vec<u8>> = (0..batch * (SAMPLES + 1))
		.map(|_| {
			let block = doc.children(None)[LARGE_DOC / 2];
			let mut transaction = doc.transaction();
			transaction
				.split(Point::new(block, 20), paragraph, Attrs::new())
				.expect("enter");
			let commit = transaction.commit();
			binding
				.apply_local(&mut doc, &commit, Track::Step)
				.expect("local");
			binding.take_updates().concat()
		})
		.collect();
	let mut updates = updates.into_iter();

	let measured = median_per_call(batch as u32, || {
		let update = updates.next().expect("one update per call");
		let commit = receiver
			.apply_update(&mut mirror, &update, Encoding::V1)
			.expect("remote");
		std::hint::black_box(commit);
	});

	// Structural edits cost O(siblings): yrs computes an array event's delta
	// by walking the whole array, and here the root holds 100k blocks.
	assert_within(
		"apply remote enter @100k blocks",
		measured,
		Duration::from_millis(30),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn encode_an_enter_in_a_100k_block_document() {
	let (mut doc, mut binding) = seeded(LARGE_DOC);
	let paragraph = doc.schema().default_text_block();
	let block = doc.children(None)[LARGE_DOC / 2];

	let measured = median_per_call(20, || {
		let mut transaction = doc.transaction();
		transaction
			.split(Point::new(block, 20), paragraph, Attrs::new())
			.expect("enter");
		let commit = transaction.commit();
		binding
			.apply_local(&mut doc, &commit, Track::Step)
			.expect("local");
		std::hint::black_box(binding.take_updates());
	});

	// O(siblings) for the same reason, plus yrs walking to the insertion
	// index.
	assert_within(
		"encode enter @100k blocks",
		measured,
		Duration::from_millis(15),
	);
}

#[test]
#[ignore = "release-mode performance budget; run with `test:perf`"]
fn join_a_10k_paragraph_document() {
	let (_, binding) = seeded(JOINED_DOC);

	let measured = median_per_call(1, || {
		std::hint::black_box(follower(&binding));
	});

	assert_within("join 10k paragraphs", measured, Duration::from_millis(50));
}
