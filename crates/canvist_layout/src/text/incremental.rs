//! Relaying out an edited paragraph without redoing all of it.
//!
//! Typing in a long paragraph changes a few words. This module re-itemizes a
//! window of text around the edit, re-breaks lines from just before it, and
//! stops as soon as a new line starts where an old one did over unchanged
//! text: greedy line breaking depends only on where a line starts, so every
//! line after that point is reused, shifted by the edit's length. The cost
//! follows the size of the edit and of the lines it reflows, plus one pass of
//! integer shifts over the items and lines after it.
//!
//! Only left-to-right paragraphs take this path. Bidi levels and the
//! paragraph direction depend on the whole paragraph, so bidi text, and any
//! edit that inserts right-to-left text, lays out from scratch.

use std::ops::Range;
use std::sync::Arc;

use canvist_model::InlineText;

use super::Draft;
use super::RunStyle;
use super::TextLayout;
use super::breaking;
use super::breaking::Cursor;
use super::breaking::RawLine;
use super::items;
use super::items::DraftItem;
use super::items::ItemContext;
use crate::LayoutError;
use crate::measure::MeasureCache;
use crate::theme::TextStyle;

/// Bytes kept between the edit and the edges of the re-itemized window, so
/// no line break rule that looks a few characters ahead or behind can see
/// across an edge.
const MARGIN: u32 = 8;
/// How far to look back for a space to start segmenting after.
const CONTEXT_SEARCH: u32 = 256;
/// Items after the window whose break opportunities are re-checked.
const CHECKED_ITEMS: usize = 3;

/// A change to one block's text: bytes `start..old_end` of the old text
/// became bytes `start..new_end` of the new text. Restyling a range is an
/// edit whose old and new ends are equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TextEdit {
	pub(crate) start: u32,
	pub(crate) old_end: u32,
	pub(crate) new_end: u32,
}

impl TextEdit {
	/// Inserting `len` bytes at `offset`.
	pub(crate) const fn insert(offset: u32, len: u32) -> Self {
		Self {
			start: offset,
			old_end: offset,
			new_end: offset + len,
		}
	}

	/// Removing `range`.
	pub(crate) const fn remove(range: &Range<u32>) -> Self {
		Self {
			start: range.start,
			old_end: range.end,
			new_end: range.start,
		}
	}

	/// Restyling `range` without changing its text.
	pub(crate) const fn restyle(range: &Range<u32>) -> Self {
		Self {
			start: range.start,
			old_end: range.end,
			new_end: range.end,
		}
	}

	/// The edit that applies `self` and then `next`, whose offsets are in
	/// the text `self` produced.
	pub(crate) fn then(self, next: Self) -> Self {
		let start = self.start.min(next.start);
		let middle = self.new_end.max(next.old_end);
		let old_end = i64::from(middle) - i64::from(self.new_end) + i64::from(self.old_end);
		let new_end = i64::from(middle) - i64::from(next.old_end) + i64::from(next.new_end);

		Self {
			start,
			old_end: (old_end.max(i64::from(start))) as u32,
			new_end: (new_end.max(i64::from(start))) as u32,
		}
	}

	/// How much the text grew.
	fn delta(self) -> i64 {
		i64::from(self.new_end) - i64::from(self.old_end)
	}
}

/// A block's previous layout and the edit made since it was built.
#[derive(Clone, Debug)]
pub(crate) struct Previous {
	pub(crate) layout: Arc<TextLayout>,
	pub(crate) edit: TextEdit,
}

impl Previous {
	/// Fold a further edit in.
	pub(crate) fn then(self, edit: TextEdit) -> Self {
		Self {
			layout: self.layout,
			edit: self.edit.then(edit),
		}
	}
}

/// A partial relayout waiting for its window's measurements.
pub(crate) struct Partial {
	previous: Arc<TextLayout>,
	text: Arc<str>,
	edit: TextEdit,
	/// The old items the window replaces.
	items: Range<usize>,
	/// Where the window ends in the old text.
	window_end: u32,
	/// The first line to break again.
	first_line: usize,
	styles: Vec<RunStyle>,
	drafts: Vec<DraftItem>,
}

/// The window a partial relayout redoes.
struct Plan {
	first_line: usize,
	items: Range<usize>,
	/// The re-itemized range in the new text.
	range: Range<u32>,
	/// The segmented range in the new text, wider than `range`.
	context: Range<u32>,
}

/// Phase one of a partial relayout: itemize the window around the edit.
/// Returns `None` when the block must be laid out from scratch.
pub(crate) fn prepare(
	context: &mut ItemContext<'_>,
	base: &TextStyle,
	inline: &InlineText,
	previous: Previous,
	width: f32,
) -> Option<Draft> {
	let Previous { layout: old, edit } = previous;
	let text = inline.as_str();

	if !reusable(&old, edit, text, width) {
		return None;
	}

	let plan = plan(&old, edit, text)?;
	let breaks = super::breaks::soft_breaks_in(text, plan.context.clone(), context.break_override);

	if !boundaries_hold(&old, edit, &plan, &breaks) {
		return None;
	}

	let window = items::Window {
		range: plan.range.clone(),
		levels: &[],
		breaks: &breaks,
	};
	let (styles, drafts) =
		items::itemize_window(context, base, inline, old.styles.clone(), &window);
	request_edge_kerning(context, text, &old, edit, &plan, &drafts);

	if slice(text, &plan.range).contains('\t') {
		context.cache.request(styles[0].font, " ");
	}

	Some(Draft::Partial(Partial {
		previous: old,
		text: Arc::from(text),
		edit,
		items: plan.items,
		window_end: shift(plan.range.end, -edit.delta()),
		first_line: plan.first_line,
		styles,
		drafts,
	}))
}

/// Queue the kerning pairs between the window's first and last items and
/// the old items either side of it, which [`splice_items`] kerns again.
fn request_edge_kerning(
	context: &mut ItemContext<'_>,
	text: &str,
	old: &TextLayout,
	edit: TextEdit,
	plan: &Plan,
	drafts: &[DraftItem],
) {
	let as_draft = |item: &items::Item, delta: i64| {
		let items::ItemKind::Text { font, .. } = item.kind else {
			return None;
		};

		Some(DraftItem {
			range: shift(item.range.start, delta)..shift(item.range.end, delta),
			style: item.style,
			level: item.level,
			kind: items::DraftKind::Text(font),
			break_after: item.break_after,
		})
	};
	let before = plan
		.items
		.start
		.checked_sub(1)
		.and_then(|index| old.items.get(index))
		.and_then(|item| as_draft(item, 0));
	let after = old
		.items
		.get(plan.items.end)
		.and_then(|item| as_draft(item, edit.delta()));
	let mut joined = String::new();
	let pairs = [
		(before.as_ref(), drafts.first()),
		(drafts.last(), after.as_ref()),
	];

	for (left, right) in pairs {
		if let (Some(left), Some(right)) = (left, right)
			&& let Some((font, pair)) = items::kerning_pair(text, left, right)
		{
			super::kerning::request(context.cache, font, pair, &mut joined);
		}
	}
}

/// Whether `old` can be patched into the layout of `text`: it was laid out
/// left to right at `width`, and the edit's offsets fit both texts.
fn reusable(old: &TextLayout, edit: TextEdit, text: &str, width: f32) -> bool {
	let lengths_match = i64::from(old.text.len() as u32) + edit.delta() == text.len() as i64;
	let inserted = text.get(edit.start as usize..edit.new_end as usize);

	!old.bidi
		&& !old.rtl
		&& !old.items.is_empty()
		&& !text.is_empty()
		&& old.width.to_bits() == width.to_bits()
		&& edit.old_end as usize <= old.text.len()
		&& lengths_match
		&& inserted.is_some_and(|inserted| !inserted.chars().any(items::is_bidi_sensitive))
}

/// Choose the window: from the line before the one where the edited
/// unbreakable unit starts, to the start of an old item at least
/// [`MARGIN`] bytes after the edit that follows a space.
///
/// Breaking again from the line *before* matters: an edit that adds a break
/// opportunity can let the start of its unit move up to the end of the
/// previous line.
fn plan(old: &TextLayout, edit: TextEdit, text: &str) -> Option<Plan> {
	let unit_start = unit_start(old, edit.start.saturating_sub(MARGIN));
	let first_line = old
		.lines
		.partition_point(|line| line.range.start <= unit_start)
		.saturating_sub(2);
	let line_start = old.lines.get(first_line)?.range.start;
	let first_item = old
		.items
		.partition_point(|item| item.range.end <= line_start);

	let start = old
		.items
		.get(first_item)
		.map_or(line_start, |item| item.range.start.min(line_start));
	let last_item = window_end_item(old, edit);
	let delta = edit.delta();
	let end = old
		.items
		.get(last_item)
		.map_or(text.len() as u32, |item| shift(item.range.start, delta));
	let context_end = old
		.items
		.get(last_item + CHECKED_ITEMS)
		.map_or(text.len() as u32, |item| shift(item.range.end, delta));

	(start <= edit.start && end >= edit.new_end).then(|| {
		Plan {
			first_line,
			items: first_item..last_item,
			range: start..end,
			context: context_start(text, start)..context_end,
		}
	})
}

/// The start of the unbreakable unit holding `offset`: the end of the last
/// item before it that a line may break after.
fn unit_start(old: &TextLayout, offset: u32) -> u32 {
	let holding = old.items.partition_point(|item| item.range.end <= offset);
	let first = old.items[..holding.min(old.items.len())]
		.iter()
		.rposition(|item| item.break_after != items::BreakAfter::None)
		.map_or(0, |index| index + 1);

	old.items.get(first).map_or(offset, |item| item.range.start)
}

/// The first old item the window keeps: one starting at least [`MARGIN`]
/// bytes past the edit, right after a space.
fn window_end_item(old: &TextLayout, edit: TextEdit) -> usize {
	let from = old
		.items
		.partition_point(|item| item.range.start < edit.old_end + MARGIN);
	let bytes = old.text.as_bytes();

	old.items[from..]
		.iter()
		.position(|item| {
			item.range.start > 0 && bytes.get(item.range.start as usize - 1) == Some(&b' ')
		})
		.map_or(old.items.len(), |index| from + index)
}

/// A place to start segmenting at or before `start`: just after a space,
/// where no line break rule looks further back, or the start of the text.
fn context_start(text: &str, start: u32) -> u32 {
	let floor = start.saturating_sub(CONTEXT_SEARCH);
	let before = &text.as_bytes()[floor as usize..start as usize];

	before
		.iter()
		.rposition(|byte| *byte == b' ')
		.map_or(0, |index| floor + index as u32 + 1)
}

/// Whether the window's end is still an item boundary in the new text, and
/// the first few kept items still break where they did.
fn boundaries_hold(old: &TextLayout, edit: TextEdit, plan: &Plan, breaks: &[u32]) -> bool {
	let is_break = |offset: u32| breaks.binary_search(&offset).is_ok();
	let delta = edit.delta();
	let checked = old.items[plan.items.end..]
		.iter()
		.take(CHECKED_ITEMS)
		.filter(|item| shift(item.range.end, delta) < plan.context.end);

	let end_holds = plan.items.end == old.items.len() || is_break(plan.range.end);

	end_holds
		&& checked.into_iter().all(|item| {
			let soft = is_break(shift(item.range.end, delta));

			match item.break_after {
				items::BreakAfter::Hard => true,
				items::BreakAfter::Soft => soft,
				items::BreakAfter::None => !soft,
			}
		})
}

/// Phase two: measure the window, splice it into the old layout, and break
/// lines again until they meet the old ones.
pub(crate) fn finish(
	partial: Partial,
	cache: &MeasureCache,
	tab_size: u32,
) -> Result<TextLayout, LayoutError> {
	let first_line = partial.first_line;
	let window_end = shift(partial.window_end, partial.edit.delta());
	let (mut layout, by, first_item) = splice_items(partial, cache)?;
	let old_lines = std::mem::take(&mut layout.lines);
	let start = Cursor {
		item: first_item,
		offset: old_lines[first_line].range.start,
	};
	let resume = |line_start: u32| {
		line_start >= window_end
			&& old_lines[first_line..]
				.binary_search_by_key(&shift(line_start, -by.bytes), |line| line.range.start)
				.is_ok()
	};
	let lines = super::line_settings(&layout, cache, tab_size)?;
	let (raw, resumed) =
		breaking::break_from(&layout.text, &mut layout.items, start, lines, resume);

	layout.lines = old_lines;
	layout.restack(first_line, raw, resumed, by);

	Ok(layout)
}

/// Take ownership of the old layout, measure the window's items, and put
/// them in place of the old ones, shifting the items after them. Returns
/// the layout, how far things after the window moved, and the window's
/// first item.
fn splice_items(
	partial: Partial,
	cache: &MeasureCache,
) -> Result<(TextLayout, Shift, usize), LayoutError> {
	let Partial {
		previous,
		text,
		edit,
		items: replaced,
		mut styles,
		drafts,
		..
	} = partial;
	let mut layout = Arc::try_unwrap(previous).unwrap_or_else(|shared| (*shared).clone());
	let window = items::measure_items(&text, drafts, &mut styles, cache)?;
	let inserted = window.len();
	let by = Shift {
		bytes: edit.delta(),
		items: inserted as i64 - replaced.len() as i64,
	};

	layout.items.splice(replaced.clone(), window);

	for item in &mut layout.items[replaced.start + inserted..] {
		item.range = shift(item.range.start, by.bytes)..shift(item.range.end, by.bytes);
	}

	layout.text = text;
	layout.styles = styles;
	// Kern the window's items, and the item before it with the window's
	// first one.
	let kerned = replaced.start.saturating_sub(1)..replaced.start + inserted + 1;
	super::kerning::apply(&layout.text, &mut layout.items, kerned, cache)?;

	Ok((layout, by, replaced.start))
}

/// How far everything after an edit moved.
#[derive(Clone, Copy, Debug)]
struct Shift {
	bytes: i64,
	items: i64,
}

impl TextLayout {
	/// Replace lines from `first_line` with `raw`, then keep the old lines
	/// from the one starting at `resumed` (in the new text) onwards, shifted.
	fn restack(&mut self, first_line: usize, raw: Vec<RawLine>, resumed: Option<u32>, by: Shift) {
		let kept_from = resumed.and_then(|start| {
			let old_start = shift(start, -by.bytes);

			self.lines[first_line..]
				.binary_search_by_key(&old_start, |line| line.range.start)
				.ok()
				.map(|index| first_line + index)
		});

		let tail: Vec<super::Line> = match kept_from {
			Some(from) => self.lines.split_off(from),
			None => Vec::new(),
		};

		self.lines.truncate(first_line);

		let top = self.lines.last().map_or(0.0, |line| line.top + line.height);
		let rebroken = self.build_lines(raw, top);
		let bottom = rebroken.last().map_or(top, |line| line.top + line.height);
		let rise = bottom - tail.first().map_or(bottom, |line| line.top);

		self.lines.extend(rebroken);
		self.lines
			.extend(tail.into_iter().map(|line| shifted_line(line, by, rise)));
		self.height = self.lines.last().map_or(0.0, |line| line.top + line.height);
	}
}

/// `line` moved by `by` in the text and down by `rise`.
fn shifted_line(mut line: super::Line, by: Shift, rise: f32) -> super::Line {
	line.range = shift(line.range.start, by.bytes)..shift(line.range.end, by.bytes);
	line.caret_end = shift(line.caret_end, by.bytes);
	line.top += rise;

	for piece in &mut line.pieces {
		piece.range = shift(piece.range.start, by.bytes)..shift(piece.range.end, by.bytes);
		piece.item = shift(piece.item, by.items);
	}

	line
}

/// `value` moved by `delta`.
fn shift(value: u32, delta: i64) -> u32 {
	(i64::from(value) + delta) as u32
}

/// The byte range `range` of `text`.
fn slice<'a>(text: &'a str, range: &Range<u32>) -> &'a str {
	items::slice(text, range)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn edits_compose() {
		let typed = TextEdit::insert(10, 3);
		let deleted_before = TextEdit::remove(&(2..3));

		assert_eq!(
			typed.then(deleted_before),
			TextEdit {
				start: 2,
				old_end: 10,
				new_end: 12,
			}
		);

		let after_typed = typed.new_end;
		let typed_again = TextEdit::insert(after_typed, 1);
		assert_eq!(
			typed.then(typed_again),
			TextEdit {
				start: 10,
				old_end: 10,
				new_end: 14,
			}
		);
	}
}
