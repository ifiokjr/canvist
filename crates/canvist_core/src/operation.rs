//! Edit operations that mutate a document.
//!
//! Operations are the atomic units of change in the editor. They are designed
//! to be invertible so that undo/redo can be implemented by keeping a stack of
//! operations and their inverses.
//!
//! # Examples
//!
//! ```
//! use canvist_core::Document;
//! use canvist_core::Position;
//! use canvist_core::operation::Operation;
//!
//! let mut doc = Document::new();
//! let op = Operation::insert(Position::zero(), "Hello!");
//! op.apply(&mut doc);
//!
//! assert_eq!(doc.plain_text(), "Hello!");
//! ```

use serde::Deserialize;
use serde::Serialize;

use crate::Document;
use crate::Selection;
use crate::Style;
use crate::position::Position;

/// A single, atomic edit operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Operation {
	/// Insert text at a position.
	Insert {
		/// Where to insert.
		position: Position,
		/// The text to insert.
		text: String,
	},

	/// Delete text covered by a selection.
	Delete {
		/// The range to delete.
		selection: Selection,
	},

	/// Apply a style to a selection.
	Format {
		/// The range to style.
		selection: Selection,
		/// The style to apply.
		style: Style,
	},
}

impl Operation {
	/// Create an insert operation.
	#[must_use]
	pub fn insert(position: Position, text: impl Into<String>) -> Self {
		Self::Insert {
			position,
			text: text.into(),
		}
	}

	/// Create a delete operation.
	#[must_use]
	pub fn delete(selection: Selection) -> Self {
		Self::Delete { selection }
	}

	/// Create a format operation.
	#[must_use]
	pub fn format(selection: Selection, style: Style) -> Self {
		Self::Format { selection, style }
	}

	/// Apply this operation to a document.
	pub fn apply(&self, doc: &mut Document) {
		match self {
			Self::Insert { position, text } => {
				doc.insert_text(*position, text);
			}
			Self::Delete { selection } => {
				doc.delete(selection);
			}
			Self::Format { selection, style } => {
				doc.apply_style(*selection, style);
			}
		}
	}
}

/// A batch of operations that should be applied atomically.
///
/// This is useful for compound edits like "replace selection" (delete + insert)
/// that should be treated as a single undo step.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
	/// The operations in this transaction, applied in order.
	operations: Vec<Operation>,
}

impl Transaction {
	/// Create a new empty transaction.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Add an operation to the transaction.
	#[must_use]
	pub fn push(mut self, op: Operation) -> Self {
		self.operations.push(op);
		self
	}

	/// Return the number of operations in this transaction.
	#[must_use]
	pub fn len(&self) -> usize {
		self.operations.len()
	}

	/// Whether this transaction has no operations.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.operations.is_empty()
	}

	/// Apply all operations in order to the document.
	pub fn apply(&self, doc: &mut Document) {
		for op in &self.operations {
			op.apply(doc);
		}
	}

	/// Return a slice of the operations.
	#[must_use]
	pub fn operations(&self) -> &[Operation] {
		&self.operations
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn insert_operation() {
		let mut doc = Document::new();
		let op = Operation::insert(Position::zero(), "Hello");
		op.apply(&mut doc);

		assert_eq!(doc.plain_text(), "Hello");
	}

	#[test]
	fn delete_operation() {
		let mut doc = Document::new();
		Operation::insert(Position::zero(), "Hello, world!").apply(&mut doc);

		let op = Operation::delete(Selection::range(Position::new(5), Position::new(7)));
		op.apply(&mut doc);

		assert_eq!(doc.plain_text(), "Helloworld!");
	}

	#[test]
	fn transaction_batch() {
		let mut doc = Document::new();

		let tx = Transaction::new()
			.push(Operation::insert(Position::zero(), "Hello"))
			.push(Operation::format(
				Selection::range(Position::zero(), Position::new(5)),
				Style::new().bold(),
			));

		assert_eq!(tx.len(), 2);
		tx.apply(&mut doc);

		assert_eq!(doc.plain_text(), "Hello");
	}
}
