//! Positions that mean the same thing in every language.

use canvist_model::BlockId;
use canvist_model::Document;
use canvist_model::Point;

use crate::HeadlessError;

/// A point in a text block, addressed the same way from Rust, TypeScript,
/// Dart, and the conformance fixtures.
///
/// - `path` walks the block tree from the root: `[2]` is the third top-level
///   block, and `[1, 0, 0]` is the first block inside the first child of the
///   second top-level block (a list's first item's paragraph).
/// - `offset` counts UTF-16 code units, the unit of JavaScript and Dart
///   strings, from the start of the block's text. Inline embeds count as one.
///
/// The model itself uses [`Point`]s, which name blocks by id and count UTF-8
/// bytes. [`Doc::point`](crate::Doc::point) and
/// [`Doc::position`](crate::Doc::position) convert between the two.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Position {
	/// Child indexes from the root to the text block.
	pub path: Vec<u32>,
	/// UTF-16 code units into the block's text.
	pub offset: u32,
}

impl Position {
	/// A position `offset` UTF-16 code units into the block at `path`.
	#[must_use]
	pub fn new(path: impl Into<Vec<u32>>, offset: u32) -> Self {
		Self {
			path: path.into(),
			offset,
		}
	}
}

/// The text block `position` names, and the point in it.
pub(crate) fn resolve(doc: &Document, position: &Position) -> Result<Point, HeadlessError> {
	let block = block_at(doc, &position.path)?;
	let text = doc.inline(block).ok_or_else(|| {
		HeadlessError::InvalidPosition(format!("{:?} is not a text block", position.path))
	})?;
	let offset = utf16_to_utf8(text.as_str(), position.offset).ok_or_else(|| {
		HeadlessError::InvalidPosition(format!(
			"offset {} does not fit the text of {:?}",
			position.offset, position.path
		))
	})?;

	Ok(Point::new(block, offset))
}

/// The position of `point`, or `None` when its block is not in `doc` or its
/// offset is not on a character boundary.
pub(crate) fn locate(doc: &Document, point: Point) -> Option<Position> {
	let text = doc.inline(point.block)?;
	let before = text.as_str().get(..point.offset as usize)?;

	Some(Position {
		path: path_of(doc, point.block)?,
		offset: before.encode_utf16().count() as u32,
	})
}

fn block_at(doc: &Document, path: &[u32]) -> Result<BlockId, HeadlessError> {
	let missing = || HeadlessError::InvalidPosition(format!("no block at {path:?}"));
	let mut parent = None;

	for &index in path {
		let child = doc
			.children(parent)
			.get(index as usize)
			.ok_or_else(missing)?;
		parent = Some(*child);
	}

	parent.ok_or_else(missing)
}

fn path_of(doc: &Document, block: BlockId) -> Option<Vec<u32>> {
	let mut path = Vec::new();
	let mut current = Some(block);

	while let Some(id) = current {
		path.push(doc.index_in_parent(id)? as u32);
		current = doc.block(id)?.parent();
	}

	path.reverse();
	Some(path)
}

/// The UTF-8 offset in `text` of `units` UTF-16 code units, or `None` when
/// it is past the end or splits a surrogate pair.
fn utf16_to_utf8(text: &str, units: u32) -> Option<u32> {
	let mut counted = 0;

	for (byte, character) in text.char_indices() {
		if counted == units {
			return Some(byte as u32);
		}

		counted += character.len_utf16() as u32;
	}

	(counted == units).then_some(text.len() as u32)
}

#[cfg(test)]
mod tests {
	use super::utf16_to_utf8;

	#[test]
	fn converts_utf16_offsets_to_utf8() {
		let text = "a😀é";

		assert_eq!(utf16_to_utf8(text, 0), Some(0));
		assert_eq!(utf16_to_utf8(text, 1), Some(1));
		assert_eq!(utf16_to_utf8(text, 2), None, "inside the surrogate pair");
		assert_eq!(utf16_to_utf8(text, 3), Some(5));
		assert_eq!(utf16_to_utf8(text, 4), Some(7));
		assert_eq!(utf16_to_utf8(text, 5), None, "past the end");
	}
}
