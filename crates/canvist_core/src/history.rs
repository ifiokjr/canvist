//! Undo and redo.
//!
//! [`History`] is the boundary the collaboration binding replaces: a
//! collaborative editor must only undo its own changes and has to rebase
//! them over remote edits, which `yrs`'s `UndoManager` already does.
//! [`UndoHistory`] is the single-user implementation built on the inverse
//! steps every commit carries.

use std::time::Duration;

use canvist_model::Selection;
use canvist_model::Step;

use crate::CommandResult;
use crate::EditKind;
use crate::EditorState;
use crate::Origin;
use crate::Update;

/// Records updates and undoes or redoes them.
pub trait History: Send {
	/// Observe an update the editor just applied, made at time `now`.
	/// Implementations ignore updates whose origin is [`Origin::History`].
	fn record(&mut self, update: &Update, now: Duration);

	/// Undo the most recent group of changes.
	fn undo(&mut self, state: &mut EditorState) -> CommandResult;

	/// Redo the most recently undone group.
	fn redo(&mut self, state: &mut EditorState) -> CommandResult;

	/// Whether [`History::undo`] has anything to undo.
	fn can_undo(&self) -> bool;

	/// Whether [`History::redo`] has anything to redo.
	fn can_redo(&self) -> bool;

	/// Make the next recorded change start a new group, e.g. after the user
	/// clicks elsewhere or a toolbar button.
	fn close_group(&mut self);
}

/// How [`UndoHistory`] groups changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryConfig {
	/// Consecutive edits of the same kind closer together than this share
	/// one undo step.
	pub group_delay: Duration,
	/// The most undo steps kept; older ones are dropped.
	pub depth: usize,
}

impl Default for HistoryConfig {
	fn default() -> Self {
		Self {
			group_delay: Duration::from_millis(500),
			depth: 100,
		}
	}
}

/// The rule [`UndoHistory`] groups changes by, on its own: whether each
/// update joins the previous undo step. A shared history that keeps its
/// steps elsewhere asks it how to track each local change, so collaborative
/// undo groups typing exactly as single-user undo does.
#[derive(Clone, Debug, Default)]
pub struct UndoGrouping {
	config: HistoryConfig,
	/// How the current group ended, until something closes it.
	last: Option<GroupEnd>,
}

/// The end of the newest undo group.
#[derive(Clone, Copy, Debug)]
struct GroupEnd {
	kind: EditKind,
	at: Duration,
	selection: Selection,
}

impl UndoGrouping {
	/// Grouping with `config`'s delay.
	#[must_use]
	pub fn new(config: HistoryConfig) -> Self {
		Self { config, last: None }
	}

	/// Whether `update`, made at `now`, joins the previous undo step: typing
	/// (or deleting) that carries on where the previous edit of the same
	/// kind left off, within the group delay. Either way, `update` now ends
	/// the newest group.
	pub fn joins(&mut self, update: &Update, now: Duration) -> bool {
		let joins = self.last.is_some_and(|last| {
			group(update.kind).is_some()
				&& group(update.kind) == group(last.kind)
				&& now.saturating_sub(last.at) <= self.config.group_delay
				&& update.selection_before == last.selection
		});
		self.last = Some(GroupEnd {
			kind: update.kind,
			at: now,
			selection: update.selection,
		});

		joins
	}

	/// Make the next update start a new step, as after an undo or a click
	/// elsewhere.
	pub fn close(&mut self) {
		self.last = None;
	}
}

/// One undo (or redo) step.
#[derive(Clone, Debug)]
struct Entry {
	/// Steps that revert the group, in the order they were produced; they
	/// are applied in reverse.
	inverses: Vec<Step>,
	/// The selection to restore after reverting.
	selection: Selection,
	/// The selection the group ended with, so a contiguous edit can join it.
	selection_after: Selection,
	kind: EditKind,
	at: Duration,
}

/// Single-user undo history with grouping by time and by kind of edit.
///
/// Typing and IME composition group together, deletions group together,
/// and every other kind of edit is its own step. A remote change clears the
/// history, because its inverse steps no longer line up with the document;
/// collaborative editors replace this type through the [`History`] trait.
#[derive(Clone, Debug, Default)]
pub struct UndoHistory {
	config: HistoryConfig,
	grouping: UndoGrouping,
	done: Vec<Entry>,
	undone: Vec<Entry>,
}

impl UndoHistory {
	/// An empty history with the given grouping rules.
	#[must_use]
	pub fn new(config: HistoryConfig) -> Self {
		Self {
			config,
			grouping: UndoGrouping::new(config),
			..Self::default()
		}
	}

	/// The number of undo steps available.
	#[must_use]
	pub fn undo_depth(&self) -> usize {
		self.done.len()
	}

	/// Revert the newest entry of `from` and push its inverse onto `to`.
	fn revert(
		from: &mut Vec<Entry>,
		to: &mut Vec<Entry>,
		state: &mut EditorState,
	) -> CommandResult {
		let Some(entry) = from.pop() else {
			return Ok(None);
		};
		let steps = entry.inverses.iter().rev().cloned();
		let update =
			state.apply_steps(steps, entry.kind, Origin::History, Some(entry.selection))?;

		to.push(Entry {
			inverses: update.commit.inverses.clone(),
			selection: entry.selection_after,
			selection_after: entry.selection,
			kind: entry.kind,
			at: entry.at,
		});
		Ok(Some(update))
	}
}

/// Kinds that may share a history entry map to the same group.
const fn group(kind: EditKind) -> Option<u8> {
	match kind {
		EditKind::Typing | EditKind::Composition => Some(0),
		EditKind::Deleting => Some(1),
		_ => None,
	}
}

impl History for UndoHistory {
	fn record(&mut self, update: &Update, now: Duration) {
		match update.origin {
			Origin::History => return,
			Origin::Remote => {
				self.done.clear();
				self.undone.clear();
				self.grouping.close();
				return;
			}
			Origin::Local => {}
		}

		if !update.doc_changed() {
			return;
		}

		self.undone.clear();

		if self.grouping.joins(update, now) {
			if let Some(last) = self.done.last_mut() {
				last.inverses.extend(update.commit.inverses.iter().cloned());
				last.selection_after = update.selection;
				last.at = now;
			}

			return;
		}

		self.done.push(Entry {
			inverses: update.commit.inverses.clone(),
			selection: update.selection_before,
			selection_after: update.selection,
			kind: update.kind,
			at: now,
		});

		if self.done.len() > self.config.depth {
			self.done.remove(0);
		}
	}

	fn undo(&mut self, state: &mut EditorState) -> CommandResult {
		self.grouping.close();
		Self::revert(&mut self.done, &mut self.undone, state)
	}

	fn redo(&mut self, state: &mut EditorState) -> CommandResult {
		self.grouping.close();
		Self::revert(&mut self.undone, &mut self.done, state)
	}

	fn can_undo(&self) -> bool {
		!self.done.is_empty()
	}

	fn can_redo(&self) -> bool {
		!self.undone.is_empty()
	}

	fn close_group(&mut self) {
		self.grouping.close();
	}
}
