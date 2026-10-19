//! Owned copies of yrs events.
//!
//! yrs reports changes from inside the transaction commit, where the model
//! is out of reach, so the observer copies each event into a [`Record`] and
//! the binding translates them once the transaction is done.

use std::sync::Arc;

use canvist_model::InlineText;
use canvist_model::Schema;
use rustc_hash::FxHashSet;
use yrs::ID;
use yrs::Out;
use yrs::TransactionMut;
use yrs::types::Change;
use yrs::types::Delta;
use yrs::types::Event;
use yrs::types::array::ArrayEvent;
use yrs::types::text::TextEvent;

use crate::registry::branch_id;
use crate::text;
use crate::values;
use crate::values::Format;

/// One shared type's changes in one transaction.
#[derive(Debug)]
pub(crate) enum Record {
	/// A `Y.Text` changed.
	Text {
		/// The text's item id.
		text: ID,
		/// What changed, in order.
		ops: Vec<TextOp>,
	},
	/// A `Y.Array` of block maps changed.
	Array {
		/// The array's item id, or `None` for the root array.
		array: Option<ID>,
		/// What changed, in order.
		ops: Vec<ArrayOp>,
		/// Item ids of block maps added in this transaction.
		inserted: FxHashSet<ID>,
		/// Item ids of items removed in this transaction.
		removed: FxHashSet<ID>,
	},
	/// Keys of a `Y.Map` changed.
	Map {
		/// The map's item id.
		map: ID,
		/// The changed keys.
		keys: Vec<Arc<str>>,
	},
}

/// One step of a `Y.Text` delta, in UTF-16 code units.
#[derive(Debug)]
pub(crate) enum TextOp {
	/// Skip unchanged content.
	Retain(u32),
	/// Change the formatting of existing content.
	Format(u32, Format),
	/// Remove content.
	Delete(u32),
	/// Insert content, already in the model's form.
	Insert(InlineText),
}

/// One step of a `Y.Array` delta.
#[derive(Debug)]
pub(crate) enum ArrayOp {
	/// Skip unchanged items.
	Retain(u32),
	/// Remove items.
	Remove(u32),
	/// Insert items: block map item ids, or `None` for anything else.
	Insert(Vec<Option<ID>>),
}

/// Copy one event, or `None` for event kinds canvist does not use.
///
/// `root` is the item id of the mirrored array when it is nested inside
/// another shared type; events on it are recorded as events on the root.
pub(crate) fn record(
	txn: &TransactionMut<'_>,
	event: &Event,
	schema: &Schema,
	root: Option<ID>,
) -> Option<Record> {
	match event {
		Event::Text(event) => text_record(txn, event, schema),
		Event::Array(event) => Some(array_record(txn, event, root)),
		Event::Map(event) => {
			Some(Record::Map {
				map: branch_id(event.target())?,
				keys: event.keys(txn).keys().cloned().collect(),
			})
		}
		_ => None,
	}
}

fn text_record(txn: &TransactionMut<'_>, event: &TextEvent, schema: &Schema) -> Option<Record> {
	let ops = event
		.delta(txn)
		.iter()
		.map(|delta| {
			match delta {
				Delta::Retain(len, None) => TextOp::Retain(*len),
				Delta::Retain(len, Some(format)) => TextOp::Format(*len, (**format).clone()),
				Delta::Deleted(len) => TextOp::Delete(*len),
				Delta::Inserted(insert, format) => {
					let marks = values::marks_from(schema, format.as_deref());
					TextOp::Insert(text::inline_from_out(schema, insert, marks))
				}
			}
		})
		.collect();

	Some(Record::Text {
		text: branch_id(event.target())?,
		ops,
	})
}

fn array_record(txn: &TransactionMut<'_>, event: &ArrayEvent, root: Option<ID>) -> Record {
	let ops = event
		.delta(txn)
		.iter()
		.map(|change| {
			match change {
				Change::Retain(len) => ArrayOp::Retain(*len),
				Change::Removed(len) => ArrayOp::Remove(*len),
				Change::Added(items) => ArrayOp::Insert(items.iter().map(map_id).collect()),
			}
		})
		.collect::<Vec<_>>();
	let inserted = ops
		.iter()
		.filter_map(|op| {
			match op {
				ArrayOp::Insert(maps) => Some(maps.iter().flatten().copied()),
				_ => None,
			}
		})
		.flatten()
		.collect();

	Record::Array {
		array: branch_id(event.target()).filter(|id| Some(*id) != root),
		ops,
		inserted,
		removed: event.removes(txn).iter().copied().collect(),
	}
}

/// The item id of a block map, or `None` for any other array item.
fn map_id(item: &Out) -> Option<ID> {
	match item {
		Out::YMap(map) => branch_id(map),
		_ => None,
	}
}
