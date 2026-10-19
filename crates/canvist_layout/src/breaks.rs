//! Browser tailorings of UAX #14, for [`LayoutOptions::break_override`].
//!
//! CSS leaves the finer points of line breaking to the browser, and engines
//! disagree around ASCII punctuation. A canvas laid out by
//! [`DocumentLayout`](crate::DocumentLayout) wraps where server-rendered
//! HTML does only when layout applies the same tailoring as the browser that
//! renders the HTML. Firefox and WebKit follow ICU's defaults (WebKit, for
//! example, breaks after the `/` in a URL path), so they need no override.
//!
//! The Chromium table is ported from `parley_engine`'s
//! `CHROMIUM_LINE_BREAK_OVERRIDE` (Apache-2.0 OR MIT), which mirrors Blink's
//! `character_property_data_generator.cc` and `text_break_iterator.cc`.
//!
//! [`LayoutOptions::break_override`]: crate::LayoutOptions::break_override

use std::ops::RangeInclusive;

/// The printable ASCII range the tables cover.
const ALL: RangeInclusive<u8> = b'!'..=0x7f;

/// One row per `before` character: bit `after` of `overridden` marks a pair
/// with an override, and the same bit of `allow` says whether it breaks.
struct Table {
	overridden: [u128; 128],
	allow: [u128; 128],
}

impl Table {
	const fn new() -> Self {
		Self {
			overridden: [0; 128],
			allow: [0; 128],
		}
	}

	/// Override every pair in `before × after`.
	const fn pairs(
		mut self,
		before: RangeInclusive<u8>,
		after: RangeInclusive<u8>,
		allow: bool,
	) -> Self {
		let mut b = *before.start();

		while b <= *before.end() {
			let mut a = *after.start();

			while a <= *after.end() {
				let bit = 1_u128 << a;
				self.overridden[b as usize] |= bit;

				if allow {
					self.allow[b as usize] |= bit;
				} else {
					self.allow[b as usize] &= !bit;
				}

				a += 1;
			}

			b += 1;
		}

		self
	}

	const fn one(c: u8) -> RangeInclusive<u8> {
		c..=c
	}

	fn lookup(&self, before: char, after: char) -> Option<bool> {
		let (b, a) = (before as u32, after as u32);

		if b >= 128 || a >= 128 {
			return None;
		}

		let bit = 1_u128 << a;
		(self.overridden[b as usize] & bit != 0).then(|| self.allow[b as usize] & bit != 0)
	}
}

/// Blink's ASCII pair table, in the order Blink's generator applies it.
static CHROMIUM: Table = Table::new()
	.pairs(ALL, ALL, false)
	.pairs(ALL, Table::one(b'('), true)
	.pairs(ALL, Table::one(b'<'), true)
	.pairs(ALL, Table::one(b'['), true)
	.pairs(ALL, Table::one(b'{'), true)
	.pairs(Table::one(b'-'), ALL, true)
	.pairs(Table::one(b'?'), ALL, true)
	.pairs(Table::one(b'-'), Table::one(b'$'), false)
	.pairs(ALL, Table::one(b'!'), false)
	.pairs(Table::one(b'?'), Table::one(b'"'), false)
	.pairs(Table::one(b'?'), Table::one(b'\''), false)
	.pairs(ALL, Table::one(b')'), false)
	.pairs(ALL, Table::one(b','), false)
	.pairs(ALL, Table::one(b'.'), false)
	.pairs(ALL, Table::one(b'/'), false)
	.pairs(Table::one(b'-'), b'0'..=b'9', false)
	.pairs(ALL, Table::one(b':'), false)
	.pairs(ALL, Table::one(b';'), false)
	.pairs(ALL, Table::one(b'?'), false)
	.pairs(ALL, Table::one(b']'), false)
	.pairs(ALL, Table::one(b'}'), false)
	.pairs(Table::one(b'$'), ALL, false)
	.pairs(Table::one(b'\''), ALL, false)
	.pairs(Table::one(b'('), ALL, false)
	.pairs(Table::one(b'/'), ALL, false)
	.pairs(b'0'..=b'9', ALL, false)
	.pairs(Table::one(b'<'), ALL, false)
	.pairs(Table::one(b'@'), ALL, false)
	.pairs(b'A'..=b'Z', ALL, false)
	.pairs(Table::one(b'['), ALL, false)
	.pairs(b'^'..=b'`', ALL, false)
	.pairs(b'a'..=b'z', ALL, false)
	.pairs(Table::one(b'{'), ALL, false)
	.pairs(Table::one(0x7f), ALL, false);

/// Chromium's (Blink's) line breaking, as a [`BreakOverride`](crate::BreakOverride).
///
/// - A break is always allowed after a run of spaces, even where UAX #14
///   rule LB13 forbids one (before `)` or `!`, for example).
/// - `-` before a digit breaks only after a letter or digit, so `-5` stays
///   whole but `ABC-123` can wrap.
/// - Every other pair of printable ASCII characters follows Blink's table:
///   no breaks inside runs of letters, digits, and most punctuation, but
///   breaks before opening brackets and after `-` and `?`.
pub fn chromium_breaks(before_before: Option<char>, before: char, after: char) -> Option<bool> {
	if before == ' ' && after != ' ' {
		return Some(true);
	}

	if before == '-' && after.is_ascii_digit() {
		return Some(before_before.is_some_and(|c| c.is_ascii_alphanumeric()));
	}

	CHROMIUM.lookup(before, after)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn hyphen_before_digits_depends_on_context() {
		assert_eq!(chromium_breaks(Some('D'), '-', '1'), Some(true));
		assert_eq!(chromium_breaks(Some(' '), '-', '1'), Some(false));
		assert_eq!(chromium_breaks(None, '-', '1'), Some(false));
	}

	#[test]
	fn breaks_after_spaces_unconditionally() {
		for after in ['}', ')', ']', '!', '.', ',', '/', ':', ';', '?', 'b', '('] {
			assert_eq!(chromium_breaks(None, ' ', after), Some(true), "{after:?}");
		}
	}

	#[test]
	fn ascii_punctuation_table() {
		assert_eq!(chromium_breaks(None, 'a', '/'), Some(false));
		assert_eq!(chromium_breaks(None, '/', 'b'), Some(false));
		assert_eq!(chromium_breaks(None, 'a', 'b'), Some(false));
		assert_eq!(chromium_breaks(None, ')', '('), Some(true));
		assert_eq!(chromium_breaks(None, 'a', '('), Some(false));
		assert_eq!(chromium_breaks(None, '-', 'b'), Some(true));
		assert_eq!(chromium_breaks(None, '?', 'b'), Some(true));
		assert_eq!(chromium_breaks(None, '?', '"'), Some(false));
		assert_eq!(chromium_breaks(Some('D'), '-', 'é'), None);
	}
}
