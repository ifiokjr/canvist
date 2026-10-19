//! Deciding which block map owns a block id.
//!
//! Yjs has no move, so moving a block deletes its map and writes a copy
//! with the same id. Two peers moving the same block concurrently leave two
//! live copies. Every peer resolves this the same way: among the copies the
//! model can show, the one whose `Y.Map` has the smallest yrs item id keeps
//! the block, and the others are deleted by whichever peer notices first
//! (deleting is idempotent, so peers that notice together agree). A hidden
//! copy, such as one inside a container whose type no longer allows it,
//! claims nothing until it becomes valid. Edits made concurrently inside a
//! losing copy are lost, like edits inside the tail of a concurrently split
//! block.

use canvist_model::BlockId;
use canvist_model::BlockTypeId;
use canvist_model::Schema;
use rustc_hash::FxHashMap;
use yrs::Array;
use yrs::ArrayRef;
use yrs::ID;
use yrs::MapRef;
use yrs::Out;
use yrs::ReadTxn;

use crate::registry::branch_id;
use crate::tree;
use crate::tree::Shape;

/// Decides whether a block map may show as the block its id names.
pub(crate) trait Claim {
	/// Whether `map`, which stores `id`, owns that block.
	fn claim(&mut self, id: BlockId, map: &ID) -> bool;
}

/// The smallest map item id carrying each block id, for a set of maps read
/// together.
///
/// Only maps the model could show compete, the same maps
/// [`Reader`](crate::tree::Reader) asks to claim an id. A hidden copy, such
/// as one inside a container whose type no longer allows it, must not win
/// over a shown one: it would hide the block and get the shown copy deleted.
#[derive(Debug, Default)]
pub(crate) struct Owners(FxHashMap<BlockId, ID>);

impl Owners {
	/// Scan `map`, a child of a parent of type `parent` (`None` for the
	/// root), and every block map nested inside it.
	pub(crate) fn scan<T: ReadTxn>(
		&mut self,
		map: &MapRef,
		parent: Option<BlockTypeId>,
		txn: &T,
		schema: &Schema,
	) {
		let Some((id, kind, shape)) = tree::header(map, parent, txn, schema) else {
			return;
		};

		if let Some(item) = branch_id(map) {
			self.0
				.entry(id)
				.and_modify(|best| *best = (*best).min(item))
				.or_insert(item);
		}

		if let Shape::Children(children) = shape {
			self.scan_array(&children, Some(kind), txn, schema);
		}
	}

	/// Scan every block map in `array`, the children of a parent of type
	/// `parent`.
	pub(crate) fn scan_array<T: ReadTxn>(
		&mut self,
		array: &ArrayRef,
		parent: Option<BlockTypeId>,
		txn: &T,
		schema: &Schema,
	) {
		for item in array.iter(txn) {
			if let Out::YMap(map) = item {
				self.scan(&map, parent, txn, schema);
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
