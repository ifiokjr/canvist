//! Local changes: committed steps become yrs operations.
//!
//! Each step maps onto one `Y.Text`, `Y.Map`, or `Y.Array` operation, or a
//! short fixed sequence of them. Text offsets are converted from the
//! model's UTF-8 bytes to UTF-16 using the shared text itself, which mirrors
//! the model exactly at the moment each step is translated.

use std::ops::Range;

use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::BlockTypeId;
use canvist_model::Document;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::Schema;
use canvist_model::Step;
use rustc_hash::FxHashSet;
use yrs::Any;
use yrs::Array;
use yrs::ArrayRef;
use yrs::BranchID;
use yrs::ID;
use yrs::Map;
use yrs::MapRef;
use yrs::Out;
use yrs::ReadTxn;
use yrs::Text;
use yrs::TextRef;
use yrs::TransactionMut;
use yrs::types::Delta;

use crate::CollabError;
use crate::keys;
use crate::registry::Owner;
use crate::registry::Registration;
use crate::registry::Registry;
use crate::registry::branch_id;
use crate::snapshot::Snapshot;
use crate::text;
use crate::text::Chunks;
use crate::tree;
use crate::values;
use crate::values::Format;

/// Translates the steps of one local commit inside one yrs transaction.
pub(crate) struct Local<'a, 'txn> {
	pub(crate) txn: &'a mut TransactionMut<'txn>,
	pub(crate) root: &'a ArrayRef,
	pub(crate) schema: &'a Schema,
	pub(crate) registry: &'a mut Registry,
	/// The document after the commit.
	pub(crate) doc: &'a Document,
	/// Whether the commit changes at most one child list, in which case the
	/// document's child indices are also valid while translating its steps.
	pub(crate) single_structural: bool,
	/// Containers whose type changed while holding hidden children, which
	/// the new type may allow.
	pub(crate) retyped: Vec<BlockId>,
}

impl Local<'_, '_> {
	/// Translate one step, given the inverse the model recorded for it.
	pub(crate) fn step(&mut self, step: &Step, inverse: &Step) -> Result<(), CollabError> {
		if is_inline(step) {
			self.inline_step(step)
		} else {
			self.block_step(step, inverse)
		}
	}

	/// Translate a step that edits the text of one block.
	fn inline_step(&mut self, step: &Step) -> Result<(), CollabError> {
		match step {
			Step::InsertInline {
				block,
				offset,
				content,
			} => self.insert_inline(*block, *offset, content),
			Step::RemoveInline { block, range } => self.remove_inline(*block, range),
			Step::AddMark { block, range, mark } => {
				let format = values::add_mark_format(self.schema, mark);
				self.format(*block, range, format)
			}
			Step::RemoveMark { block, range, kind } => {
				let name = self.schema.mark(*kind).name.clone();
				self.format(*block, range, Format::from([(name, Any::Null)]))
			}
			Step::RestoreMarks {
				block,
				offset,
				runs,
			} => self.restore_marks(*block, *offset, runs),
			// `step` sends only inline steps here.
			_ => Ok(()),
		}
	}

	/// Translate a step that changes blocks themselves.
	fn block_step(&mut self, step: &Step, inverse: &Step) -> Result<(), CollabError> {
		match step {
			Step::SetBlock { block, kind, attrs } => self.set_block(*block, *kind, attrs),
			Step::Split {
				block,
				offset,
				new_block,
				kind,
				attrs,
			} => self.split(*block, *offset, (*new_block, *kind, attrs)),
			Step::Join { block, next } => self.join(*block, *next),
			Step::InsertBlocks {
				parent,
				index,
				blocks,
			} => self.insert_blocks(*parent, *index, blocks),
			Step::RemoveBlocks {
				parent,
				index,
				count,
			} => self.remove_blocks(*parent, *index, *count, inverse),
			Step::MoveBlocks {
				parent,
				index,
				count,
				to_parent,
				to_index,
			} => self.move_blocks((*parent, *index, *count), (*to_parent, *to_index)),
			// `step` sends inline steps to `inline_step` instead.
			_ => Ok(()),
		}
	}

	fn map(&self, block: BlockId) -> Result<MapRef, CollabError> {
		let found = self
			.registry
			.block(block)
			.ok_or(CollabError::NotShared(block))?;
		map_ref(&*self.txn, &found.map).ok_or(CollabError::NotShared(block))
	}

	fn text(&self, block: BlockId) -> Result<TextRef, CollabError> {
		tree::text_of(&self.map(block)?, &*self.txn).ok_or(CollabError::NotShared(block))
	}

	fn array(&self, parent: Option<BlockId>) -> Result<ArrayRef, CollabError> {
		let Some(parent) = parent else {
			return Ok(self.root.clone());
		};

		tree::children_of(&self.map(parent)?, &*self.txn).ok_or(CollabError::NotShared(parent))
	}

	/// UTF-16 positions in `text` for each UTF-8 offset in `offsets`.
	fn utf16<const N: usize>(
		&self,
		block: BlockId,
		text: &TextRef,
		offsets: [u32; N],
	) -> Result<[u32; N], CollabError> {
		let chunks = Chunks::read(text, &*self.txn);
		let mut converted = [0; N];

		for (slot, offset) in converted.iter_mut().zip(offsets) {
			*slot = chunks
				.utf16_index(offset)
				.ok_or(CollabError::Offset { block, offset })?;
		}

		Ok(converted)
	}

	fn insert_inline(
		&mut self,
		block: BlockId,
		offset: u32,
		content: &InlineText,
	) -> Result<(), CollabError> {
		let text = self.text(block)?;
		let [at] = self.utf16(block, &text, [offset])?;
		text::insert_inline(&text, self.txn, at, self.schema, content);
		Ok(())
	}

	fn remove_inline(&mut self, block: BlockId, range: &Range<u32>) -> Result<(), CollabError> {
		let text = self.text(block)?;
		let [start, end] = self.utf16(block, &text, [range.start, range.end])?;
		text.remove_range(self.txn, start, end - start);
		Ok(())
	}

	fn format(
		&mut self,
		block: BlockId,
		range: &Range<u32>,
		format: Format,
	) -> Result<(), CollabError> {
		let text = self.text(block)?;
		let [start, end] = self.utf16(block, &text, [range.start, range.end])?;
		text.format(self.txn, start, end - start, format);
		Ok(())
	}

	fn restore_marks(
		&mut self,
		block: BlockId,
		offset: u32,
		runs: &[(u32, MarkSet)],
	) -> Result<(), CollabError> {
		let text = self.text(block)?;
		let chunks = Chunks::read(&text, &*self.txn);
		let mut start = offset;

		for (len, marks) in runs {
			let end = start + len;
			let utf16 = |offset| {
				chunks
					.utf16_index(offset)
					.ok_or(CollabError::Offset { block, offset })
			};
			let (from, to) = (utf16(start)?, utf16(end)?);
			text.format(
				self.txn,
				from,
				to - from,
				values::full_format(self.schema, marks),
			);
			start = end;
		}

		Ok(())
	}

	/// Write only what changed: the type if it differs, and attribute keys
	/// that were added, changed, or removed, so concurrent edits to other
	/// attributes survive.
	fn set_block(
		&mut self,
		block: BlockId,
		kind: BlockTypeId,
		attrs: &Attrs,
	) -> Result<(), CollabError> {
		let map = self.map(block)?;
		self.set_type(block, &map, kind);

		let Some(shared) = tree::attrs_of(&map, &*self.txn) else {
			let created: MapRef = map.insert(self.txn, keys::ATTRS, tree::attrs_prelim(attrs));
			self.registry.set_attrs(block, branch_id(&created));

			return Ok(());
		};

		let stale: Vec<String> = shared
			.keys(&*self.txn)
			.filter(|key| attrs.get(key).is_none())
			.map(str::to_owned)
			.collect();

		for key in stale {
			shared.remove(self.txn, &key);
		}

		for (key, value) in attrs.iter() {
			let any = values::to_any(value);

			if shared.get(&*self.txn, key) != Some(Out::Any(any.clone())) {
				shared.insert(self.txn, key, any);
			}
		}

		Ok(())
	}

	/// Write `kind` as the type of `block` if it differs, noting containers
	/// whose hidden children the new type may allow.
	fn set_type(&mut self, block: BlockId, map: &MapRef, kind: BlockTypeId) {
		let name = self.schema.block(kind).name.clone();

		if tree::string(map, &*self.txn, keys::TYPE).as_deref() == Some(&*name) {
			return;
		}

		map.insert(self.txn, keys::TYPE, Any::String(name));

		if !self.registry.is_mirrored(Some(block)) {
			self.retyped.push(block);
		}
	}

	fn split(
		&mut self,
		block: BlockId,
		offset: u32,
		(new_block, kind, attrs): (BlockId, BlockTypeId, &Attrs),
	) -> Result<(), CollabError> {
		let text = self.text(block)?;
		let chunks = Chunks::read(&text, &*self.txn);
		let at = chunks
			.utf16_index(offset)
			.ok_or(CollabError::Offset { block, offset })?;
		let tail = chunks.delta_from(&*self.txn, at);
		let len = text.len(&*self.txn);
		text.remove_range(self.txn, at, len - at);

		let parent = self.parent_of(block)?;
		let index = self.index_of(block, parent)? + 1;
		let tree = BlockTree::text(new_block, kind, InlineText::new()).with_attrs(attrs.clone());
		let mut registration = Registration::default();
		let array = self.array(parent)?;
		tree::write(
			self.txn,
			self.schema,
			(&array, index, parent),
			&tree,
			&mut registration,
		)?;
		let new_text = self.text_after(registration, new_block)?;
		insert_delta(&new_text, self.txn, 0, tail);
		Ok(())
	}

	/// Register a freshly written block and return its `Y.Text`.
	fn text_after(
		&mut self,
		registration: Registration,
		block: BlockId,
	) -> Result<TextRef, CollabError> {
		self.registry.register(registration);
		self.text(block)
	}

	fn join(&mut self, block: BlockId, next: BlockId) -> Result<(), CollabError> {
		let text = self.text(block)?;
		let next_text = self.text(next)?;
		let content = Chunks::read(&next_text, &*self.txn).delta_from(&*self.txn, 0);
		let end = text.len(&*self.txn);
		insert_delta(&text, self.txn, end, content);

		let parent = self.parent_of(next)?;
		let index = self.index_of(next, parent)?;
		self.array(parent)?.remove(self.txn, index);
		self.registry.remove(&FxHashSet::from_iter([next]));
		Ok(())
	}

	fn insert_blocks(
		&mut self,
		parent: Option<BlockId>,
		index: usize,
		blocks: &[BlockTree],
	) -> Result<(), CollabError> {
		let array = self.array(parent)?;
		let start = self.insert_position(&array, parent, index);
		let mut registration = Registration::default();

		for (offset, tree) in (start..).zip(blocks) {
			tree::write(
				self.txn,
				self.schema,
				(&array, offset, parent),
				tree,
				&mut registration,
			)?;
		}

		self.registry.register(registration);
		Ok(())
	}

	fn remove_blocks(
		&mut self,
		parent: Option<BlockId>,
		index: usize,
		count: usize,
		inverse: &Step,
	) -> Result<(), CollabError> {
		let array = self.array(parent)?;
		let positions = self.shown_positions(&array, parent, index..index + count);

		for &position in positions.iter().rev() {
			array.remove(self.txn, position);
		}

		if let Step::InsertBlocks { blocks, .. } = inverse {
			self.registry.remove(&subtree_ids(blocks));
		}

		Ok(())
	}

	fn move_blocks(
		&mut self,
		(parent, index, count): (Option<BlockId>, usize, usize),
		(to_parent, to_index): (Option<BlockId>, usize),
	) -> Result<(), CollabError> {
		let source = self.array(parent)?;
		let positions = self.shown_positions(&source, parent, index..index + count);
		let snapshots: Vec<Snapshot> = positions
			.iter()
			.filter_map(|&position| {
				match source.get(&*self.txn, position)? {
					Out::YMap(map) => Snapshot::read(&map, &*self.txn),
					_ => None,
				}
			})
			.collect();

		for &position in positions.iter().rev() {
			source.remove(self.txn, position);
		}

		let target = self.array(to_parent)?;
		let start = self.insert_position(&target, to_parent, to_index);
		let mut copies = Vec::new();

		for (offset, snapshot) in (start..).zip(&snapshots) {
			snapshot.write(self.txn, (&target, offset, to_parent), &mut copies)?;
		}

		self.registry.rebind(copies);
		Ok(())
	}

	fn parent_of(&self, block: BlockId) -> Result<Option<BlockId>, CollabError> {
		self.registry
			.block(block)
			.map(|found| found.parent)
			.ok_or(CollabError::NotShared(block))
	}

	/// The yrs index of `block` among the items of its parent's array.
	fn index_of(&self, block: BlockId, parent: Option<BlockId>) -> Result<u32, CollabError> {
		let same_parent = self
			.doc
			.block(block)
			.is_some_and(|found| found.parent() == parent);

		if self.single_structural
			&& same_parent
			&& self.registry.is_mirrored(parent)
			&& let Some(index) = self.doc.index_in_parent(block)
		{
			return Ok(index as u32);
		}

		let map = self
			.registry
			.block(block)
			.ok_or(CollabError::NotShared(block))?
			.map;
		let array = self.array(parent)?;
		position_of(&array, &*self.txn, &map).ok_or(CollabError::NotShared(block))
	}

	/// The yrs index at which to insert before model child `index`.
	fn insert_position(&self, array: &ArrayRef, parent: Option<BlockId>, index: usize) -> u32 {
		if self.registry.is_mirrored(parent) {
			return index as u32;
		}

		let shown = self.shown_positions(array, parent, index..index + 1);
		shown
			.first()
			.copied()
			.unwrap_or_else(|| array.len(&*self.txn))
	}

	/// The yrs indices of the model children of `parent` in `range`.
	fn shown_positions(
		&self,
		array: &ArrayRef,
		parent: Option<BlockId>,
		range: Range<usize>,
	) -> Vec<u32> {
		if self.registry.is_mirrored(parent) {
			return (range.start as u32..range.end as u32).collect();
		}

		(0_u32..)
			.zip(array.iter(&*self.txn))
			.filter(|(_, item)| {
				let Out::YMap(map) = item else {
					return false;
				};
				branch_id(map)
					.and_then(|id| self.registry.owner(&id))
					.is_some_and(|owner| matches!(owner, Owner::Block(_)))
			})
			.map(|(position, _)| position)
			.skip(range.start)
			.take(range.len())
			.collect()
	}
}

/// Whether `step` edits the text of a single block.
fn is_inline(step: &Step) -> bool {
	matches!(
		step,
		Step::InsertInline { .. }
			| Step::RemoveInline { .. }
			| Step::AddMark { .. }
			| Step::RemoveMark { .. }
			| Step::RestoreMarks { .. }
	)
}

/// Insert a copied delta into `text` at UTF-16 `index`, keeping each chunk's
/// formatting exactly.
fn insert_delta(
	text: &TextRef,
	txn: &mut TransactionMut<'_>,
	index: u32,
	delta: Vec<Delta<yrs::In>>,
) {
	let mut at = index;

	for op in delta {
		let Delta::Inserted(value, format) = op else {
			continue;
		};
		let format = format.map(|format| *format).unwrap_or_default();

		match value {
			yrs::In::Any(Any::String(chunk)) => {
				text.insert_with_attributes(txn, at, &chunk, format);
				at += text::utf16_len(&chunk);
			}
			yrs::In::Any(embed) => {
				text.insert_embed_with_attributes(txn, at, embed, format);
				at += 1;
			}
			_ => {}
		}
	}
}

/// The `Y.Map` with item id `id`, if it is still part of the document.
pub(crate) fn map_ref<T: ReadTxn>(txn: &T, id: &ID) -> Option<MapRef> {
	BranchID::get_nested(txn, id).map(MapRef::from)
}

/// The index of the block map `map` in `array`.
pub(crate) fn position_of<T: ReadTxn>(array: &ArrayRef, txn: &T, map: &ID) -> Option<u32> {
	(0_u32..)
		.zip(array.iter(txn))
		.find(
			|(_, item)| matches!(item, Out::YMap(found) if branch_id(found).as_ref() == Some(map)),
		)
		.map(|(position, _)| position)
}

/// Every block id in `trees`, descendants included.
pub(crate) fn subtree_ids(trees: &[BlockTree]) -> FxHashSet<BlockId> {
	let mut ids = FxHashSet::default();

	for tree in trees {
		tree.walk(&mut |node| {
			ids.insert(node.id);
		});
	}

	ids
}
