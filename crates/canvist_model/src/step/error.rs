//! Step errors.

use crate::BlockId;
use crate::BlockTypeId;
use crate::InlineError;

/// Why a step could not be applied. A failed step leaves the document
/// unchanged.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StepError {
	/// No block has this id.
	#[error("unknown block {0}")]
	UnknownBlock(BlockId),
	/// The block does not hold inline text.
	#[error("block {0} is not a text block")]
	NotTextBlock(BlockId),
	/// The block does not hold child blocks.
	#[error("block {0} is not a container")]
	NotContainer(BlockId),
	/// An inline offset or range was invalid.
	#[error(transparent)]
	Inline(#[from] InlineError),
	/// The schema does not allow this block type here.
	#[error("block type {kind:?} is not allowed in {parent:?}")]
	NotAllowed {
		/// The container (`None` is the root).
		parent: Option<BlockId>,
		/// The rejected type.
		kind: BlockTypeId,
	},
	/// The block's content does not match its type's content rule.
	#[error("content of block {0} does not match its type")]
	ContentMismatch(BlockId),
	/// A block id is already in use.
	#[error("block id {0} already exists")]
	DuplicateId(BlockId),
	/// A child index or range is out of bounds.
	#[error("child range {start}..{end} is out of bounds for {len} children")]
	OutOfBounds {
		/// The range start.
		start: usize,
		/// The range end.
		end: usize,
		/// The number of children.
		len: usize,
	},
	/// `Join` was given blocks that are not adjacent siblings.
	#[error("block {next} does not directly follow {block}")]
	NotAdjacent {
		/// The first block.
		block: BlockId,
		/// The block expected to follow it.
		next: BlockId,
	},
	/// A move would place a block inside itself.
	#[error("cannot move block {0} inside itself")]
	MoveIntoSelf(BlockId),
	/// The client's block id space is exhausted.
	#[error("block ids exhausted for this client")]
	IdsExhausted,
}
