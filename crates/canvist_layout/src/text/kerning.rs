//! Kerning between items.
//!
//! Layout measures each item on its own: a word with its trailing spaces,
//! or a single character in scripts such as Japanese that may break between
//! any two. Browsers shape a whole run of one font, so a kerning pair that
//! straddles two items (two kana, or letters either side of a mark that
//! keeps the font) moves every glyph after it. A shaper adds a pair's
//! adjustment to the first glyph's advance; layout does the same, measuring
//! each pair as the difference between the pair and its two halves.
//!
//! An item ends with its trailing spaces, so the pair of a space and the
//! letter after it straddles two items as well. Fonts with Times's metrics
//! kern that pair (a space before `A`, `T`, or `Y`), and browsers shape
//! across the space when a font does; for every other font the pair
//! measures as nothing. Items separated by any other white space, a font
//! change, or a bidi level change are shaped apart in the browser too, so
//! they are never kerned.

use std::ops::Range;
use std::sync::Arc;

use super::items::Item;
use super::items::ItemKind;
use super::items::grapheme_boundaries;
use super::items::slice;
use crate::FontId;
use crate::LayoutError;
use crate::measure::MeasureCache;
use crate::measure::Measured;

/// The last grapheme of `left` and the first of `right`, when the two touch:
/// nothing but a space (which fonts can kern) between them.
pub(crate) fn touching<'t>(
	text: &'t str,
	left: &Range<u32>,
	right: &Range<u32>,
) -> Option<(&'t str, &'t str)> {
	let left = slice(text, left);
	let right = slice(text, right);
	let ends_apart = left
		.chars()
		.next_back()
		.is_none_or(|last| last.is_whitespace() && !is_space(last));
	let starts_apart = right.chars().next().is_none_or(char::is_whitespace);

	if ends_apart || starts_apart {
		return None;
	}

	let last = *grapheme_boundaries(left).iter().rev().nth(1)? as usize;
	let first = *grapheme_boundaries(right).get(1)? as usize;

	Some((&left[last..], &right[..first]))
}

/// A space a font can kern against the next letter: not a tab or a break.
fn is_space(character: char) -> bool {
	matches!(character, ' ' | '\u{a0}')
}

/// Queue the measurements the pair `a`, `b` needs.
pub(crate) fn request(
	cache: &mut MeasureCache,
	font: FontId,
	(a, b): (&str, &str),
	joined: &mut String,
) {
	joined.clear();
	joined.push_str(a);
	joined.push_str(b);
	cache.request(font, a);
	cache.request(font, b);
	cache.request(font, joined);
}

/// The kerning adjustment between `a` and `b`, once measured.
fn kern(
	cache: &MeasureCache,
	font: FontId,
	(a, b): (&str, &str),
	joined: &mut String,
) -> Result<f32, LayoutError> {
	joined.clear();
	joined.push_str(a);
	joined.push_str(b);
	let width = |text: &str| {
		cache
			.get(font, text)
			.map(|measured| measured.width)
			.ok_or(LayoutError::MissingMeasurement)
	};

	Ok(width(joined)? - width(a)? - width(b)?)
}

/// The font of a text item.
fn font(item: &Item) -> Option<FontId> {
	match item.kind {
		ItemKind::Text { font, .. } => Some(font),
		_ => None,
	}
}

/// Recompute the kerning between each item in `range` and the item after
/// it. Items keep their adjustment separately, so this is idempotent and
/// can be run again over a re-itemized window.
pub(crate) fn apply(
	text: &str,
	items: &mut [Item],
	range: Range<usize>,
	cache: &MeasureCache,
) -> Result<(), LayoutError> {
	let mut joined = String::new();
	let end = range.end.min(items.len());

	for index in range.start..end {
		let adjustment = match (items.get(index), items.get(index + 1)) {
			(Some(left), Some(right))
				if font(left).is_some()
					&& font(left) == font(right)
					&& left.level == right.level =>
			{
				match (font(left), touching(text, &left.range, &right.range)) {
					(Some(font), Some(pair)) => kern(cache, font, pair, &mut joined)?,
					_ => 0.0,
				}
			}
			_ => 0.0,
		};

		set(text, &mut items[index], adjustment);
	}

	Ok(())
}

/// Give `item` a kerning adjustment of `kern` on its last grapheme.
fn set(text: &str, item: &mut Item, kern: f32) {
	let delta = kern - item.kern;

	if delta == 0.0 {
		return;
	}

	let ItemKind::Text { measured, .. } = &mut item.kind else {
		return;
	};

	let source = slice(text, &item.range);
	let last = grapheme_boundaries(source)
		.iter()
		.rev()
		.nth(1)
		.copied()
		.unwrap_or(0) as usize;
	let index = source[..last].chars().count();
	let mut advances = measured.advances.to_vec();

	if let Some(advance) = advances.get_mut(index) {
		*advance += delta;
	}

	*measured = Arc::new(Measured {
		width: measured.width + delta,
		advances: advances.into_boxed_slice(),
	});
	item.width += delta;
	item.kern = kern;
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pairs_need_touching_text() {
		let text = "キス word next";

		assert_eq!(touching(text, &(0..3), &(3..6)), Some(("キ", "ス")));
		assert_eq!(
			touching(text, &(3..7), &(7..12)),
			Some((" ", "w")),
			"a space kerns with the letter after it"
		);
		assert_eq!(
			touching("a\tb", &(0..2), &(2..3)),
			None,
			"a tab separates them"
		);
		assert_eq!(
			touching("a  b", &(0..1), &(1..4)),
			None,
			"white space starts the right"
		);
		assert_eq!(
			touching("e\u{301}x", &(0..3), &(3..4)),
			Some(("e\u{301}", "x"))
		);
	}
}
