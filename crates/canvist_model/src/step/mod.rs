//! Steps: the only way to change a document.
//!
//! Every step is invertible and reports how it moves [`Point`]s. Steps are
//! shaped after the operations a block-structured CRDT supports (insert,
//! delete, and format inside one text; insert, delete, and move in one
//! array), so a collaboration binding can translate them one-to-one in both
//! directions instead of diffing document trees.

mod error;
mod map;
mod ops;

use std::ops::Range;

pub use error::StepError;
pub use map::Mapped;
pub use map::StepMap;

use crate::Attrs;
use crate::BlockId;
use crate::BlockTree;
use crate::BlockTypeId;
use crate::Document;
use crate::InlineText;
use crate::Mark;
use crate::MarkSet;
use crate::MarkTypeId;

/// An atomic, invertible document change.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
	/// Insert inline content into a text block.
	InsertInline {
		/// The text block.
		block: BlockId,
		/// Where to insert.
		offset: u32,
		/// What to insert.
		content: InlineText,
	},
	/// Remove a range of inline content from a text block.
	RemoveInline {
		/// The text block.
		block: BlockId,
		/// The byte range to remove.
		range: Range<u32>,
	},
	/// Add a mark to a range, honouring the schema's exclusions.
	AddMark {
		/// The text block.
		block: BlockId,
		/// The byte range to format.
		range: Range<u32>,
		/// The mark to add.
		mark: Mark,
	},
	/// Remove every mark of one type from a range.
	RemoveMark {
		/// The text block.
		block: BlockId,
		/// The byte range to unformat.
		range: Range<u32>,
		/// The mark type to remove.
		kind: MarkTypeId,
	},
	/// Reapply captured run formatting; the inverse of mark changes.
	RestoreMarks {
		/// The text block.
		block: BlockId,
		/// Where the first run starts.
		offset: u32,
		/// `(len, marks)` for consecutive runs.
		runs: Vec<(u32, MarkSet)>,
	},
	/// Change a block's type and attributes, e.g. paragraph to heading.
	SetBlock {
		/// The block.
		block: BlockId,
		/// The new type, which must accept the block's existing content.
		kind: BlockTypeId,
		/// The new attributes.
		attrs: Attrs,
	},
	/// Split a text block at `offset`, moving the tail into a new sibling
	/// placed directly after it.
	Split {
		/// The text block to split.
		block: BlockId,
		/// Where to split.
		offset: u32,
		/// The id of the new sibling.
		new_block: BlockId,
		/// The new sibling's type.
		kind: BlockTypeId,
		/// The new sibling's attributes.
		attrs: Attrs,
	},
	/// Append a text block's content to its previous sibling and remove it.
	Join {
		/// The text block that absorbs `next`.
		block: BlockId,
		/// The immediately following text block.
		next: BlockId,
	},
	/// Insert block trees into a container (or the root).
	InsertBlocks {
		/// The container, or `None` for the root.
		parent: Option<BlockId>,
		/// The child index to insert at.
		index: usize,
		/// The trees to insert, in order.
		blocks: Vec<BlockTree>,
	},
	/// Remove consecutive child blocks and their descendants.
	RemoveBlocks {
		/// The container, or `None` for the root.
		parent: Option<BlockId>,
		/// The first child index to remove.
		index: usize,
		/// How many children to remove.
		count: usize,
	},
	/// Move consecutive child blocks, keeping their ids and content. Used by
	/// drag and drop, indent, and outdent.
	MoveBlocks {
		/// The source container.
		parent: Option<BlockId>,
		/// The first child index to move.
		index: usize,
		/// How many children to move.
		count: usize,
		/// The destination container.
		to_parent: Option<BlockId>,
		/// The destination index, counted after the blocks are removed.
		to_index: usize,
	},
}

/// What applying a step produced.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
	/// The step that undoes this one.
	pub inverse: Step,
	/// How points move.
	pub map: StepMap,
	/// Blocks whose own content or attributes changed.
	pub touched: Vec<BlockId>,
	/// Whether block structure (child lists) changed.
	pub structural: bool,
}

impl Applied {
	fn inline(inverse: Step, map: StepMap, block: BlockId) -> Self {
		Self {
			inverse,
			map,
			touched: vec![block],
			structural: false,
		}
	}

	fn structural(inverse: Step, map: StepMap, touched: Vec<BlockId>) -> Self {
		Self {
			inverse,
			map,
			touched,
			structural: true,
		}
	}
}

impl Step {
	/// Apply this step to `doc`. On error the document is unchanged.
	pub fn apply(self, doc: &mut Document) -> Result<Applied, StepError> {
		doc.record_before(&self);

		let applied = match self {
			Self::InsertInline {
				block,
				offset,
				content,
			} => ops::insert_inline(doc, block, offset, content),
			Self::RemoveInline { block, range } => ops::remove_inline(doc, block, range),
			Self::AddMark { block, range, mark } => {
				let schema = doc.schema().clone();
				ops::format(doc, block, range, |text, range| {
					text.add_mark(range, &mark, &schema)
				})
			}
			Self::RemoveMark { block, range, kind } => {
				ops::format(doc, block, range, |text, range| {
					text.remove_mark(range, kind)
				})
			}
			Self::RestoreMarks {
				block,
				offset,
				runs,
			} => ops::restore_marks(doc, block, offset, &runs),
			Self::SetBlock { block, kind, attrs } => ops::set_block(doc, block, kind, attrs),
			Self::Split {
				block,
				offset,
				new_block,
				kind,
				attrs,
			} => {
				ops::split(
					doc,
					block,
					offset,
					BlockTree::text(new_block, kind, InlineText::new()).with_attrs(attrs),
				)
			}
			Self::Join { block, next } => ops::join(doc, block, next),
			Self::InsertBlocks {
				parent,
				index,
				blocks,
			} => ops::insert_blocks(doc, parent, index, blocks),
			Self::RemoveBlocks {
				parent,
				index,
				count,
			} => ops::remove_blocks(doc, parent, index, count),
			Self::MoveBlocks {
				parent,
				index,
				count,
				to_parent,
				to_index,
			} => ops::move_blocks(doc, (parent, index, count), (to_parent, to_index)),
		}?;

		doc.bump_version();
		doc.record_inverse(&applied.inverse);
		Ok(applied)
	}
}
