//! Savepoints: return a document to an earlier state without a trace.
//!
//! Undoing steps with their inverses restores the content but not the
//! bookkeeping: the version counter and block revisions keep counting up,
//! and allocated ids stay used. Layout caches key on revisions, so a dry run
//! ("could this command run?") that left them bumped would force relayout
//! on every toolbar refresh. A savepoint records what it needs to restore
//! all of it, which makes dry runs and all-or-nothing command chains free
//! of side effects.

use rustc_hash::FxHashMap;

use crate::BlockId;
use crate::Document;
use crate::IdGenerator;
use crate::Step;
use crate::StepError;

/// A point a document can roll back to exactly. Every savepoint must be
/// passed to [`Document::rollback_to`] or [`Document::release`]; until then
/// the document records the inverse of every step.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "a savepoint must be rolled back to or released"]
pub struct Savepoint {
	depth: usize,
}

/// What one open savepoint needs to restore.
#[derive(Clone, Debug)]
pub(crate) struct SavepointLog {
	version: u64,
	ids: IdGenerator,
	/// Each affected block's revision when first touched after the savepoint.
	revs: FxHashMap<BlockId, u32>,
	/// Inverses of the steps applied since, in application order.
	inverses: Vec<Step>,
}

impl SavepointLog {
	/// Fold a nested savepoint that is being kept into this one.
	fn absorb(&mut self, inner: Self) {
		self.inverses.extend(inner.inverses);

		for (id, rev) in inner.revs {
			self.revs.entry(id).or_insert(rev);
		}
	}
}

impl Document {
	/// Open a savepoint. Savepoints nest; an outer rollback also reverts the
	/// changes of inner savepoints that were released.
	pub fn savepoint(&mut self) -> Savepoint {
		self.savepoints.push(SavepointLog {
			version: self.version(),
			ids: self.ids().clone(),
			revs: FxHashMap::default(),
			inverses: Vec::new(),
		});

		Savepoint {
			depth: self.savepoints.len() - 1,
		}
	}

	/// Revert every step since `savepoint` and restore the version, block
	/// revisions, and id allocation it saw, closing it and any savepoints
	/// opened after it.
	#[allow(
		clippy::needless_pass_by_value,
		reason = "taking the savepoint by value stops it being used twice"
	)]
	pub fn rollback_to(&mut self, savepoint: Savepoint) -> Result<(), StepError> {
		let Savepoint { depth } = savepoint;
		self.collapse_into(depth);
		let Some(log) = self.savepoints.pop() else {
			return Ok(());
		};
		// Reverting must not be recorded by the enclosing savepoints: the
		// steps it reverts never reached them.
		let enclosing = std::mem::take(&mut self.savepoints);
		let reverted = log
			.inverses
			.into_iter()
			.rev()
			.try_for_each(|inverse| inverse.apply(self).map(drop));
		self.savepoints = enclosing;
		reverted?;

		for (id, rev) in log.revs {
			self.set_rev(id, rev);
		}

		self.restore_counters(log.version, log.ids);
		Ok(())
	}

	/// Keep every change since `savepoint` and close it (and any savepoints
	/// opened after it). An enclosing savepoint can still revert them.
	#[allow(
		clippy::needless_pass_by_value,
		reason = "taking the savepoint by value stops it being used twice"
	)]
	pub fn release(&mut self, savepoint: Savepoint) {
		let Savepoint { depth } = savepoint;
		self.collapse_into(depth);
		let Some(log) = self.savepoints.pop() else {
			return;
		};

		if let Some(outer) = self.savepoints.last_mut() {
			outer.absorb(log);
		}
	}

	/// Merge savepoints nested deeper than `depth` into the one at `depth`.
	fn collapse_into(&mut self, depth: usize) {
		while self.savepoints.len() > depth + 1 {
			let Some(inner) = self.savepoints.pop() else {
				return;
			};

			if let Some(outer) = self.savepoints.last_mut() {
				outer.absorb(inner);
			}
		}
	}

	/// Remember the revisions of the blocks `step` may change or remove.
	pub(crate) fn record_before(&mut self, step: &Step) {
		if self.savepoints.is_empty() {
			return;
		}

		let affected = self.affected_by(step);
		let revs: Vec<(BlockId, u32)> = affected
			.into_iter()
			.filter_map(|id| self.block(id).map(|block| (id, block.rev())))
			.collect();

		if let Some(log) = self.savepoints.last_mut() {
			for (id, rev) in revs {
				log.revs.entry(id).or_insert(rev);
			}
		}
	}

	/// Log the inverse of a step that just applied.
	pub(crate) fn record_inverse(&mut self, inverse: &Step) {
		if let Some(log) = self.savepoints.last_mut() {
			log.inverses.push(inverse.clone());
		}
	}

	/// Blocks whose revision `step` changes, or which it removes (a removed
	/// block comes back with a fresh revision when the removal is undone).
	fn affected_by(&self, step: &Step) -> Vec<BlockId> {
		match step {
			Step::InsertInline { block, .. }
			| Step::RemoveInline { block, .. }
			| Step::AddMark { block, .. }
			| Step::RemoveMark { block, .. }
			| Step::RestoreMarks { block, .. }
			| Step::SetBlock { block, .. }
			| Step::Split { block, .. } => vec![*block],
			Step::Join { block, next } => vec![*block, *next],
			Step::RemoveBlocks {
				parent,
				index,
				count,
			} => self.removed_subtrees(*parent, *index, *count),
			Step::InsertBlocks { .. } | Step::MoveBlocks { .. } => Vec::new(),
		}
	}

	fn removed_subtrees(
		&self,
		parent: Option<BlockId>,
		index: usize,
		count: usize,
	) -> Vec<BlockId> {
		let children = self.children(parent);
		let roots = children.get(index..index + count).unwrap_or_default();
		let mut ids = Vec::new();

		for tree in roots.iter().filter_map(|&root| self.tree(root)) {
			tree.walk(&mut |node| ids.push(node.id));
		}

		ids
	}
}
