//! Stable node identifiers.

use std::fmt;
use std::str::FromStr;

use canvist_model::BlockId;
use canvist_model::IdGenerator;

/// A globally unique, stable identifier for a scene node.
///
/// Ids pack the creating client (high 32 bits) and that client's sequence
/// number (low 32 bits), exactly like [`BlockId`], so concurrent peers never
/// allocate the same id as long as their client ids differ. They format as
/// `client.seq` in base 36, e.g. `k3f9a.1c`.
///
/// [`NodeId::ROOT`] is the implicit parent of every page. Client id
/// `u32::MAX` is reserved for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u64);

impl NodeId {
	/// The implicit root: the parent of every page. Formats as `root`.
	pub const ROOT: Self = Self(u64::MAX);

	/// Build an id from a client id and a client-local sequence number.
	#[must_use]
	pub const fn new(client: u32, seq: u32) -> Self {
		Self(BlockId::new(client, seq).as_u64())
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

	/// Whether this is [`NodeId::ROOT`].
	#[must_use]
	pub const fn is_root(self) -> bool {
		self.0 == u64::MAX
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

impl fmt::Display for NodeId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.is_root() {
			return f.write_str("root");
		}

		BlockId::from_u64(self.0).fmt(f)
	}
}

/// The error returned when parsing a malformed [`NodeId`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid node id `{0}`: expected `root` or `client.seq` in base 36")]
pub struct ParseNodeIdError(String);

impl FromStr for NodeId {
	type Err = ParseNodeIdError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if s == "root" {
			return Ok(Self::ROOT);
		}

		let id = s
			.parse::<BlockId>()
			.map_err(|_| ParseNodeIdError(s.to_owned()))?;

		if id.client() == u32::MAX {
			return Err(ParseNodeIdError(s.to_owned()));
		}

		Ok(Self(id.as_u64()))
	}
}

/// Allocates fresh [`NodeId`]s for one client.
#[derive(Clone, Debug)]
pub struct NodeIdGenerator(IdGenerator);

impl NodeIdGenerator {
	/// A generator for `client`, starting at sequence zero.
	///
	/// # Panics
	///
	/// Panics when `client` is `u32::MAX`, which is reserved for
	/// [`NodeId::ROOT`].
	#[must_use]
	pub const fn new(client: u32) -> Self {
		assert!(client != u32::MAX, "client id u32::MAX is reserved");
		Self(IdGenerator::new(client))
	}

	/// The client id this generator allocates for.
	#[must_use]
	pub const fn client(&self) -> u32 {
		self.0.client()
	}

	/// Allocate the next id, or `None` once the sequence space is exhausted.
	pub fn next_id(&mut self) -> Option<NodeId> {
		self.0.next_id().map(|id| NodeId(id.as_u64()))
	}

	/// Ensure future ids never collide with `id`, for nodes created elsewhere.
	pub fn observe(&mut self, id: NodeId) {
		self.0.observe(BlockId::from_u64(id.as_u64()));
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn formats_and_parses() {
		let id = NodeId::new(123_456, 7);

		assert_eq!(id.to_string(), "2n9c.7");
		assert_eq!("2n9c.7".parse::<NodeId>(), Ok(id));
		assert_eq!(NodeId::ROOT.to_string(), "root");
		assert_eq!("root".parse::<NodeId>(), Ok(NodeId::ROOT));
		assert!("nope".parse::<NodeId>().is_err());
	}

	#[test]
	fn the_root_client_is_reserved() {
		let reserved = BlockId::new(u32::MAX, 3).to_string();

		assert!(reserved.parse::<NodeId>().is_err());
	}

	#[test]
	fn generator_skips_observed_ids() {
		let mut ids = NodeIdGenerator::new(4);
		ids.observe(NodeId::new(4, 10));

		assert_eq!(ids.next_id(), Some(NodeId::new(4, 11)));
	}
}
