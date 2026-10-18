//! Fixtures and invariant checks shared by integration tests and benchmarks.

#![allow(dead_code)]

use std::sync::Arc;

use canvist_model::Attrs;
use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::BlockSpec;
use canvist_model::BlockTree;
use canvist_model::BlockTypeId;
use canvist_model::Content;
use canvist_model::Document;
use canvist_model::EmbedSpec;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::MarkSpec;
use canvist_model::Schema;

/// A StarterKit-like schema: paragraphs, headings, quotes, nested lists, a
/// divider, common marks, and a mention embed.
pub fn schema() -> Arc<Schema> {
	let schema = Schema::builder()
		.block(BlockSpec::text("paragraph"))
		.block(BlockSpec::text("heading").attrs(Attrs::new().with("level", 1)))
		.block(BlockSpec::container("blockquote", &["block"]))
		.block(BlockSpec::container("bullet_list", &["list_item"]))
		.block(BlockSpec::container("list_item", &["paragraph", "bullet_list"]).groups(&[]))
		.block(BlockSpec::leaf("divider"))
		.mark(MarkSpec::new("bold"))
		.mark(MarkSpec::new("italic"))
		.mark(MarkSpec::new("code").excludes(&["bold", "italic"]))
		.mark(MarkSpec::new("link").inclusive(false))
		.embed(EmbedSpec::new("mention"))
		.build();
	Arc::new(schema.expect("the support schema is valid"))
}

/// The block type named `name`.
pub fn kind(doc: &Document, name: &str) -> BlockTypeId {
	doc.schema()
		.block_type(name)
		.expect("type exists in the support schema")
}

/// A document with `count` paragraphs of `text`.
pub fn paragraphs(count: usize, text: &str) -> Document {
	let schema = schema();
	let paragraph = schema.default_text_block();
	let mut doc = Document::empty(schema, 1);
	let blocks = (0..count)
		.map(|index| {
			let id = BlockId::new(2, u32::try_from(index).expect("count fits u32"));
			BlockTree::text(id, paragraph, InlineText::plain(text, MarkSet::empty()))
		})
		.collect();
	doc.transaction()
		.insert_blocks(None, 0, blocks)
		.expect("paragraphs are valid at the root");
	doc
}

/// Panic unless every structural invariant of `doc` holds.
pub fn assert_invariants(doc: &Document) {
	let mut seen = 0;
	check_children(doc, None, &mut seen);
	assert_eq!(
		seen,
		doc.block_count(),
		"every block is reachable exactly once"
	);

	for block in doc.iter() {
		check_block(doc, block);
	}
}

fn check_children(doc: &Document, parent: Option<BlockId>, seen: &mut usize) {
	let parent_kind = parent.and_then(|id| doc.block(id)).map(Block::kind);

	for &child in doc.children(parent) {
		let block = doc.block(child).expect("child ids resolve");
		*seen += 1;
		assert_eq!(block.parent(), parent, "parent pointer of {child}");
		assert!(
			doc.schema().allows_child(parent_kind, block.kind()),
			"schema allows {child} here"
		);
		check_children(doc, Some(child), seen);
	}
}

fn check_block(doc: &Document, block: &Block) {
	let Content::Inline(text) = block.content() else {
		return;
	};
	let mut end = 0;
	let mut previous: Option<(MarkSet, bool)> = None;

	for run in text.runs() {
		assert_eq!(
			run.range.start,
			end,
			"runs are contiguous in {}",
			block.id()
		);
		assert!(
			run.range.end > run.range.start,
			"runs are non-empty in {}",
			block.id()
		);
		let is_embed = run.embed.is_some();

		if let Some((marks, was_embed)) = &previous {
			let mergeable = !is_embed && !was_embed && marks == run.marks;
			assert!(!mergeable, "adjacent text runs differ in {}", block.id());
		}

		previous = Some((run.marks.clone(), is_embed));
		end = run.range.end;
	}

	assert_eq!(end, text.len(), "runs cover the text of {}", block.id());
	assert!(doc.schema().block(block.kind()).content == canvist_model::ContentKind::Inline);
}
