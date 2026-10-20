//! Find and replace: a search term held beside the document, its matches,
//! and commands that step through and replace them.

use std::ops::Range;

use canvist_editor::Command;
use canvist_editor::CommandResult;
use canvist_editor::CommandSpec;
use canvist_editor::EditKind;
use canvist_editor::EditorError;
use canvist_editor::EditorState;
use canvist_editor::Extension;
use canvist_editor::Field;
use canvist_editor::QuerySpec;
use canvist_editor::Registration;
use canvist_editor::TypedExtension;
use canvist_editor::ValueType;
use canvist_editor::command;
use canvist_editor::commands;
use canvist_editor::position;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Selection;
use canvist_model::Value;

const NAME: &str = "findAndReplace";
const TERM: &str = "searchTerm";
const CASE_SENSITIVE: &str = "searchCaseSensitive";

/// Find and replace, as TipTap's search-and-replace extensions offer it.
///
/// `setSearchTerm` stores the term in the editor's settings, beside the
/// document and outside its history. Matches are found in text blocks, never
/// across them, and case-insensitively unless `setCaseSensitive` says
/// otherwise. `nextSearchResult` and `previousSearchResult` select the next
/// or previous match from the selection, wrapping at the ends; `replace`
/// replaces the selected match and selects the next one, and `replaceAll`
/// replaces every match as one undo step. Hosts draw the matches by passing
/// [`FindAndReplaceHandle::matches`] to the layout's highlights.
#[derive(Clone, Copy, Debug, Default)]
pub struct FindAndReplace;

/// One occurrence of the search term: a range of a text block's text.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SearchMatch {
	/// The text block.
	pub block: BlockId,
	/// The byte range of the match in its text.
	pub range: Range<u32>,
}

impl SearchMatch {
	/// The match as a text selection.
	#[must_use]
	pub fn selection(&self) -> Selection {
		Selection::text(
			Point::new(self.block, self.range.start),
			Point::new(self.block, self.range.end),
		)
	}
}

/// Typed access to [`FindAndReplace`].
#[derive(Clone, Copy, Debug)]
pub struct FindAndReplaceHandle {
	_registration: Registration,
}

#[allow(
	clippy::unused_self,
	reason = "handle methods require proof of registration"
)]
impl FindAndReplaceHandle {
	/// Search for `term`, or stop searching when it is empty.
	#[must_use]
	pub fn set_term(self, term: &str) -> Command {
		set_term(term)
	}

	/// Match case exactly, or ignore it.
	#[must_use]
	pub fn set_case_sensitive(self, case_sensitive: bool) -> Command {
		command(move |state| {
			commands::set_setting(state, CASE_SENSITIVE, Value::Bool(case_sensitive))
		})
	}

	/// Select the next match after the selection.
	#[must_use]
	pub fn next(self) -> Command {
		command(|state| step(state, true))
	}

	/// Select the previous match before the selection.
	#[must_use]
	pub fn previous(self) -> Command {
		command(|state| step(state, false))
	}

	/// Replace the selected match with `replacement` and select the next.
	#[must_use]
	pub fn replace(self, replacement: &str) -> Command {
		replace(replacement)
	}

	/// Replace every match with `replacement`.
	#[must_use]
	pub fn replace_all(self, replacement: &str) -> Command {
		replace_all(replacement)
	}

	/// Every match of the search term, in document order.
	#[must_use]
	pub fn matches(self, state: &EditorState) -> Vec<SearchMatch> {
		matches(state)
	}

	/// The index of the match the selection covers exactly, if any.
	#[must_use]
	pub fn current(self, state: &EditorState) -> Option<usize> {
		current(state, &matches(state))
	}
}

/// The search term, if one is set and not empty.
fn term(state: &EditorState) -> Option<&str> {
	state
		.settings()
		.get(TERM)
		.and_then(Value::as_str)
		.filter(|term| !term.is_empty())
}

fn case_sensitive(state: &EditorState) -> bool {
	state
		.settings()
		.get(CASE_SENSITIVE)
		.and_then(Value::as_bool)
		.unwrap_or(false)
}

/// The character `character` is compared as: itself when matching case,
/// otherwise its first lowercase form.
fn fold(character: char, exact: bool) -> char {
	if exact {
		return character;
	}

	character.to_lowercase().next().unwrap_or(character)
}

/// The non-overlapping byte ranges of `term` in `text`, left to right.
pub(crate) fn find_in(text: &str, term: &str, exact: bool) -> Vec<Range<u32>> {
	let wanted: Vec<char> = term
		.chars()
		.map(|character| fold(character, exact))
		.collect();
	let mut found = Vec::new();
	let mut from = 0;

	while from < text.len() {
		let mut ends = text[from..]
			.char_indices()
			.map(|(offset, character)| (from + offset, character));
		let mut end = from;
		let mut matched = true;

		for &want in &wanted {
			match ends.next() {
				Some((offset, character)) if fold(character, exact) == want => {
					end = offset + character.len_utf8();
				}
				_ => {
					matched = false;
					break;
				}
			}
		}

		if matched && !wanted.is_empty() {
			found.push(from as u32..end as u32);
			from = end;
		} else {
			from += text[from..].chars().next().map_or(1, char::len_utf8);
		}
	}

	found
}

fn matches(state: &EditorState) -> Vec<SearchMatch> {
	let Some(term) = term(state) else {
		return Vec::new();
	};
	let exact = case_sensitive(state);

	state
		.doc()
		.text_blocks()
		.flat_map(|block| {
			let text = block
				.inline()
				.map(canvist_model::InlineText::as_str)
				.unwrap_or_default();
			find_in(text, term, exact).into_iter().map(move |range| {
				SearchMatch {
					block: block.id(),
					range,
				}
			})
		})
		.collect()
}

fn current(state: &EditorState, found: &[SearchMatch]) -> Option<usize> {
	found
		.iter()
		.position(|candidate| candidate.selection() == state.selection())
}

fn set_term(term: &str) -> Command {
	let term = term.to_owned();

	command(move |state| commands::set_setting(state, TERM, Value::from(term.as_str())))
}

/// Select the first match after the selection (`forward`) or the last one
/// before it, wrapping around the document.
fn step(state: &mut EditorState, forward: bool) -> CommandResult {
	let found = matches(state);
	let doc = state.doc();
	let Some(range) = position::text_range(doc, state.selection()) else {
		return Ok(None);
	};
	let target = if forward {
		found
			.iter()
			.find(|candidate| {
				let start = Point::new(candidate.block, candidate.range.start);
				position::compare_points(doc, start, range.to).is_ge()
					&& candidate.selection() != state.selection()
			})
			.or_else(|| found.first())
	} else {
		found
			.iter()
			.rev()
			.find(|candidate| {
				let end = Point::new(candidate.block, candidate.range.end);
				position::compare_points(doc, end, range.from).is_le()
					&& candidate.selection() != state.selection()
			})
			.or_else(|| found.last())
	};
	let Some(selection) = target.map(SearchMatch::selection) else {
		return Ok(None);
	};

	state.change(EditKind::Selection, |tr| {
		tr.set_selection(selection);
		Ok(true)
	})
}

/// Replace one match's text with `replacement`, keeping the marks at its
/// start.
fn replace_match(
	tr: &mut canvist_editor::EditorTransaction<'_>,
	found: &SearchMatch,
	replacement: &str,
) -> Result<Point, EditorError> {
	let marks = commands::marks_at(tr.doc(), Point::new(found.block, found.range.start));
	tr.delete_text(found.block, found.range.clone())?;
	let end = tr.insert_text(
		Point::new(found.block, found.range.start),
		replacement,
		marks,
	)?;
	Ok(end)
}

fn replace(replacement: &str) -> Command {
	let replacement = replacement.to_owned();

	command(move |state| {
		let found = matches(state);
		let Some(index) = current(state, &found) else {
			return step(state, true);
		};
		let target = found[index].clone();

		state.atomic(|state| {
			let replaced = state.change(EditKind::Typing, |tr| {
				let end = replace_match(tr, &target, &replacement)?;
				tr.set_selection(Selection::caret(end));
				Ok(true)
			})?;
			let stepped = step(state, true)?;

			Ok(match (replaced, stepped) {
				(Some(replaced), Some(stepped)) => Some(replaced.then(stepped)),
				(replaced, stepped) => replaced.or(stepped),
			})
		})
	})
}

fn replace_all(replacement: &str) -> Command {
	let replacement = replacement.to_owned();

	command(move |state| {
		let found = matches(state);

		if found.is_empty() {
			return Ok(None);
		}

		let selection = state.selection();

		state.change(EditKind::Structure, |tr| {
			// From the last match back, so replacing one never moves those
			// before it.
			for target in found.iter().rev() {
				replace_match(tr, target, &replacement)?;
			}

			tr.set_selection(position::valid_selection(tr.doc(), selection));
			Ok(true)
		})
	})
}

/// The `searchResults` query's answer.
fn results(state: &EditorState) -> Value {
	let Some(term) = term(state) else {
		return Value::Null;
	};
	let found = matches(state);
	let current = current(state, &found).map_or(Value::Null, |index| Value::Int(index as i64));
	let record = Attrs::new()
		.with("term", term)
		.with("count", found.len() as i64)
		.with("current", current)
		.with("caseSensitive", case_sensitive(state));

	Value::from(record)
}

fn replacement_field() -> Field {
	Field::required(
		"replacement",
		ValueType::String,
		"The text to put in place of a match.",
	)
}

impl Extension for FindAndReplace {
	fn name(&self) -> &'static str {
		NAME
	}

	fn commands(&self, _schema: &Schema) -> Result<Vec<CommandSpec>, EditorError> {
		let term = CommandSpec::new(
			"setSearchTerm",
			"Search for a term, or stop searching when it is empty.",
			|args| Ok(set_term(args.string("term")?)),
		)
		.arg(Field::required(
			"term",
			ValueType::String,
			"The text to find.",
		));
		let case = CommandSpec::new(
			"setCaseSensitive",
			"Match case exactly, or ignore it.",
			|args| {
				let exact = args.bool("caseSensitive")?;
				Ok(command(move |state| {
					commands::set_setting(state, CASE_SENSITIVE, Value::Bool(exact))
				}))
			},
		)
		.arg(Field::required(
			"caseSensitive",
			ValueType::Bool,
			"Whether case must match.",
		));
		let replace_spec = CommandSpec::new(
			"replace",
			"Replace the selected match and select the next, or select the next match.",
			|args| Ok(replace(args.string("replacement")?)),
		)
		.arg(replacement_field());
		let replace_all_spec = CommandSpec::new(
			"replaceAll",
			"Replace every match of the search term.",
			|args| Ok(replace_all(args.string("replacement")?)),
		)
		.arg(replacement_field());

		Ok(vec![
			term,
			case,
			CommandSpec::simple(
				"nextSearchResult",
				"Select the next match after the selection, wrapping at the end.",
				command(|state| step(state, true)),
			),
			CommandSpec::simple(
				"previousSearchResult",
				"Select the previous match before the selection, wrapping at the start.",
				command(|state| step(state, false)),
			),
			replace_spec,
			replace_all_spec,
		])
	}

	fn queries(&self, _schema: &Schema) -> Result<Vec<QuerySpec>, EditorError> {
		Ok(vec![QuerySpec::new(
			"searchResults",
			"The search term, how many matches it has, and which one is selected, or null when \
			 not searching.",
			ValueType::nullable(ValueType::Record(vec![
				Field::required("term", ValueType::String, "The search term."),
				Field::required("count", ValueType::Int, "How many matches there are."),
				Field::required(
					"current",
					ValueType::nullable(ValueType::Int),
					"The index of the selected match, or null.",
				),
				Field::required("caseSensitive", ValueType::Bool, "Whether case must match."),
			])),
			|state, _| Ok(results(state)),
		)])
	}
}

impl TypedExtension for FindAndReplace {
	type Handle = FindAndReplaceHandle;

	fn handle(&self, registration: Registration) -> FindAndReplaceHandle {
		FindAndReplaceHandle {
			_registration: registration,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn finds_every_occurrence_without_overlap() {
		assert_eq!(find_in("aaaa", "aa", true), vec![0..2, 2..4]);
		assert_eq!(
			find_in("Cat cat CAT", "cat", false),
			vec![0..3, 4..7, 8..11]
		);
		assert_eq!(find_in("Cat cat CAT", "cat", true), vec![4..7]);
		assert!(find_in("abc", "", false).is_empty());
	}

	#[test]
	fn offsets_are_bytes_of_the_original_text() {
		assert_eq!(find_in("Ça va, ça va", "ça", false), vec![0..3, 8..11]);
		assert_eq!(find_in("日本語の日本", "日本", true), vec![0..6, 12..18]);
	}
}
