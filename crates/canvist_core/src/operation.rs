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
use crate::document::Node;
use crate::document::NodeId;
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

/// Immutable actor identifier for operation-log entries.
pub type ActorId = String;

/// Deterministic identifier for a persisted log entry.
pub type OpId = String;

/// Preconditions that must hold before applying a [`LogEntry`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preconditions {
	/// Expected document hash before this operation applies.
	pub expected_document_hash: Option<String>,
	/// Expected character count before this operation applies.
	pub expected_char_count: Option<usize>,
}

impl Preconditions {
	/// Create empty preconditions.
	#[must_use]
	pub fn empty() -> Self {
		Self {
			expected_document_hash: None,
			expected_char_count: None,
		}
	}

	/// Validate this precondition set against a document.
	#[must_use]
	pub fn validate(&self, doc: &Document) -> bool {
		if let Some(expected_hash) = &self.expected_document_hash
			&& doc.state_hash() != *expected_hash
		{
			return false;
		}

		if let Some(expected_chars) = self.expected_char_count
			&& doc.char_count() != expected_chars
		{
			return false;
		}

		true
	}
}

impl Default for Preconditions {
	fn default() -> Self {
		Self::empty()
	}
}

/// Reference to another deterministic state capture in the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRef {
	/// Identifier of a trusted checkpoint event.
	pub checkpoint_id: String,
}

/// Recovery payload for undo/redo and deterministic replay validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RecoveryRef {
	/// Inverse operation to undo this log entry.
	Inverse(Operation),
	/// Link to a checkpoint that can restore state.
	Checkpoint(CheckpointRef),
}

/// An immutable diff-log envelope around an [`Operation`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
	/// Deterministic operation identifier.
	pub op_id: OpId,
	/// Logical clock for deterministic ordering.
	pub logical_clock: u64,
	/// Wall-clock timestamp in unix milliseconds.
	pub timestamp_ms: u64,
	/// Actor/session that authored this operation.
	pub actor: ActorId,
	/// Preconditions to validate before apply.
	pub preconditions: Preconditions,
	/// The actual operation payload.
	pub operation: Operation,
	/// Optional undo/recovery metadata.
	pub recovery: Option<RecoveryRef>,
}

impl LogEntry {
	/// Construct a new log entry from operation metadata.
	#[must_use]
	pub fn new(
		op_id: impl Into<OpId>,
		logical_clock: u64,
		timestamp_ms: u64,
		actor: impl Into<ActorId>,
		operation: Operation,
	) -> Self {
		Self {
			op_id: op_id.into(),
			logical_clock,
			timestamp_ms,
			actor: actor.into(),
			preconditions: Preconditions::default(),
			operation,
			recovery: None,
		}
	}

	/// Attach preconditions to this entry.
	#[must_use]
	pub fn with_preconditions(mut self, preconditions: Preconditions) -> Self {
		self.preconditions = preconditions;
		self
	}

	/// Attach recovery metadata to this entry.
	#[must_use]
	pub fn with_recovery(mut self, recovery: RecoveryRef) -> Self {
		self.recovery = Some(recovery);
		self
	}

	/// Validate preconditions and apply the wrapped operation.
	///
	/// Returns `true` on success and `false` if preconditions fail.
	pub fn apply_checked(&self, doc: &mut Document) -> bool {
		if !self.preconditions.validate(doc) {
			return false;
		}

		self.operation.apply(doc);
		true
	}
}

/// Ordered immutable operation log for deterministic replay.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OperationLog {
	entries: Vec<LogEntry>,
}

impl OperationLog {
	/// Create an empty operation log.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Append an entry to the log.
	#[must_use]
	pub fn push(mut self, entry: LogEntry) -> Self {
		self.entries.push(entry);
		self
	}

	/// Borrow log entries in insertion order.
	#[must_use]
	pub fn entries(&self) -> &[LogEntry] {
		&self.entries
	}

	/// Replay the log onto a document, validating preconditions.
	pub fn replay(&self, doc: &mut Document) -> Result<(), ReplayError> {
		for entry in &self.entries {
			if !entry.apply_checked(doc) {
				return Err(ReplayError::PreconditionFailed {
					op_id: entry.op_id.clone(),
				});
			}
		}

		Ok(())
	}
}

/// Failure modes when replaying a deterministic operation log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
	/// Preconditions for the referenced operation did not match document state.
	PreconditionFailed { op_id: OpId },
}

impl Operation {
	/// Convert this operation into a CRDT text delta operation (`offset`, `delete_len`, `insert_text`).
	///
	/// Returns [`None`] for non-textual operations (e.g. formatting).
	#[must_use]
	pub fn as_text_delta(&self) -> Option<(u32, u32, String)> {
		match self {
			Self::Insert { position, text } => Some((position.offset() as u32, 0, text.clone())),
			Self::Delete { selection } => {
				let start = selection.start().offset() as u32;
				let end = selection.end().offset() as u32;
				Some((start, end.saturating_sub(start), String::new()))
			}
			Self::Format { .. } => None,
		}
	}
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

/// A first-class, serializable patch describing document changes.
///
/// Unlike [`Transaction`], which represents intent as replayable operations,
/// a patch can carry concrete structural deltas suitable for transport,
/// persistence, and inspection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Patch {
	changes: Vec<Change>,
}

/// A single change entry in a [`Patch`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Change {
	/// Apply a semantic edit operation.
	Operation(Operation),
	/// Set or clear document title metadata.
	SetTitle {
		title: Option<String>,
	},
	/// Upsert a node in the document tree.
	UpsertNode {
		node: Node,
	},
	/// Remove a node from the document tree.
	RemoveNode {
		node_id: NodeId,
	},
}

impl Patch {
	/// Create a new empty patch.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a change entry to this patch.
	#[must_use]
	pub fn push(mut self, change: Change) -> Self {
		self.changes.push(change);
		self
	}

	/// Return the number of change entries.
	#[must_use]
	pub fn len(&self) -> usize {
		self.changes.len()
	}

	/// Whether this patch has no changes.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.changes.is_empty()
	}

	/// Return a slice of all changes.
	#[must_use]
	pub fn changes(&self) -> &[Change] {
		&self.changes
	}

	/// Apply all changes in order to a document.
	pub fn apply(&self, doc: &mut Document) {
		for change in &self.changes {
			change.apply(doc);
		}
	}
}

impl Change {
	/// Apply this single change to a document.
	pub fn apply(&self, doc: &mut Document) {
		match self {
			Self::Operation(op) => op.apply(doc),
			Self::SetTitle { title } => {
				doc.set_title_opt(title.clone());
			}
			Self::UpsertNode { node } => {
				doc.upsert_node(node.clone());
			}
			Self::RemoveNode { node_id } => {
				doc.remove_node(*node_id);
			}
		}
	}
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

	#[test]
	fn patch_applies_semantic_and_metadata_changes() {
		let mut doc = Document::new();

		let patch = Patch::new()
			.push(Change::Operation(Operation::insert(Position::zero(), "Hello")))
			.push(Change::SetTitle {
				title: Some("Patched".to_string()),
			});

		assert_eq!(patch.len(), 2);
		patch.apply(&mut doc);

		assert_eq!(doc.plain_text(), "Hello");
		assert_eq!(doc.title(), Some("Patched"));
	}

	#[test]
	fn operation_log_replay_is_deterministic() {
		let mut doc = Document::new();

		let before = doc.state_hash();
		let entry = LogEntry::new(
			"op-1",
			1,
			1_700_000_000_000,
			"agent:a",
			Operation::insert(Position::zero(), "Hello"),
		)
		.with_preconditions(Preconditions {
			expected_document_hash: Some(before),
			expected_char_count: Some(0),
		});

		let log = OperationLog::new().push(entry);
		assert!(log.replay(&mut doc).is_ok());
		assert_eq!(doc.plain_text(), "Hello");
	}

	#[test]
	fn operation_log_rejects_failed_preconditions() {
		let mut doc = Document::new();
		doc.insert_text(Position::zero(), "Existing");

		let entry = LogEntry::new(
			"op-2",
			2,
			1_700_000_000_001,
			"agent:b",
			Operation::insert(Position::zero(), "Hello"),
		)
		.with_preconditions(Preconditions {
			expected_document_hash: Some("mismatch".to_string()),
			expected_char_count: Some(0),
		});

		let log = OperationLog::new().push(entry);
		assert_eq!(
			log.replay(&mut doc),
			Err(ReplayError::PreconditionFailed {
				op_id: "op-2".to_string(),
			})
		);
	}
}
