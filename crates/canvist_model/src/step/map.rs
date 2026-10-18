//! How steps move points.

use crate::BlockId;
use crate::selection::Bias;
use crate::selection::Point;

/// How a step moves points.
#[derive(Clone, Debug, PartialEq)]
pub enum StepMap {
	/// Points are unaffected.
	Identity,
	/// Inline content changed in one block.
	Inline {
		/// The text block.
		block: BlockId,
		/// Where the change starts.
		offset: u32,
		/// Bytes removed at `offset`.
		removed: u32,
		/// Bytes inserted at `offset`.
		inserted: u32,
	},
	/// A text block was split.
	Split {
		/// The split block.
		block: BlockId,
		/// Where it was split.
		offset: u32,
		/// The block that received the tail.
		new_block: BlockId,
	},
	/// A text block was joined into its previous sibling.
	Join {
		/// The absorbing block.
		block: BlockId,
		/// The removed block.
		next: BlockId,
		/// The absorbing block's length before the join.
		offset: u32,
	},
	/// Blocks were removed.
	Removed {
		/// Every removed id, including descendants.
		blocks: Vec<BlockId>,
		/// The nearest surviving point, for points that were inside them.
		fallback: Option<Point>,
	},
}

/// The result of mapping a point through a change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mapped {
	/// The point's new position.
	pub point: Point,
	/// Whether the content around the point was deleted.
	pub deleted: bool,
}

impl Mapped {
	const fn kept(point: Point) -> Self {
		Self {
			point,
			deleted: false,
		}
	}
}

impl StepMap {
	/// Map `point` through this change.
	#[must_use]
	pub fn map(&self, point: Point, bias: Bias) -> Mapped {
		if !self.affects(point.block) {
			return Mapped::kept(point);
		}

		match *self {
			Self::Identity => Mapped::kept(point),
			Self::Inline {
				offset,
				removed,
				inserted,
				..
			} => map_inline(point, offset, removed, inserted, bias),
			Self::Split {
				offset, new_block, ..
			} => Mapped::kept(map_split(point, offset, new_block, bias)),
			Self::Join { block, offset, .. } => {
				Mapped::kept(Point::new(block, offset + point.offset))
			}
			Self::Removed { fallback, .. } => {
				Mapped {
					point: fallback.unwrap_or(point),
					deleted: true,
				}
			}
		}
	}

	/// Whether points in `block` can move.
	fn affects(&self, block: BlockId) -> bool {
		match self {
			Self::Identity => false,
			Self::Inline { block: changed, .. } | Self::Split { block: changed, .. } => {
				*changed == block
			}
			Self::Join { next, .. } => *next == block,
			Self::Removed { blocks, .. } => blocks.contains(&block),
		}
	}
}

fn map_inline(point: Point, offset: u32, removed: u32, inserted: u32, bias: Bias) -> Mapped {
	let end = offset + removed;
	let at = |offset| Point::new(point.block, offset);
	let (offset_after, deleted) = if point.offset < offset {
		(point.offset, false)
	} else if point.offset == offset && (removed > 0 || bias == Bias::Before) {
		(offset, false)
	} else if point.offset < end {
		(offset, true)
	} else {
		(point.offset - removed + inserted, false)
	};

	Mapped {
		point: at(offset_after),
		deleted,
	}
}

fn map_split(point: Point, offset: u32, new_block: BlockId, bias: Bias) -> Point {
	let moves = point.offset > offset || (point.offset == offset && bias == Bias::After);

	if moves {
		Point::new(new_block, point.offset - offset)
	} else {
		point
	}
}
