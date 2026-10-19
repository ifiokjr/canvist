//! Which shared types mirror which model blocks.

use canvist_model::BlockId;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use yrs::BranchID;
use yrs::ID;
use yrs::branch::Branch;

/// The shared types behind one shown block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BlockRef {
	/// The block's `Y.Map`.
	pub(crate) map: ID,
	/// The block's parent, `None` for the root.
	pub(crate) parent: Option<BlockId>,
	/// Its `Y.Text`, for text blocks.
	pub(crate) text: Option<ID>,
	/// Its attribute `Y.Map`.
	pub(crate) attrs: Option<ID>,
	/// Its child `Y.Array`, for containers.
	pub(crate) children: Option<ID>,
}

impl BlockRef {
	fn branches(self) -> impl Iterator<Item = ID> {
		[Some(self.map), self.text, self.attrs, self.children]
			.into_iter()
			.flatten()
	}
}

/// What a shared type is to the binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
	/// The `Y.Map` of a shown block.
	Block(BlockId),
	/// The `Y.Text` of a shown block.
	Text(BlockId),
	/// The attribute `Y.Map` of a shown block.
	Attrs(BlockId),
	/// The child `Y.Array` of a shown container.
	Children(BlockId),
	/// A block `Y.Map` that the model does not show, directly inside the
	/// given shown parent (`None` is the root). It may become visible when
	/// it or its parent changes.
	Hidden(Option<BlockId>),
}

/// Shared types found while reading a subtree, registered once the model
/// accepts it.
#[derive(Debug, Default)]
pub(crate) struct Registration {
	/// Shown blocks.
	pub(crate) blocks: Vec<(BlockId, BlockRef)>,
	/// Hidden block maps and their parents.
	pub(crate) hidden: Vec<(ID, Option<BlockId>)>,
	/// Items the model does not mirror, per parent.
	pub(crate) unmirrored: Vec<(Option<BlockId>, u32)>,
	/// Hidden block maps that lost their block id to another map, and their
	/// parents. The binding deletes them.
	pub(crate) losers: Vec<(ID, Option<BlockId>)>,
}

/// The two-way index between model blocks and shared types.
#[derive(Debug, Default)]
pub(crate) struct Registry {
	blocks: FxHashMap<BlockId, BlockRef>,
	owners: FxHashMap<ID, Owner>,
	/// How many items in each parent's array the model does not show.
	unmirrored: FxHashMap<Option<BlockId>, u32>,
}

impl Registry {
	/// The shared types of block `id`.
	pub(crate) fn block(&self, id: BlockId) -> Option<BlockRef> {
		self.blocks.get(&id).copied()
	}

	/// What the shared type `branch` is to the binding.
	pub(crate) fn owner(&self, branch: &ID) -> Option<Owner> {
		self.owners.get(branch).copied()
	}

	/// The shown block whose `Y.Map` is `map`.
	pub(crate) fn block_of_map(&self, map: &ID) -> Option<BlockId> {
		match self.owner(map)? {
			Owner::Block(id) => Some(id),
			_ => None,
		}
	}

	/// The model children of `parent` match its shared array one-to-one.
	pub(crate) fn is_mirrored(&self, parent: Option<BlockId>) -> bool {
		self.unmirrored.get(&parent).is_none_or(|&count| count == 0)
	}

	/// Record everything found while reading a subtree.
	pub(crate) fn register(&mut self, registration: Registration) {
		for (id, block) in registration.blocks {
			self.insert(id, block);
		}

		for (map, parent) in registration.hidden {
			self.owners.insert(map, Owner::Hidden(parent));
		}

		for (parent, count) in registration.unmirrored {
			*self.unmirrored.entry(parent).or_default() += count;
		}
	}

	/// Record one shown block.
	pub(crate) fn insert(&mut self, id: BlockId, block: BlockRef) {
		self.owners.insert(block.map, Owner::Block(id));
		block
			.text
			.map(|text| self.owners.insert(text, Owner::Text(id)));
		block
			.attrs
			.map(|attrs| self.owners.insert(attrs, Owner::Attrs(id)));
		block
			.children
			.map(|children| self.owners.insert(children, Owner::Children(id)));
		self.blocks.insert(id, block);
	}

	/// Forget the blocks `ids` (a removed subtree), the hidden maps directly
	/// inside them, and their unmirrored counts.
	pub(crate) fn remove(&mut self, ids: &FxHashSet<BlockId>) {
		for &id in ids {
			self.forget_block(id);
		}

		let had_hidden = ids
			.iter()
			.filter_map(|id| self.unmirrored.remove(&Some(*id)))
			.any(|count| count > 0);

		if had_hidden {
			self.owners.retain(
				|_, owner| !matches!(owner, Owner::Hidden(Some(parent)) if ids.contains(parent)),
			);
		}
	}

	/// Forget a hidden block map that left its parent or became visible.
	pub(crate) fn remove_hidden(&mut self, map: &ID) {
		if let Some(Owner::Hidden(parent)) = self.owners.remove(map) {
			self.adjust_unmirrored(parent, -1);
		}
	}

	/// Forget a hidden block map whose parent's count was just recounted.
	pub(crate) fn forget_hidden(&mut self, map: &ID) {
		if matches!(self.owners.get(map), Some(Owner::Hidden(_))) {
			self.owners.remove(map);
		}
	}

	/// Record that `map`, directly inside `parent`, is hidden.
	pub(crate) fn insert_hidden(&mut self, map: ID, parent: Option<BlockId>) {
		self.owners.insert(map, Owner::Hidden(parent));
		self.adjust_unmirrored(parent, 1);
	}

	/// Change the unmirrored count of `parent` by `delta`.
	pub(crate) fn adjust_unmirrored(&mut self, parent: Option<BlockId>, delta: i32) {
		let count = self.unmirrored.entry(parent).or_default();
		*count = count.saturating_add_signed(delta);
	}

	/// Replace the unmirrored count of `parent` after recounting it.
	pub(crate) fn set_unmirrored(&mut self, parent: Option<BlockId>, count: u32) {
		self.unmirrored.insert(parent, count);
	}

	/// How many items in all shared arrays the model does not show.
	pub(crate) fn unmirrored_total(&self) -> u32 {
		self.unmirrored.values().sum()
	}

	/// Every hidden block map directly inside `parent`.
	pub(crate) fn hidden_in(&self, parent: Option<BlockId>) -> Vec<ID> {
		if self.is_mirrored(parent) {
			return Vec::new();
		}

		self.owners
			.iter()
			.filter(|(_, owner)| **owner == Owner::Hidden(parent))
			.map(|(map, _)| *map)
			.collect()
	}

	/// Point block `id` at a replacement attribute map.
	pub(crate) fn set_attrs(&mut self, id: BlockId, attrs: Option<ID>) {
		let Some(block) = self.blocks.get(&id).copied() else {
			return;
		};

		if let Some(old) = block.attrs {
			self.owners.remove(&old);
		}

		self.insert(id, BlockRef { attrs, ..block });
	}

	/// Forget block `id` and the owners of its shared types.
	fn forget_block(&mut self, id: BlockId) {
		let Some(block) = self.blocks.remove(&id) else {
			return;
		};

		for branch in block.branches() {
			self.owners.remove(&branch);
		}
	}

	/// Move registrations from copied block maps to their copies. Block ids,
	/// and so hidden and unmirrored bookkeeping keyed by them, are unchanged.
	pub(crate) fn rebind(&mut self, copies: Vec<(ID, BlockRef)>) {
		for (source, copy) in copies {
			match self.owners.remove(&source) {
				Some(Owner::Block(id)) => {
					self.forget_block(id);
					self.insert(id, copy);
				}
				Some(Owner::Hidden(parent)) => {
					self.owners.insert(copy.map, Owner::Hidden(parent));
				}
				Some(owner) => {
					self.owners.insert(source, owner);
				}
				None => {}
			}
		}
	}
}

/// The item id of a nested shared type, or `None` for a root type.
pub(crate) fn branch_id(branch: &impl AsRef<Branch>) -> Option<ID> {
	match branch.as_ref().id() {
		BranchID::Nested(id) => Some(id),
		BranchID::Root(_) => None,
	}
}
