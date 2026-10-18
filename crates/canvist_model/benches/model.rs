//! Model hot-path benchmarks.
//!
//! The headline number is `type_char`: the cost of applying one keystroke,
//! which must stay flat as documents grow. Run with
//! `cargo bench -p canvist_model`.

#[path = "../tests/support/mod.rs"]
mod support;

use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Point;
use divan::Bencher;
use divan::black_box;

const SIZES: [usize; 3] = [100, 10_000, 100_000];
const PARAGRAPH: &str =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";

fn main() {
	divan::main();
}

/// A document of `blocks` paragraphs and the middle paragraph's id.
fn fixture(blocks: usize) -> (Document, BlockId) {
	let doc = support::paragraphs(blocks, PARAGRAPH);
	let middle = doc.children(None)[blocks / 2];
	(doc, middle)
}

/// One keystroke in the middle of a document of the given size.
#[divan::bench(args = SIZES)]
fn type_char(bencher: Bencher<'_, '_>, blocks: usize) {
	let (mut doc, block) = fixture(blocks);
	let caret = Point::new(block, 10);
	bencher.bench_local(|| {
		let mut transaction = doc.transaction();
		transaction
			.insert_text(black_box(caret), "a", MarkSet::empty())
			.expect("valid caret");
		black_box(transaction.commit());
	});
}

/// One keystroke followed by Backspace, so the block stays the same size.
#[divan::bench(args = SIZES)]
fn type_and_backspace(bencher: Bencher<'_, '_>, blocks: usize) {
	let (mut doc, block) = fixture(blocks);
	bencher.bench_local(|| {
		let mut transaction = doc.transaction();
		let end = transaction
			.insert_text(Point::new(block, 10), "a", MarkSet::empty())
			.expect("type");
		transaction
			.delete_text(block, 10..end.offset)
			.expect("backspace");
		black_box(transaction.commit());
	});
}

/// A keystroke in one 100 KB paragraph: the worst case for string splicing.
#[divan::bench]
fn type_char_in_huge_paragraph(bencher: Bencher<'_, '_>) {
	let mut doc = support::paragraphs(1, &PARAGRAPH.repeat(1_200));
	let block = doc.children(None)[0];
	bencher.bench_local(|| {
		let mut transaction = doc.transaction();
		transaction
			.insert_text(Point::new(block, 50_000), "a", MarkSet::empty())
			.expect("valid caret");
		black_box(transaction.commit());
	});
}

/// A keystroke in a paragraph with 500 alternating formatting runs.
#[divan::bench]
fn type_char_in_heavily_formatted_paragraph(bencher: Bencher<'_, '_>) {
	let mut doc = support::paragraphs(1, &PARAGRAPH.repeat(13));
	let block = doc.children(None)[0];
	let bold = Mark::new(doc.schema().mark_type("bold").expect("bold"));
	let mut transaction = doc.transaction();

	for run in 0..500 {
		transaction
			.add_mark(block, run * 2..run * 2 + 1, bold.clone())
			.expect("format");
	}

	let _ = transaction.commit();

	bencher.bench_local(|| {
		let mut transaction = doc.transaction();
		transaction
			.insert_text(Point::new(block, 501), "a", MarkSet::empty())
			.expect("valid caret");
		black_box(transaction.commit());
	});
}

/// Enter then Backspace in the middle of a document: split and join.
#[divan::bench(args = SIZES)]
fn enter_and_join(bencher: Bencher<'_, '_>, blocks: usize) {
	let (mut doc, block) = fixture(blocks);
	let paragraph = doc.schema().default_text_block();
	bencher.bench_local(|| {
		let mut transaction = doc.transaction();
		let start = transaction
			.split(Point::new(block, 20), paragraph, Attrs::new())
			.expect("enter");
		transaction
			.join(block, start.block)
			.expect("backspace at block start");
		black_box(transaction.commit());
	});
}

/// Bold then unbold a whole paragraph.
#[divan::bench(args = SIZES)]
fn toggle_bold(bencher: Bencher<'_, '_>, blocks: usize) {
	let (mut doc, block) = fixture(blocks);
	let bold = Mark::new(doc.schema().mark_type("bold").expect("bold"));
	let len = PARAGRAPH.len() as u32;
	bencher.bench_local(|| {
		let mut transaction = doc.transaction();
		transaction
			.add_mark(block, 0..len, bold.clone())
			.expect("bold");
		transaction
			.remove_mark(block, 0..len, bold.kind)
			.expect("unbold");
		black_box(transaction.commit());
	});
}

/// Paste 1 000 paragraphs into the middle of a document, then undo it.
#[divan::bench(args = SIZES)]
fn paste_and_undo_1000_blocks(bencher: Bencher<'_, '_>, blocks: usize) {
	let (mut doc, _) = fixture(blocks);
	let paragraph = doc.schema().default_text_block();
	let pasted: Vec<BlockTree> = (0..1_000)
		.map(|index| {
			BlockTree::text(
				BlockId::new(77, index),
				paragraph,
				InlineText::plain(PARAGRAPH, MarkSet::empty()),
			)
		})
		.collect();
	bencher.bench_local(|| {
		let mut transaction = doc.transaction();
		transaction
			.insert_blocks(None, blocks / 2, pasted.clone())
			.expect("paste");
		let commit = transaction.commit();
		black_box(doc.apply_steps(commit.undo_steps()).expect("undo"));
	});
}

/// Visit every text block, as a full relayout or export would.
#[divan::bench(args = SIZES)]
fn iterate_text_blocks(bencher: Bencher<'_, '_>, blocks: usize) {
	let (doc, _) = fixture(blocks);
	bencher.bench_local(|| {
		black_box(
			doc.text_blocks()
				.map(canvist_model::Block::rev)
				.sum::<u32>(),
		)
	});
}

/// Serialize to ProseMirror JSON.
#[divan::bench(args = [100, 10_000])]
fn to_json(bencher: Bencher<'_, '_>, blocks: usize) {
	let (doc, _) = fixture(blocks);
	bencher.bench_local(|| black_box(doc.to_json()));
}

/// Parse ProseMirror JSON.
#[divan::bench(args = [100, 10_000])]
fn from_json(bencher: Bencher<'_, '_>, blocks: usize) {
	let (doc, _) = fixture(blocks);
	let json = doc.to_json();
	let schema = doc.schema().clone();
	bencher
		.bench_local(|| black_box(Document::from_json(schema.clone(), 9, &json).expect("parses")));
}
