//! Structural changes: array deltas become block removals, moves, and
//! insertions.
//!
//! Each array delta is turned into removed blocks plus inserted maps, each
//! anchored after the item that precedes it in the final array. Anchors
//! instead of indices let the changes apply in any order: moves out of
//! removed containers first, then removals, then insertions, each placed
//! after its nearest anchor already in the model.
//!
//! A removed block reinserted in the same transaction with identical content
//! is a move (Yjs has no move, so peers move blocks by deleting and copying
//! them), and is applied as `MoveBlocks` so carets inside it survive.

use canvist_model::BlockId;
use canvist_model::BlockTree;
use canvist_model::Document;
use canvist_model::Step;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use yrs::Array;
use yrs::ID;
use yrs::Out;
use yrs::ReadTxn;

use super::Remote;
use super::Repair;
use super::descendants;
use super::record::ArrayOp;
use crate::CollabError;
use crate::claims::Claim;
use crate::claims::Owners;
use crate::local::map_ref;
use crate::local::subtree_ids;
use crate::registry::Owner;
use crate::registry::Registration;
use crate::registry::Registry;
use crate::registry::branch_id;
use crate::tree::Parent;
use crate::tree::Reader;

/// What the array deltas of one transaction do.
#[derive(Debug, Default)]
pub(crate) struct Plan {
	/// Shown blocks whose maps were deleted, in delta order.
	removed: Vec<BlockId>,
	/// Inserted maps, in delta order.
	inserts: Vec<Insert>,
}

/// One inserted block map.
#[derive(Clone, Copy, Debug)]
struct Insert {
	parent: Option<BlockId>,
	map: ID,
	anchor: Anchor,
}

/// The item an inserted map follows in its final array.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Anchor {
	/// The start of the array.
	Start,
	/// The block map with this item id.
	After(ID),
}

/// Whose children an array holds.
#[derive(Clone, Copy, Debug)]
enum Holder {
	/// The root array.
	Root,
	/// The child array of a shown container.
	Block(BlockId),
}

impl Holder {
	fn parent(self) -> Option<BlockId> {
		match self {
			Self::Root => None,
			Self::Block(block) => Some(block),
		}
	}
}

/// What one item of an array walk is.
enum Item {
	/// A block map inserted in this transaction.
	Inserted(ID),
	/// The map of a shown block.
	Shown(BlockId, ID),
	/// Anything the model does not show.
	Unmirrored,
}

/// An inserted map, read.
struct Read {
	insert: Insert,
	tree: Option<BlockTree>,
	registration: Registration,
}

/// Consecutive insertions into one parent, applied as one step.
#[derive(Default)]
struct Batch {
	parent: Option<BlockId>,
	index: usize,
	last: Option<ID>,
	trees: Vec<BlockTree>,
	registrations: Vec<Registration>,
}

/// Claims for maps read during a remote transaction: among the incoming
/// maps the smallest item id wins, and an incoming winner also beats an
/// existing block with a larger map id unless that block is being removed
/// anyway or contains the insertion point.
pub(crate) struct RemoteClaims<'a> {
	doc: &'a Document,
	registry: &'a Registry,
	incoming: &'a Owners,
	doomed: &'a FxHashSet<BlockId>,
	/// The parent the map being read is inserted into.
	pub(crate) target: Option<BlockId>,
	/// Existing blocks that lost their id to an incoming map.
	pub(crate) evicted: Vec<BlockId>,
}

impl<'a> RemoteClaims<'a> {
	pub(crate) fn new(
		doc: &'a Document,
		registry: &'a Registry,
		(incoming, doomed): (&'a Owners, &'a FxHashSet<BlockId>),
	) -> Self {
		Self {
			doc,
			registry,
			incoming,
			doomed,
			target: None,
			evicted: Vec::new(),
		}
	}

	fn contains_target(&self, block: BlockId) -> bool {
		self.target
			.is_some_and(|target| target == block || self.doc.is_ancestor(block, target))
	}
}

impl Claim for RemoteClaims<'_> {
	fn claim(&mut self, id: BlockId, map: &ID) -> bool {
		if self.incoming.get(id) != Some(map) {
			return false;
		}

		let Some(existing) = self.registry.block(id) else {
			return true;
		};

		if self.doomed.contains(&id) || self.evicted.contains(&id) || existing.map == *map {
			return true;
		}

		if *map < existing.map && !self.contains_target(id) {
			self.evicted.push(id);

			return true;
		}

		false
	}
}

impl<T: ReadTxn> Remote<'_, '_, T> {
	/// Add one array record to the plan.
	pub(super) fn plan_array(
		&mut self,
		array: Option<ID>,
		ops: &[ArrayOp],
		(inserted, removed): (&FxHashSet<ID>, &FxHashSet<ID>),
		plan: &mut Plan,
	) -> Result<(), CollabError> {
		let Some(holder) = self.holder(array) else {
			return Ok(());
		};
		let parent = holder.parent();

		if self.registry.is_mirrored(parent) {
			self.plan_delta(parent, ops, plan)
		} else {
			self.plan_walk(parent, (inserted, removed), plan)
		}
	}

	/// Whose children `array` holds, or `None` for arrays the model does not
	/// mirror, such as the children of a hidden block.
	fn holder(&self, array: Option<ID>) -> Option<Holder> {
		let Some(array) = array else {
			return Some(Holder::Root);
		};

		match self.registry.owner(&array)? {
			Owner::Children(block) => Some(Holder::Block(block)),
			_ => None,
		}
	}

	/// Plan from the delta, which lines up with the model's children.
	fn plan_delta(
		&mut self,
		parent: Option<BlockId>,
		ops: &[ArrayOp],
		plan: &mut Plan,
	) -> Result<(), CollabError> {
		let children = self.tx.doc().children(parent).to_vec();
		let mut cursor = 0;
		let mut anchor = Anchor::Start;

		for op in ops {
			match op {
				ArrayOp::Retain(len) => {
					cursor += *len as usize;
					anchor = self.anchor_at(&children, cursor)?;
				}
				ArrayOp::Remove(len) => {
					let end = cursor + *len as usize;
					let removed = children.get(cursor..end).ok_or(CollabError::Diverged)?;
					plan.removed.extend_from_slice(removed);
					cursor = end;
				}
				ArrayOp::Insert(items) => {
					anchor = self.plan_inserts(parent, items, anchor, plan);
				}
			}
		}

		Ok(())
	}

	/// The anchor after the first `count` of `children`.
	fn anchor_at(&self, children: &[BlockId], count: usize) -> Result<Anchor, CollabError> {
		let last = count
			.checked_sub(1)
			.and_then(|index| children.get(index))
			.and_then(|&block| self.registry.block(block))
			.ok_or(CollabError::Diverged)?;

		Ok(Anchor::After(last.map))
	}

	/// Plan the inserted `items`, the first following `anchor`, and return
	/// the anchor after them. Items that are not block maps stay unmirrored.
	fn plan_inserts(
		&mut self,
		parent: Option<BlockId>,
		items: &[Option<ID>],
		mut anchor: Anchor,
		plan: &mut Plan,
	) -> Anchor {
		for item in items {
			let Some(map) = *item else {
				self.registry.adjust_unmirrored(parent, 1);
				continue;
			};

			plan.inserts.push(Insert {
				parent,
				map,
				anchor,
			});
			anchor = Anchor::After(map);
		}

		anchor
	}

	/// Plan by walking the final array, for arrays holding hidden items that
	/// the delta's positions count but the model does not.
	fn plan_walk(
		&mut self,
		parent: Option<BlockId>,
		(inserted, removed): (&FxHashSet<ID>, &FxHashSet<ID>),
		plan: &mut Plan,
	) -> Result<(), CollabError> {
		let array = self.array(parent)?;
		let mut kept = FxHashSet::default();
		let mut anchor = Anchor::Start;
		let mut unmirrored = 0;

		for item in array.iter(self.txn) {
			match self.classify(&item, inserted) {
				Item::Inserted(map) => {
					plan.inserts.push(Insert {
						parent,
						map,
						anchor,
					});
					anchor = Anchor::After(map);
				}
				Item::Shown(block, map) => {
					kept.insert(block);
					anchor = Anchor::After(map);
				}
				Item::Unmirrored => unmirrored += 1,
			}
		}

		for map in removed {
			self.registry.forget_hidden(map);
		}

		self.registry.set_unmirrored(parent, unmirrored);
		let children = self.tx.doc().children(parent);
		plan.removed
			.extend(children.iter().filter(|block| !kept.contains(block)));
		Ok(())
	}

	/// What an item of a walked array is to the plan.
	fn classify(&self, item: &Out, inserted: &FxHashSet<ID>) -> Item {
		let Some(map) = (match item {
			Out::YMap(map) => branch_id(map),
			_ => None,
		}) else {
			return Item::Unmirrored;
		};

		if inserted.contains(&map) {
			return Item::Inserted(map);
		}

		self.registry
			.block_of_map(&map)
			.map_or(Item::Unmirrored, |block| Item::Shown(block, map))
	}

	/// Apply the plan: moves, then removals, then insertions.
	pub(super) fn execute(&mut self, plan: Plan) -> Result<(), CollabError> {
		let Plan { removed, inserts } = plan;
		let mut doomed = FxHashSet::default();

		for &block in &removed {
			doomed.extend(descendants(self.tx.doc(), block));
		}

		let mut anchors: FxHashMap<ID, Anchor> = inserts
			.iter()
			.map(|insert| (insert.map, insert.anchor))
			.collect();
		let (reads, evicted) = self.read_inserts(inserts, &doomed);
		let rest = self.apply_moves(reads, &mut doomed, &anchors)?;

		for block in removed {
			if doomed.contains(&block) && self.tx.doc().contains(block) {
				self.remove_block(block)?;
			}
		}

		for block in evicted {
			if let Some((map, anchor)) = self.anchor_of(block) {
				anchors.insert(map, anchor);
				self.evict(block)?;
			}
		}

		let mut batch = Batch::default();

		for read in rest {
			self.insert(&mut batch, read, &anchors)?;
		}

		self.flush(&mut batch)
	}

	/// Apply the reads that re-insert a removed block unchanged as moves,
	/// sparing them from removal, and return the others.
	fn apply_moves(
		&mut self,
		reads: Vec<Read>,
		doomed: &mut FxHashSet<BlockId>,
		anchors: &FxHashMap<ID, Anchor>,
	) -> Result<Vec<Read>, CollabError> {
		let mut rest = Vec::with_capacity(reads.len());

		for read in reads {
			let Some(tree) = read
				.tree
				.as_ref()
				.filter(|tree| self.unchanged(tree, doomed))
			else {
				rest.push(read);
				continue;
			};

			let moved = subtree_ids(std::slice::from_ref(tree));
			doomed.retain(|id| !moved.contains(id));
			self.move_block(read, anchors)?;
		}

		Ok(rest)
	}

	/// Read every inserted map whose parent survives.
	fn read_inserts(
		&self,
		inserts: Vec<Insert>,
		doomed: &FxHashSet<BlockId>,
	) -> (Vec<Read>, Vec<BlockId>) {
		let mut incoming = Owners::default();

		for insert in &inserts {
			if let Some(map) = map_ref(self.txn, &insert.map) {
				incoming.scan(&map, self.txn);
			}
		}

		let doc = self.tx.doc();
		let mut claims = RemoteClaims::new(doc, self.registry, (&incoming, doomed));
		let mut reads = Vec::new();

		for insert in inserts {
			let survives = insert
				.parent
				.is_none_or(|parent| doc.contains(parent) && !doomed.contains(&parent));
			let Some(map) = map_ref(self.txn, &insert.map).filter(|_| survives) else {
				continue;
			};
			claims.target = insert.parent;
			let parent = Parent {
				id: insert.parent,
				kind: self.parent_kind(insert.parent),
			};
			let mut registration = Registration::default();
			let mut reader = Reader {
				txn: self.txn,
				schema: self.schema,
				claims: &mut claims,
			};

			let tree = reader.block(&map, parent, &mut registration);
			reads.push(Read {
				insert,
				tree,
				registration,
			});
		}

		(reads, claims.evicted)
	}

	/// Whether `tree` re-inserts a removed block unchanged.
	fn unchanged(&self, tree: &BlockTree, doomed: &FxHashSet<BlockId>) -> bool {
		doomed.contains(&tree.id) && self.tx.doc().tree(tree.id).as_ref() == Some(tree)
	}

	fn move_block(
		&mut self,
		read: Read,
		anchors: &FxHashMap<ID, Anchor>,
	) -> Result<(), CollabError> {
		let Some(tree) = &read.tree else {
			return Ok(());
		};
		let doc = self.tx.doc();
		let parent = doc
			.block(tree.id)
			.ok_or(CollabError::NotShared(tree.id))?
			.parent();
		let index = doc
			.index_in_parent(tree.id)
			.ok_or(CollabError::NotShared(tree.id))?;
		let to_parent = read.insert.parent;
		let mut to_index = self.resolve(to_parent, read.insert.anchor, anchors);

		if parent == to_parent && index < to_index {
			to_index -= 1;
		}

		self.tx.step(Step::MoveBlocks {
			parent,
			index,
			count: 1,
			to_parent,
			to_index,
		})?;
		self.registry
			.remove(&subtree_ids(std::slice::from_ref(tree)));
		self.repair_losers(&read.registration);
		self.registry.register(read.registration);
		Ok(())
	}

	/// Place one read map, batching consecutive insertions.
	fn insert(
		&mut self,
		batch: &mut Batch,
		read: Read,
		anchors: &FxHashMap<ID, Anchor>,
	) -> Result<(), CollabError> {
		let Read {
			insert,
			tree,
			registration,
		} = read;
		self.repairs.extend(
			registration
				.losers
				.iter()
				.map(|&(map, parent)| Repair { parent, map }),
		);

		let Some(tree) = tree else {
			self.registry.insert_hidden(insert.map, insert.parent);

			return Ok(());
		};

		let continues = batch.parent == insert.parent
			&& batch.last.is_some()
			&& insert.anchor == batch.last.map_or(Anchor::Start, Anchor::After);

		if !continues {
			self.flush(batch)?;
			batch.parent = insert.parent;
			batch.index = self.resolve(insert.parent, insert.anchor, anchors);
		}

		batch.last = Some(insert.map);
		batch.trees.push(tree);
		batch.registrations.push(registration);
		Ok(())
	}

	fn flush(&mut self, batch: &mut Batch) -> Result<(), CollabError> {
		if batch.trees.is_empty() {
			return Ok(());
		}

		self.tx.step(Step::InsertBlocks {
			parent: batch.parent,
			index: batch.index,
			blocks: std::mem::take(&mut batch.trees),
		})?;

		for registration in batch.registrations.drain(..) {
			self.registry.register(registration);
		}

		batch.last = None;
		Ok(())
	}

	/// The model index just after the nearest placed item `anchor` leads to.
	fn resolve(
		&self,
		parent: Option<BlockId>,
		mut anchor: Anchor,
		anchors: &FxHashMap<ID, Anchor>,
	) -> usize {
		let doc = self.tx.doc();

		for _ in 0..=anchors.len() {
			let Anchor::After(map) = anchor else {
				return 0;
			};
			let placed = self
				.registry
				.block_of_map(&map)
				.filter(|&block| {
					doc.block(block)
						.is_some_and(|found| found.parent() == parent)
				})
				.and_then(|block| doc.index_in_parent(block));

			if let Some(index) = placed {
				return index + 1;
			}

			match anchors.get(&map) {
				Some(next) => anchor = *next,
				None => return 0,
			}
		}

		0
	}

	/// The map of `block` and the anchor its position corresponds to.
	fn anchor_of(&self, block: BlockId) -> Option<(ID, Anchor)> {
		let doc = self.tx.doc();
		let map = self.registry.block(block)?.map;
		let index = doc.index_in_parent(block)?;
		let parent = doc.block(block)?.parent();
		let anchor = index
			.checked_sub(1)
			.and_then(|previous| doc.children(parent).get(previous))
			.and_then(|&previous| self.registry.block(previous))
			.map_or(Anchor::Start, |previous| Anchor::After(previous.map));
		Some((map, anchor))
	}
}
