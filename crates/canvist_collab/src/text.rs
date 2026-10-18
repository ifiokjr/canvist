//! Inline text on both sides of the binding, and UTF-8 ⇄ UTF-16 offsets.
//!
//! The model measures text in UTF-8 bytes and `Y.Text` in UTF-16 code units
//! (what JavaScript Yjs peers use). Every inline item is one character on
//! both sides: model embeds are U+FFFC and Yjs embeds count as one unit, so
//! conversion walks characters and treats each embed as a three-byte,
//! one-unit character. Content the model cannot show (shared types embedded
//! in text, embeds of unknown types, and literal U+FFFC typed by a
//! JavaScript peer) becomes U+FFFD, which has the same widths, so offsets
//! stay aligned without the binding tracking hidden inline content.

use canvist_model::EMBED_CHAR;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::Schema;
use yrs::Any;
use yrs::In;
use yrs::Out;
use yrs::ReadTxn;
use yrs::Text;
use yrs::TextRef;
use yrs::types::Delta;
use yrs::types::ToJson;
use yrs::types::text::Diff;

use crate::values;
use crate::values::Format;

/// What the model shows for inline content it cannot represent.
pub(crate) const REPLACEMENT: char = '\u{FFFD}';

/// The UTF-8 width of one inline item (an embed or its replacement).
const ITEM_UTF8: u32 = EMBED_CHAR.len_utf8() as u32;

/// The UTF-16 length of `text`.
pub(crate) fn utf16_len(text: &str) -> u32 {
	text.chars().map(|ch| ch.len_utf16() as u32).sum()
}

/// A position in a model block's text, in both units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Cursor {
	/// UTF-8 bytes from the start of the block.
	pub(crate) utf8: u32,
	/// UTF-16 code units from the start of the block.
	pub(crate) utf16: u32,
}

impl Cursor {
	/// Move forward `units` UTF-16 code units through `text`, returning
	/// `None` if that runs past the end or splits a surrogate pair.
	pub(crate) fn advance(&mut self, text: &str, units: u32) -> Option<()> {
		let target = self.utf16 + units;
		let mut chars = text.get(self.utf8 as usize..)?.chars();

		while self.utf16 < target {
			let ch = chars.next()?;
			self.utf8 += ch.len_utf8() as u32;
			self.utf16 += ch.len_utf16() as u32;
		}

		(self.utf16 == target).then_some(())
	}
}

/// The content of a `Y.Text`, chunked by formatting.
pub(crate) struct Chunks(Vec<Diff<()>>);

impl Chunks {
	/// Read the current content of `text`.
	pub(crate) fn read<T: ReadTxn>(text: &TextRef, txn: &T) -> Self {
		Self(text.diff(txn, |_| ()))
	}

	/// Convert a UTF-8 offset in the mirrored model text to a UTF-16 index,
	/// or `None` if it is out of range or inside a character.
	pub(crate) fn utf16_index(&self, utf8: u32) -> Option<u32> {
		let mut remaining = utf8;
		let mut index = 0;

		for chunk in &self.0 {
			let (bytes, units) = widths(chunk);

			if remaining < bytes {
				return within(chunk, remaining).map(|inner| index + inner);
			}

			remaining -= bytes;
			index += units;
		}

		(remaining == 0).then_some(index)
	}

	/// The content from UTF-16 index `start` to the end, as a delta that
	/// recreates it (formatting and embeds included) in another `Y.Text`.
	pub(crate) fn delta_from<T: ReadTxn>(&self, txn: &T, start: u32) -> Vec<Delta<In>> {
		let mut skip = start;
		let mut delta = Vec::new();

		for chunk in &self.0 {
			let (_, units) = widths(chunk);

			if skip >= units {
				skip -= units;
				continue;
			}

			delta.push(Delta::Inserted(
				copy(chunk, skip, txn),
				chunk.attributes.clone(),
			));
			skip = 0;
		}

		delta
	}

	/// The model's view of this content.
	pub(crate) fn to_inline(&self, schema: &Schema) -> InlineText {
		// Most blocks are one chunk; building it directly skips a copy.
		if let [chunk] = self.0.as_slice() {
			let marks = values::marks_from(schema, chunk.attributes.as_deref());
			return inline_from_out(schema, &chunk.insert, marks);
		}

		let mut inline = InlineText::new();

		for chunk in &self.0 {
			let marks = values::marks_from(schema, chunk.attributes.as_deref());
			inline.append(inline_from_out(schema, &chunk.insert, marks));
		}

		inline
	}
}

/// A chunk's string, or `None` for an inline item.
fn chunk_text(chunk: &Diff<()>) -> Option<&str> {
	match &chunk.insert {
		Out::Any(Any::String(text)) => Some(text),
		_ => None,
	}
}

/// A chunk's width in UTF-8 bytes and UTF-16 code units.
fn widths(chunk: &Diff<()>) -> (u32, u32) {
	chunk_text(chunk).map_or((ITEM_UTF8, 1), |text| (text.len() as u32, utf16_len(text)))
}

/// The UTF-16 offset of byte `utf8` inside one chunk, if it is a character
/// boundary. Inline items have no boundary inside them.
fn within(chunk: &Diff<()>, utf8: u32) -> Option<u32> {
	if utf8 == 0 {
		return Some(0);
	}

	chunk_text(chunk)?.get(..utf8 as usize).map(utf16_len)
}

/// A chunk without its first `skip` UTF-16 code units, as a value to insert.
fn copy<T: ReadTxn>(chunk: &Diff<()>, skip: u32, txn: &T) -> In {
	match &chunk.insert {
		Out::Any(Any::String(text)) => In::Any(Any::String(skip_utf16(text, skip).into())),
		Out::Any(any) => In::Any(any.clone()),
		shared => In::Any(shared.to_json(txn)),
	}
}

/// The part of `text` after its first `units` UTF-16 code units.
fn skip_utf16(text: &str, units: u32) -> &str {
	let mut seen = 0;

	for (index, ch) in text.char_indices() {
		if seen >= units {
			return &text[index..];
		}

		seen += ch.len_utf16() as u32;
	}

	""
}

/// The model's view of one inserted Yjs value.
pub(crate) fn inline_from_out(schema: &Schema, insert: &Out, marks: MarkSet) -> InlineText {
	match insert {
		Out::Any(Any::String(text)) => inline_from_str(text, marks),
		Out::Any(any) => {
			values::embed_from(schema, any).map_or_else(
				|| replacement(marks.clone()),
				|embed| InlineText::embed(embed, marks.clone()),
			)
		}
		_ => replacement(marks),
	}
}

/// Model text for a Yjs string, keeping literal U+FFFC visible as U+FFFD.
pub(crate) fn inline_from_str(text: &str, marks: MarkSet) -> InlineText {
	if text.contains(EMBED_CHAR) {
		let text = text.replace(EMBED_CHAR, &REPLACEMENT.to_string());

		return InlineText::plain(&text, marks);
	}

	InlineText::plain(text, marks)
}

fn replacement(marks: MarkSet) -> InlineText {
	InlineText::plain(&REPLACEMENT.to_string(), marks)
}

/// Model inline content as a delta for `Y.Text`.
pub(crate) fn inline_delta(schema: &Schema, inline: &InlineText) -> Vec<Delta<In>> {
	inline
		.runs()
		.map(|run| {
			let format = values::format_of(schema, run.marks);
			let insert = run.embed.map_or_else(
				|| In::Any(Any::String(run.text.into())),
				|embed| In::Any(values::embed_to_any(schema, embed)),
			);
			Delta::Inserted(insert, Some(Box::new(format)))
		})
		.collect()
}

/// Insert model inline content into `text` at UTF-16 `index`.
pub(crate) fn insert_inline(
	text: &TextRef,
	txn: &mut yrs::TransactionMut<'_>,
	index: u32,
	schema: &Schema,
	inline: &InlineText,
) {
	let mut at = index;

	for run in inline.runs() {
		let format: Format = values::format_of(schema, run.marks);

		if let Some(embed) = run.embed {
			text.insert_embed_with_attributes(txn, at, values::embed_to_any(schema, embed), format);
			at += 1;
		} else {
			text.insert_with_attributes(txn, at, run.text, format);
			at += utf16_len(run.text);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn cursor_walks_utf16_units() {
		let text = "a😀é";
		let mut cursor = Cursor::default();

		assert_eq!(cursor.advance(text, 1), Some(()));
		assert_eq!(cursor.advance(text, 2), Some(()));
		assert_eq!(cursor, Cursor { utf8: 5, utf16: 3 });
		assert_eq!(cursor.clone().advance(text, 2), None, "past the end");

		let mut inside = Cursor::default();
		assert_eq!(inside.advance("😀", 1), None, "splits a surrogate pair");
	}

	#[test]
	fn skips_utf16_prefixes() {
		assert_eq!(skip_utf16("a😀b", 3), "b");
		assert_eq!(skip_utf16("ab", 0), "ab");
		assert_eq!(skip_utf16("ab", 5), "");
	}

	#[test]
	fn literal_object_replacement_characters_stay_visible() {
		let inline = inline_from_str("a\u{FFFC}b", MarkSet::empty());

		assert_eq!(inline.as_str(), "a\u{FFFD}b");
	}
}
