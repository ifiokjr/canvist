//! Collaboration hot paths.
//!
//! - `encode_keystroke`: mirror one typed character into yrs and encode the
//!   update to broadcast. Budget: 20 µs.
//! - `apply_remote_keystroke`: integrate a peer's one-character update and
//!   mirror it into the model. Budget: 20 µs.
//! - `initial_load`: join a 10,000-paragraph document from its full state.
//!
//! The keystroke benchmarks type into one paragraph throughout, so it grows
//! over a run; converting offsets costs `O(paragraph length)` by design.
//!
//! Run with `cargo bench -p canvist_collab`.

use std::cell::RefCell;
use std::sync::Arc;

use canvist_collab::Binding;
use canvist_collab::Encoding;
use canvist_collab::Track;
use canvist_model::BlockId;
use canvist_model::BlockSpec;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::MarkSpec;
use canvist_model::Point;
use canvist_model::Schema;
use divan::Bencher;
use divan::black_box;

const SIZES: [usize; 3] = [100, 10_000, 100_000];
const PARAGRAPH: &str =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";

fn main() {
	divan::main();
}

fn schema() -> Arc<Schema> {
	let schema = Schema::builder()
		.block(BlockSpec::text("paragraph"))
		.mark(MarkSpec::new("bold"))
		.build();
	Arc::new(schema.expect("valid schema"))
}

/// A document of `blocks` paragraphs, seeded into a binding.
fn seeded(blocks: usize) -> (Document, Binding) {
	let schema = schema();
	let paragraph = schema.default_text_block();
	let mut doc = Document::empty(schema.clone(), 1);
	let trees = (0..blocks)
		.map(|index| {
			let id = BlockId::new(1, u32::try_from(index).expect("fits"));
			BlockTree::text(
				id,
				paragraph,
				InlineText::plain(PARAGRAPH, MarkSet::empty()),
			)
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

/// Type one character in the middle block and return the update.
fn keystroke(doc: &mut Document, binding: &mut Binding) -> Vec<u8> {
	let block = doc.children(None)[doc.children(None).len() / 2];
	let mut transaction = doc.transaction();
	transaction
		.insert_text(Point::new(block, 10), "x", MarkSet::empty())
		.expect("insert");
	let commit = transaction.commit();
	binding
		.apply_local(doc, &commit, Track::Merge)
		.expect("local");
	binding.take_updates().concat()
}

#[divan::bench(args = SIZES)]
fn encode_keystroke(bencher: Bencher<'_, '_>, blocks: usize) {
	let (mut doc, mut binding) = seeded(blocks);

	bencher.bench_local(|| black_box(keystroke(&mut doc, &mut binding)));
}

#[divan::bench(args = SIZES)]
fn apply_remote_keystroke(bencher: Bencher<'_, '_>, blocks: usize) {
	let (doc, binding) = seeded(blocks);
	let sender = RefCell::new((doc, binding));
	let state = sender.borrow().1.encode_state(Encoding::V1);
	let mut receiver = Binding::with_client_id(schema(), 2).expect("binding");
	let mut mirror = Document::empty(schema(), 2);
	receiver
		.apply_update(&mut mirror, &state, Encoding::V1)
		.expect("join");

	bencher
		.with_inputs(|| {
			let (doc, binding) = &mut *sender.borrow_mut();
			keystroke(doc, binding)
		})
		.bench_local_refs(|update| {
			black_box(
				receiver
					.apply_update(&mut mirror, update, Encoding::V1)
					.expect("remote"),
			)
		});
}

#[divan::bench(sample_count = 10)]
fn initial_load(bencher: Bencher<'_, '_>) {
	let (_, binding) = seeded(10_000);
	let state = binding.encode_state(Encoding::V1);

	bencher.bench_local(|| {
		let mut binding = Binding::with_client_id(schema(), 2).expect("binding");
		let mut doc = Document::empty(schema(), 2);
		binding
			.apply_update(&mut doc, &state, Encoding::V1)
			.expect("join");
		black_box(doc)
	});
}
