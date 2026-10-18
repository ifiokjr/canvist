//! Faithful copies of block maps, for moving blocks.
//!
//! Yjs has no move, so the binding moves a block by deleting its map and
//! writing a copy. The copy keeps every key and value, including ones the
//! model does not understand, so moving a block never loses data that a
//! newer schema or another app stored in it.

use std::sync::Arc;

use canvist_model::BlockId;
use yrs::Any;
use yrs::Array;
use yrs::ArrayPrelim;
use yrs::ArrayRef;
use yrs::ID;
use yrs::In;
use yrs::Map;
use yrs::MapPrelim;
use yrs::MapRef;
use yrs::Out;
use yrs::ReadTxn;
use yrs::Text;
use yrs::TextPrelim;
use yrs::TextRef;
use yrs::TransactionMut;
use yrs::types::Delta;
use yrs::types::ToJson;

use crate::CollabError;
use crate::keys;
use crate::registry::BlockRef;
use crate::registry::branch_id;
use crate::text::Chunks;
use crate::tree::nested_id;

/// An owned copy of a block map and everything inside it.
#[derive(Debug)]
pub(crate) struct Snapshot {
	/// The copied map's item id.
	source: ID,
	entries: Vec<(Arc<str>, Entry)>,
}

/// One copied map entry.
#[derive(Debug)]
enum Entry {
	Any(Any),
	Text(Vec<Delta<In>>),
	Map(Vec<(Arc<str>, Any)>),
	Array(Vec<Element>),
}

/// One copied array element.
#[derive(Debug)]
enum Element {
	Block(Snapshot),
	Any(Any),
}

/// Each copied block map paired with its copy, so registrations can follow.
pub(crate) type Copies = Vec<(ID, BlockRef)>;

/// Where a copy goes: an array, an index in it, and the block that owns the
/// array (`None` for the root).
pub(crate) type Target<'a> = (&'a ArrayRef, u32, Option<BlockId>);

impl Snapshot {
	/// Copy `map` and its contents.
	pub(crate) fn read<T: ReadTxn>(map: &MapRef, txn: &T) -> Option<Self> {
		let entries = map
			.iter(txn)
			.map(|(key, value)| (Arc::from(key), Entry::read(value, txn)))
			.collect();

		Some(Self {
			source: branch_id(map)?,
			entries,
		})
	}

	/// Write the copy at `target`, recording which new map copies which old
	/// one.
	pub(crate) fn write(
		&self,
		txn: &mut TransactionMut<'_>,
		(array, index, parent): Target<'_>,
		copies: &mut Copies,
	) -> Result<(), CollabError> {
		let map: MapRef = array.insert(txn, index, MapPrelim::default());
		let mut block = BlockRef {
			map: nested_id(&map)?,
			parent,
			text: None,
			attrs: None,
			children: None,
		};
		let id = self.id();

		for (key, entry) in &self.entries {
			let branch = entry.write(txn, (&map, key.clone()), id, copies)?;

			match &**key {
				keys::TEXT => block.text = branch,
				keys::ATTRS => block.attrs = branch,
				keys::CHILDREN => block.children = branch,
				_ => {}
			}
		}

		copies.push((self.source, block));
		Ok(())
	}

	/// The block id stored in the copy, if it parses.
	fn id(&self) -> Option<BlockId> {
		let (_, entry) = self.entries.iter().find(|(key, _)| **key == *keys::ID)?;
		let Entry::Any(Any::String(id)) = entry else {
			return None;
		};

		id.parse().ok()
	}
}

impl Entry {
	fn read<T: ReadTxn>(value: Out, txn: &T) -> Self {
		match value {
			Out::Any(any) => Self::Any(any),
			Out::YText(text) => Self::Text(Chunks::read(&text, txn).delta_from(txn, 0)),
			Out::YMap(map) => {
				let entries = map
					.iter(txn)
					.map(|(key, value)| (Arc::from(key), value.to_json(txn)))
					.collect();
				Self::Map(entries)
			}
			Out::YArray(array) => {
				let elements = array
					.iter(txn)
					.map(|value| Element::read(value, txn))
					.collect();
				Self::Array(elements)
			}
			other => Self::Any(other.to_json(txn)),
		}
	}

	/// Write this entry under `key` of `map` and return the item id of the
	/// shared type it created, if any. `block` owns any child array.
	fn write(
		&self,
		txn: &mut TransactionMut<'_>,
		(map, key): (&MapRef, Arc<str>),
		block: Option<BlockId>,
		copies: &mut Copies,
	) -> Result<Option<ID>, CollabError> {
		let branch = match self {
			Self::Any(any) => {
				map.insert(txn, key, any.clone());
				None
			}
			Self::Text(delta) => {
				let text: TextRef = map.insert(txn, key, TextPrelim::new(""));
				text.apply_delta(txn, delta.iter().cloned());
				branch_id(&text)
			}
			Self::Map(entries) => {
				let prelim: MapPrelim = entries
					.iter()
					.map(|(key, any)| (key.clone(), In::Any(any.clone())))
					.collect();
				let inner: MapRef = map.insert(txn, key, prelim);
				branch_id(&inner)
			}
			Self::Array(elements) => {
				let inner: ArrayRef = map.insert(txn, key, ArrayPrelim::default());
				write_elements(txn, (&inner, block), elements, copies)?;
				branch_id(&inner)
			}
		};

		Ok(branch)
	}
}

impl Element {
	fn read<T: ReadTxn>(value: Out, txn: &T) -> Self {
		let Out::YMap(map) = value else {
			return Self::Any(value.to_json(txn));
		};

		Snapshot::read(&map, txn).map_or(Self::Any(Any::Null), Self::Block)
	}
}

fn write_elements(
	txn: &mut TransactionMut<'_>,
	(array, parent): (&ArrayRef, Option<BlockId>),
	elements: &[Element],
	copies: &mut Copies,
) -> Result<(), CollabError> {
	for (offset, element) in (0_u32..).zip(elements) {
		match element {
			Element::Block(snapshot) => snapshot.write(txn, (array, offset, parent), copies)?,
			Element::Any(any) => {
				array.insert(txn, offset, any.clone());
			}
		}
	}

	Ok(())
}
