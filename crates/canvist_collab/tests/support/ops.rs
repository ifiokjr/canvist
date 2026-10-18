//! Random edits for property tests.
//!
//! An [`Op`] is four random numbers. They are interpreted against a peer's
//! current document, picking targets by modulo, so every value is a
//! meaningful edit in every state and proptest can shrink them freely.

use std::ops::Range;

use canvist_model::Attrs;
use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Content;
use canvist_model::Document;
use canvist_model::Embed;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Schema;
use canvist_model::Step;

use super::Peer;

/// How many kinds of edit there are; `Op::kind` picks one by modulo.
const KINDS: u8 = 10;
/// One in this many inserts is an embed rather than text.
const EMBED_ONE_IN: u32 = 7;
/// One in this many inserts carries a mark.
const MARKED_ONE_IN: u32 = 3;
/// One in this many block insertions or moves targets the root.
const ROOT_ONE_IN: u32 = 3;
/// Inserted text, including multi-byte and surrogate-pair characters.
const SNIPPETS: [&str; 5] = ["a", "xyz", "é", "😀", " fox "];
/// Heading levels and mention ids cycle through this many values.
const VARIANTS: u32 = 3;

/// One random edit.
#[derive(Clone, Copy, Debug)]
pub struct Op {
	/// Which kind of edit.
	pub kind: u8,
	/// Usually picks the target block.
	pub a: u32,
	/// Usually picks an offset or index.
	pub b: u32,
	/// Usually picks content or a second offset.
	pub c: u32,
}

impl Op {
	/// Apply this edit to `peer`, returning whether the model accepted it.
	pub fn apply(self, peer: &mut Peer) -> bool {
		let Some(step) = self.step(&mut peer.doc) else {
			return false;
		};

		peer.step(step).is_some()
	}

	/// The step this edit means in `doc`, if any block fits it.
	fn step(self, doc: &mut Document) -> Option<Step> {
		match self.kind % KINDS {
			0 | 1 => self.insert_inline(doc),
			2 => self.remove_inline(doc),
			3 => self.format(doc),
			4 => self.set_block(doc),
			5 => self.split(doc),
			6 => self.join(doc),
			7 => self.insert_blocks(doc),
			8 => self.remove_blocks(doc),
			_ => self.move_blocks(doc),
		}
	}

	fn text_block(self, doc: &Document) -> Option<BlockId> {
		let texts: Vec<BlockId> = doc.text_blocks().map(Block::id).collect();
		pick(&texts, self.a)
	}

	fn any_block(self, doc: &Document) -> Option<BlockId> {
		let blocks: Vec<BlockId> = doc.iter().map(Block::id).collect();
		pick(&blocks, self.a)
	}

	/// A character range of `block` picked by `b` and `c`.
	fn range(self, doc: &Document, block: BlockId) -> Option<Range<u32>> {
		let text = doc.inline(block)?;
		let (start, end) = (boundary(text, self.b), boundary(text, self.c));
		Some(start.min(end)..start.max(end))
	}

	fn insert_inline(self, doc: &Document) -> Option<Step> {
		let block = self.text_block(doc)?;
		let offset = boundary(doc.inline(block)?, self.b);
		let schema = doc.schema();
		let content = if self.c.is_multiple_of(EMBED_ONE_IN) {
			let mention = Embed {
				kind: schema.embed_type("mention")?,
				attrs: Attrs::new().with("id", i64::from(self.c % VARIANTS)),
			};
			InlineText::embed(mention, MarkSet::empty())
		} else {
			let snippet = SNIPPETS[self.c as usize % SNIPPETS.len()];
			InlineText::plain(snippet, random_marks(schema, self.c))
		};

		Some(Step::InsertInline {
			block,
			offset,
			content,
		})
	}

	fn remove_inline(self, doc: &Document) -> Option<Step> {
		let block = self.text_block(doc)?;
		let range = self.range(doc, block)?;
		Some(Step::RemoveInline { block, range })
	}

	/// Add or remove a mark; links get an href so mark attributes travel.
	fn format(self, doc: &Document) -> Option<Step> {
		let block = self.text_block(doc)?;
		let range = self.range(doc, block)?;
		let schema = doc.schema();
		let marks = schema.marks();
		let kind = marks[self.c as usize % marks.len()].id;

		if !self.c.is_multiple_of(2) {
			return Some(Step::RemoveMark { block, range, kind });
		}

		let attrs = if schema.mark(kind).name.as_ref() == "link" {
			Attrs::new().with("href", format!("https://{}.dev", self.b % VARIANTS))
		} else {
			Attrs::new()
		};

		Some(Step::AddMark {
			block,
			range,
			mark: Mark::with_attrs(kind, attrs),
		})
	}

	/// Retype a block to another type with the same kind of content.
	fn set_block(self, doc: &Document) -> Option<Step> {
		let block = self.any_block(doc)?;
		let options: &[&str] = match doc.block(block)?.content() {
			Content::Inline(_) => &["paragraph", "heading"],
			Content::Blocks(_) => &["bullet_list", "ordered_list", "blockquote"],
			Content::Leaf => return None,
		};
		let schema = doc.schema();
		let kind = schema.block_type(options[self.b as usize % options.len()])?;
		let attrs = if schema.block(kind).name.as_ref() == "heading" {
			Attrs::new().with("level", i64::from(self.c % VARIANTS + 1))
		} else {
			Attrs::new()
		};

		Some(Step::SetBlock { block, kind, attrs })
	}

	fn split(self, doc: &mut Document) -> Option<Step> {
		let block = self.text_block(doc)?;
		let offset = boundary(doc.inline(block)?, self.b);
		let found = doc.block(block)?;
		let (kind, attrs) = (found.kind(), found.attrs().clone());

		Some(Step::Split {
			block,
			offset,
			new_block: doc.allocate_id().ok()?,
			kind,
			attrs,
		})
	}

	fn join(self, doc: &Document) -> Option<Step> {
		let block = self.text_block(doc)?;
		let parent = doc.block(block)?.parent();
		let index = doc.index_in_parent(block)?;
		let next = *doc.children(parent).get(index + 1)?;
		Some(Step::Join { block, next })
	}

	fn insert_blocks(self, doc: &mut Document) -> Option<Step> {
		let (parent, index) = slot(doc, self.a, self.b);
		let tree = new_tree(doc, parent, self.c)?;

		Some(Step::InsertBlocks {
			parent,
			index,
			blocks: vec![tree],
		})
	}

	fn remove_blocks(self, doc: &Document) -> Option<Step> {
		let block = self.any_block(doc)?;
		let parent = doc.block(block)?.parent();
		let index = doc.index_in_parent(block)?;
		let following = doc.children(parent).len() - index - 1;

		Some(Step::RemoveBlocks {
			parent,
			index,
			count: 1 + (self.b as usize % 2).min(following),
		})
	}

	fn move_blocks(self, doc: &Document) -> Option<Step> {
		let block = self.any_block(doc)?;
		let parent = doc.block(block)?.parent();
		let index = doc.index_in_parent(block)?;
		let (to_parent, to_index) = slot(doc, self.b, self.c);
		let remaining = doc.children(to_parent).len() - usize::from(parent == to_parent);

		Some(Step::MoveBlocks {
			parent,
			index,
			count: 1,
			to_parent,
			to_index: to_index.min(remaining),
		})
	}
}

fn pick(list: &[BlockId], value: u32) -> Option<BlockId> {
	list.get(value as usize % list.len().max(1)).copied()
}

/// A random container (or the root) and a child index in it.
fn slot(doc: &Document, which: u32, index: u32) -> (Option<BlockId>, usize) {
	let containers: Vec<BlockId> = doc
		.iter()
		.filter(|block| matches!(block.content(), Content::Blocks(_)))
		.map(Block::id)
		.collect();
	let parent = if which.is_multiple_of(ROOT_ONE_IN) || containers.is_empty() {
		None
	} else {
		Some(containers[which as usize % containers.len()])
	};

	let len = doc.children(parent).len();
	(parent, index as usize % (len + 1))
}

/// A small new block suited to `parent`: a list item inside lists, else a
/// paragraph, a divider, or a list.
fn new_tree(doc: &mut Document, parent: Option<BlockId>, choice: u32) -> Option<BlockTree> {
	let schema = doc.schema().clone();
	let parent_kind = parent.and_then(|parent| doc.block(parent)).map(Block::kind);
	let list_item = schema.block_type("list_item")?;

	if parent_kind.is_some_and(|parent| schema.allows_child(Some(parent), list_item)) {
		return item(doc, &schema, "item");
	}

	match choice % 3 {
		0 => paragraph(doc, &schema, "new"),
		1 => {
			let divider = schema.block_type("divider")?;
			Some(BlockTree::leaf(doc.allocate_id().ok()?, divider))
		}
		_ => {
			let item = item(doc, &schema, "inner")?;
			let list = schema.block_type("bullet_list")?;
			Some(BlockTree::container(
				doc.allocate_id().ok()?,
				list,
				vec![item],
			))
		}
	}
}

fn paragraph(doc: &mut Document, schema: &Schema, text: &str) -> Option<BlockTree> {
	Some(BlockTree::text(
		doc.allocate_id().ok()?,
		schema.block_type("paragraph")?,
		InlineText::plain(text, MarkSet::empty()),
	))
}

fn item(doc: &mut Document, schema: &Schema, text: &str) -> Option<BlockTree> {
	let paragraph = paragraph(doc, schema, text)?;
	let kind = schema.block_type("list_item")?;
	Some(BlockTree::container(
		doc.allocate_id().ok()?,
		kind,
		vec![paragraph],
	))
}

fn random_marks(schema: &Schema, seed: u32) -> MarkSet {
	if !seed.is_multiple_of(MARKED_ONE_IN) {
		return MarkSet::empty();
	}

	let marks = schema.marks();
	MarkSet::from_marks([Mark::new(marks[seed as usize % 2].id)])
}

/// A character boundary of `text` chosen by `value`.
fn boundary(text: &InlineText, value: u32) -> u32 {
	let boundaries: Vec<u32> = text
		.as_str()
		.char_indices()
		.map(|(index, _)| index as u32)
		.chain([text.len()])
		.collect();
	boundaries[value as usize % boundaries.len()]
}
