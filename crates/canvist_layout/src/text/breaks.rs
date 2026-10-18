//! Line break opportunities: UAX #14 via ICU4X, with an optional host
//! tailoring.

use std::ops::Range;

use icu_segmenter::LineSegmenter;
use icu_segmenter::options::LineBreakOptions;
use icu_segmenter::options::LineBreakStrictness;
use icu_segmenter::options::LineBreakWordOption;

use super::items::is_newline;
use super::items::slice;
use crate::BreakOverride;

/// The line segmenter, configured to match CSS `line-break: strict` and
/// `word-break: normal`, which the HTML contract sets explicitly.
const LINE_SEGMENTER: icu_segmenter::LineSegmenterBorrowed<'static> = {
	let mut options = LineBreakOptions::default();
	options.strictness = Some(LineBreakStrictness::Strict);
	options.word_option = Some(LineBreakWordOption::Normal);
	LineSegmenter::new_for_non_complex_scripts(options)
};

/// Soft wrap opportunities inside `range` of `text`, found by segmenting
/// only that range. The segmenter reports a break at the end of any range it
/// is given, so that one is kept only when the range reaches the text's end.
/// Callers start `range` just after a space, where no break rule looks
/// further back.
pub(crate) fn soft_breaks_in(
	text: &str,
	range: Range<u32>,
	override_fn: Option<BreakOverride>,
) -> Vec<u32> {
	let local = slice(text, &range);
	let reaches_end = range.end as usize == text.len();

	soft_breaks(local, override_fn)
		.into_iter()
		.filter(|offset| reaches_end || *offset < local.len() as u32)
		.map(|offset| range.start + offset)
		.collect()
}

/// Soft wrap opportunities of `text`, excluding 0, including the end.
pub(crate) fn soft_breaks(text: &str, override_fn: Option<BreakOverride>) -> Vec<u32> {
	let mut breaks: Vec<u32> = LINE_SEGMENTER
		.segment_str(text)
		.filter(|offset| *offset > 0)
		.map(|offset| offset as u32)
		.collect();

	if let Some(override_fn) = override_fn {
		apply_override(text, &mut breaks, override_fn);
	}

	breaks
}

/// Apply a host's break override to every pair of adjacent characters.
fn apply_override(text: &str, breaks: &mut Vec<u32>, override_fn: BreakOverride) {
	let mut before_before = None;
	let mut chars = text.char_indices().peekable();
	let mut changed = Vec::new();

	while let Some((_, before)) = chars.next() {
		let Some(&(offset, after)) = chars.peek() else {
			break;
		};

		let decision = (!is_newline(before)).then(|| override_fn(before_before, before, after));

		if let Some(allow) = decision.flatten() {
			changed.push((offset as u32, allow));
		}

		before_before = Some(before);
	}

	for (offset, allow) in changed {
		match (breaks.binary_search(&offset), allow) {
			(Err(index), true) => breaks.insert(index, offset),
			(Ok(index), false) => {
				breaks.remove(index);
			}
			_ => {}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn soft_breaks_follow_uax14() {
		assert_eq!(soft_breaks("hello world", None), vec![6, 11]);
		assert_eq!(soft_breaks("a-b", None), vec![2, 3]);
		assert_eq!(soft_breaks("中文字", None), vec![3, 6, 9]);
		assert_eq!(soft_breaks("", None), Vec::<u32>::new());
	}

	#[test]
	fn overrides_add_and_remove_breaks() {
		fn no_hyphen_breaks(_: Option<char>, before: char, _: char) -> Option<bool> {
			(before == '-').then_some(false)
		}

		fn break_after_slash(_: Option<char>, before: char, _: char) -> Option<bool> {
			(before == '/').then_some(true)
		}

		assert_eq!(soft_breaks("a-b", Some(no_hyphen_breaks)), vec![3]);
		assert_eq!(soft_breaks("a/b", Some(break_after_slash)), vec![2, 3]);
	}
}
