//! Property tests over random documents mixing scripts, directions, marks,
//! tabs, and embeds.

#![allow(clippy::float_cmp, reason = "layout with the fixed measurer is exact")]

mod support;

use std::ops::Range;
use std::sync::Arc;

use canvist_layout::Affinity;
use canvist_layout::Caret;
use canvist_layout::DocumentLayout;
use canvist_layout::FixedMeasurer;
use canvist_layout::LayoutOptions;
use canvist_layout::Theme;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Point;
use proptest::prelude::*;
use serde_json::Value as Json;
use serde_json::json;

/// Words from several scripts. None contains a zero-width grapheme, so every
/// caret stop has a distinct x position.
const LTR_WORDS: &[&str] = &[
	"a",
	"quick",
	"brown",
	"fox",
	"jumps",
	"Supercalifragilistic",
	"café",
	"naïve",
	"e\u{301}clair",
	"日本語",
	"中文",
	"😀",
	"👨\u{200D}👩\u{200D}👧",
	"👍🏽",
	"🇳🇬",
	"x-y",
	"1,234",
	"(ok)",
];
const RTL_WORDS: &[&str] = &["שלום", "עולם", "مرحبا", "بالعالم", "كتاب"];
/// How often a token is a word, relative to a tab, break, or mention.
const WORD_WEIGHT: u32 = 6;
/// How often a token is a space, relative to a tab, break, or mention.
const SPACE_WEIGHT: u32 = 4;
/// Cases per property; each lays out a random document several times.
const CASES: u32 = 64;

#[derive(Clone, Debug)]
enum Token {
	Word(&'static str),
	Space,
	Tab,
	Break,
	Mention,
}

fn token(rtl: bool) -> impl Strategy<Value = Token> {
	let words = if rtl {
		[LTR_WORDS, RTL_WORDS].concat()
	} else {
		LTR_WORDS.to_vec()
	};

	prop_oneof![
		WORD_WEIGHT => proptest::sample::select(words).prop_map(Token::Word),
		SPACE_WEIGHT => Just(Token::Space),
		1 => Just(Token::Tab),
		1 => Just(Token::Break),
		1 => Just(Token::Mention),
	]
}

/// A TipTap paragraph from tokens, with bold and code toggled per token.
fn paragraph(tokens: &[(Token, u8)]) -> Json {
	let content: Vec<Json> = tokens
		.iter()
		.map(|(token, style)| {
			let marks = match style % 4 {
				1 => json!([{ "type": "bold" }]),
				2 => json!([{ "type": "code" }]),
				_ => json!([]),
			};
			match token {
				Token::Word(word) => json!({ "type": "text", "text": word, "marks": marks }),
				Token::Space => json!({ "type": "text", "text": " ", "marks": marks }),
				Token::Tab => json!({ "type": "text", "text": "\t", "marks": marks }),
				Token::Break => json!({ "type": "hardBreak" }),
				Token::Mention => json!({ "type": "mention", "attrs": { "label": "ada" } }),
			}
		})
		.collect();

	json!({ "type": "paragraph", "content": content })
}

/// Apply every edit of `batch` to `doc` and `layout`, without relaying out
/// in between, so the layout must fold several edits into one.
fn apply_batch(doc: &mut Document, layout: &mut DocumentLayout, batch: &[Edit]) {
	for edit in batch {
		if let Some(commit) = apply(doc, edit) {
			layout.apply(doc, &commit);
		}
	}
}

/// One to three paragraphs of 40 to 160 tokens, so edits land far from
/// both ends of a paragraph.
fn long_document() -> impl Strategy<Value = Document> {
	let tokens = proptest::collection::vec((token(false), any::<u8>()), 40..160);

	proptest::collection::vec(tokens, 1..3).prop_map(|paragraphs| {
		support::doc(Json::Array(
			paragraphs.iter().map(|tokens| paragraph(tokens)).collect(),
		))
	})
}

fn document(rtl: bool) -> impl Strategy<Value = Document> {
	let tokens = proptest::collection::vec((token(rtl), any::<u8>()), 0..24);
	proptest::collection::vec(tokens, 1..4).prop_map(|paragraphs| {
		support::doc(Json::Array(
			paragraphs.iter().map(|p| paragraph(p)).collect(),
		))
	})
}

/// Every grapheme boundary of every block, with both affinities.
fn carets(doc: &Document) -> Vec<Caret> {
	let mut carets = Vec::new();

	for block in doc.text_blocks() {
		let text = block.inline().map(InlineText::as_str).unwrap_or_default();
		let boundaries = grapheme_boundaries(text);

		for offset in boundaries {
			for affinity in [Affinity::Upstream, Affinity::Downstream] {
				carets.push(Caret::with_affinity(
					Point::new(block.id(), offset),
					affinity,
				));
			}
		}
	}

	carets
}

/// Grapheme boundaries with the segmenter the editor moves the caret with.
fn grapheme_boundaries(text: &str) -> Vec<u32> {
	use unicode_segmentation::UnicodeSegmentation;

	text.grapheme_indices(true)
		.map(|(offset, _)| offset as u32)
		.chain(std::iter::once(text.len() as u32))
		.collect()
}

fn round_trip(doc: &Document, width: f32, exact: bool) -> Result<(), TestCaseError> {
	let layout = support::layout(doc, width);

	for caret in carets(doc) {
		let rect = layout
			.caret_rect(caret)
			.expect("every text block is laid out");
		let block = layout
			.block_rect(caret.point.block)
			.expect("every block has a rectangle");

		// Trailing spaces hang past the end of a line, as CSS `pre-wrap` lets
		// them; in a table cell that can be over the next cell, where a click
		// belongs to that cell.
		if rect.x + rect.width / 2.0 > block.x + block.width {
			continue;
		}

		let (x, y) = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
		let hit = layout.hit_test(x, y).expect("the document has rows");

		let landed = layout
			.caret_rect(hit)
			.expect("hits land in laid-out blocks");
		let same_spot =
			(landed.x - rect.x).abs() < 0.01 && landed.y == rect.y && landed.height == rect.height;
		prop_assert!(
			same_spot,
			"{:?} drawn at {:?} hit {:?} drawn at {:?}",
			caret,
			rect,
			hit,
			landed
		);

		if exact {
			prop_assert_eq!(hit.point, caret.point, "{:?} hit {:?}", caret, hit);
		}
	}

	Ok(())
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(CASES))]

	/// In one-directional text every caret stop is a distinct position, so a
	/// hit test at a caret returns exactly that caret's point.
	#[test]
	fn hit_test_inverts_caret_rect(doc in document(false), width in 60.0f32..400.0) {
		round_trip(&doc, width, true)?;
	}

	/// The same holds for text placed side by side in table cells.
	#[test]
	fn hit_test_inverts_caret_rect_in_tables(doc in table_document(), width in 120.0f32..480.0) {
		round_trip(&doc, width, true)?;
	}

	/// In mixed-direction text one position can belong to two offsets (the
	/// end of an LTR run and the start of an RTL run), so the hit must draw
	/// in the same place.
	#[test]
	fn bidi_hit_test_lands_on_the_same_spot(doc in document(true), width in 60.0f32..400.0) {
		round_trip(&doc, width, false)?;
	}

	/// Laying out twice, or after a round trip through another width, gives
	/// the same result.
	#[test]
	fn layout_is_deterministic(doc in document(true), width in 60.0f32..400.0, other in 60.0f32..400.0) {
		let first = support::layout(&doc, width);
		let second = support::layout(&doc, width);
		prop_assert_eq!(first.dump(), second.dump());

		let mut resized = support::layout(&doc, width);
		let mut measurer = FixedMeasurer::new();
		resized.set_width(&doc, other);
		resized.layout_all(&doc, &mut measurer).expect("measured");
		resized.set_width(&doc, width);
		resized.layout_all(&doc, &mut measurer).expect("measured");
		prop_assert_eq!(resized.dump(), first.dump());
	}

	/// Applying commits incrementally gives exactly the layout of laying out
	/// the edited document from scratch.
	#[test]
	fn incremental_relayout_matches_full_layout(
		doc in document(true),
		edits in proptest::collection::vec(edit(), 1..12),
		width in 60.0f32..400.0,
	) {
		let mut doc = doc;
		let mut measurer = FixedMeasurer::new();
		let mut layout = DocumentLayout::new(&doc, Arc::new(Theme::starter_kit()), LayoutOptions::new(width));
		layout.layout_all(&doc, &mut measurer).expect("measured");

		for edit in edits {
			let Some(commit) = apply(&mut doc, &edit) else {
				continue;
			};
			layout.apply(&doc, &commit);
			layout.ensure(&doc, &mut measurer, 0.0, f32::MAX).expect("measured");
			prop_assert_eq!(layout.dump(), support::layout(&doc, width).dump(), "after {:?}", edit);
		}
	}

	/// Long paragraphs relay out only around each edit; the result must still
	/// match laying out from scratch, including when several commits land
	/// between relayouts.
	#[test]
	fn partial_paragraph_relayout_matches_full_layout(
		doc in long_document(),
		batches in proptest::collection::vec(proptest::collection::vec(edit(), 1..4), 1..8),
		width in 60.0f32..400.0,
	) {
		let mut doc = doc;
		let mut measurer = FixedMeasurer::new();
		let mut layout = DocumentLayout::new(&doc, Arc::new(Theme::starter_kit()), LayoutOptions::new(width));
		layout.layout_all(&doc, &mut measurer).expect("measured");

		for batch in batches {
			apply_batch(&mut doc, &mut layout, &batch);
			layout.ensure(&doc, &mut measurer, 0.0, f32::MAX).expect("measured");
			prop_assert_eq!(layout.dump(), support::layout(&doc, width).dump(), "after {:?}", batch);
		}
	}

	/// The same holds for documents with nested lists, quotes, and rules,
	/// under edits that insert, remove, move, and restyle whole blocks.
	#[test]
	fn structural_relayout_matches_full_layout(
		doc in structured_document(),
		edits in proptest::collection::vec(block_edit(), 1..16),
		width in 60.0f32..400.0,
	) {
		let mut doc = doc;
		let mut measurer = FixedMeasurer::new();
		let mut layout = DocumentLayout::new(&doc, Arc::new(Theme::starter_kit()), LayoutOptions::new(width));
		layout.layout_all(&doc, &mut measurer).expect("measured");

		for edit in edits {
			let Some(commit) = apply_block_edit(&mut doc, &edit) else {
				continue;
			};

			layout.apply(&doc, &commit);
			layout.ensure(&doc, &mut measurer, 0.0, f32::MAX).expect("measured");
			prop_assert_eq!(layout.dump(), support::layout(&doc, width).dump(), "after {:?}", edit);
		}
	}
}

/// A root-level block for structured documents.
fn root_node() -> impl Strategy<Value = Json> {
	let words = || proptest::collection::vec(proptest::sample::select(LTR_WORDS.to_vec()), 0..6);
	let text = move || words().prop_map(|words| support::paragraph(&words.join(" ")));
	let item =
		move || text().prop_map(|paragraph| json!({ "type": "listItem", "content": [paragraph] }));

	prop_oneof![
		3 => text(),
		1 => proptest::collection::vec(item(), 1..4)
			.prop_map(|items| json!({ "type": "bulletList", "content": items })),
		1 => proptest::collection::vec(item(), 1..3)
			.prop_map(|items| json!({ "type": "orderedList", "content": items })),
		1 => proptest::collection::vec(text(), 1..3)
			.prop_map(|paragraphs| json!({ "type": "blockquote", "content": paragraphs })),
		1 => Just(json!({ "type": "horizontalRule" })),
		1 => table_node(),
	]
}

/// A table of one to three rows of one to three cells, some spanning two
/// columns or rows, each holding one or two paragraphs.
fn table_node() -> impl Strategy<Value = Json> {
	let words = || proptest::collection::vec(proptest::sample::select(LTR_WORDS.to_vec()), 0..8);
	let paragraphs = proptest::collection::vec(words(), 1..3);
	let cell = (paragraphs, 1..3i64, 1..3i64, any::<bool>()).prop_map(
		|(texts, colspan, rowspan, header)| {
			let content: Vec<Json> = texts
				.iter()
				.map(|words| support::paragraph(&words.join(" ")))
				.collect();
			json!({
				"type": if header { "tableHeader" } else { "tableCell" },
				"attrs": { "colspan": colspan, "rowspan": rowspan },
				"content": content,
			})
		},
	);
	let row = proptest::collection::vec(cell, 1..4)
		.prop_map(|cells| json!({ "type": "tableRow", "content": cells }));

	proptest::collection::vec(row, 1..4)
		.prop_map(|rows| json!({ "type": "table", "content": rows }))
}

fn table_document() -> impl Strategy<Value = Document> {
	proptest::collection::vec(
		prop_oneof![table_node(), Just(support::paragraph("between"))],
		1..4,
	)
	.prop_map(|roots| support::doc(Json::Array(roots)))
}

fn structured_document() -> impl Strategy<Value = Document> {
	proptest::collection::vec(root_node(), 1..8).prop_map(|roots| support::doc(Json::Array(roots)))
}

/// Edits that change block structure, alongside the inline edits.
#[derive(Clone, Debug)]
enum BlockEdit {
	Inline(Edit),
	InsertRoot { at: usize, list: bool },
	RemoveRoot { at: usize },
	MoveRoot { from: usize, to: usize },
	Restyle { block: usize },
}

fn block_edit() -> impl Strategy<Value = BlockEdit> {
	prop_oneof![
		4 => edit().prop_map(BlockEdit::Inline),
		1 => (any::<usize>(), any::<bool>()).prop_map(|(at, list)| BlockEdit::InsertRoot { at, list }),
		1 => any::<usize>().prop_map(|at| BlockEdit::RemoveRoot { at }),
		1 => (any::<usize>(), any::<usize>()).prop_map(|(from, to)| BlockEdit::MoveRoot { from, to }),
		1 => any::<usize>().prop_map(|block| BlockEdit::Restyle { block }),
	]
}

fn apply_block_edit(doc: &mut Document, edit: &BlockEdit) -> Option<canvist_model::Commit> {
	let roots = doc.children(None).len();

	match *edit {
		BlockEdit::Inline(ref edit) => apply(doc, edit),
		BlockEdit::InsertRoot { at, list } => {
			let tree = new_root(doc, list)?;
			let mut transaction = doc.transaction();
			transaction
				.insert_blocks(None, at % (roots + 1), vec![tree])
				.ok()?;

			Some(transaction.commit())
		}
		BlockEdit::RemoveRoot { at } => {
			let mut transaction = doc.transaction();
			(roots > 1).then_some(())?;
			transaction.remove_blocks(None, at % roots, 1).ok()?;

			Some(transaction.commit())
		}
		BlockEdit::MoveRoot { from, to } => {
			(roots > 0).then_some(())?;
			let mut transaction = doc.transaction();
			transaction
				.move_blocks((None, from % roots, 1), (None, to % roots))
				.ok()?;

			Some(transaction.commit())
		}
		BlockEdit::Restyle { block } => restyle(doc, block),
	}
}

/// A new paragraph, or a one-item bullet list, with fresh ids.
fn new_root(doc: &mut Document, list: bool) -> Option<BlockTree> {
	let schema = doc.schema().clone();
	let text = InlineText::plain("inserted words wrap", MarkSet::empty());
	let paragraph = BlockTree::text(doc.allocate_id().ok()?, schema.default_text_block(), text);

	if !list {
		return Some(paragraph);
	}

	let item = BlockTree::container(
		doc.allocate_id().ok()?,
		schema.block_type("listItem")?,
		vec![paragraph],
	);

	Some(BlockTree::container(
		doc.allocate_id().ok()?,
		schema.block_type("bulletList")?,
		vec![item],
	))
}

/// Turn a text block into a level-two heading, or back into a paragraph.
fn restyle(doc: &mut Document, block: usize) -> Option<canvist_model::Commit> {
	let blocks: Vec<_> = doc.text_blocks().map(canvist_model::Block::id).collect();
	let id = *blocks.get(block % blocks.len().max(1))?;
	let schema = doc.schema().clone();
	let heading = schema.block_type("heading")?;
	let (kind, attrs) = if doc.block(id)?.kind() == heading {
		(schema.default_text_block(), Attrs::new())
	} else {
		(heading, Attrs::new().with("level", 2))
	};

	let mut transaction = doc.transaction();

	if transaction.set_block(id, kind, attrs).is_err() {
		let _ = transaction.rollback();

		return None;
	}

	Some(transaction.commit())
}

#[derive(Clone, Debug)]
enum Edit {
	Insert {
		block: usize,
		at: usize,
		text: &'static str,
	},
	Delete {
		block: usize,
		at: usize,
		len: usize,
	},
	Bold {
		block: usize,
		at: usize,
		len: usize,
	},
	Split {
		block: usize,
		at: usize,
	},
	Join {
		block: usize,
	},
}

fn edit() -> impl Strategy<Value = Edit> {
	let text = proptest::sample::select(vec![
		"a",
		" ",
		"שלום ",
		"日本",
		"😀",
		"long words wrap ",
		"\t",
	]);
	prop_oneof![
		4 => (any::<usize>(), any::<usize>(), text).prop_map(|(block, at, text)| Edit::Insert { block, at, text }),
		2 => (any::<usize>(), any::<usize>(), 1usize..8).prop_map(|(block, at, len)| Edit::Delete { block, at, len }),
		1 => (any::<usize>(), any::<usize>(), 1usize..8).prop_map(|(block, at, len)| Edit::Bold { block, at, len }),
		1 => (any::<usize>(), any::<usize>()).prop_map(|(block, at)| Edit::Split { block, at }),
		1 => any::<usize>().prop_map(|block| Edit::Join { block }),
	]
}

/// The `index`-th char boundary of `text`, wrapping.
fn boundary(text: &str, index: usize) -> u32 {
	let boundaries: Vec<usize> = text
		.char_indices()
		.map(|(offset, _)| offset)
		.chain([text.len()])
		.collect();
	boundaries[index % boundaries.len()] as u32
}

/// The text of block `id`, empty for a block without inline content.
fn text_of(doc: &Document, id: BlockId) -> String {
	doc.inline(id)
		.map(|inline| inline.as_str().to_owned())
		.unwrap_or_default()
}

/// The char-boundary range of `text` from the `at`-th boundary spanning `len`
/// boundaries, wrapping, and empty rather than reversed when it wraps.
fn span(text: &str, at: usize, len: usize) -> Range<u32> {
	let start = boundary(text, at);
	let end = boundary(text, at + len).max(start);

	start..end
}

fn apply(doc: &mut Document, edit: &Edit) -> Option<canvist_model::Commit> {
	let blocks: Vec<_> = doc.text_blocks().map(canvist_model::Block::id).collect();

	if blocks.is_empty() {
		return None;
	}

	let bold = Mark::new(doc.schema().mark_type("bold")?);
	let paragraph = doc.schema().default_text_block();
	let pick = |index: usize| blocks[index % blocks.len()];
	let mut transaction = doc.transaction();

	let ok = match *edit {
		Edit::Insert { block, at, text } => {
			let id = pick(block);
			let offset = boundary(&text_of(transaction.doc(), id), at);
			transaction
				.insert_text(Point::new(id, offset), text, MarkSet::empty())
				.is_ok()
		}
		Edit::Delete { block, at, len } => {
			let id = pick(block);
			let range = span(&text_of(transaction.doc(), id), at, len);
			transaction.delete_text(id, range).is_ok()
		}
		Edit::Bold { block, at, len } => {
			let id = pick(block);
			let range = span(&text_of(transaction.doc(), id), at, len);
			transaction.add_mark(id, range, bold).is_ok()
		}
		Edit::Split { block, at } => {
			let id = pick(block);
			let offset = boundary(&text_of(transaction.doc(), id), at);
			transaction
				.split(Point::new(id, offset), paragraph, Attrs::new())
				.is_ok()
		}
		Edit::Join { block } => {
			let index = block % blocks.len();
			index > 0 && transaction.join(blocks[index - 1], blocks[index]).is_ok()
		}
	};

	if !ok || transaction.is_empty() {
		let _ = transaction.rollback();

		return None;
	}

	Some(transaction.commit())
}
