//! Grapheme and word boundaries inside one block's text.
//!
//! The model counts UTF-8 bytes; users count characters as they see them.
//! These helpers convert between the two with Unicode segmentation (UAX #29),
//! so a caret never lands inside an emoji, a flag, or a combining sequence.
//! Inline embeds are a single [`EMBED_CHAR`] and act as one-character words,
//! which makes them atomic for every kind of motion.

use canvist_model::EMBED_CHAR;
use unicode_segmentation::UnicodeSegmentation;

/// The boundary after the grapheme cluster that starts at `offset`.
#[must_use]
pub fn next_grapheme(text: &str, offset: u32) -> Option<u32> {
	let rest = text.get(offset as usize..)?;
	let cluster = rest.graphemes(true).next()?;
	Some(offset + cluster.len() as u32)
}

/// The boundary before the grapheme cluster that ends at `offset`.
#[must_use]
pub fn prev_grapheme(text: &str, offset: u32) -> Option<u32> {
	let head = text.get(..offset as usize)?;
	let cluster = head.graphemes(true).next_back()?;
	Some(offset - cluster.len() as u32)
}

/// The end of the next word after `offset`, skipping spaces and
/// punctuation first.
#[must_use]
pub fn next_word(text: &str, offset: u32) -> Option<u32> {
	let rest = text.get(offset as usize..)?;
	let (start, word) = rest
		.split_word_bound_indices()
		.find(|(_, segment)| is_word(segment))?;
	Some(offset + (start + word.len()) as u32)
}

/// The start of the word before `offset`, skipping spaces and punctuation
/// first.
#[must_use]
pub fn prev_word(text: &str, offset: u32) -> Option<u32> {
	let head = text.get(..offset as usize)?;
	let (start, _) = head
		.split_word_bound_indices()
		.rev()
		.find(|(_, segment)| is_word(segment))?;
	Some(start as u32)
}

/// Words are segments with a letter or digit, plus embeds.
fn is_word(segment: &str) -> bool {
	segment.starts_with(EMBED_CHAR) || segment.chars().any(char::is_alphanumeric)
}

/// The number of grapheme clusters before `offset`.
#[must_use]
pub fn column(text: &str, offset: u32) -> usize {
	text.get(..offset as usize)
		.map_or(0, |head| head.graphemes(true).count())
}

/// The byte offset after `column` grapheme clusters, clamped to the end.
#[must_use]
pub fn offset_at_column(text: &str, column: usize) -> u32 {
	text.grapheme_indices(true)
		.nth(column)
		.map_or(text.len(), |(offset, _)| offset) as u32
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn steps_over_whole_grapheme_clusters() {
		let text = "a👩‍👩‍👧b";
		let family = "👩‍👩‍👧".len() as u32;

		assert_eq!(next_grapheme(text, 1), Some(1 + family));
		assert_eq!(prev_grapheme(text, 1 + family), Some(1));
		assert_eq!(next_grapheme(text, text.len() as u32), None);
		assert_eq!(prev_grapheme(text, 0), None);
	}

	#[test]
	fn word_motion_skips_punctuation_and_stops_at_embeds() {
		let text = "hello, world \u{FFFC} end";

		assert_eq!(next_word(text, 0), Some(5));
		assert_eq!(next_word(text, 5), Some(12));
		assert_eq!(prev_word(text, 12), Some(7));
		assert_eq!(prev_word(text, 7), Some(0));
		assert_eq!(next_word(text, 12), Some(16));
		assert_eq!(prev_word(text, 0), None);
	}

	#[test]
	fn converts_between_columns_and_offsets() {
		let text = "é🎨x";

		assert_eq!(column(text, text.len() as u32), 3);
		assert_eq!(offset_at_column(text, 2), "é🎨".len() as u32);
		assert_eq!(offset_at_column(text, 9), text.len() as u32);
	}
}
