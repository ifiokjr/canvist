//! Editor transactions: document steps plus the selection, stored marks, and
//! composition they leave behind.

use std::ops::Deref;
use std::ops::DerefMut;

use canvist_model::BlockId;
use canvist_model::Commit;
use canvist_model::MarkSet;
use canvist_model::Selection;
use canvist_model::StepMap;
use canvist_model::Transaction;

use crate::Composition;

/// What kind of edit an update made.
///
/// History merges consecutive updates of a groupable kind, so a typed word
/// undoes in one step while a format change always undoes on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditKind {
	/// Only the selection or stored marks changed.
	Selection,
	/// Text was typed.
	Typing,
	/// Text was deleted inside a block.
	Deleting,
	/// IME preedit text changed.
	Composition,
	/// Marks were added or removed.
	Formatting,
	/// Blocks were split, joined, wrapped, lifted, moved, or retyped.
	Structure,
	/// Content was pasted or dropped.
	Paste,
	/// An input rule rewrote the text before the caret.
	InputRule,
}

impl EditKind {
	/// Whether consecutive updates of this kind may share one history entry.
	#[must_use]
	pub const fn groups(self) -> bool {
		matches!(self, Self::Typing | Self::Deleting | Self::Composition)
	}
}

/// Where an update came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Origin {
	/// The local user, through a command.
	Local,
	/// Undo or redo.
	History,
	/// A collaborator, through the collaboration binding.
	Remote,
}

/// Everything one change did: the model commit plus the editor state around
/// it. Hosts use it to relayout touched blocks and repaint the selection;
/// history uses it to record undo steps.
#[derive(Clone, Debug, PartialEq)]
pub struct Update {
	/// The applied steps, their inverses, and how points moved.
	pub commit: Commit,
	/// The selection before the change.
	pub selection_before: Selection,
	/// The selection after the change.
	pub selection: Selection,
	/// What kind of edit this was.
	pub kind: EditKind,
	/// Who made it.
	pub origin: Origin,
}

impl Update {
	/// Whether the document changed.
	#[must_use]
	pub fn doc_changed(&self) -> bool {
		!self.commit.is_empty()
	}

	/// Whether the selection changed.
	#[must_use]
	pub fn selection_changed(&self) -> bool {
		self.selection != self.selection_before
	}

	/// Fold a later update into this one, as if both were one change.
	///
	/// The result keeps this update's kind and origin, so a fix-up appended
	/// after a keystroke (such as a trailing paragraph) undoes with it.
	#[must_use]
	pub fn then(self, next: Self) -> Self {
		Self {
			commit: compose(self.commit, next.commit),
			selection_before: self.selection_before,
			selection: next.selection,
			kind: self.kind,
			origin: self.origin,
		}
	}
}

/// Concatenate two commits made one after the other.
pub(crate) fn compose(mut first: Commit, second: Commit) -> Commit {
	let removed = removed_blocks(&second.maps);
	first.touched.retain(|id| !removed.contains(id));
	first.touched.extend(second.touched);
	first.touched.sort_unstable();
	first.touched.dedup();
	first.steps.extend(second.steps);
	first.inverses.extend(second.inverses);
	first.maps.extend(second.maps);

	first.structural |= second.structural;
	first.version = second.version;
	first
}

/// Blocks that the given maps removed, directly or by joining them away.
fn removed_blocks(maps: &[StepMap]) -> Vec<BlockId> {
	let mut removed = Vec::new();

	for map in maps {
		match map {
			StepMap::Removed { blocks, .. } => removed.extend_from_slice(blocks),
			StepMap::Join { next, .. } => removed.push(*next),
			StepMap::Identity | StepMap::Inline { .. } | StepMap::Split { .. } => {}
		}
	}

	removed
}

/// Whether a transaction replaces a piece of editor state or leaves it to
/// the default rules in [`EditorState::change`](crate::EditorState::change).
#[derive(Clone, Debug, Default)]
pub(crate) enum Override<T> {
	/// Apply the default rule (map, keep, or clear).
	#[default]
	Default,
	/// Replace the value.
	Set(T),
}

/// The editor state a transaction asks for once its steps are applied.
#[derive(Clone, Debug)]
pub(crate) struct Effects {
	pub(crate) kind: EditKind,
	pub(crate) origin: Origin,
	pub(crate) selection: Option<Selection>,
	pub(crate) stored_marks: Override<Option<MarkSet>>,
	pub(crate) composition: Override<Option<Composition>>,
	pub(crate) goal: Option<f32>,
}

impl Effects {
	pub(crate) const fn new(kind: EditKind, origin: Origin) -> Self {
		Self {
			kind,
			origin,
			selection: None,
			stored_marks: Override::Default,
			composition: Override::Default,
			goal: None,
		}
	}
}

/// A model [`Transaction`] that also sets the selection, stored marks, and
/// composition, created by [`EditorState::change`](crate::EditorState::change).
///
/// It dereferences to the model transaction, so every step helper
/// (`insert_text`, `split`, `move_blocks`, ...) is available directly.
#[derive(Debug)]
pub struct EditorTransaction<'doc> {
	inner: Transaction<'doc>,
	selection_before: Selection,
	effects: Effects,
}

impl<'doc> EditorTransaction<'doc> {
	pub(crate) fn new(
		inner: Transaction<'doc>,
		selection_before: Selection,
		kind: EditKind,
	) -> Self {
		Self {
			inner,
			selection_before,
			effects: Effects::new(kind, Origin::Local),
		}
	}

	/// The selection when the transaction started.
	#[must_use]
	pub fn selection_before(&self) -> Selection {
		self.selection_before
	}

	/// Set the selection after the transaction, in the coordinates of the
	/// finished document. Call it after the steps that produce those
	/// positions. Without it, the old selection is mapped through the steps.
	pub fn set_selection(&mut self, selection: Selection) {
		self.effects.selection = Some(selection);
	}

	/// Set the marks the next typed text receives, or `None` to use the
	/// marks around the caret. Without it, stored marks are cleared whenever
	/// the document or selection changes.
	pub fn set_stored_marks(&mut self, marks: Option<MarkSet>) {
		self.effects.stored_marks = Override::Set(marks);
	}

	/// Set or clear the IME composition. Without it, an active composition
	/// is mapped through the steps.
	pub fn set_composition(&mut self, composition: Option<Composition>) {
		self.effects.composition = Override::Set(composition);
	}

	/// Keep a horizontal goal for the next vertical caret move.
	pub fn set_goal(&mut self, goal: f32) {
		self.effects.goal = Some(goal);
	}

	/// Override the kind recorded in history.
	pub fn set_kind(&mut self, kind: EditKind) {
		self.effects.kind = kind;
	}

	pub(crate) fn finish(self) -> (Transaction<'doc>, Effects) {
		(self.inner, self.effects)
	}
}

impl<'doc> Deref for EditorTransaction<'doc> {
	type Target = Transaction<'doc>;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl DerefMut for EditorTransaction<'_> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}
