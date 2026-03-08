//! Cursor and selection model for the canvist editor.
//!
//! A [`Selection`] represents either a collapsed cursor (caret) or a range
//! of selected text. Positions are expressed as character offsets from the
//! start of the document.
//!
//! # Examples
//!
//! ```
//! use canvist_core::Position;
//! use canvist_core::Selection;
//!
//! // A collapsed cursor at character offset 5.
//! let cursor = Selection::collapsed(Position::new(5));
//! assert!(cursor.is_collapsed());
//!
//! // A range selection from offset 2 to offset 10.
//! let range = Selection::range(Position::new(2), Position::new(10));
//! assert!(!range.is_collapsed());
//! assert_eq!(range.len(), 8);
//! ```

use serde::Deserialize;
use serde::Serialize;

use crate::Document;
use crate::position::Position;

/// A text selection within a document.
///
/// Selections have an *anchor* (where the selection started) and a *focus*
/// (where it currently ends). When anchor == focus, the selection is
/// *collapsed* — i.e. a blinking caret with no highlighted text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Selection {
	/// The position where the selection was initiated.
	anchor: Position,
	/// The position where the selection currently ends.
	focus: Position,
}

impl Selection {
	/// Create a collapsed selection (caret) at the given position.
	#[must_use]
	pub fn collapsed(position: Position) -> Self {
		Self {
			anchor: position,
			focus: position,
		}
	}

	/// Create a range selection from `anchor` to `focus`.
	///
	/// The anchor and focus may be in any order — use [`start`](Self::start)
	/// and [`end`](Self::end) to get the ordered bounds.
	#[must_use]
	pub fn range(anchor: Position, focus: Position) -> Self {
		Self { anchor, focus }
	}

	/// Create a selection that covers the entire document.
	#[must_use]
	pub fn all(doc: &Document) -> Self {
		let len = doc.char_count();
		Self::range(Position::zero(), Position::new(len))
	}

	/// The anchor position (where the selection started).
	#[must_use]
	pub fn anchor(&self) -> Position {
		self.anchor
	}

	/// The focus position (where the selection currently ends).
	#[must_use]
	pub fn focus(&self) -> Position {
		self.focus
	}

	/// The earlier of anchor and focus.
	#[must_use]
	pub fn start(&self) -> Position {
		if self.anchor <= self.focus {
			self.anchor
		} else {
			self.focus
		}
	}

	/// The later of anchor and focus.
	#[must_use]
	pub fn end(&self) -> Position {
		if self.anchor >= self.focus {
			self.anchor
		} else {
			self.focus
		}
	}

	/// Return the position of this selection (alias for [`start`](Self::start)
	/// when collapsed).
	#[must_use]
	pub fn position(&self) -> Position {
		self.start()
	}

	/// Whether this selection is collapsed (a caret, no highlighted text).
	#[must_use]
	pub fn is_collapsed(&self) -> bool {
		self.anchor == self.focus
	}

	/// The number of characters covered by this selection.
	#[must_use]
	pub fn len(&self) -> usize {
		self.end().offset() - self.start().offset()
	}

	/// Whether this selection covers zero characters.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Move the focus forward by `n` characters, extending the selection.
	#[must_use]
	pub fn extend_forward(self, n: usize) -> Self {
		Self {
			anchor: self.anchor,
			focus: Position::new(self.focus.offset() + n),
		}
	}

	/// Move the focus backward by `n` characters, extending the selection.
	#[must_use]
	pub fn extend_backward(self, n: usize) -> Self {
		Self {
			anchor: self.anchor,
			focus: Position::new(self.focus.offset().saturating_sub(n)),
		}
	}

	/// Collapse the selection to the start position.
	#[must_use]
	pub fn collapse_to_start(self) -> Self {
		Self::collapsed(self.start())
	}

	/// Collapse the selection to the end position.
	#[must_use]
	pub fn collapse_to_end(self) -> Self {
		Self::collapsed(self.end())
	}
}

impl Default for Selection {
	fn default() -> Self {
		Self::collapsed(Position::zero())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn collapsed_selection() {
		let sel = Selection::collapsed(Position::new(5));
		assert!(sel.is_collapsed());
		assert!(sel.is_empty());
		assert_eq!(sel.len(), 0);
		assert_eq!(sel.position(), Position::new(5));
	}

	#[test]
	fn range_selection_ordered() {
		let sel = Selection::range(Position::new(3), Position::new(10));
		assert!(!sel.is_collapsed());
		assert_eq!(sel.start(), Position::new(3));
		assert_eq!(sel.end(), Position::new(10));
		assert_eq!(sel.len(), 7);
	}

	#[test]
	fn range_selection_reversed() {
		let sel = Selection::range(Position::new(10), Position::new(3));
		assert_eq!(sel.start(), Position::new(3));
		assert_eq!(sel.end(), Position::new(10));
		assert_eq!(sel.len(), 7);
	}

	#[test]
	fn extend_forward() {
		let sel = Selection::collapsed(Position::new(5)).extend_forward(3);
		assert_eq!(sel.anchor(), Position::new(5));
		assert_eq!(sel.focus(), Position::new(8));
		assert_eq!(sel.len(), 3);
	}

	#[test]
	fn extend_backward_clamps_to_zero() {
		let sel = Selection::collapsed(Position::new(2)).extend_backward(5);
		assert_eq!(sel.focus(), Position::zero());
	}

	#[test]
	fn collapse_to_start() {
		let sel = Selection::range(Position::new(3), Position::new(10));
		let collapsed = sel.collapse_to_start();
		assert!(collapsed.is_collapsed());
		assert_eq!(collapsed.position(), Position::new(3));
	}

	#[test]
	fn collapse_to_end() {
		let sel = Selection::range(Position::new(3), Position::new(10));
		let collapsed = sel.collapse_to_end();
		assert!(collapsed.is_collapsed());
		assert_eq!(collapsed.position(), Position::new(10));
	}
}
