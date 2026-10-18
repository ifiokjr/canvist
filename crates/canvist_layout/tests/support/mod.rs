//! Fixtures shared by integration tests and benchmarks.

#![allow(dead_code)]

use std::sync::Arc;

use canvist_layout::DocumentLayout;
use canvist_layout::FixedMeasurer;
use canvist_layout::LayoutOptions;
use canvist_layout::Theme;
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
use serde_json::Value as Json;

/// The schema of TipTap's `StarterKit`, plus task lists, images, highlights, and
/// mentions, using TipTap's type names.
pub fn schema() -> Arc<Schema> {
	let schema = Schema::builder()
		.block(BlockSpec::text("paragraph"))
		.block(BlockSpec::text("heading").attrs(Attrs::new().with("level", 1)))
		.block(BlockSpec::container("blockquote", &["block"]))
		.block(BlockSpec::text("codeBlock"))
		.block(BlockSpec::leaf("horizontalRule"))
		.block(BlockSpec::leaf("image"))
		.block(BlockSpec::container("bulletList", &["listItem"]))
		.block(
			BlockSpec::container("orderedList", &["listItem"]).attrs(Attrs::new().with("start", 1)),
		)
		.block(
			BlockSpec::container("listItem", &["paragraph", "bulletList", "orderedList"])
				.groups(&[]),
		)
		.block(BlockSpec::container("taskList", &["taskItem"]))
		.block(
			BlockSpec::container("taskItem", &["paragraph", "taskList"])
				.attrs(Attrs::new().with("checked", false))
				.groups(&[]),
		)
		.mark(MarkSpec::new("bold"))
		.mark(MarkSpec::new("italic"))
		.mark(MarkSpec::new("underline"))
		.mark(MarkSpec::new("strike"))
		.mark(MarkSpec::new("code"))
		.mark(MarkSpec::new("link").inclusive(false))
		.mark(MarkSpec::new("highlight"))
		.embed(EmbedSpec::new("hardBreak"))
		.embed(EmbedSpec::new("mention"))
		.embed(EmbedSpec::new("inlineImage"))
		.build();
	Arc::new(schema.expect("the support schema is valid"))
}

/// A document from TipTap JSON content (the `content` array of a `doc`).
pub fn doc(content: Json) -> Document {
	let mut json = serde_json::Map::new();
	json.insert("type".to_owned(), Json::from("doc"));
	json.insert("content".to_owned(), content);
	Document::from_json(schema(), 1, &Json::Object(json)).expect("valid TipTap JSON")
}

/// A document of paragraphs, one per string.
pub fn paragraphs(texts: &[&str]) -> Document {
	doc(Json::Array(
		texts.iter().map(|text| paragraph(text)).collect(),
	))
}

/// A TipTap paragraph node, empty when `text` is.
pub fn paragraph(text: &str) -> Json {
	if text.is_empty() {
		return serde_json::json!({ "type": "paragraph" });
	}

	serde_json::json!({ "type": "paragraph", "content": [{ "type": "text", "text": text }] })
}

/// A document with `count` copies of `text`, built directly for speed.
pub fn large(count: usize, text: &str) -> Document {
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

/// A fully laid-out document at `width` with the default theme.
pub fn layout(doc: &Document, width: f32) -> DocumentLayout {
	let mut layout = DocumentLayout::new(
		doc,
		Arc::new(Theme::starter_kit()),
		LayoutOptions::new(width),
	);
	layout
		.layout_all(doc, &mut FixedMeasurer::new())
		.expect("the fixed measurer honours the contract");
	layout
}

/// Round to 1/100 px for stable snapshots.
pub fn r(value: f32) -> f32 {
	(value * 100.0).round() / 100.0
}
