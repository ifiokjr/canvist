//! Greedy line breaking with CSS `white-space: pre-wrap` and
//! `overflow-wrap: break-word` semantics.
//!
//! - Lines break only at UAX #14 opportunities, filling each line greedily as
//!   browsers do for `text-wrap: wrap`.
//! - Trailing spaces and tabs *hang*: they stay on the line but never cause
//!   it to overflow.
//! - A word wider than the whole line is split at grapheme boundaries.
//! - Tabs advance to the next multiple of `tab-size` space widths, skipping a
//!   stop closer than half a space, per CSS Text 3.

use std::ops::Range;

use super::items::BreakAfter;
use super::items::Item;
use super::items::ItemKind;
use super::items::grapheme_boundaries;
use super::items::slice;

/// Widths within this tolerance of the available width still fit, so float
/// rounding in summed advances never wraps a line that fits exactly.
const EPSILON: f32 = 1.0 / 128.0;

/// One line before metrics and reordering.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RawLine {
	/// Byte range including trailing spaces and any forced break character.
	pub(crate) range: Range<u32>,
	/// The last caret offset on the line: before a forced break character,
	/// otherwise the end of the range.
	pub(crate) caret_end: u32,
	/// Whether the line ended at a soft wrap, so `range.end` is also the
	/// first offset of the next line.
	pub(crate) soft: bool,
}

/// Where tab stops fall.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TabStops {
	/// Distance between stops.
	pub(crate) interval: f32,
	/// The width of a space, for the half-space minimum.
	pub(crate) space: f32,
}

impl TabStops {
	/// The width of a tab starting at `x`.
	fn width_at(self, x: f32) -> f32 {
		if self.interval <= 0.0 {
			return 0.0;
		}

		let mut next = ((x / self.interval).floor() + 1.0) * self.interval;

		if next - x < self.space / 2.0 {
			next += self.interval;
		}

		next - x
	}
}

/// A position inside the item list.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cursor {
	/// The item the position falls in.
	pub(crate) item: usize,
	/// The byte offset in the block text.
	pub(crate) offset: u32,
}

/// Break `items` into lines no wider than `available`.
pub(crate) fn break_lines(
	text: &str,
	items: &mut [Item],
	available: f32,
	tabs: TabStops,
) -> Vec<RawLine> {
	let mut breaker = Breaker {
		text,
		items,
		available,
		tabs,
		lines: Vec::new(),
		line_start: 0,
		x: 0.0,
	};
	breaker.run();
	breaker.lines
}

/// Break lines from `start`, the first offset of a line, until `resume`
/// accepts the start of a new line. Returns the lines broken and, if
/// breaking stopped early, the offset where the next line starts.
///
/// Greedy breaking only depends on where a line starts, so once a new line
/// starts where an old one did over unchanged items, every line after it is
/// unchanged too and the caller can reuse them.
pub(crate) fn break_from(
	text: &str,
	items: &mut [Item],
	start: Cursor,
	lines: Lines,
	mut resume: impl FnMut(u32) -> bool,
) -> (Vec<RawLine>, Option<u32>) {
	let mut breaker = Breaker {
		text,
		items,
		available: lines.available,
		tabs: lines.tabs,
		lines: Vec::new(),
		line_start: start.offset,
		x: 0.0,
	};
	let mut cursor = start;

	while cursor.item < breaker.items.len() {
		let finished = breaker.lines.len();
		cursor = breaker.place_unit(cursor);

		if breaker.lines.len() > finished && resume(breaker.line_start) {
			return (breaker.lines, Some(breaker.line_start));
		}
	}

	breaker.finish_text();

	(breaker.lines, None)
}

/// How lines are filled.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Lines {
	/// The width lines must fit.
	pub(crate) available: f32,
	/// Where tabs stop.
	pub(crate) tabs: TabStops,
}

struct Breaker<'a> {
	text: &'a str,
	items: &'a mut [Item],
	available: f32,
	tabs: TabStops,
	lines: Vec<RawLine>,
	line_start: u32,
	/// Width of the current line so far, including trailing spaces.
	x: f32,
}

impl Breaker<'_> {
	fn run(&mut self) {
		let mut cursor = Cursor { item: 0, offset: 0 };

		while cursor.item < self.items.len() {
			cursor = self.place_unit(cursor);
		}

		self.finish_text();
	}

	/// End the last line at the end of the text.
	fn finish_text(&mut self) {
		let end = self.text.len() as u32;
		self.lines.push(RawLine {
			range: self.line_start..end,
			caret_end: end,
			soft: false,
		});
	}

	/// Place the unbreakable unit at `cursor` and return where the next one
	/// starts. A unit that does not fit wraps to a new line first, and a
	/// unit wider than a whole line is cut at a grapheme boundary instead,
	/// leaving its rest to the next call.
	fn place_unit(&mut self, cursor: Cursor) -> Cursor {
		let last = self.unit_end(cursor.item);
		let (width, hang) = self.measure_unit(cursor, last, self.x);
		let has_content = self.x > 0.0 || cursor.offset > self.line_start;

		if has_content && self.x + width - hang > self.available + EPSILON {
			self.finish(cursor.offset, cursor.offset, true);

			return cursor;
		}

		let overflows = !has_content && width - hang > self.available + EPSILON;

		if let Some(cut) = overflows
			.then(|| self.emergency_cut(cursor, last))
			.flatten()
		{
			self.finish(cut, cut, true);

			return self.cursor_at(cursor.item, cut);
		}

		self.x += width;
		let item = &self.items[last];
		let end = item.range.end;

		if item.break_after == BreakAfter::Hard {
			self.finish(end, item.range.start, false);
		}

		Cursor {
			item: last + 1,
			offset: end,
		}
	}

	fn finish(&mut self, end: u32, caret_end: u32, soft: bool) {
		self.lines.push(RawLine {
			range: self.line_start..end,
			caret_end,
			soft,
		});
		self.line_start = end;
		self.x = 0.0;
	}

	/// The index of the last item of the unbreakable unit starting at `first`.
	fn unit_end(&self, first: usize) -> usize {
		self.items[first..]
			.iter()
			.position(|item| item.break_after != BreakAfter::None)
			.map_or(self.items.len() - 1, |index| first + index)
	}

	fn cursor_at(&self, from: usize, offset: u32) -> Cursor {
		let item = self.items[from..]
			.iter()
			.position(|item| item.range.end > offset)
			.map_or(self.items.len(), |index| from + index);

		Cursor { item, offset }
	}

	/// The width of the unit from `cursor` through item `last`, and how much
	/// of that is hanging white space. Lays out tabs starting at `x`.
	fn measure_unit(&mut self, cursor: Cursor, last: usize, x: f32) -> (f32, f32) {
		let mut width = 0.0;

		for item in &mut self.items[cursor.item..=last] {
			if item.kind == ItemKind::Tab {
				item.width = self.tabs.width_at(x + width);
			}

			width += item.span(self.text, cursor.offset, item.range.end);
		}

		let mut hang = 0.0;

		for item in self.items[cursor.item..=last].iter().rev() {
			if item.is_space(self.text) {
				hang += item.span(self.text, cursor.offset, item.range.end);
				continue;
			}

			hang += item.trailing_space(self.text, cursor.offset);
			break;
		}

		(width, hang)
	}

	/// The furthest grapheme boundary after `cursor` that keeps the line
	/// within the available width, taking at least one grapheme. `None` when
	/// the unit is a single grapheme.
	fn emergency_cut(&self, cursor: Cursor, last: usize) -> Option<u32> {
		let unit_end = self.items[last].range.end;
		let mut width = 0.0;
		let mut cut = None;
		let steps = self.items[cursor.item..=last]
			.iter()
			.flat_map(|item| self.cut_steps(item, cursor.offset));

		for (item, step) in steps {
			width += item.span(self.text, step.start, step.end);

			let fits = width <= self.available + EPSILON;

			if (fits || cut.is_none()) && step.end < unit_end {
				cut = Some(step.end);
			}

			if !fits {
				return cut;
			}
		}

		cut
	}

	/// The ranges between the places an emergency cut may fall in `item`
	/// from `offset` on: every grapheme boundary of text, but only the end
	/// of an atomic item.
	fn cut_steps<'item>(
		&self,
		item: &'item Item,
		offset: u32,
	) -> impl Iterator<Item = (&'item Item, Range<u32>)> {
		let start = item.range.start.max(offset);

		let boundaries = if matches!(item.kind, ItemKind::Text { .. }) {
			let local = slice(self.text, &(start..item.range.end));
			grapheme_boundaries(local)
				.into_iter()
				.skip(1)
				.map(|boundary| start + boundary)
				.collect()
		} else {
			vec![item.range.end]
		};

		boundaries
			.into_iter()
			.scan(start, move |previous, boundary| {
				let step = *previous..boundary;
				*previous = boundary;
				Some((item, step))
			})
	}
}

#[cfg(test)]
#[allow(
	clippy::float_cmp,
	reason = "the values compared are exactly representable"
)]
mod tests {
	use std::sync::Arc;

	use super::*;
	use crate::FontId;
	use crate::measure::Measured;

	/// Items for `text` split after spaces, every char 10 px wide.
	fn items(text: &str) -> Vec<Item> {
		let mut items = Vec::new();
		let mut start = 0;

		for (index, c) in text.char_indices() {
			let end = index + c.len_utf8();
			let boundary = c == ' ' && !text[end..].starts_with(' ');

			if boundary || end == text.len() {
				items.push(item(text, start..end));
				start = end;
			}
		}

		items
	}

	fn item(text: &str, range: Range<usize>) -> Item {
		let count = text[range.clone()].chars().count();
		let advances = vec![10.0; count].into_boxed_slice();
		Item {
			range: range.start as u32..range.end as u32,
			style: 0,
			level: 0,
			kind: ItemKind::Text {
				font: FontId(0),
				measured: Arc::new(Measured {
					width: 10.0 * count as f32,
					advances,
				}),
			},
			width: 10.0 * count as f32,
			kern: 0.0,
			break_after: BreakAfter::Soft,
		}
	}

	fn lines(text: &str, available: f32) -> Vec<&str> {
		let mut items = items(text);
		let tabs = TabStops {
			interval: 80.0,
			space: 10.0,
		};
		break_lines(text, &mut items, available, tabs)
			.into_iter()
			.map(|line| &text[line.range.start as usize..line.range.end as usize])
			.collect()
	}

	#[test]
	fn wraps_greedily_with_hanging_spaces() {
		assert_eq!(lines("aaa bbb ccc", 70.0), ["aaa bbb ", "ccc"]);
		assert_eq!(lines("aaa bbb   ccc", 70.0), ["aaa bbb   ", "ccc"]);
		assert_eq!(lines("aaa bbb", 1000.0), ["aaa bbb"]);
	}

	#[test]
	fn splits_words_wider_than_the_line() {
		assert_eq!(lines("abcdefgh ij", 30.0), ["abc", "def", "gh ", "ij"]);
		assert_eq!(lines("x", 1.0), ["x"]);
	}

	#[test]
	fn empty_text_has_one_line() {
		assert_eq!(lines("", 100.0), [""]);
	}

	#[test]
	fn tab_stops_skip_close_stops() {
		let tabs = TabStops {
			interval: 80.0,
			space: 10.0,
		};

		assert_eq!(tabs.width_at(0.0), 80.0);
		assert_eq!(tabs.width_at(30.0), 50.0);
		assert_eq!(tabs.width_at(76.0), 84.0);
	}
}
