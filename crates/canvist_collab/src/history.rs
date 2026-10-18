//! Undo history an editor can swap between local and shared.
//!
//! An editor records each local commit in a [`History`] and asks it to undo
//! and redo. Without collaboration, history can keep each commit's inverse
//! steps. With collaboration, undoing a step must not revert work
//! collaborators did since, so the [`Binding`] implements `History` on top
//! of a yrs `UndoManager` that only tracks local changes.
//!
//! `canvist_editor` is expected to own this trait once its history lands;
//! it lives here until then so the binding can implement it.

use canvist_model::Commit;
use canvist_model::Document;

use crate::Binding;
use crate::CollabError;
use crate::Track;

/// Undo and redo for one document.
pub trait History {
	/// Why recording, undoing, or redoing failed.
	type Error;

	/// Record a local commit that was just applied to `doc`. `merge` joins
	/// it to the previous undo step, as consecutive keystrokes of a word do.
	///
	/// Returns a follow-up commit already applied to `doc`, usually empty. A
	/// shared history also sends the commit to collaborators here, and
	/// retyping a container can reveal children a collaborator added.
	fn record(
		&mut self,
		doc: &mut Document,
		commit: &Commit,
		merge: bool,
	) -> Result<Commit, Self::Error>;

	/// Undo the latest step, returning the commit that did, or `None` when
	/// there is nothing to undo.
	fn undo(&mut self, doc: &mut Document) -> Result<Option<Commit>, Self::Error>;

	/// Redo the latest undone step, returning the commit that did, or `None`
	/// when there is nothing to redo.
	fn redo(&mut self, doc: &mut Document) -> Result<Option<Commit>, Self::Error>;

	/// Whether there is a step to undo.
	fn can_undo(&self) -> bool;

	/// Whether there is a step to redo.
	fn can_redo(&self) -> bool;
}

/// Shared history: undo reverts only this peer's changes, and every recorded
/// commit is mirrored to collaborators.
impl History for Binding {
	type Error = CollabError;

	fn record(
		&mut self,
		doc: &mut Document,
		commit: &Commit,
		merge: bool,
	) -> Result<Commit, Self::Error> {
		let track = if merge { Track::Merge } else { Track::Step };
		self.apply_local(doc, commit, track)
	}

	fn undo(&mut self, doc: &mut Document) -> Result<Option<Commit>, Self::Error> {
		Binding::undo(self, doc)
	}

	fn redo(&mut self, doc: &mut Document) -> Result<Option<Commit>, Self::Error> {
		Binding::redo(self, doc)
	}

	fn can_undo(&self) -> bool {
		Binding::can_undo(self)
	}

	fn can_redo(&self) -> bool {
		Binding::can_redo(self)
	}
}
