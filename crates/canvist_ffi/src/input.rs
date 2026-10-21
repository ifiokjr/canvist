//! Platform text input: what `UITextInput` and Android's `InputConnection`
//! ask of a text view, in UTF-16 offsets into the editor's text, and the
//! geometry they place carets, handles, and candidate windows with.

use canvist_headless::canvist_editor::LineEdge;
use canvist_headless::canvist_editor::VerticalDirection;
use canvist_headless::canvist_model::InlineText;
use canvist_headless::canvist_model::Selection;
use canvist_layout::Affinity;
use canvist_layout::Caret;
use unicode_segmentation::UnicodeSegmentation;

use crate::Bounds;
use crate::Editor;
use crate::OffsetRange;
use crate::error::Result;
use crate::text::utf8_offset;
use crate::text::utf16_offset;

#[uniffi::export]
impl Editor {
	/// The editor's text length in UTF-16 code units.
	pub fn text_length(&self) -> u32 {
		self.lock().text.len()
	}

	/// The editor's text in `range`: text blocks joined by line feeds.
	pub fn text_in(&self, range: OffsetRange) -> String {
		let session = self.lock();
		session.text.text(session.doc.document(), range)
	}

	/// The whole document as plain text.
	pub fn plain_text(&self) -> String {
		let session = self.lock();
		let all = OffsetRange {
			start: 0,
			end: session.text.len(),
		};
		session.text.text(session.doc.document(), all)
	}

	/// Type `text` at the selection, replacing it, or commit it in place of
	/// the marked text. Input rules apply (`## ` makes a heading), and line
	/// breaks press Enter.
	pub fn insert_text(&self, text: &str) -> Result<bool> {
		self.change(|session| session.insert_text(text))
	}

	/// Replace `range` with `text`, as an input method's autocorrection or
	/// dictation does. An empty `text` deletes the range.
	pub fn replace(&self, range: OffsetRange, text: &str) -> Result<bool> {
		self.change(|session| session.replace(range, text))
	}

	/// Delete backwards from the caret, or delete the selection, the way the
	/// Backspace key does: at the start of a list item it lifts the item.
	pub fn delete_backward(&self) -> Result<bool> {
		self.press("Backspace", crate::Modifiers::default())
	}

	/// The marked (composing) text, if an input method is composing.
	pub fn marked_range(&self) -> Option<OffsetRange> {
		self.lock().marked_range()
	}

	/// Show `text` as marked text, replacing the marked text or else the
	/// selection, with `selected` (offsets into `text`) selected. An empty
	/// `text` removes the marked text.
	pub fn set_marked_text(&self, text: &str, selected: OffsetRange) -> Result<bool> {
		self.change(|session| session.set_marked_text(text, selected))
	}

	/// Commit the marked text as it is.
	pub fn unmark_text(&self) -> Result<bool> {
		self.change(super::session::Session::unmark_text)
	}

	/// Mark existing text as what the input method is composing (Android's
	/// `setComposingRegion`), or forget the mark with `None`. The text is
	/// underlined and the next marked text replaces it.
	pub fn set_composing_region(&self, range: Option<OffsetRange>) -> Result<bool> {
		self.change(|session| session.set_composing_region(range))
	}

	/// The caret's rectangle at `offset`.
	pub fn caret_rect(&self, offset: u32) -> Option<Bounds> {
		self.lock().caret_rect(offset)
	}

	/// Rectangles covering `range`, one or more per line.
	pub fn selection_rects(&self, range: OffsetRange) -> Vec<Bounds> {
		self.lock().selection_rects(range)
	}

	/// The first line's rectangle of `range`, where input methods place
	/// candidate windows; the caret's for an empty range.
	pub fn first_rect(&self, range: OffsetRange) -> Option<Bounds> {
		let session = self.lock();
		session
			.selection_rects(range)
			.into_iter()
			.next()
			.or_else(|| session.caret_rect(range.start))
	}

	/// The offset nearest to `(x, y)`.
	pub fn offset_at(&self, x: f32, y: f32) -> Option<u32> {
		self.lock().offset_at(x, y)
	}

	/// The word, run of spaces, or punctuation at `offset`.
	pub fn word_range(&self, offset: u32) -> OffsetRange {
		let session = self.lock();
		let doc = session.doc.document();
		let Some(block) = session.text.block_range(offset) else {
			return OffsetRange::default();
		};
		let Some(text) = session
			.point(offset)
			.and_then(|point| doc.inline(point.block))
			.map(InlineText::as_str)
		else {
			return OffsetRange::between(offset, offset);
		};
		let local = utf8_offset(text, offset.saturating_sub(block.start));

		let (start, end) = word_around(text, local);

		OffsetRange {
			start: block.start + utf16_offset(text, start),
			end: block.start + utf16_offset(text, end),
		}
	}

	/// The text block (paragraph, heading, list item) at `offset`.
	pub fn block_range(&self, offset: u32) -> OffsetRange {
		self.lock().text.block_range(offset).unwrap_or_default()
	}

	/// The offset one visual line above (`up`) or below `offset`, aiming
	/// for `goal_x` when given, or `None` on the first or last line.
	pub fn vertical_offset(&self, offset: u32, up: bool, goal_x: Option<f32>) -> Option<u32> {
		let mut session = self.lock();
		let point = session.point(offset)?;
		let direction = if up {
			VerticalDirection::Up
		} else {
			VerticalDirection::Down
		};

		let caret = canvist_headless::canvist_editor::Caret::new(point);
		let motion = session
			.navigate(|lines, doc| lines.vertical(doc, caret, direction, goal_x).ok().flatten())?;

		let doc = session.doc.document();
		session.text.offset(doc, motion.caret.point)
	}

	/// The start or end (`end`) of the visual line holding `offset`.
	pub fn line_edge_offset(&self, offset: u32, end: bool) -> Option<u32> {
		let mut session = self.lock();
		let point = session.point(offset)?;
		let edge = if end { LineEdge::End } else { LineEdge::Start };
		let caret = canvist_headless::canvist_editor::Caret::new(point);
		let found = session.navigate(|lines, doc| lines.line_edge(doc, caret, edge).ok())?;
		let doc = session.doc.document();
		session.text.offset(doc, found.point)
	}

	/// The selection's bounding box, for anchoring a toolbar or menu, or
	/// `None` for a caret.
	pub fn selection_bounds(&self) -> Option<Bounds> {
		let session = self.lock();
		let selection = session.doc.selection();

		if matches!(selection, Selection::Text { anchor, head } if anchor == head) {
			return None;
		}

		session
			.layout
			.selection_bounds(&selection)
			.map(Bounds::from)
	}

	/// Whether `(x, y)` is on the selected text, where a long press offers
	/// the edit menu rather than selecting a word.
	pub fn is_in_selection(&self, x: f32, y: f32) -> bool {
		let session = self.lock();
		let selection = session.selection().range();

		if selection.start == selection.end {
			return false;
		}

		session
			.layout
			.hit_test(x, y)
			.and_then(|caret| {
				let Caret { point, affinity } = caret;
				let offset = session.text.offset(session.doc.document(), point)?;
				Some((offset, affinity))
			})
			.is_some_and(|(offset, affinity)| {
				let after_start = offset > selection.start
					|| (offset == selection.start && affinity == Affinity::Downstream);
				after_start && offset < selection.end
			})
	}
}

/// The word, run of spaces, or punctuation that byte `offset` is in or just
/// before, as a byte range.
fn word_around(text: &str, offset: u32) -> (u32, u32) {
	let offset = offset as usize;
	let mut last = (0, 0);

	for (start, segment) in text.split_word_bound_indices() {
		let end = start + segment.len();
		last = (start as u32, end as u32);

		if offset < end {
			return last;
		}
	}

	last
}

#[cfg(test)]
mod tests {
	use super::word_around;

	#[test]
	fn words_around_offsets() {
		let text = "hello, wide world";

		assert_eq!(word_around(text, 2), (0, 5));
		assert_eq!(word_around(text, 5), (5, 6), "the comma");
		assert_eq!(word_around(text, 9), (7, 11));
		assert_eq!(word_around(text, text.len() as u32), (12, 17));
		assert_eq!(word_around("", 0), (0, 0));
	}
}
