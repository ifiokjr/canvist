//! Deciding which block map owns a block id.
//!
//! Yjs has no move, so moving a block deletes its map and writes a copy
//! with the same id. Two peers moving the same block concurrently leave two
//! live copies. Every peer resolves this the same way: the copy whose `Y.Map`
//! has the smallest yrs item id keeps the block, and the others are deleted
//! by whichever peer notices first (deleting is idempotent, so peers that
//! notice together agree). Edits made concurrently inside a losing copy are
//! lost, like edits inside the tail of a concurrently split block.

use canvist_model::BlockId;
use rustc_hash::FxHashMap;
use yrs::Array;
use yrs::ArrayRef;
use yrs::ID;
use yrs::MapRef;
use yrs::Out;
use yrs::ReadTxn;

use crate::keys;
use crate::registry::branch_id;
use crate::tree;

/// Decides whether a block map may show as the block its id names.
pub(crate) trait Claim {
	/// Whether `map`, which stores `id`, owns that block.
	fn claim(&mut self, id: BlockId, map: &ID) -> bool;
}

/// The smallest map item id carrying each block id, for a set of maps read
/// together.
#[derive(Debug, Default)]
pub(crate) struct Owners(FxHashMap<BlockId, ID>);

impl Owners {
	/// Scan `map` and every block map nested inside it.
	pub(crate) fn scan<T: ReadTxn>(&mut self, map: &MapRef, txn: &T) {
		let id = tree::string(map, txn, keys::ID).and_then(|id| id.parse::<BlockId>().ok());

		if let (Some(id), Some(item)) = (id, branch_id(map)) {
			self.0
				.entry(id)
				.and_modify(|best| *best = (*best).min(item))
				.or_insert(item);
		}

		if let Some(children) = tree::children_of(map, txn) {
			self.scan_array(&children, txn);
		}
	}

	/// Scan every block map in `array`.
	pub(crate) fn scan_array<T: ReadTxn>(&mut self, array: &ArrayRef, txn: &T) {
		for item in array.iter(txn) {
			if let Out::YMap(map) = item {
				self.scan(&map, txn);
			}
		}
	}

	/// The winning map for `id` among the scanned maps.
	pub(crate) fn get(&self, id: BlockId) -> Option<&ID> {
		self.0.get(&id)
	}
}

impl Claim for Owners {
	fn claim(&mut self, id: BlockId, map: &ID) -> bool {
		self.get(id) == Some(map)
	}
}
