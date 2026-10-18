//! Blocks and owned block trees.

use crate::Attrs;
use crate::BlockId;
use crate::BlockTypeId;
use crate::InlineText;

/// What a block holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
	/// Inline text, for paragraphs, headings, code blocks, and similar.
	Inline(InlineText),
	/// Child block ids, for lists, quotes, tables, and other containers.
	Blocks(Vec<BlockId>),
	/// Nothing, for atomic blocks such as dividers and images.
	Leaf,
}

/// A block stored in a [`Document`](crate::Document).
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
	pub(crate) id: BlockId,
	pub(crate) kind: BlockTypeId,
	pub(crate) attrs: Attrs,
	pub(crate) parent: Option<BlockId>,
	pub(crate) content: Content,
	pub(crate) rev: u32,
}

impl Block {
	/// The block's stable id.
	#[must_use]
	pub fn id(&self) -> BlockId {
		self.id
	}

	/// The block's type.
	#[must_use]
	pub fn kind(&self) -> BlockTypeId {
		self.kind
	}

	/// The block's attributes.
	#[must_use]
	pub fn attrs(&self) -> &Attrs {
		&self.attrs
	}

	/// The containing block, or `None` at the document root.
	#[must_use]
	pub fn parent(&self) -> Option<BlockId> {
		self.parent
	}

	/// The block's content.
	#[must_use]
	pub fn content(&self) -> &Content {
		&self.content
	}

	/// The inline text, if this is a text block.
	#[must_use]
	pub fn inline(&self) -> Option<&InlineText> {
		match &self.content {
			Content::Inline(text) => Some(text),
			_ => None,
		}
	}

	/// The child ids, if this is a container (empty otherwise).
	#[must_use]
	pub fn children(&self) -> &[BlockId] {
		match &self.content {
			Content::Blocks(children) => children,
			_ => &[],
		}
	}

	/// A counter bumped on every change to this block's attributes or inline
	/// content. Layout caches key on `(id, rev)` to skip unchanged blocks.
	#[must_use]
	pub fn rev(&self) -> u32 {
		self.rev
	}

	pub(crate) fn touch(&mut self) {
		self.rev = self.rev.wrapping_add(1);
	}
}

/// Content of an owned [`BlockTree`].
#[derive(Clone, Debug, PartialEq)]
pub enum TreeContent {
	/// Inline text.
	Inline(InlineText),
	/// Owned child trees.
	Blocks(Vec<BlockTree>),
	/// No content.
	Leaf,
}

/// An owned block with its descendants, detached from any document.
///
/// Used to insert new blocks, to capture removed blocks for undo, and as the
/// unit of copy, paste, and drag-and-drop.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockTree {
	/// The block's id.
	pub id: BlockId,
	/// The block's type.
	pub kind: BlockTypeId,
	/// The block's attributes.
	pub attrs: Attrs,
	/// The block's content.
	pub content: TreeContent,
}

impl BlockTree {
	/// A text block.
	#[must_use]
	pub fn text(id: BlockId, kind: BlockTypeId, text: InlineText) -> Self {
		Self {
			id,
			kind,
			attrs: Attrs::new(),
			content: TreeContent::Inline(text),
		}
	}

	/// A container block.
	#[must_use]
	pub fn container(id: BlockId, kind: BlockTypeId, children: Vec<Self>) -> Self {
		Self {
			id,
			kind,
			attrs: Attrs::new(),
			content: TreeContent::Blocks(children),
		}
	}

	/// A leaf block.
	#[must_use]
	pub fn leaf(id: BlockId, kind: BlockTypeId) -> Self {
		Self {
			id,
			kind,
			attrs: Attrs::new(),
			content: TreeContent::Leaf,
		}
	}

	/// Builder-style attribute replacement.
	#[must_use]
	pub fn with_attrs(mut self, attrs: Attrs) -> Self {
		self.attrs = attrs;
		self
	}

	/// Visit this block and every descendant in document order.
	pub fn walk(&self, visit: &mut impl FnMut(&Self)) {
		visit(self);

		if let TreeContent::Blocks(children) = &self.content {
			for child in children {
				child.walk(visit);
			}
		}
	}
}
