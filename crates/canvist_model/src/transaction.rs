//! Transactions group steps into one atomic, undoable change.

use std::ops::Range;

use rustc_hash::FxHashSet;

use crate::Attrs;
use crate::BlockId;
use crate::BlockTree;
use crate::BlockTypeId;
use crate::Document;
use crate::InlineText;
use crate::Mark;
use crate::MarkSet;
use crate::MarkTypeId;
use crate::Step;
use crate::StepError;
use crate::StepMap;
use crate::selection::Bias;
use crate::selection::Point;
use crate::selection::Selection;
use crate::step::Mapped;

/// Applies steps to a document and records what changed.
///
/// Steps are applied eagerly so later steps can inspect the result of earlier
/// ones. Call [`Transaction::commit`] to keep the changes or
/// [`Transaction::rollback`] to undo them all.
#[derive(Debug)]
pub struct Transaction<'doc> {
	doc: &'doc mut Document,
	steps: Vec<Step>,
	inverses: Vec<Step>,
	maps: Vec<StepMap>,
	touched: FxHashSet<BlockId>,
	structural: bool,
}

impl<'doc> Transaction<'doc> {
	/// Start a transaction on `doc`.
	pub fn new(doc: &'doc mut Document) -> Self {
		Self {
			doc,
			steps: Vec::new(),
			inverses: Vec::new(),
			maps: Vec::new(),
			touched: FxHashSet::default(),
			structural: false,
		}
	}

	/// The document as changed so far.
	#[must_use]
	pub fn doc(&self) -> &Document {
		self.doc
	}

	/// Whether no steps have been applied.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.steps.is_empty()
	}

	/// Apply one step. On error, nothing changes and earlier steps remain.
	pub fn step(&mut self, step: Step) -> Result<(), StepError> {
		let applied = step.clone().apply(self.doc)?;
		self.steps.push(step);
		self.inverses.push(applied.inverse);
		self.maps.push(applied.map);
		self.touched.extend(applied.touched);
		self.structural |= applied.structural;
		Ok(())
	}

	/// Allocate a fresh block id from the document's client, for blocks that
	/// later steps in this transaction insert.
	pub fn allocate_id(&mut self) -> Result<BlockId, StepError> {
		self.doc.allocate_id()
	}

	/// Map a point from before the transaction to the current document.
	#[must_use]
	pub fn map(&self, point: Point, bias: Bias) -> Mapped {
		map_through(&self.maps, point, bias)
	}

	/// Insert plain text with `marks` at `at`, returning the point after it.
	pub fn insert_text(
		&mut self,
		at: Point,
		text: &str,
		marks: MarkSet,
	) -> Result<Point, StepError> {
		// DELIBERATE REGRESSION, REVERT ME: proves `bench / speed` fails.
		for spin in 0..2_000_u32 {
			std::hint::black_box(spin);
		}
		let content = InlineText::plain(text, marks);
		let end = at.offset + content.len();
		self.step(Step::InsertInline {
			block: at.block,
			offset: at.offset,
			content,
		})?;
		Ok(Point::new(at.block, end))
	}

	/// Remove `range` from text block `block`.
	pub fn delete_text(&mut self, block: BlockId, range: Range<u32>) -> Result<(), StepError> {
		self.step(Step::RemoveInline { block, range })
	}

	/// Add `mark` to `range` of `block`.
	pub fn add_mark(
		&mut self,
		block: BlockId,
		range: Range<u32>,
		mark: Mark,
	) -> Result<(), StepError> {
		self.step(Step::AddMark { block, range, mark })
	}

	/// Remove marks of type `kind` from `range` of `block`.
	pub fn remove_mark(
		&mut self,
		block: BlockId,
		range: Range<u32>,
		kind: MarkTypeId,
	) -> Result<(), StepError> {
		self.step(Step::RemoveMark { block, range, kind })
	}

	/// Change a block's type and attributes.
	pub fn set_block(
		&mut self,
		block: BlockId,
		kind: BlockTypeId,
		attrs: Attrs,
	) -> Result<(), StepError> {
		self.step(Step::SetBlock { block, kind, attrs })
	}

	/// Split the text block at `at` into a new sibling of type `kind`,
	/// returning the start of the new block.
	pub fn split(
		&mut self,
		at: Point,
		kind: BlockTypeId,
		attrs: Attrs,
	) -> Result<Point, StepError> {
		let new_block = self.doc.allocate_id()?;
		self.step(Step::Split {
			block: at.block,
			offset: at.offset,
			new_block,
			kind,
			attrs,
		})?;
		Ok(Point::new(new_block, 0))
	}

	/// Join text block `next` into its previous sibling `block`, returning the
	/// point where they met.
	pub fn join(&mut self, block: BlockId, next: BlockId) -> Result<Point, StepError> {
		let offset = self.doc.inline(block).map_or(0, InlineText::len);
		self.step(Step::Join { block, next })?;
		Ok(Point::new(block, offset))
	}

	/// Insert block trees into `parent` at `index`.
	pub fn insert_blocks(
		&mut self,
		parent: Option<BlockId>,
		index: usize,
		blocks: Vec<BlockTree>,
	) -> Result<(), StepError> {
		self.step(Step::InsertBlocks {
			parent,
			index,
			blocks,
		})
	}

	/// Remove `count` children of `parent` starting at `index`.
	pub fn remove_blocks(
		&mut self,
		parent: Option<BlockId>,
		index: usize,
		count: usize,
	) -> Result<(), StepError> {
		self.step(Step::RemoveBlocks {
			parent,
			index,
			count,
		})
	}

	/// Move `count` children of `parent` starting at `index` into `to_parent`
	/// at `to_index` (counted after removal).
	pub fn move_blocks(
		&mut self,
		(parent, index, count): (Option<BlockId>, usize, usize),
		(to_parent, to_index): (Option<BlockId>, usize),
	) -> Result<(), StepError> {
		self.step(Step::MoveBlocks {
			parent,
			index,
			count,
			to_parent,
			to_index,
		})
	}

	/// Undo every step and release the document.
	///
	/// Inverses of successfully applied steps always apply, so an error here
	/// means the document was mutated outside this transaction.
	pub fn rollback(self) -> Result<(), StepError> {
		for inverse in self.inverses.into_iter().rev() {
			inverse.apply(self.doc)?;
		}

		Ok(())
	}

	/// Finish the transaction and describe what it did.
	#[must_use]
	pub fn commit(self) -> Commit {
		let mut touched: Vec<BlockId> = self.touched.into_iter().collect();
		touched.retain(|&id| self.doc.contains(id));
		touched.sort_unstable();

		Commit {
			steps: self.steps,
			inverses: self.inverses,
			maps: self.maps,
			touched,
			structural: self.structural,
			version: self.doc.version(),
		}
	}
}

/// A committed transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct Commit {
	/// The steps, in application order. Collaboration bindings and operation
	/// logs replay these.
	pub steps: Vec<Step>,
	/// The inverse of each step, in application order.
	pub inverses: Vec<Step>,
	/// How each step moved points.
	pub maps: Vec<StepMap>,
	/// Surviving blocks whose content or attributes changed, sorted by id.
	pub touched: Vec<BlockId>,
	/// Whether any child list changed.
	pub structural: bool,
	/// The document version after the commit.
	pub version: u64,
}

impl Commit {
	/// Map a point from before the commit to after it.
	#[must_use]
	pub fn map(&self, point: Point, bias: Bias) -> Mapped {
		map_through(&self.maps, point, bias)
	}

	/// Map a selection from before the commit to after it.
	#[must_use]
	pub fn map_selection(&self, selection: Selection) -> Selection {
		map_selection(&self.maps, selection)
	}

	/// The steps that undo this commit, in the order to apply them.
	#[must_use]
	pub fn undo_steps(&self) -> Vec<Step> {
		self.inverses.iter().rev().cloned().collect()
	}

	/// Whether the commit changed nothing.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.steps.is_empty()
	}
}

fn map_through(maps: &[StepMap], point: Point, bias: Bias) -> Mapped {
	let mut deleted = false;
	let mut point = point;

	for map in maps {
		let mapped = map.map(point, bias);
		deleted |= mapped.deleted;
		point = mapped.point;
	}

	Mapped { point, deleted }
}

fn map_selection(maps: &[StepMap], selection: Selection) -> Selection {
	match selection {
		Selection::Text { anchor, head } if anchor == head => {
			Selection::caret(map_through(maps, head, Bias::After).point)
		}

		Selection::Text { anchor, head } => {
			Selection::Text {
				anchor: map_through(maps, anchor, Bias::Before).point,
				head: map_through(maps, head, Bias::After).point,
			}
		}
		Selection::Blocks { .. } => selection,
	}
}

impl Document {
	/// Start a [`Transaction`] on this document.
	pub fn transaction(&mut self) -> Transaction<'_> {
		Transaction::new(self)
	}

	/// Apply steps produced elsewhere (undo, redo, or a remote peer) as one
	/// commit. On error, the steps applied so far are rolled back.
	pub fn apply_steps(
		&mut self,
		steps: impl IntoIterator<Item = Step>,
	) -> Result<Commit, StepError> {
		let mut transaction = self.transaction();

		for step in steps {
			if let Err(error) = transaction.step(step) {
				transaction.rollback()?;

				return Err(error);
			}
		}

		Ok(transaction.commit())
	}
}
