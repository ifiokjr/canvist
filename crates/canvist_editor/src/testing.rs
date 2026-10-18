//! Shared fixtures for unit tests.

use std::sync::Arc;

use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockSpec;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::EmbedSpec;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::MarkSpec;
use canvist_model::Schema;

use crate::EditorState;

/// A StarterKit-shaped schema with TipTap's names.
pub(crate) fn schema() -> Arc<Schema> {
	let schema = Schema::builder()
		.block(BlockSpec::text("paragraph"))
		.block(BlockSpec::text("heading").attrs(Attrs::new().with("level", 1)))
		.block(BlockSpec::container("blockquote", &["block"]))
		.block(BlockSpec::container("bulletList", &["listItem"]))
		.block(BlockSpec::container("orderedList", &["listItem"]))
		.block(
			BlockSpec::container("listItem", &["paragraph", "bulletList", "orderedList"])
				.groups(&[]),
		)
		.block(BlockSpec::leaf("horizontalRule"))
		.mark(MarkSpec::new("bold"))
		.mark(MarkSpec::new("italic"))
		.mark(MarkSpec::new("code").excludes(&["bold", "italic"]))
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
	let mut paragraph = |text| {
		next += 1;
		BlockTree::text(
			BlockId::new(9, next),
			kind("paragraph"),
			InlineText::plain(text, MarkSet::empty()),
		)
	};
	let [intro, one, two, outro] = [
		paragraph("intro"),
		paragraph("one"),
		paragraph("two"),
		paragraph("outro"),
	];
	let item =
		|seq, child| BlockTree::container(BlockId::new(9, seq), kind("listItem"), vec![child]);
	let list = BlockTree::container(
		BlockId::new(9, 102),
		kind("bulletList"),
		vec![item(100, one), item(101, two)],
	);
	let mut doc = Document::empty(schema.clone(), 1);

	doc.transaction()
		.insert_blocks(None, 0, vec![intro, list, outro])
		.expect("the nested fixture is valid");
	doc
}

/// An editor state over [`nested_doc`].
pub(crate) fn nested_state() -> EditorState {
	EditorState::new(nested_doc()).expect("fixture state")
}
