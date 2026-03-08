//! Character-offset positions within a document.

use serde::Deserialize;
use serde::Serialize;

/// A position within a document, expressed as a character offset from the
/// start.
///
/// Positions are zero-indexed and refer to the gap *before* a character.
/// Position 0 is before the first character, position 1 is between the first
/// and second character, etc.
///
/// # Examples
///
/// ```
/// use canvist_core::Position;
///
/// let pos = Position::new(5);
/// assert_eq!(pos.offset(), 5);
/// assert_eq!(pos, Position::new(5));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Position {
	offset: usize,
}

impl Position {
	/// The position at the very start of the document (offset 0).
	#[must_use]
	pub const fn zero() -> Self {
		Self { offset: 0 }
	}

	/// Create a position at the given character offset.
	#[must_use]
	pub const fn new(offset: usize) -> Self {
		Self { offset }
	}

	/// Return the character offset.
	#[must_use]
	pub const fn offset(self) -> usize {
		self.offset
	}

	/// Move the position forward by `n` characters.
	#[must_use]
	pub const fn forward(self, n: usize) -> Self {
		Self {
			offset: self.offset + n,
		}
	}

	/// Move the position backward by `n` characters, clamping to zero.
	#[must_use]
	pub fn backward(self, n: usize) -> Self {
		Self {
			offset: self.offset.saturating_sub(n),
		}
	}
}

impl Default for Position {
	fn default() -> Self {
		Self::zero()
	}
}

impl std::fmt::Display for Position {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.offset)
	}
}

impl From<usize> for Position {
	fn from(offset: usize) -> Self {
		Self::new(offset)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn position_ordering() {
		assert!(Position::new(0) < Position::new(1));
		assert!(Position::new(5) == Position::new(5));
	}

	#[test]
	fn forward_backward() {
		let pos = Position::new(5);
		assert_eq!(pos.forward(3), Position::new(8));
		assert_eq!(pos.backward(3), Position::new(2));
		assert_eq!(pos.backward(10), Position::zero());
	}

	#[test]
	fn from_usize() {
		let pos: Position = 42.into();
		assert_eq!(pos.offset(), 42);
	}
}
