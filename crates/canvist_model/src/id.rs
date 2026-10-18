//! Stable block identifiers.

use std::fmt;
use std::str::FromStr;

/// A globally unique, stable identifier for a block.
///
/// Block ids never change for the lifetime of a block, which lets layout
/// caches, collaboration bindings, and drag-and-drop refer to blocks without
/// positional bookkeeping. An id packs the creating client (high 32 bits) and
/// that client's sequence number (low 32 bits), so concurrent peers never
/// allocate the same id as long as their client ids differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(u64);

impl BlockId {
	/// Build an id from a client id and a client-local sequence number.
	#[must_use]
	pub const fn new(client: u32, seq: u32) -> Self {
		Self(((client as u64) << 32) | seq as u64)
	}

	/// The client that allocated this id.
	#[must_use]
	pub const fn client(self) -> u32 {
		(self.0 >> 32) as u32
	}

	/// The client-local sequence number.
	#[must_use]
	pub const fn seq(self) -> u32 {
		self.0 as u32
	}

	/// The packed 64-bit representation.
	#[must_use]
	pub const fn as_u64(self) -> u64 {
		self.0
	}

	/// Rebuild an id from its packed representation.
	#[must_use]
	pub const fn from_u64(raw: u64) -> Self {
		Self(raw)
	}
}

/// Formats as `client.seq` in base 36, e.g. `k3f9a.1c`.
impl fmt::Display for BlockId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}.{}", base36(self.client()), base36(self.seq()))
	}
}

/// The error returned when parsing a malformed [`BlockId`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid block id `{0}`: expected `client.seq` in base 36")]
pub struct ParseBlockIdError(String);

impl FromStr for BlockId {
	type Err = ParseBlockIdError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let invalid = || ParseBlockIdError(s.to_owned());
		let (client, seq) = s.split_once('.').ok_or_else(invalid)?;
		let client = u32::from_str_radix(client, 36).map_err(|_| invalid())?;
		let seq = u32::from_str_radix(seq, 36).map_err(|_| invalid())?;

		Ok(Self::new(client, seq))
	}
}

fn base36(mut value: u32) -> String {
	const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

	if value == 0 {
		return "0".to_owned();
	}

	let mut out = Vec::with_capacity(7);

	while value > 0 {
		out.push(DIGITS[(value % 36) as usize]);
		value /= 36;
	}

	out.reverse();

	String::from_utf8(out).unwrap_or_default()
}

/// Allocates fresh [`BlockId`]s for one client.
///
/// When a client exhausts its 2³² sequence space it must be given a new
/// client id; [`IdGenerator::next_id`] returns `None` instead of wrapping so
/// ids can never be reused silently.
#[derive(Clone, Debug)]
pub struct IdGenerator {
	client: u32,
	/// The next free sequence number, or `None` once all are used.
	next: Option<u32>,
}

impl IdGenerator {
	/// Create a generator for `client`, starting at sequence zero.
	#[must_use]
	pub const fn new(client: u32) -> Self {
		Self {
			client,
			next: Some(0),
		}
	}

	/// The client id this generator allocates for.
	#[must_use]
	pub const fn client(&self) -> u32 {
		self.client
	}

	/// Allocate the next id, or `None` once the sequence space is exhausted.
	pub fn next_id(&mut self) -> Option<BlockId> {
		let seq = self.next?;
		self.next = seq.checked_add(1);

		Some(BlockId::new(self.client, seq))
	}

	/// Ensure future ids from this generator never collide with `id`.
	///
	/// Called when blocks created elsewhere (deserialised, pasted, or synced)
	/// enter a document that shares this client id.
	pub fn observe(&mut self, id: BlockId) {
		let Some(next) = self.next else {
			return;
		};

		if id.client() == self.client && id.seq() >= next {
			self.next = id.seq().checked_add(1);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn packs_client_and_sequence() {
		let id = BlockId::new(7, 42);

		assert_eq!(id.client(), 7);
		assert_eq!(id.seq(), 42);
		assert_eq!(BlockId::from_u64(id.as_u64()), id);
	}

	#[test]
	fn display_round_trips_through_from_str() {
		let id = BlockId::new(123_456, 0);

		assert_eq!(id.to_string(), "2n9c.0");
		assert_eq!("2n9c.0".parse::<BlockId>(), Ok(id));
		assert!("nope".parse::<BlockId>().is_err());
		assert!("1.zzzzzzzzzz".parse::<BlockId>().is_err());
	}

	#[test]
	fn generator_skips_observed_ids() {
		let mut ids = IdGenerator::new(1);
		ids.observe(BlockId::new(1, 9));
		ids.observe(BlockId::new(2, 50));

		assert_eq!(ids.next_id(), Some(BlockId::new(1, 10)));
	}

	#[test]
	fn generator_refuses_to_wrap() {
		let mut ids = IdGenerator::new(1);
		ids.observe(BlockId::new(1, u32::MAX - 1));

		assert_eq!(ids.next_id(), Some(BlockId::new(1, u32::MAX)));
		assert_eq!(ids.next_id(), None);
	}
}
