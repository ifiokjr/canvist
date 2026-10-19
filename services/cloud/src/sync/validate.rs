//! Checks on incoming Yjs updates, before they touch a document.
//!
//! The relay does not run the canvist model on the hot path; every canvist
//! client already hides blocks that break the schema without deleting them
//! (see the `canvist_collab` crate docs). What the relay enforces is what
//! keeps documents loadable and bounded for everyone:
//!
//! - the update is a well-formed v1 update;
//! - it is no larger than the message limit;
//! - it only writes to the canvist root, the `Y.Array` named `blocks`;
//! - each top-level entry of `blocks` is a shared type (a block `Y.Map`), not
//!   a bare value.
//!
//! The scan walks the update's structs with yrs's own decoder, so it reads
//! exactly what yrs would integrate.

use canvist_collab::keys;
use yrs::block::BLOCK_GC_REF_NUMBER;
use yrs::block::BLOCK_ITEM_DELETED_REF_NUMBER;
use yrs::block::BLOCK_ITEM_TYPE_REF_NUMBER;
use yrs::block::BLOCK_SKIP_REF_NUMBER;
use yrs::block::HAS_ORIGIN;
use yrs::block::HAS_PARENT_SUB;
use yrs::block::HAS_RIGHT_ORIGIN;
use yrs::block::ItemContent;
use yrs::encoding::read::Cursor;
use yrs::encoding::read::Read;
use yrs::updates::decoder::Decoder;
use yrs::updates::decoder::DecoderV1;

/// Why an update was refused.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Rejected {
	/// The update is larger than the message limit.
	#[error("update of {0} bytes exceeds the limit")]
	TooLarge(usize),
	/// The bytes are not a v1 update.
	#[error("malformed update")]
	Malformed,
	/// The update writes a root type other than `blocks`.
	#[error("update writes to root `{0}`; only `blocks` is allowed")]
	ForeignRoot(String),
	/// The update puts a bare value, not a block map, into `blocks`.
	#[error("top-level entries of `blocks` must be block maps")]
	NotABlock,
	/// The client may only read this document.
	#[error("read-only session")]
	ReadOnly,
	/// The document would grow past its size limit.
	#[error("document size limit reached")]
	DocumentFull,
}

/// Check `update` against the rules above.
pub fn check(update: &[u8], max_bytes: usize) -> Result<(), Rejected> {
	if update.len() > max_bytes {
		return Err(Rejected::TooLarge(update.len()));
	}

	scan(update).map_err(|error| {
		match error {
			ScanError::Rejected(rejected) => rejected,
			ScanError::Decode => Rejected::Malformed,
		}
	})
}

enum ScanError {
	Rejected(Rejected),
	Decode,
}

impl From<yrs::encoding::read::Error> for ScanError {
	fn from(_: yrs::encoding::read::Error) -> Self {
		Self::Decode
	}
}

fn scan(update: &[u8]) -> Result<(), ScanError> {
	let mut decoder = DecoderV1::new(Cursor::new(update));
	let clients: u32 = decoder.read_var()?;

	for _ in 0..clients {
		let structs: u32 = decoder.read_var()?;
		decoder.read_client()?;
		let _clock: u32 = decoder.read_var()?;

		for _ in 0..structs {
			scan_struct(&mut decoder)?;
		}
	}

	// The delete set follows; yrs validates it when decoding.
	Ok(())
}

fn scan_struct(decoder: &mut DecoderV1<'_>) -> Result<(), ScanError> {
	let info = decoder.read_info()?;

	match info {
		BLOCK_SKIP_REF_NUMBER => {
			let _length: u32 = decoder.read_var()?;
			return Ok(());
		}
		BLOCK_GC_REF_NUMBER => {
			decoder.read_len()?;
			return Ok(());
		}
		_ => {}
	}

	if info & HAS_ORIGIN != 0 {
		decoder.read_left_id()?;
	}

	if info & HAS_RIGHT_ORIGIN != 0 {
		decoder.read_right_id()?;
	}

	// Items with neither origin carry their parent explicitly; the others
	// sit next to an existing item and inherit its parent.
	let explicit_parent = info & (HAS_ORIGIN | HAS_RIGHT_ORIGIN) == 0;
	let mut in_root = false;

	if explicit_parent {
		if decoder.read_parent_info()? {
			let root = decoder.read_string()?;

			if root != keys::ROOT {
				return Err(ScanError::Rejected(Rejected::ForeignRoot(root.to_owned())));
			}

			in_root = true;
		} else {
			decoder.read_left_id()?;
		}

		if info & HAS_PARENT_SUB != 0 {
			decoder.read_string()?;
		}
	}

	ItemContent::decode(decoder, info).map_err(|_| ScanError::Decode)?;

	if in_root && !is_block_content(info) {
		return Err(ScanError::Rejected(Rejected::NotABlock));
	}

	Ok(())
}

/// Whether content of kind `info` can be a block in `blocks`: a shared type,
/// or content that was deleted before it arrived.
fn is_block_content(info: u8) -> bool {
	matches!(
		info & 0b1111,
		BLOCK_ITEM_TYPE_REF_NUMBER | BLOCK_ITEM_DELETED_REF_NUMBER
	)
}

#[cfg(test)]
mod tests {
	use yrs::Array;
	use yrs::Doc;
	use yrs::Map;
	use yrs::MapPrelim;
	use yrs::Text;
	use yrs::Transact;

	use super::*;

	/// The update a client writes when it makes `change`. Root types are
	/// fetched before the transaction opens, as yrs requires.
	fn edit<R>(
		root: impl FnOnce(&Doc) -> R,
		change: impl FnOnce(&R, &mut yrs::TransactionMut<'_>),
	) -> Vec<u8> {
		let doc = Doc::with_client_id(7);
		let root = root(&doc);
		let mut txn = doc.transact_mut();
		change(&root, &mut txn);
		txn.encode_update_v1()
	}

	#[test]
	fn canvist_blocks_pass() {
		let update = edit(
			|doc| doc.get_or_insert_array(keys::ROOT),
			|blocks, txn| {
				let block = blocks.push_back(txn, MapPrelim::default());
				block.insert(txn, keys::TYPE, "paragraph");
				let text = block.insert(txn, keys::TEXT, yrs::TextPrelim::new("Hello"));
				text.insert(txn, 5, ", world");
			},
		);
		assert_eq!(check(&update, 1 << 20), Ok(()));
	}

	#[test]
	fn other_roots_are_refused() {
		let update = edit(
			|doc| doc.get_or_insert_text("prosemirror"),
			|text, txn| {
				text.insert(txn, 0, "hi");
			},
		);
		assert_eq!(
			check(&update, 1 << 20),
			Err(Rejected::ForeignRoot("prosemirror".to_owned()))
		);
	}

	#[test]
	fn bare_values_in_blocks_are_refused() {
		let update = edit(
			|doc| doc.get_or_insert_array(keys::ROOT),
			|blocks, txn| {
				blocks.push_back(txn, 42);
			},
		);
		assert_eq!(check(&update, 1 << 20), Err(Rejected::NotABlock));
	}

	#[test]
	fn garbage_and_oversized_updates_are_refused() {
		assert_eq!(
			check(&[0xff, 0xff, 0xff], 1 << 20),
			Err(Rejected::Malformed)
		);
		assert_eq!(check(&[0; 64], 16), Err(Rejected::TooLarge(64)));
		assert_eq!(check(yrs::Update::EMPTY_V1, 16), Ok(()));
	}
}
