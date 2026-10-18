//! Points and selections.

use crate::BlockId;

/// A caret position: a byte offset inside a text block.
///
/// Points name their block directly instead of using a document-wide integer
/// like ProseMirror, so an edit in one block never shifts points in another
/// and remote collaborators' carets stay stable without remapping the whole
/// document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Point {
	/// The text block containing the point.
	pub block: BlockId,
	/// The UTF-8 byte offset within the block's inline text.
	pub offset: u32,
}

impl Point {
	/// A point at `offset` in `block`.
	#[must_use]
	pub const fn new(block: BlockId, offset: u32) -> Self {
		Self { block, offset }
	}
}

/// Which side of an insertion a point sticks to when text is inserted
/// exactly at its offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Bias {
	/// Stay before inserted content.
	Before,
	/// Move after inserted content. Carets use this so typing advances them.
	#[default]
	After,
}

/// The editor selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Selection {
	/// A caret or text range between two points, possibly in different
	/// blocks. `anchor` is where the selection started, `head` where it ends.
	Text {
		/// The fixed end.
		anchor: Point,
		/// The moving end.
		head: Point,
	},
	/// Whole blocks selected as objects, e.g. an image or a dragged range of
	/// sibling blocks.
	Blocks {
		/// The first block selected.
		anchor: BlockId,
		/// The last block selected.
		head: BlockId,
	},
}

impl Selection {
	/// A collapsed caret at `point`.
	#[must_use]
	pub const fn caret(point: Point) -> Self {
		Self::Text {
			anchor: point,
			head: point,
		}
	}

	/// A text range from `anchor` to `head`.
	#[must_use]
	pub const fn text(anchor: Point, head: Point) -> Self {
		Self::Text { anchor, head }
	}

	/// A selection of the single block `block`.
	#[must_use]
	pub const fn block(block: BlockId) -> Self {
		Self::Blocks {
			anchor: block,
			head: block,
		}
	}

	/// Whether this is a collapsed caret.
	#[must_use]
	pub fn is_caret(&self) -> bool {
		matches!(self, Self::Text { anchor, head } if anchor == head)
	}

	/// The caret position, if collapsed.
	#[must_use]
	pub fn as_caret(&self) -> Option<Point> {
		match self {
			Self::Text { anchor, head } if anchor == head => Some(*head),
			_ => None,
		}
	}
}
