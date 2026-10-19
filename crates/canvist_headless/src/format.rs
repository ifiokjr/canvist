//! Reading and writing documents as JSON, HTML, Markdown, and plain text.

use std::str::FromStr;
use std::sync::Arc;

use canvist_editor::Slice;
use canvist_extensions::clipboard;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::Schema;
use canvist_model::Step;
use canvist_model::TreeContent;
use serde_json::Value as Json;

use crate::HeadlessError;

/// A text format documents are read from and written to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
	/// ProseMirror and TipTap JSON (`{ "type": "doc", "content": [...] }`).
	/// Block ids travel in `attrs.id`.
	Json,
	/// Semantic HTML: `<p>`, `<h1>`, `<ul>`, `<strong>`, and so on.
	Html,
	/// Markdown, with GitHub task lists and strikethrough.
	Markdown,
	/// Plain text: one paragraph per line.
	Text,
}

impl Format {
	/// Every format.
	pub const ALL: [Self; 4] = [Self::Json, Self::Html, Self::Markdown, Self::Text];

	/// The format's name as the bindings spell it: `json`, `html`,
	/// `markdown`, or `text`.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Json => "json",
			Self::Html => "html",
			Self::Markdown => "markdown",
			Self::Text => "text",
		}
	}
}

impl std::fmt::Display for Format {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str(self.name())
	}
}

impl FromStr for Format {
	type Err = HeadlessError;

	fn from_str(name: &str) -> Result<Self, Self::Err> {
		Self::ALL
			.into_iter()
			.find(|format| format.name() == name)
			.ok_or_else(|| HeadlessError::parse("format", format!("unknown format `{name}`")))
	}
}

/// Read `input` as `format` into a new document whose ids come from
/// `client`.
pub(crate) fn read(
	schema: &Arc<Schema>,
	client: u32,
	format: Format,
	input: &str,
) -> Result<Document, HeadlessError> {
	match format {
		Format::Json => {
			let json: Json =
				serde_json::from_str(input).map_err(|error| HeadlessError::parse("json", error))?;
			Ok(Document::from_json(schema.clone(), client, &json)?)
		}
		Format::Html => from_slice(schema, client, clipboard::from_html(schema, input)),
		Format::Markdown => from_slice(schema, client, clipboard::from_markdown(schema, input)),
		Format::Text => from_slice(schema, client, clipboard::from_plain_text(schema, input)),
	}
}

/// Read `input` as `format` into a slice to paste.
pub(crate) fn read_slice(
	schema: &Arc<Schema>,
	format: Format,
	input: &str,
) -> Result<Slice, HeadlessError> {
	Ok(match format {
		Format::Json => whole(&read(schema, 0, format, input)?),
		Format::Html => clipboard::from_html(schema, input),
		Format::Markdown => clipboard::from_markdown(schema, input),
		Format::Text => clipboard::from_plain_text(schema, input),
	})
}

/// Write `doc` as `format`.
pub(crate) fn write(doc: &Document, format: Format) -> String {
	let schema = doc.schema();

	match format {
		Format::Json => doc.to_json().to_string(),
		Format::Html => clipboard::to_html(schema, &whole(doc)),
		Format::Markdown => clipboard::to_markdown(schema, &whole(doc)),
		Format::Text => clipboard::to_plain_text(schema, &whole(doc)),
	}
}

/// The whole document as a slice, the shape the codecs write.
fn whole(doc: &Document) -> Slice {
	let blocks = doc
		.children(None)
		.iter()
		.filter_map(|&id| doc.tree(id))
		.collect();
	Slice::new(blocks)
}

/// A document holding the slice's blocks, with ids allocated in document
/// order. The codecs give slices placeholder ids.
fn from_slice(schema: &Arc<Schema>, client: u32, slice: Slice) -> Result<Document, HeadlessError> {
	let mut doc = Document::empty(schema.clone(), client);
	let blocks = slice
		.into_blocks()
		.into_iter()
		.map(|tree| with_fresh_ids(&mut doc, tree))
		.collect::<Result<Vec<_>, _>>()?;

	if !blocks.is_empty() {
		doc.apply_steps([Step::InsertBlocks {
			parent: None,
			index: 0,
			blocks,
		}])?;
	}

	Ok(doc)
}

fn with_fresh_ids(doc: &mut Document, tree: BlockTree) -> Result<BlockTree, HeadlessError> {
	let id = doc.allocate_id()?;
	let content = match tree.content {
		TreeContent::Blocks(children) => {
			let children = children
				.into_iter()
				.map(|child| with_fresh_ids(doc, child))
				.collect::<Result<_, _>>()?;
			TreeContent::Blocks(children)
		}
		content => content,
	};

	Ok(BlockTree {
		id,
		content,
		..tree
	})
}
