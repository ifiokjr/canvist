use crate::Action;
use crate::ActionArgs;
use crate::ActionMeta;
use crate::ActionValidationContext;
use crate::Document;
use crate::EditorEvent;
use crate::EventSource;
use crate::ExtensionRuntime;
use crate::Position;
use crate::Selection;
use crate::Transaction;
use crate::operation::LogEntry;
use crate::operation::Operation;
use crate::operation::OperationLog;
use crate::operation::Preconditions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalidation {
	None,
	Document,
	Selection,
	DocumentAndSelection,
}

#[derive(Debug, Clone)]
pub struct RuntimeOutput {
	pub action: Action,
	pub transaction: Option<Transaction>,
	pub invalidation: Invalidation,
}

pub struct EditorRuntime {
	document: Document,
	selection: Selection,
	extensions: Option<ExtensionRuntime>,
	logical_clock: u64,
	op_log: OperationLog,
	actor: String,
	action_seq: u64,
	timestamp_ms: u64,
}

impl EditorRuntime {
	#[must_use]
	pub fn export_log(&self) -> OperationLog {
		self.op_log.clone()
	}

	pub fn replay_log_into(
		&self,
		target: &mut Document,
	) -> Result<(), crate::operation::ReplayError> {
		self.op_log.replay(target)
	}
	#[must_use]
	pub fn new(document: Document, selection: Selection, actor: impl Into<String>) -> Self {
		Self {
			document,
			selection,
			extensions: None,
			logical_clock: 0,
			op_log: OperationLog::new(),
			actor: actor.into(),
			action_seq: 0,
			timestamp_ms: 0,
		}
	}

	#[must_use]
	pub fn with_extensions(mut self, extensions: ExtensionRuntime) -> Self {
		self.extensions = Some(extensions);
		self
	}

	#[must_use]
	pub fn document(&self) -> &Document {
		&self.document
	}

	pub fn document_mut(&mut self) -> &mut Document {
		&mut self.document
	}

	pub fn apply_transaction(&mut self, tx: Transaction) {
		self.apply_and_log(tx);
	}

	pub fn apply_operation(&mut self, operation: Operation) {
		self.apply_transaction(Transaction::new().push(operation));
	}

	#[must_use]
	pub fn selection(&self) -> Selection {
		self.selection
	}

	#[must_use]
	pub fn logical_clock(&self) -> u64 {
		self.logical_clock
	}

	#[must_use]
	pub fn operation_log(&self) -> &OperationLog {
		&self.op_log
	}

	pub fn poll<S: EventSource>(&mut self, source: &mut S) -> Option<RuntimeOutput> {
		let event = source.poll_event()?;
		self.handle_event(event).ok()
	}

	pub fn handle_event(&mut self, event: EditorEvent) -> Result<RuntimeOutput, RuntimeError> {
		let action = self.action_from_event(event.clone())?;
		let (transaction, invalidation) = self.dispatch(event, &action);
		if let Some(tx) = transaction.clone() {
			self.apply_and_log(tx);
		}

		Ok(RuntimeOutput {
			action,
			transaction,
			invalidation,
		})
	}

	fn action_from_event(&mut self, event: EditorEvent) -> Result<Action, RuntimeError> {
		let previous = if self.logical_clock == 0 {
			None
		} else {
			Some(self.logical_clock)
		};
		self.logical_clock = self.logical_clock.saturating_add(1);
		self.action_seq = self.action_seq.saturating_add(1);
		self.timestamp_ms = self.timestamp_ms.saturating_add(1);

		let meta = ActionMeta {
			id: format!("act-{}", self.action_seq),
			actor: self.actor.clone(),
			logical_clock: self.logical_clock,
			timestamp_ms: self.timestamp_ms as i64,
		};

		Action::from_event(
			event,
			meta,
			ActionValidationContext {
				previous_logical_clock: previous,
			},
		)
		.map_err(RuntimeError::InvalidAction)
	}

	fn dispatch(
		&mut self,
		event: EditorEvent,
		action: &Action,
	) -> (Option<Transaction>, Invalidation) {
		if let Some(tx) = self.transaction_from_action(action) {
			return (Some(tx), Invalidation::DocumentAndSelection);
		}

		if let Some(runtime) = &self.extensions {
			if let Some(runtime_tx) = runtime.run_commands(&self.document, self.selection, &event) {
				return (
					Some(runtime_tx.transaction),
					Invalidation::DocumentAndSelection,
				);
			}
			if let Some(runtime_tx) =
				runtime.run_input_rules(&self.document, self.selection, &event)
			{
				return (
					Some(runtime_tx.transaction),
					Invalidation::DocumentAndSelection,
				);
			}
		}

		match action.args {
			ActionArgs::SelectionSet { selection } => {
				self.selection = selection;
				(None, Invalidation::Selection)
			}
			ActionArgs::CursorMove { position, extend } => {
				self.selection = if extend {
					Selection::range(self.selection.start(), position)
				} else {
					Selection::collapsed(position)
				};
				(None, Invalidation::Selection)
			}
			_ => (None, Invalidation::None),
		}
	}

	fn transaction_from_action(&self, action: &Action) -> Option<Transaction> {
		let doc_chars = self.document.char_count();
		let selection = self.clamped_selection(doc_chars);
		match &action.args {
			ActionArgs::TextInsert { text } | ActionArgs::ClipboardPaste { text } => {
				Some(self.replace_selection_with_text(selection, text.clone()))
			}
			ActionArgs::TextDeleteBackward { count } => {
				if !selection.is_collapsed() {
					return Some(Transaction::new().push(Operation::delete(selection)));
				}
				let caret = selection.start().offset();
				let start = caret.saturating_sub(*count);
				let delete_selection = Selection::range(Position::new(start), Position::new(caret));
				Some(Transaction::new().push(Operation::delete(delete_selection)))
			}
			ActionArgs::TextDeleteForward { count } => {
				if !selection.is_collapsed() {
					return Some(Transaction::new().push(Operation::delete(selection)));
				}
				let caret = selection.start().offset();
				let end = caret.saturating_add(*count).min(doc_chars);
				let delete_selection = Selection::range(Position::new(caret), Position::new(end));
				Some(Transaction::new().push(Operation::delete(delete_selection)))
			}
			_ => None,
		}
	}

	fn clamped_selection(&self, doc_chars: usize) -> Selection {
		let start = self.selection.start().offset().min(doc_chars);
		let end = self.selection.end().offset().min(doc_chars);
		Selection::range(Position::new(start), Position::new(end))
	}

	fn replace_selection_with_text(&self, selection: Selection, text: String) -> Transaction {
		let mut tx = Transaction::new();
		if !selection.is_collapsed() {
			tx = tx.push(Operation::delete(selection));
		}
		tx.push(Operation::insert(selection.start(), text))
	}

	fn apply_and_log(&mut self, tx: Transaction) {
		if tx.is_empty() {
			return;
		}

		let tx = if let Some(runtime) = &self.extensions {
			runtime.resolve_before_hooks(
				&self.document,
				&crate::TransactionMeta::new("runtime"),
				tx,
			)
		} else {
			tx
		};

		let mut replay_shadow = self.document.clone();
		for (index, op) in tx.operations().iter().enumerate() {
			let entry = LogEntry::new(
				format!("op-{}-{index}", self.logical_clock),
				self.logical_clock,
				self.timestamp_ms,
				self.actor.clone(),
				op.clone(),
			)
			.with_preconditions(Preconditions {
				expected_document_hash: Some(replay_shadow.state_hash()),
				expected_char_count: Some(replay_shadow.char_count()),
			});
			self.op_log.push_mut(entry);
			op.apply(&mut replay_shadow);
		}

		tx.apply(&mut self.document);

		if let Some(runtime) = &self.extensions {
			runtime.run_after_hooks(&self.document, &crate::TransactionMeta::new("runtime"), &tx);
		}

		self.selection = Selection::collapsed(Position::new(self.document.char_count()));
	}
}

#[derive(Debug)]
pub enum RuntimeError {
	InvalidAction(crate::ActionValidationError),
}

#[cfg(test)]
mod tests {
	use super::*;

	fn runtime_with_plaintext(text: &str) -> EditorRuntime {
		let mut document = Document::new();
		if !text.is_empty() {
			Operation::insert(Position::new(0), text.to_string()).apply(&mut document);
		}
		EditorRuntime::new(document, Selection::collapsed(Position::new(0)), "tester")
	}

	#[test]
	fn maps_text_insert_event_to_action_and_transaction() {
		let mut runtime = runtime_with_plaintext("");
		let output = runtime
			.handle_event(EditorEvent::TextInsert {
				text: "hi".to_string(),
			})
			.expect("text insert should map to action");

		assert_eq!(output.action.meta.id, "act-1");
		assert_eq!(output.action.meta.logical_clock, 1);
		assert_eq!(output.action.meta.timestamp_ms, 1);
		assert_eq!(output.invalidation, Invalidation::DocumentAndSelection);
		assert!(output.transaction.is_some());
		assert_eq!(runtime.logical_clock(), 1);
		assert_eq!(runtime.document().plain_text(), "hi");
	}

	#[test]
	fn action_mapped_transaction_takes_precedence_over_extensions() {
		let mut runtime = runtime_with_plaintext("");
		let output = runtime
			.handle_event(EditorEvent::TextInsert {
				text: "abc".to_string(),
			})
			.expect("event should be handled");

		assert_eq!(output.invalidation, Invalidation::DocumentAndSelection);
		assert_eq!(runtime.document().plain_text(), "abc");
	}

	#[test]
	fn operation_log_has_preconditions_and_deterministic_ids() {
		let mut runtime = runtime_with_plaintext("abc");
		let before_hash = runtime.document().state_hash();
		let before_chars = runtime.document().char_count();

		let _ = runtime
			.handle_event(EditorEvent::TextInsert {
				text: "z".to_string(),
			})
			.expect("text insert should be handled");

		let entries = runtime.operation_log().entries();
		assert_eq!(entries.len(), 1);
		let entry = &entries[0];
		assert_eq!(entry.op_id, "op-1-0");
		assert_eq!(entry.logical_clock, 1);
		assert_eq!(entry.timestamp_ms, 1);
		assert_eq!(
			entry.preconditions.expected_document_hash.as_deref(),
			Some(before_hash.as_str())
		);
		assert_eq!(entry.preconditions.expected_char_count, Some(before_chars));
	}

	#[test]
	fn invalidation_is_selection_for_cursor_and_selection_events() {
		let mut runtime = runtime_with_plaintext("abc");
		let sel_output = runtime
			.handle_event(EditorEvent::SelectionSet {
				selection: Selection::range(Position::new(0), Position::new(2)),
			})
			.expect("selection set should be handled");
		assert_eq!(sel_output.invalidation, Invalidation::Selection);
		assert!(sel_output.transaction.is_none());

		let cursor_output = runtime
			.handle_event(EditorEvent::CursorMove {
				position: Position::new(1),
				extend: false,
			})
			.expect("cursor move should be handled");
		assert_eq!(cursor_output.invalidation, Invalidation::Selection);
		assert!(cursor_output.transaction.is_none());
	}

	#[test]
	fn export_log_replays_deterministically_into_fresh_document() {
		let mut runtime = runtime_with_plaintext("");
		runtime
			.handle_event(EditorEvent::TextInsert {
				text: "hello".to_string(),
			})
			.expect("first insert should succeed");
		runtime
			.handle_event(EditorEvent::TextInsert {
				text: "!".to_string(),
			})
			.expect("second insert should succeed");

		let log = runtime.export_log();
		let mut replayed = Document::new();
		log.replay(&mut replayed).expect("replay should succeed");

		assert_eq!(replayed.plain_text(), runtime.document().plain_text());
		assert_eq!(replayed.state_hash(), runtime.document().state_hash());
	}

	#[test]
	fn replay_handles_multi_operation_selection_replacement() {
		let mut runtime = runtime_with_plaintext("hello");
		runtime
			.handle_event(EditorEvent::SelectionSet {
				selection: Selection::range(Position::new(1), Position::new(4)),
			})
			.expect("selection should be handled");
		runtime
			.handle_event(EditorEvent::TextInsert {
				text: "i".to_string(),
			})
			.expect("replacement insert should succeed");

		assert_eq!(runtime.document().plain_text(), "hio");
		let entries = runtime.operation_log().entries();
		assert_eq!(entries.len(), 2);
		assert_eq!(entries[0].op_id, "op-2-0");
		assert_eq!(entries[1].op_id, "op-2-1");
		assert_eq!(entries[1].preconditions.expected_char_count, Some(2));

		let mut replayed = Document::new();
		Operation::insert(Position::new(0), "hello".to_string()).apply(&mut replayed);
		runtime
			.replay_log_into(&mut replayed)
			.expect("replay should preserve transactional replacements");
		assert_eq!(replayed.plain_text(), runtime.document().plain_text());
	}

	#[test]
	fn replay_into_returns_error_when_preconditions_do_not_match() {
		let mut runtime = runtime_with_plaintext("abc");
		runtime
			.handle_event(EditorEvent::TextInsert {
				text: "z".to_string(),
			})
			.expect("insert should succeed");

		let mut mismatched = Document::new();
		Operation::insert(Position::new(0), "different".to_string()).apply(&mut mismatched);

		let err = runtime
			.replay_log_into(&mut mismatched)
			.expect_err("preconditions should fail on mismatched document");
		assert!(matches!(
			err,
			crate::operation::ReplayError::PreconditionFailed { .. }
		));
	}
}
