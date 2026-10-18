//! The editor state: a document plus everything editing needs around it.

use std::sync::Arc;

use canvist_model::Bias;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Commit;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Selection;
use canvist_model::Step;

use crate::Affinity;
use crate::CommandResult;
use crate::EditKind;
use crate::EditorError;
use crate::EditorTransaction;
use crate::Origin;
use crate::Update;
use crate::position;
use crate::transaction;
use crate::transaction::Effects;
use crate::transaction::Override;

/// The text an input method editor is still composing (its preedit).
///
/// Preedit text lives in the document like ordinary text, so layout and
/// painting need no special case; the composition only remembers its range
/// so the next preedit update can replace it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Composition {
	/// The text block holding the preedit.
	pub block: BlockId,
	/// Where the preedit starts.
	pub start: u32,
	/// Where the preedit ends.
	pub end: u32,
}

impl Composition {
	/// An empty composition at `point`.
	#[must_use]
	pub const fn at(point: Point) -> Self {
		Self {
			block: point.block,
			start: point.offset,
			end: point.offset,
		}
	}

	/// Where the preedit starts.
	#[must_use]
	pub const fn start_point(self) -> Point {
		Point::new(self.block, self.start)
	}

	/// Where the preedit ends.
	#[must_use]
	pub const fn end_point(self) -> Point {
		Point::new(self.block, self.end)
	}

	/// Map the composition through `commit`, dropping it if its text was
	/// deleted or split across blocks.
	fn map(self, commit: &Commit) -> Option<Self> {
		let start = commit.map(self.start_point(), Bias::After);
		let end = commit.map(self.end_point(), Bias::Before);
		let collapsed = self.start == self.end;
		let end_point = if collapsed { start.point } else { end.point };
		let moved_apart =
			start.point.block != end_point.block || end_point.offset < start.point.offset;

		if start.deleted || end.deleted || moved_apart {
			return None;
		}

		Some(Self {
			block: start.point.block,
			start: start.point.offset,
			end: end_point.offset,
		})
	}
}

/// Everything in an [`EditorState`] besides the document.
#[derive(Clone, Debug)]
struct Transient {
	selection: Selection,
	stored_marks: Option<MarkSet>,
	composition: Option<Composition>,
	goal: Option<f32>,
	affinity: Affinity,
}

/// A document, its selection, and the transient state commands consult.
///
/// Every change goes through one path: [`EditorState::change`] for local
/// commands and [`EditorState::apply_steps`] for undo, redo, and remote steps.
/// Both end in the same private `apply`, which maps or sets the selection,
/// maps the composition, clears stale stored marks, and keeps the document
/// non-empty. That single path is what keeps the state consistent.
#[derive(Clone, Debug)]
pub struct EditorState {
	doc: Document,
	selection: Selection,
	stored_marks: Option<MarkSet>,
	composition: Option<Composition>,
	goal: Option<f32>,
	affinity: Affinity,
}

impl EditorState {
	/// Wrap `doc`, placing a caret at the start of its first text block.
	/// An empty document gets one empty block of the schema's default type.
	pub fn new(doc: Document) -> Result<Self, EditorError> {
		let selection = position::start_selection(&doc);
		let mut state = Self {
			doc,
			selection,
			stored_marks: None,
			composition: None,
			goal: None,
			affinity: Affinity::Downstream,
		};

		if state.doc.children(None).is_empty() {
			state.ensure_block()?;
			state.selection = position::start_selection(&state.doc);
		}

		Ok(state)
	}

	/// The document.
	#[must_use]
	pub fn doc(&self) -> &Document {
		&self.doc
	}

	/// The document's schema.
	#[must_use]
	pub fn schema(&self) -> &Arc<Schema> {
		self.doc.schema()
	}

	/// The selection.
	#[must_use]
	pub fn selection(&self) -> Selection {
		self.selection
	}

	/// Marks queued for the next typed text, set by toggling a mark at a
	/// collapsed caret.
	#[must_use]
	pub fn stored_marks(&self) -> Option<&MarkSet> {
		self.stored_marks.as_ref()
	}

	/// The active IME composition, if any.
	#[must_use]
	pub fn composition(&self) -> Option<Composition> {
		self.composition
	}

	/// The horizontal position vertical caret motion tries to keep.
	#[must_use]
	pub fn goal(&self) -> Option<f32> {
		self.goal
	}

	/// Which visual line the caret belongs to at a soft wrap.
	#[must_use]
	pub fn affinity(&self) -> Affinity {
		self.affinity
	}

	/// Release the document.
	#[must_use]
	pub fn into_doc(self) -> Document {
		self.doc
	}

	/// Run `build` inside a transaction and apply the result.
	///
	/// `build` returns `Ok(true)` to commit, or `Ok(false)` when the command
	/// turns out not to apply; the transaction is then rolled back and the
	/// result is `Ok(None)`. An error also rolls back every step.
	pub fn change(
		&mut self,
		kind: EditKind,
		build: impl FnOnce(&mut EditorTransaction<'_>) -> Result<bool, EditorError>,
	) -> CommandResult {
		let mut transaction = EditorTransaction::new(self.doc.transaction(), self.selection, kind);
		let outcome = build(&mut transaction);
		let (inner, effects) = transaction.finish();

		match outcome {
			Ok(true) => {
				let commit = inner.commit();
				self.apply(commit, effects).map(Some)
			}
			Ok(false) => {
				inner.rollback()?;
				Ok(None)
			}
			Err(error) => {
				inner.rollback()?;
				Err(error)
			}
		}
	}

	/// Apply steps produced elsewhere: undo, redo, or a collaborator.
	///
	/// `selection`, if given, replaces the mapped selection afterwards, which
	/// is how undo restores the selection from before the undone change. On
	/// error, nothing changes.
	pub fn apply_steps(
		&mut self,
		steps: impl IntoIterator<Item = Step>,
		kind: EditKind,
		origin: Origin,
		selection: Option<Selection>,
	) -> Result<Update, EditorError> {
		let commit = self.doc.apply_steps(steps)?;
		let mut effects = Effects::new(kind, origin);
		effects.selection = selection;
		self.apply(commit, effects)
	}

	/// Whether `run` would apply, without changing anything: every step it
	/// takes is rolled back through a model savepoint, which also restores
	/// the document version and block revisions, so layout caches stay warm.
	pub fn dry_run(
		&mut self,
		run: impl FnOnce(&mut Self) -> CommandResult,
	) -> Result<bool, EditorError> {
		let saved = self.transient();
		let savepoint = self.doc.savepoint();
		let outcome = run(self);
		self.doc.rollback_to(savepoint)?;
		self.restore(saved);
		outcome.map(|update| update.is_some())
	}

	/// Run `run` all or nothing: if it does not apply, or fails, every change
	/// it made is rolled back exactly.
	pub fn atomic(&mut self, run: impl FnOnce(&mut Self) -> CommandResult) -> CommandResult {
		let saved = self.transient();
		let savepoint = self.doc.savepoint();
		let outcome = run(self);

		if let Ok(Some(update)) = outcome {
			self.doc.release(savepoint);

			return Ok(Some(update));
		}

		self.doc.rollback_to(savepoint)?;
		self.restore(saved);
		outcome
	}

	/// The state besides the document.
	fn transient(&self) -> Transient {
		Transient {
			selection: self.selection,
			stored_marks: self.stored_marks.clone(),
			composition: self.composition,
			goal: self.goal,
			affinity: self.affinity,
		}
	}

	fn restore(&mut self, saved: Transient) {
		self.selection = saved.selection;
		self.stored_marks = saved.stored_marks;
		self.composition = saved.composition;
		self.goal = saved.goal;
		self.affinity = saved.affinity;
	}

	/// The one place where a commit becomes the new state.
	fn apply(&mut self, commit: Commit, effects: Effects) -> Result<Update, EditorError> {
		let selection_before = self.selection;
		let mut commit = commit;

		if let Some(extra) = self.ensure_block()? {
			commit = transaction::compose(commit, extra);
		}

		let selection = effects
			.selection
			.unwrap_or_else(|| commit.map_selection(selection_before));
		self.selection = position::valid_selection(&self.doc, selection);

		self.composition = match effects.composition {
			Override::Set(composition) => composition,
			Override::Default => {
				self.composition
					.and_then(|composition| composition.map(&commit))
			}
		};

		let unchanged = commit.is_empty() && self.selection == selection_before;

		self.stored_marks = match effects.stored_marks {
			Override::Set(marks) => marks,

			Override::Default if unchanged => self.stored_marks.take(),
			Override::Default => None,
		};

		// A collaborator's edit that leaves the caret where it was keeps it on
		// its visual line and column; any other change starts afresh.
		let keeps_line = effects.origin == Origin::Remote && self.selection == selection_before;

		if !keeps_line {
			self.goal = effects.goal;
			self.affinity = effects.affinity.unwrap_or_default();
		}

		Ok(Update {
			commit,
			selection_before,
			selection: self.selection,
			kind: effects.kind,
			origin: effects.origin,
		})
	}

	/// Insert an empty default text block if the document has no blocks, so
	/// the selection always has somewhere to be.
	fn ensure_block(&mut self) -> Result<Option<Commit>, EditorError> {
		if !self.doc.children(None).is_empty() {
			return Ok(None);
		}

		let kind = self.doc.schema().default_text_block();
		let mut transaction = self.doc.transaction();
		let id = transaction.allocate_id()?;
		let block = BlockTree::text(id, kind, InlineText::new());
		transaction.insert_blocks(None, 0, vec![block])?;
		Ok(Some(transaction.commit()))
	}
}
