//! Shared fixtures for unit tests.

use std::sync::Arc;

use crate::BlockId;
use crate::BlockSpec;
use crate::BlockTree;
use crate::Document;
use crate::EmbedSpec;
use crate::InlineText;
use crate::MarkSet;
use crate::MarkSpec;
use crate::Schema;

/// Paragraphs, headings, nested bullet lists, a divider, three marks, and
/// a mention embed.
pub(crate) fn schema() -> Arc<Schema> {
	let schema = Schema::builder()
		.block(BlockSpec::text("paragraph"))
		.block(BlockSpec::text("heading"))
		.block(BlockSpec::container("bullet_list", &["list_item"]))
		.block(BlockSpec::container("list_item", &["paragraph", "bullet_list"]).groups(&[]))
		.block(BlockSpec::leaf("divider"))
		.mark(MarkSpec::new("bold"))
		.mark(MarkSpec::new("italic"))
		.mark(MarkSpec::new("link").inclusive(false))
		.embed(EmbedSpec::new("mention"))
		.build();
	Arc::new(schema.expect("the test schema is valid"))
}

/// `intro`, a two-item bullet list (`one`, `two`), and `outro`.
pub(crate) fn nested_doc() -> Document {
	let schema = schema();
	let kind = |name| schema.block_type(name).expect("type exists");
	let mut next = 0;
	let mut id = || {
		next += 1;
		BlockId::new(9, next)
	};
	let mut paragraph = |text| {
		BlockTree::text(
			id(),
			kind("paragraph"),
			InlineText::plain(text, MarkSet::empty()),
		)
	};
	let blocks = [
		paragraph("intro"),
		paragraph("one"),
		paragraph("two"),
		paragraph("outro"),
	];
	let [intro, one, two, outro] = blocks;
	let item_one = BlockTree::container(BlockId::new(9, 100), kind("list_item"), vec![one]);

	let item_two = BlockTree::container(BlockId::new(9, 101), kind("list_item"), vec![two]);
	let list = BlockTree::container(
		BlockId::new(9, 102),
		kind("bullet_list"),
		vec![item_one, item_two],
	);

	let mut doc = Document::empty(schema.clone(), 1);

	for (index, tree) in [intro, list, outro].into_iter().enumerate() {
		doc.insert_tree(None, index, tree);
	}

	doc
}
