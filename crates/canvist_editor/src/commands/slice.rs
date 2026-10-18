//! Slices: the unit of copy, cut, paste, and drop.

use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::EMBED_CHAR;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Selection;
use canvist_model::StepError;
use canvist_model::TreeContent;

use crate::EditorError;
use crate::EditorState;
use crate::position;

/// A detached piece of a document.
///
/// When the first or last block is a text block it is *open*: pasting
/// merges its text into the block at the caret instead of adding a new
/// block, which is how copying half a paragraph and pasting it mid-sentence
/// works. Block ids in a slice are placeholders; pasting assigns fresh ones.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Slice {
	blocks: Vec<BlockTree>,
}

impl Slice {
	/// A slice of `blocks`.
	#[must_use]
	pub fn new(blocks: Vec<BlockTree>) -> Self {
		Self { blocks }
	}

	/// One open text block of the schema's default type, for inline content.
	#[must_use]
	pub fn inline(schema: &Schema, text: InlineText) -> Self {
		let kind = schema.default_text_block();
		Self::new(vec![BlockTree::text(BlockId::new(0, 0), kind, text)])
	}

	/// One default text block per line of `text`.
	#[must_use]
	pub fn from_plain_text(schema: &Schema, text: &str) -> Self {
		let kind = schema.default_text_block();
		let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
		let blocks = normalized
			.split('\n')
			.enumerate()
			.map(|(index, line)| {
				let id = BlockId::new(0, index as u32);
				BlockTree::text(id, kind, InlineText::plain(line, MarkSet::empty()))
			})
			.collect();

		Self::new(blocks)
	}

	/// The slice's blocks.
	#[must_use]
	pub fn blocks(&self) -> &[BlockTree] {
		&self.blocks
	}

	/// Mutable access to the blocks, e.g. for paste rules.
	pub fn blocks_mut(&mut self) -> &mut Vec<BlockTree> {
		&mut self.blocks
	}

	/// Release the blocks.
	#[must_use]
	pub fn into_blocks(self) -> Vec<BlockTree> {
		self.blocks
	}

	/// Whether the slice holds nothing.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.blocks.is_empty()
	}

	/// The text of every text block, one per line, without embeds.
	#[must_use]
	pub fn to_plain_text(&self) -> String {
		let mut lines = Vec::new();

		for tree in &self.blocks {
			tree.walk(&mut |node| {
				lines.extend(inline_of(node).map(|text| text.as_str().replace(EMBED_CHAR, "")));
			});
		}

		lines.join("\n")
	}
}

/// Copy the selection into a slice. Containers that are only partly
/// selected are kept around their selected children, so copying two list
/// items yields a list holding those items.
pub fn slice_selection(state: &EditorState) -> Result<Slice, EditorError> {
	let doc = state.doc();

	match state.selection() {
		Selection::Blocks { anchor, head } => {
			let Some((parent, range)) = position::sibling_range(doc, anchor, head)
				.or_else(|| position::sibling_range(doc, head, anchor))
			else {
				return Ok(Slice::default());
			};
			let trees = doc.children(parent)[range]
				.iter()
				.filter_map(|&id| doc.tree(id))
				.collect();
			Ok(Slice::new(wrap_for_root(doc, parent, trees)))
		}
		selection @ Selection::Text { .. } => {
			let Some(range) =
				position::text_range(doc, selection).filter(|range| !range.is_empty())
			else {
				return Ok(Slice::default());
			};
			let Some((parent, children)) =
				position::sibling_range(doc, range.from.block, range.to.block)
			else {
				return Ok(Slice::default());
			};
			let trees = doc.children(parent)[children]
				.iter()
				.map(|&id| cut(doc, id, range.from, range.to))
				.collect::<Result<Vec<_>, _>>()?;

			if range.from.block == range.to.block {
				return Ok(Slice::new(trees));
			}

			Ok(Slice::new(wrap_for_root(doc, parent, trees)))
		}
	}
}

/// Copy block `id`, keeping only what lies between `from` and `to`.
fn cut(doc: &Document, id: BlockId, from: Point, to: Point) -> Result<BlockTree, EditorError> {
	let block = doc.block(id).ok_or(StepError::UnknownBlock(id))?;
	let contains = |point: Point| point.block == id || doc.is_ancestor(id, point.block);
	let mut tree = BlockTree::leaf(id, block.kind()).with_attrs(block.attrs().clone());

	if let Some(text) = block.inline() {
		let start = if from.block == id { from.offset } else { 0 };
		let end = if to.block == id {
			to.offset
		} else {
			text.len()
		};

		tree.content = TreeContent::Inline(text.slice(start..end).map_err(StepError::from)?);

		return Ok(tree);
	}

	let children = block.children();

	if children.is_empty() {
		return Ok(doc.tree(id).unwrap_or(tree));
	}

	let child_with = |point: Point| {
		children
			.iter()
			.position(|&child| child == point.block || doc.is_ancestor(child, point.block))
	};
	let start = if contains(from) {
		child_with(from).unwrap_or(0)
	} else {
		0
	};
	let end = if contains(to) {
		child_with(to).unwrap_or(children.len() - 1)
	} else {
		children.len() - 1
	};

	let kept = children[start..=end]
		.iter()
		.map(|&child| cut(doc, child, from, to))
		.collect::<Result<Vec<_>, _>>()?;
	tree.content = TreeContent::Blocks(kept);
	Ok(tree)
}

/// Wrap `trees` (children of `parent`) in copies of their ancestors until
/// the root accepts them.
fn wrap_for_root(doc: &Document, parent: Option<BlockId>, trees: Vec<BlockTree>) -> Vec<BlockTree> {
	let schema = doc.schema();
	let mut trees = trees;
	let mut current = parent;

	while let Some(container) = current {
		if trees
			.iter()
			.all(|tree| schema.allows_child(None, tree.kind))
		{
			break;
		}

		let Some(block) = doc.block(container) else {
			break;
		};
		let wrapper =
			BlockTree::container(container, block.kind(), trees).with_attrs(block.attrs().clone());
		trees = vec![wrapper];
		current = block.parent();
	}

	trees
}

/// The inline content of a text block tree.
pub(crate) fn inline_of(tree: &BlockTree) -> Option<&InlineText> {
	match &tree.content {
		TreeContent::Inline(text) => Some(text),
		TreeContent::Blocks(_) | TreeContent::Leaf => None,
	}
}
