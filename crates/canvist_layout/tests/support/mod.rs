//! Fixtures shared by integration tests and benchmarks.

#![allow(dead_code)]

use std::fmt::Write;
use std::sync::Arc;

use canvist_layout::DisplayItem;
use canvist_layout::DisplayList;
use canvist_layout::DocumentLayout;
use canvist_layout::FixedMeasurer;
use canvist_layout::LayoutOptions;
use canvist_layout::Rect;
use canvist_layout::TextRun;
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

/// The schema of TipTap's `StarterKit`, plus task lists, tables, images, highlights, scripts, text
/// styles, and mentions, using TipTap's type names.
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
		.block(BlockSpec::container("table", &["tableRow"]))
		.block(BlockSpec::container("tableRow", &["tableCell", "tableHeader"]).groups(&[]))
		.block(
			BlockSpec::container("tableCell", &["block"])
				.attrs(cell_attrs())
				.groups(&[]),
		)
		.block(
			BlockSpec::container("tableHeader", &["block"])
				.attrs(cell_attrs())
				.groups(&[]),
		)
		.mark(MarkSpec::new("bold"))
		.mark(MarkSpec::new("italic"))
		.mark(MarkSpec::new("underline"))
		.mark(MarkSpec::new("strike"))
		.mark(MarkSpec::new("code"))
		.mark(MarkSpec::new("link").inclusive(false))
		.mark(MarkSpec::new("highlight"))
		.mark(MarkSpec::new("subscript"))
		.mark(MarkSpec::new("superscript"))
		.mark(MarkSpec::new("textStyle"))
		.embed(EmbedSpec::new("hardBreak"))
		.embed(EmbedSpec::new("mention"))
		.embed(EmbedSpec::new("inlineImage"))
		.build();
	Arc::new(schema.expect("the support schema is valid"))
}

/// TipTap's table cell attributes and their defaults.
fn cell_attrs() -> Attrs {
	Attrs::new()
		.with("colspan", 1)
		.with("rowspan", 1)
		.with("colwidth", canvist_model::Value::Null)
}

/// A TipTap table of rows of cells, each cell a paragraph of its text.
/// Cells written `"text|2x1"` span two columns and one row.
pub fn table(rows: &[&[&str]]) -> Json {
	let cell = |text: &str| {
		let (text, spans) = text.split_once('|').unwrap_or((text, "1x1"));
		let (colspan, rowspan) = spans.split_once('x').expect("spans are written CxR");
		serde_json::json!({
			"type": "tableCell",
			"attrs": {
				"colspan": colspan.parse::<i64>().expect("a number"),
				"rowspan": rowspan.parse::<i64>().expect("a number"),
			},
			"content": [paragraph(text)],
		})
	};
	let rows: Vec<Json> = rows
		.iter()
		.map(|row| serde_json::json!({ "type": "tableRow", "content": row.iter().map(|text| cell(text)).collect::<Vec<_>>() }))
		.collect();

	serde_json::json!({ "type": "table", "content": rows })
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

/// A compact, stable description of a display list, one line per item.
pub fn describe(list: &DisplayList) -> String {
	let mut out = String::new();

	for item in &list.items {
		let line = describe_item(list, item);
		writeln!(out, "{line}").expect("writing to a String cannot fail");
	}

	out
}

/// One display item, without the trailing newline.
fn describe_item(list: &DisplayList, item: &DisplayItem) -> String {
	match item {
		DisplayItem::Rect {
			rect, color, kind, ..
		} => format!("rect {kind:?} {} {color}", describe_rect(rect)),
		DisplayItem::Text(run) => describe_text(list, run),
		DisplayItem::Decoration {
			kind, x, y, width, ..
		} => format!("line {kind:?} x={} y={} w={}", r(*x), r(*y), r(*width)),
		DisplayItem::Slot {
			kind, rect, offset, ..
		} => {
			let offset = offset
				.map(|offset| format!(" @{offset}"))
				.unwrap_or_default();
			format!("slot {kind:?} {}{offset}", describe_rect(rect))
		}
	}
}

/// A rectangle as `x y {width}x{height}`, rounded.
fn describe_rect(rect: &Rect) -> String {
	let size = format!("{}x{}", r(rect.width), r(rect.height));
	format!("{} {} {size}", r(rect.x), r(rect.y))
}

/// A text run with its position, width, and font.
fn describe_text(list: &DisplayList, run: &TextRun) -> String {
	let font = &list.fonts[run.font.index()];
	let italic = if font.italic { " italic" } else { "" };
	let rtl = if run.rtl { " rtl" } else { "" };
	let position = format!("x={} base={} w={}", r(run.x), r(run.baseline), r(run.width));
	let style = format!("{}px/{}{italic}{rtl}", font.size, font.weight);

	format!("text {:?} {position} {style}", run.text())
}

/// Round to 1/100 px for stable snapshots.
pub fn r(value: f32) -> f32 {
	(value * 100.0).round() / 100.0
}
