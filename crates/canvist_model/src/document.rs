//! The document: an arena of blocks addressed by stable ids.

use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::Block;
use crate::BlockId;
use crate::BlockTree;
use crate::Content;
use crate::IdGenerator;
use crate::InlineText;
use crate::Schema;
use crate::StepError;
use crate::TreeContent;

/// A rich-text document.
///
/// Blocks live in a hash map keyed by [`BlockId`], and each container (plus
/// the root) keeps an ordered list of child ids. Typing touches exactly one
/// block, so its cost is independent of document size; only structural edits
/// touch child lists.
///
/// Documents are only mutated through [`Step`](crate::Step)s, usually via a
/// [`Transaction`](crate::Transaction), which keeps undo, collaboration, and
/// rendering on one pipeline.
#[derive(Clone, Debug)]
pub struct Document {
	schema: Arc<Schema>,
	blocks: FxHashMap<BlockId, Block>,
	root: Vec<BlockId>,
	ids: IdGenerator,
	version: u64,
}

impl Document {
	/// A document with a single empty text block of the schema's default
	/// text type. `client` seeds block id allocation and must be unique among
	/// collaborators.
	#[must_use]
	pub fn new(schema: Arc<Schema>, client: u32) -> Self {
		let mut doc = Self::empty(schema, client);
		let kind = doc.schema.default_text_block();

		if let Ok(id) = doc.allocate_id() {
			doc.insert_tree(None, 0, BlockTree::text(id, kind, InlineText::new()));
		}

		doc
	}

	/// A document with no blocks at all.
	#[must_use]
	pub fn empty(schema: Arc<Schema>, client: u32) -> Self {
		Self {
			schema,
			blocks: FxHashMap::default(),
			root: Vec::new(),
			ids: IdGenerator::new(client),
			version: 0,
		}
	}

	/// The document's schema.
	#[must_use]
	pub fn schema(&self) -> &Arc<Schema> {
		&self.schema
	}

	/// A counter bumped by every applied step.
	#[must_use]
	pub fn version(&self) -> u64 {
		self.version
	}

	/// The number of blocks, including nested ones.
	#[must_use]
	pub fn block_count(&self) -> usize {
		self.blocks.len()
	}

	/// The block with `id`, if it exists.
	#[must_use]
	pub fn block(&self, id: BlockId) -> Option<&Block> {
		self.blocks.get(&id)
	}

	/// Whether a block with `id` exists.
	#[must_use]
	pub fn contains(&self, id: BlockId) -> bool {
		self.blocks.contains_key(&id)
	}

	/// The inline text of block `id`, if it is a text block.
	#[must_use]
	pub fn inline(&self, id: BlockId) -> Option<&InlineText> {
		self.block(id)?.inline()
	}

	/// The children of `parent`, or of the root when `parent` is `None`.
	/// Unknown and non-container parents have no children.
	#[must_use]
	pub fn children(&self, parent: Option<BlockId>) -> &[BlockId] {
		match parent {
			None => &self.root,
			Some(id) => self.block(id).map_or(&[], Block::children),
		}
	}

	/// The position of block `id` among its siblings.
	#[must_use]
	pub fn index_in_parent(&self, id: BlockId) -> Option<usize> {
		let parent = self.block(id)?.parent;
		self.children(parent)
			.iter()
			.position(|&sibling| sibling == id)
	}

	/// Whether `ancestor` contains `id` (a block is not its own ancestor).
	#[must_use]
	pub fn is_ancestor(&self, ancestor: BlockId, id: BlockId) -> bool {
		let mut current = self.block(id).and_then(Block::parent);

		while let Some(parent) = current {
			if parent == ancestor {
				return true;
			}

			current = self.block(parent).and_then(Block::parent);
		}

		false
	}

	/// Every block in document (pre-)order, in `O(blocks)` total.
	pub fn iter(&self) -> impl Iterator<Item = &Block> {
		let mut stack: Vec<std::slice::Iter<'_, BlockId>> = vec![self.root.iter()];
		std::iter::from_fn(move || {
			loop {
				let level = stack.last_mut()?;
				let Some(id) = level.next() else {
					stack.pop();
					continue;
				};
				let block = self.block(*id)?;
				stack.push(block.children().iter());
				return Some(block);
			}
		})
	}

	/// Every text block in document order.
	pub fn text_blocks(&self) -> impl Iterator<Item = &Block> {
		self.iter().filter(|block| block.inline().is_some())
	}

	/// The block after `id` in document order.
	#[must_use]
	pub fn next_block(&self, id: BlockId) -> Option<BlockId> {
		if let Some(&first) = self.block(id)?.children().first() {
			return Some(first);
		}

		let mut current = id;

		loop {
			let parent = self.block(current)?.parent;
			let siblings = self.children(parent);
			let index = siblings.iter().position(|&sibling| sibling == current)?;

			if let Some(&next) = siblings.get(index + 1) {
				return Some(next);
			}

			current = parent?;
		}
	}

	/// The block before `id` in document order.
	#[must_use]
	pub fn prev_block(&self, id: BlockId) -> Option<BlockId> {
		let parent = self.block(id)?.parent;
		let index = self.index_in_parent(id)?;
		let Some(previous) = index.checked_sub(1) else {
			return parent;
		};
		Some(self.last_descendant(self.children(parent)[previous]))
	}

	/// The next text block after `id` in document order.
	#[must_use]
	pub fn next_text_block(&self, id: BlockId) -> Option<BlockId> {
		std::iter::successors(self.next_block(id), |&id| self.next_block(id))
			.find(|&id| self.inline(id).is_some())
	}

	/// The previous text block before `id` in document order.
	#[must_use]
	pub fn prev_text_block(&self, id: BlockId) -> Option<BlockId> {
		std::iter::successors(self.prev_block(id), |&id| self.prev_block(id))
			.find(|&id| self.inline(id).is_some())
	}

	/// The first text block in the document.
	#[must_use]
	pub fn first_text_block(&self) -> Option<BlockId> {
		self.text_blocks().next().map(Block::id)
	}

	/// The last text block in the document.
	#[must_use]
	pub fn last_text_block(&self) -> Option<BlockId> {
		let last = self.last_descendant(*self.root.last()?);

		if self.inline(last).is_some() {
			return Some(last);
		}

		self.prev_text_block(last)
	}

	/// All text, with text blocks separated by newlines and embeds shown as
	/// the object replacement character.
	#[must_use]
	pub fn plain_text(&self) -> String {
		let texts: Vec<&str> = self
			.text_blocks()
			.filter_map(Block::inline)
			.map(InlineText::as_str)
			.collect();
		texts.join("\n")
	}

	/// Allocate a fresh block id for this document's client.
	pub fn allocate_id(&mut self) -> Result<BlockId, StepError> {
		self.ids.next_id().ok_or(StepError::IdsExhausted)
	}

	/// An owned copy of block `id` and its descendants.
	#[must_use]
	pub fn tree(&self, id: BlockId) -> Option<BlockTree> {
		let block = self.block(id)?;

		let content = match &block.content {
			Content::Inline(text) => TreeContent::Inline(text.clone()),
			Content::Leaf => TreeContent::Leaf,
			Content::Blocks(children) => {
				TreeContent::Blocks(
					children
						.iter()
						.filter_map(|&child| self.tree(child))
						.collect(),
				)
			}
		};

		Some(BlockTree {
			id,
			kind: block.kind,
			attrs: block.attrs.clone(),
			content,
		})
	}

	fn last_descendant(&self, id: BlockId) -> BlockId {
		let mut current = id;

		while let Some(&last) = self
			.block(current)
			.and_then(|block| block.children().last())
		{
			current = last;
		}
		current
	}

	pub(crate) fn observe_id(&mut self, id: BlockId) {
		self.ids.observe(id);
	}

	pub(crate) fn bump_version(&mut self) {
		self.version += 1;
	}

	pub(crate) fn block_mut(&mut self, id: BlockId) -> Result<&mut Block, StepError> {
		self.blocks.get_mut(&id).ok_or(StepError::UnknownBlock(id))
	}

	pub(crate) fn inline_mut(&mut self, id: BlockId) -> Result<&mut InlineText, StepError> {
		let block = self.block_mut(id)?;
		block.touch();

		match &mut block.content {
			Content::Inline(text) => Ok(text),
			_ => Err(StepError::NotTextBlock(id)),
		}
	}

	pub(crate) fn children_mut(
		&mut self,
		parent: Option<BlockId>,
	) -> Result<&mut Vec<BlockId>, StepError> {
		let Some(id) = parent else {
			return Ok(&mut self.root);
		};

		match &mut self.block_mut(id)?.content {
			Content::Blocks(children) => Ok(children),
			_ => Err(StepError::NotContainer(id)),
		}
	}

	/// Insert an already validated tree. Callers check ids and schema rules.
	pub(crate) fn insert_tree(&mut self, parent: Option<BlockId>, index: usize, tree: BlockTree) {
		let id = tree.id;
		self.attach(parent, tree);

		if let Ok(children) = self.children_mut(parent) {
			children.insert(index, id);
		}
	}

	fn attach(&mut self, parent: Option<BlockId>, tree: BlockTree) {
		self.ids.observe(tree.id);

		let content = match tree.content {
			TreeContent::Inline(text) => Content::Inline(text),
			TreeContent::Leaf => Content::Leaf,
			TreeContent::Blocks(children) => {
				let ids = children.iter().map(|child| child.id).collect();
				for child in children {
					self.attach(Some(tree.id), child);
				}

				Content::Blocks(ids)
			}
		};

		self.blocks.insert(
			tree.id,
			Block {
				id: tree.id,
				kind: tree.kind,
				attrs: tree.attrs,
				parent,
				content,
				rev: 0,
			},
		);
	}

	/// Detach block `id` and its descendants, returning them as a tree. The
	/// caller removes `id` from its parent's child list.
	pub(crate) fn detach(&mut self, id: BlockId) -> Result<BlockTree, StepError> {
		let block = self.blocks.remove(&id).ok_or(StepError::UnknownBlock(id))?;

		let content = match block.content {
			Content::Inline(text) => TreeContent::Inline(text),
			Content::Leaf => TreeContent::Leaf,
			Content::Blocks(children) => {
				let trees = children.into_iter().map(|child| self.detach(child));
				TreeContent::Blocks(trees.collect::<Result<_, _>>()?)
			}
		};

		Ok(BlockTree {
			id,
			kind: block.kind,
			attrs: block.attrs,
			content,
		})
	}

	pub(crate) fn set_parent(
		&mut self,
		id: BlockId,
		parent: Option<BlockId>,
	) -> Result<(), StepError> {
		self.block_mut(id)?.parent = parent;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::BlockSpec;
	use crate::MarkSet;
	use crate::testing;

	#[test]
	fn new_documents_have_one_empty_text_block() {
		let doc = Document::new(testing::schema(), 1);
		let first = doc.first_text_block().expect("one text block");

		assert_eq!(doc.block_count(), 1);
		assert_eq!(doc.inline(first).map(InlineText::len), Some(0));
		assert_eq!(doc.last_text_block(), Some(first));
	}

	#[test]
	fn walks_nested_blocks_in_document_order() {
		let doc = testing::nested_doc();
		let texts: Vec<_> = doc
			.text_blocks()
			.filter_map(Block::inline)
			.map(InlineText::as_str)
			.collect();

		assert_eq!(texts, ["intro", "one", "two", "outro"]);
		assert_eq!(doc.plain_text(), "intro\none\ntwo\noutro");
	}

	#[test]
	fn navigates_between_text_blocks_across_nesting() {
		let doc = testing::nested_doc();
		let ids: Vec<_> = doc.text_blocks().map(Block::id).collect();

		for pair in ids.windows(2) {
			assert_eq!(doc.next_text_block(pair[0]), Some(pair[1]));
			assert_eq!(doc.prev_text_block(pair[1]), Some(pair[0]));
		}

		assert_eq!(doc.next_text_block(ids[3]), None);
		assert_eq!(doc.prev_text_block(ids[0]), None);
	}

	#[test]
	fn tracks_ancestry() {
		let doc = testing::nested_doc();
		let list = doc.children(None)[1];
		let item = doc.children(Some(list))[0];
		let paragraph = doc.children(Some(item))[0];

		assert!(doc.is_ancestor(list, paragraph));
		assert!(!doc.is_ancestor(paragraph, list));
		assert!(!doc.is_ancestor(list, list));
	}

	#[test]
	fn tree_round_trips_through_detach_and_insert() {
		let mut doc = testing::nested_doc();
		let list = doc.children(None)[1];
		let tree = doc.tree(list).expect("list exists");

		let detached = doc.detach(list).expect("list exists");
		doc.children_mut(None)
			.expect("root")
			.retain(|&id| id != list);
		assert_eq!(detached, tree);
		assert_eq!(doc.plain_text(), "intro\noutro");

		doc.insert_tree(None, 1, detached);
		assert_eq!(doc.plain_text(), "intro\none\ntwo\noutro");
	}

	#[test]
	fn last_text_block_skips_trailing_leaves() {
		let schema = Arc::new(
			Schema::builder()
				.block(BlockSpec::text("paragraph"))
				.block(BlockSpec::leaf("divider"))
				.build()
				.expect("valid schema"),
		);
		let paragraph = schema.block_type("paragraph").expect("paragraph");
		let divider = schema.block_type("divider").expect("divider");
		let mut doc = Document::empty(schema, 1);
		doc.insert_tree(
			None,
			0,
			BlockTree::text(
				BlockId::new(1, 0),
				paragraph,
				InlineText::plain("x", MarkSet::empty()),
			),
		);
		doc.insert_tree(None, 1, BlockTree::leaf(BlockId::new(1, 1), divider));

		assert_eq!(doc.last_text_block(), Some(BlockId::new(1, 0)));
		assert_eq!(doc.allocate_id(), Ok(BlockId::new(1, 2)));
	}
}
