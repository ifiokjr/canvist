//! Positions, in the shape every binding already speaks.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as Json;

/// A caret or selection endpoint: child indexes from the root to a block,
/// and UTF-16 code units into that block's text.
///
/// The same shape as `canvist_headless::Position`, the FFI `DocPosition`,
/// and the page's `Position`, so tests write one address for every target.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
	/// Child indexes from the root to the text block.
	pub path: Vec<u32>,
	/// UTF-16 code units into the block's text.
	pub offset: u32,
}

impl Position {
	/// The position `path`/`offset` name.
	#[must_use]
	pub fn new(path: impl Into<Vec<u32>>, offset: u32) -> Self {
		Self {
			path: path.into(),
			offset,
		}
	}

	/// The start of the block `path` names.
	#[must_use]
	pub fn start(path: impl Into<Vec<u32>>) -> Self {
		Self::new(path, 0)
	}
}

impl From<canvist_headless::Position> for Position {
	fn from(position: canvist_headless::Position) -> Self {
		Self::new(position.path, position.offset)
	}
}

impl From<Position> for canvist_headless::Position {
	fn from(position: Position) -> Self {
		canvist_headless::Position::new(position.path, position.offset)
	}
}

/// The last caret offset of the block `path` names, read off a ProseMirror
/// JSON export: the end of its text, in UTF-16 code units.
///
/// Positions do not repair offsets past the end, so "the end of the
/// document" has to be computed; every target can export JSON, so every
/// target computes the same caret. Paths that name no block read as 0,
/// which the following select reports as out of bounds.
#[must_use]
pub fn text_end(json: &Json, path: &[u32]) -> u32 {
	let mut node = json;

	for index in path {
		let Some(children) = node["content"].as_array() else {
			return 0;
		};

		match children.get(*index as usize) {
			Some(child) => node = child,
			None => return 0,
		}
	}

	text_length(node)
}

/// The last caret of the document: the end of its last block that holds
/// text, read off a ProseMirror JSON export.
///
/// Blocks with no text — an empty trailing paragraph, an image — are
/// skipped, so the caret lands where typing continues the document.
#[must_use]
pub fn document_end(json: &Json) -> Position {
	let mut path = Vec::new();
	let mut node = json;

	while let Some(children) = node["content"].as_array() {
		// A block whose children are only inline text and breaks is a
		// text block: this is where the caret goes.
		let inline = children
			.iter()
			.all(|child| matches!(child["type"].as_str(), Some("text" | "hardBreak")));

		if inline && children.iter().any(|child| child["type"] == "text") {
			return Position::new(path, text_length(node));
		}

		let Some((index, child)) = children
			.iter()
			.enumerate()
			.rev()
			.find(|(_, child)| text_length(child) > 0)
		else {
			break;
		};

		path.push(index as u32);
		node = child;
	}

	Position::new(path, 0)
}

/// The UTF-16 length of a block's text: its text descendants summed.
fn text_length(node: &Json) -> u32 {
	if node["type"].as_str() == Some("text") {
		return node["text"]
			.as_str()
			.map_or(0, |text| text.encode_utf16().count() as u32);
	}

	node["content"]
		.as_array()
		.map_or(0, |children| children.iter().map(text_length).sum())
}

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::*;

	#[test]
	fn the_end_of_a_text_block_is_its_utf16_length() {
		let doc = json!({
			"type": "doc",
			"content": [
				{ "type": "paragraph", "content": [{ "type": "text", "text": "hello" }] },
				{ "type": "paragraph" },
			],
		});

		assert_eq!(text_end(&doc, &[0]), 5);
		assert_eq!(text_end(&doc, &[1]), 0);
	}

	#[test]
	fn astral_characters_count_two_utf16_units() {
		let doc = json!({
			"type": "doc",
			"content": [
				{ "type": "paragraph", "content": [{ "type": "text", "text": "👍x" }] },
			],
		});

		assert_eq!(text_end(&doc, &[0]), 3);
	}

	#[test]
	fn inline_content_between_texts_counts() {
		let doc = json!({
			"type": "doc",
			"content": [
				{ "type": "paragraph", "content": [
					{ "type": "text", "text": "a" },
					{ "type": "hardBreak" },
					{ "type": "text", "text": "b" },
				] },
			],
		});

		assert_eq!(text_end(&doc, &[0]), 2);
	}

	#[test]
	fn nested_paths_walk_children() {
		let doc = json!({
			"type": "doc",
			"content": [
				{ "type": "bulletList", "content": [
					{ "type": "listItem", "content": [
						{ "type": "paragraph", "content": [
							{ "type": "text", "text": "item" },
						] },
					] },
				] },
			],
		});

		assert_eq!(text_end(&doc, &[0, 0, 0]), 4);
	}

	#[test]
	fn the_document_ends_at_its_last_text_block() {
		let doc = json!({
			"type": "doc",
			"content": [
				{ "type": "heading", "content": [{ "type": "text", "text": "Top" }] },
				{ "type": "paragraph", "content": [{ "type": "text", "text": "Tail" }] },
			],
		});

		assert_eq!(document_end(&doc), Position::new([1], 4));
	}

	#[test]
	fn the_document_ends_inside_nested_lists() {
		let doc = json!({
			"type": "doc",
			"content": [
				{ "type": "bulletList", "content": [
					{ "type": "listItem", "content": [
						{ "type": "paragraph", "content": [{ "type": "text", "text": "deep" }] },
					] },
				] },
			],
		});

		assert_eq!(document_end(&doc), Position::new([0, 0, 0], 4));
	}

	#[test]
	fn empty_trailing_blocks_are_skipped() {
		let doc = json!({
			"type": "doc",
			"content": [
				{ "type": "paragraph", "content": [{ "type": "text", "text": "words" }] },
				{ "type": "paragraph" },
			],
		});

		assert_eq!(document_end(&doc), Position::new([0], 5));
	}

	#[test]
	fn a_textless_document_ends_at_the_root() {
		let doc = json!({ "type": "doc", "content": [{ "type": "paragraph" }] });

		assert_eq!(document_end(&doc), Position::new([], 0));
	}

	#[test]
	fn missing_paths_read_as_zero() {
		let doc = json!({ "type": "doc", "content": [{ "type": "paragraph" }] });

		assert_eq!(text_end(&doc, &[4]), 0);
		assert_eq!(text_end(&doc, &[]), 0);
	}
}
