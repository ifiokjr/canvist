//! Input from the host: keys, IME composition, the text model an
//! `EditContext` edits, pointer selection, the clipboard, and drops.
//!
//! The host owns the DOM events and calls these methods; every change goes
//! through the editor (so key bindings, input rules, and history apply) and
//! through layout (so only what changed is laid out again).

use std::cmp::Ordering;

use canvist_headless::canvist_editor::KeyPress;
use canvist_headless::canvist_editor::Modifiers;
use canvist_headless::canvist_editor::Slice;
use canvist_headless::canvist_editor::Update;
use canvist_headless::canvist_editor::command;
use canvist_headless::canvist_editor::commands;
use canvist_headless::canvist_editor::position;
use canvist_headless::canvist_extensions::clipboard;
use canvist_headless::canvist_model::Bias;
use canvist_headless::canvist_model::BlockId;
use canvist_headless::canvist_model::InlineText;
use canvist_headless::canvist_model::Point;
use canvist_headless::canvist_model::Selection;
use canvist_layout::Affinity;
use canvist_layout::Caret;
use unicode_segmentation::UnicodeSegmentation;
use wasm_bindgen::prelude::*;

use crate::WebEditor;
use crate::error::JsResult;
use crate::error::headless;

/// Modifier bits in [`WebEditor::key_down`]'s `modifiers`.
const SHIFT: u32 = 1;
const CTRL: u32 = 2;
const ALT: u32 = 4;
const META: u32 = 8;

/// Clicks that select a word; more select the whole text block.
const WORD_CLICKS: u32 = 2;

/// What a pointer drag extends the selection by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
	Character,
	Word,
	Block,
}

/// A pointer drag in progress: the range the press selected, which every
/// later position extends from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Drag {
	unit: Unit,
	from: Point,
	to: Point,
}

#[wasm_bindgen]
impl WebEditor {
	/// Run the key binding for `key`, a DOM `KeyboardEvent.key`, with
	/// `modifiers` (bits: 1 Shift, 2 Ctrl, 4 Alt, 8 Meta). Returns whether a
	/// binding applied; when none did, the host lets the browser handle the
	/// key.
	#[wasm_bindgen(js_name = keyDown)]
	pub fn key_down(&mut self, key: &str, modifiers: u32) -> JsResult<bool> {
		let modifiers = Modifiers {
			shift: modifiers & SHIFT != 0,
			ctrl: modifiers & CTRL != 0,
			alt: modifiers & ALT != 0,
			meta: modifiers & META != 0,
		};
		let press = KeyPress::new(key, modifiers);

		#[cfg(feature = "collab")]
		if let Some(redo) = self.doc.shared_history_key(press) {
			return self.history(redo);
		}

		self.edit(|editor, env| editor.handle_key(press, env))
	}

	/// Show `text` as IME preedit text at the selection, replacing any
	/// earlier preedit.
	#[wasm_bindgen(js_name = setComposition)]
	pub fn set_composition(&mut self, text: &str) -> JsResult<bool> {
		self.edit(|editor, env| editor.set_composition(text, env))
	}

	/// Commit the composition as `text`.
	#[wasm_bindgen(js_name = commitComposition)]
	pub fn commit_composition(&mut self, text: &str) -> JsResult<bool> {
		self.edit(|editor, env| editor.commit_composition(text, env))
	}

	/// The text an `EditContext` edits: the text block holding the
	/// selection's head, and the selection within it in UTF-16 code units, as
	/// `[text, start, end]`. A selection reaching into another block is
	/// clamped to this one. Empty when the head is not in a text block.
	#[wasm_bindgen(
		js_name = editContextState,
		unchecked_return_type = "[text: string, start: number, end: number] | []"
	)]
	pub fn edit_context_state(&self) -> js_sys::Array {
		let Some((block, text)) = self.head_block() else {
			return js_sys::Array::new();
		};
		let (anchor, head) = self.selection_points();
		let clamp = |point: Point| {
			if point.block == block {
				utf16(&text, point.offset)
			} else if position::compare_points(self.doc.document(), point, Point::new(block, 0))
				== Ordering::Less
			{
				0
			} else {
				utf16(&text, text.len() as u32)
			}
		};
		let (start, end) = (clamp(anchor), clamp(head));

		[
			JsValue::from(text.as_str()),
			JsValue::from(start.min(end)),
			JsValue::from(start.max(end)),
		]
		.iter()
		.collect()
	}

	/// Replace UTF-16 range `start..end` of the head's text block with
	/// `text`, as an input method or the platform's text input does. A
	/// replacement at a caret is typing, so input rules apply.
	#[wasm_bindgen(js_name = replaceText)]
	pub fn replace_text(&mut self, start: u32, end: u32, text: &str) -> JsResult<bool> {
		let Some((block, content)) = self.head_block() else {
			return Ok(false);
		};
		let range = Selection::text(
			Point::new(block, byte_offset(&content, start)),
			Point::new(block, byte_offset(&content, end)),
		);
		let moved =
			range != self.doc.selection() && self.edit(|editor, env| editor.select(range, env))?;

		if text.is_empty() {
			let deleted = self.delete_selection()?.is_some();
			return Ok(moved || deleted);
		}

		let typed = self.edit(|editor, env| editor.insert_text(text, env))?;
		Ok(moved || typed)
	}

	/// The rectangles of the characters in UTF-16 range `start..end` of the
	/// head's text block, as `[x, y, width, height]` per character in
	/// document coordinates, for placing IME candidate windows.
	#[wasm_bindgen(js_name = characterBounds)]
	pub fn character_bounds(&self, start: u32, end: u32) -> Vec<f32> {
		let Some((block, text)) = self.head_block() else {
			return Vec::new();
		};
		let (from, to) = (byte_offset(&text, start), byte_offset(&text, end));
		let mut bounds = Vec::new();

		for (offset, character) in text[from as usize..to as usize].char_indices() {
			let left = from + offset as u32;
			let right = left + character.len_utf8() as u32;
			let caret = |offset, affinity| {
				self.layout
					.caret_rect(Caret::with_affinity(Point::new(block, offset), affinity))
			};

			if let (Some(a), Some(b)) = (
				caret(left, Affinity::Downstream),
				caret(right, Affinity::Upstream),
			) {
				let x = a.x.min(b.x);
				bounds.extend([x, a.y, (a.x - b.x).abs(), a.height]);
			}

			// The rest of a UTF-16 surrogate pair has no rectangle of its own.
			if character.len_utf16() == 2 {
				bounds.extend([0.0; 4]);
			}
		}

		bounds
	}

	/// Press the pointer at document coordinates `(x, y)`. One click places
	/// the caret, or extends the selection with `extend`; two select a word
	/// and three the whole text block, and dragging afterwards extends by
	/// that unit.
	#[wasm_bindgen(js_name = pointerDown)]
	pub fn pointer_down(&mut self, x: f32, y: f32, extend: bool, clicks: u32) -> JsResult<bool> {
		let Some(point) = self.point_at(x, y) else {
			return Ok(false);
		};
		let unit = match clicks {
			0 | 1 => Unit::Character,
			WORD_CLICKS => Unit::Word,
			_ => Unit::Block,
		};
		let (from, to) = self.unit_around(point, unit);

		if extend && unit == Unit::Character {
			let (anchor, _) = self.selection_points();
			self.drag = Some(Drag {
				unit,
				from: anchor,
				to: anchor,
			});
			return self.extend_to(point);
		}

		self.drag = Some(Drag { unit, from, to });
		self.edit(|editor, env| editor.select(Selection::text(from, to), env))
	}

	/// Move a pressed pointer to `(x, y)`, extending the selection from the
	/// press.
	#[wasm_bindgen(js_name = pointerMove)]
	pub fn pointer_move(&mut self, x: f32, y: f32) -> JsResult<bool> {
		match self.point_at(x, y) {
			Some(point) if self.drag.is_some() => self.extend_to(point),
			_ => Ok(false),
		}
	}

	/// Release the pointer.
	#[wasm_bindgen(js_name = pointerUp)]
	pub fn pointer_up(&mut self) {
		self.drag = None;
	}

	/// Whether `(x, y)` is inside a non-empty selection, where a press starts
	/// dragging the selection rather than selecting.
	#[wasm_bindgen(js_name = isInSelection)]
	pub fn is_in_selection(&self, x: f32, y: f32) -> bool {
		let Some(range) = position::text_range(self.doc.document(), self.doc.selection()) else {
			return false;
		};
		let Some(point) = self.point_at(x, y) else {
			return false;
		};
		let doc = self.doc.document();

		!range.is_empty()
			&& position::compare_points(doc, range.from, point) == Ordering::Less
			&& position::compare_points(doc, point, range.to) == Ordering::Less
	}

	/// The selection as `[html, text]` for the clipboard, or empty when
	/// nothing is selected.
	#[wasm_bindgen(js_name = copySelection)]
	pub fn copy_selection(&self) -> JsResult<Vec<String>> {
		if self.selection_is_collapsed() {
			return Ok(Vec::new());
		}

		let slice = self
			.doc
			.editor()
			.copy()
			.map_err(|error| headless(error.into()))?;
		Ok(self.clipboard_formats(&slice))
	}

	/// Cut the selection, returning it as [`WebEditor::copy_selection`]
	/// does.
	#[wasm_bindgen(js_name = cutSelection)]
	pub fn cut_selection(&mut self) -> JsResult<Vec<String>> {
		let copied = self.copy_selection()?;

		if !copied.is_empty() {
			self.delete_selection()?;
		}

		Ok(copied)
	}

	/// Paste HTML over the selection.
	#[wasm_bindgen(js_name = pasteHtml)]
	pub fn paste_html(&mut self, html: &str) -> JsResult<bool> {
		let slice = clipboard::from_html(self.doc.document().schema(), html);
		self.edit(|editor, env| editor.paste(slice, env))
	}

	/// Paste plain text over the selection; paste rules apply.
	#[wasm_bindgen(js_name = pasteText)]
	pub fn paste_text(&mut self, text: &str) -> JsResult<bool> {
		self.edit(|editor, env| editor.paste_text(text, env))
	}

	/// Drop content at `(x, y)`: `html` when the source offered it, otherwise
	/// `text`. With `moving`, the content is this editor's own selection,
	/// dragged: it leaves its old place. A drop inside the dragged selection
	/// does nothing.
	#[wasm_bindgen(js_name = dropAt)]
	pub fn drop_at(
		&mut self,
		x: f32,
		y: f32,
		html: Option<String>,
		text: &str,
		moving: bool,
	) -> JsResult<bool> {
		let Some(target) = self.point_at(x, y) else {
			return Ok(false);
		};

		if moving && self.is_in_selection(x, y) {
			return Ok(false);
		}

		let target = if moving {
			self.delete_selection()?.map_or(target, |update| {
				update.commit.map(target, Bias::After).point
			})
		} else {
			target
		};
		self.edit(|editor, env| editor.select(Selection::caret(target), env))?;

		match html {
			Some(html) => self.paste_html(&html),
			None => self.paste_text(text),
		}
	}
}

impl WebEditor {
	/// The text block holding the selection's head, with its text.
	fn head_block(&self) -> Option<(BlockId, String)> {
		let (_, head) = self.selection_points();
		let text = self.doc.document().inline(head.block)?.as_str().to_owned();
		Some((head.block, text))
	}

	/// The anchor and head of a text selection, or the document start.
	fn selection_points(&self) -> (Point, Point) {
		match self.doc.selection() {
			Selection::Text { anchor, head } => (anchor, head),
			selection @ Selection::Blocks { .. } => {
				let doc = self.doc.document();
				let start = position::text_range(doc, selection)
					.map(|range| range.from)
					.or_else(|| {
						match position::start_selection(doc) {
							Selection::Text { head, .. } => Some(head),
							Selection::Blocks { .. } => None,
						}
					});
				let point = start.unwrap_or(Point::new(doc.children(None)[0], 0));
				(point, point)
			}
		}
	}

	fn selection_is_collapsed(&self) -> bool {
		position::text_range(self.doc.document(), self.doc.selection())
			.is_none_or(|range| range.is_empty())
	}

	/// The model point at document coordinates `(x, y)`.
	fn point_at(&self, x: f32, y: f32) -> Option<Point> {
		self.layout.hit_test(x, y).map(|caret| caret.point)
	}

	/// The range `unit` covers around `point`.
	fn unit_around(&self, point: Point, unit: Unit) -> (Point, Point) {
		let Some(text) = self
			.doc
			.document()
			.inline(point.block)
			.map(InlineText::as_str)
		else {
			return (point, point);
		};

		match unit {
			Unit::Character => (point, point),
			Unit::Word => {
				let (start, end) = word_around(text, point.offset);
				(Point::new(point.block, start), Point::new(point.block, end))
			}
			Unit::Block => {
				(
					Point::new(point.block, 0),
					Point::new(point.block, text.len() as u32),
				)
			}
		}
	}

	/// Extend the dragged selection to cover `point` by the drag's unit.
	fn extend_to(&mut self, point: Point) -> JsResult<bool> {
		let Some(drag) = self.drag else {
			return Ok(false);
		};
		let (start, end) = self.unit_around(point, drag.unit);
		let doc = self.doc.document();
		let selection = if position::compare_points(doc, point, drag.from) == Ordering::Less {
			Selection::text(drag.to, start)
		} else {
			Selection::text(drag.from, end)
		};

		self.edit(|editor, env| editor.select(selection, env))
	}

	/// Delete the selected text or blocks.
	fn delete_selection(&mut self) -> JsResult<Option<Update>> {
		self.edit_update(|editor, env| editor.run(&command(commands::delete_selection), env))
	}

	fn clipboard_formats(&self, slice: &Slice) -> Vec<String> {
		let schema = self.doc.document().schema();
		vec![
			clipboard::to_html(schema, slice),
			clipboard::to_plain_text(schema, slice),
		]
	}
}

/// The UTF-16 offset of byte `offset` in `text`.
fn utf16(text: &str, offset: u32) -> u32 {
	text.get(..offset as usize)
		.map_or(0, |head| head.encode_utf16().count() as u32)
}

/// The byte offset of UTF-16 offset `units` in `text`, snapped back to a
/// character boundary.
fn byte_offset(text: &str, units: u32) -> u32 {
	let mut counted = 0;

	for (offset, character) in text.char_indices() {
		if counted >= units {
			return offset as u32;
		}

		counted += character.len_utf16() as u32;
	}

	text.len() as u32
}

/// The word, run of spaces, or punctuation that `offset` is in or just
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
	use super::*;

	#[test]
	fn utf16_offsets_round_trip() {
		let text = "a😀b";

		assert_eq!(utf16(text, 5), 3);
		assert_eq!(byte_offset(text, 3), 5);
		assert_eq!(byte_offset(text, 99), text.len() as u32);
	}

	#[test]
	fn words_around_offsets() {
		let text = "hello, wide world";

		assert_eq!(word_around(text, 2), (0, 5));
		assert_eq!(word_around(text, 5), (5, 6), "the comma");
		assert_eq!(word_around(text, 9), (7, 11));
		assert_eq!(
			word_around(text, text.len() as u32),
			(12, 17),
			"the end selects the last word"
		);
	}
}
