//! Suggestions: the text a trigger character starts before the caret, such
//! as `@ad` while typing a mention, which hosts show a popup for.
//!
//! This is the shared engine behind mentions, hashtags, and emoji, like
//! TipTap's Suggestion utility and Remirror's `prosemirror-suggest`. It only
//! finds the active suggestion and remembers a dismissed one; drawing the
//! popup and fetching its items stays with the host, which positions it at
//! the caret.

#![cfg_attr(
	not(all(feature = "mention", feature = "hashtag", feature = "emoji")),
	allow(dead_code, reason = "each extension uses a different subset")
)]

use std::ops::Range;

use canvist_core::CommandResult;
use canvist_core::EditKind;
use canvist_core::EditorState;
use canvist_core::Field;
use canvist_core::ValueType;
use canvist_core::commands;
use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::EMBED_CHAR;
use canvist_model::InlineText;
use canvist_model::Point;
use canvist_model::Selection;
use canvist_model::Step;
use canvist_model::Value;

/// How far before the caret a suggestion may start, in bytes. Queries are
/// short, and the bound keeps every keystroke's check cheap however long
/// the paragraph is.
const MAX_SUGGESTION: usize = 64;

/// The text block type suggestions never start in.
const CODE_BLOCK: &str = "codeBlock";

/// What starts a suggestion and what its query may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Trigger {
	/// The extension the trigger belongs to, which also names the setting
	/// that remembers a dismissed suggestion.
	pub(crate) name: &'static str,
	/// The character that starts it, such as `@`.
	pub(crate) char: char,
	/// Whether the query may hold spaces, as `@Ada Lovelace` does.
	pub(crate) allow_spaces: bool,
}

/// An active suggestion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Suggestion {
	/// The text block it is in.
	pub(crate) block: BlockId,
	/// The trigger character and the query, as byte offsets in the block.
	pub(crate) range: Range<u32>,
	/// The text after the trigger character.
	pub(crate) query: String,
}

impl Trigger {
	/// The editor setting remembering where a dismissed suggestion started.
	fn dismissed_key(self) -> String {
		format!("{}Dismissed", self.name)
	}

	/// The suggestion the caret is in, unless it was dismissed.
	pub(crate) fn find(self, state: &EditorState) -> Option<Suggestion> {
		let found = self.find_any(state)?;
		let dismissed = state.settings().get(&self.dismissed_key());

		(dismissed != Some(&Value::from(where_starts(&found)))).then_some(found)
	}

	/// The suggestion the caret is in, dismissed or not.
	fn find_any(self, state: &EditorState) -> Option<Suggestion> {
		let Selection::Text { anchor, head } = state.selection() else {
			return None;
		};

		if anchor != head || in_code(state, head.block) {
			return None;
		}

		let text = state.doc().inline(head.block)?;
		let before = before_caret(text, head.offset);
		let start = self.trigger_start(before)?;
		let query = &before[start + self.char.len_utf8()..];
		let offset = head.offset as usize - before.len();

		Some(Suggestion {
			block: head.block,
			range: (offset + start) as u32..head.offset,
			query: query.to_owned(),
		})
	}

	/// Where the last trigger character that starts a valid query sits in
	/// `before`: after white space or at the block's start, followed only
	/// by query characters.
	fn trigger_start(self, before: &str) -> Option<usize> {
		let start = before.rfind(self.char)?;
		let query = &before[start + self.char.len_utf8()..];
		let prefix = before[..start].chars().next_back();
		let valid_prefix = prefix.is_none_or(char::is_whitespace);
		let valid_query = query
			.chars()
			.all(|character| self.allow_spaces || !character.is_whitespace());

		(valid_prefix && valid_query && !query.contains(EMBED_CHAR)).then_some(start)
	}

	/// Stop suggesting at the current trigger until another one starts.
	pub(crate) fn dismiss(self, state: &mut EditorState) -> CommandResult {
		let Some(found) = self.find(state) else {
			return Ok(None);
		};

		commands::set_setting(
			state,
			&self.dismissed_key(),
			Value::from(where_starts(&found)),
		)
	}

	/// The `…Suggestion` query's answer: the trigger and the query, or null.
	pub(crate) fn query_value(self, state: &EditorState) -> Value {
		self.find(state).map_or(Value::Null, |found| {
			let record = Attrs::new()
				.with("char", self.char.to_string())
				.with("query", found.query);
			Value::from(record)
		})
	}
}

/// The type of the `…Suggestion` queries.
pub(crate) fn query_type() -> ValueType {
	ValueType::nullable(ValueType::Record(vec![
		Field::required("char", ValueType::String, "The trigger character."),
		Field::required("query", ValueType::String, "The text typed after it."),
	]))
}

/// Up to [`MAX_SUGGESTION`] bytes of `text` before `offset`, starting on a
/// character boundary.
fn before_caret(text: &InlineText, offset: u32) -> &str {
	let text = text.as_str();
	let end = offset as usize;
	let mut start = end.saturating_sub(MAX_SUGGESTION);

	while !text.is_char_boundary(start) {
		start += 1;
	}

	&text[start..end]
}

fn in_code(state: &EditorState, block: BlockId) -> bool {
	let Some(block) = state.doc().block(block) else {
		return false;
	};

	&*state.schema().block(block.kind()).name == CODE_BLOCK
}

fn where_starts(found: &Suggestion) -> String {
	format!("{}@{}", found.block, found.range.start)
}

/// Replace the suggestion (or insert at the caret, when there is none) with
/// `content`, then a space unless one already follows, and leave the caret
/// after it, as TipTap's mention command does.
pub(crate) fn replace(
	state: &mut EditorState,
	found: Option<Suggestion>,
	content: InlineText,
) -> CommandResult {
	let Some(caret) = state.selection().as_caret() else {
		return Ok(None);
	};
	let (block, range) = found.map_or((caret.block, caret.offset..caret.offset), |found| {
		(found.block, found.range)
	});
	let marks = commands::marks_at(state.doc(), Point::new(block, range.start));
	let spaced = state
		.doc()
		.inline(block)
		.is_some_and(|text| text.as_str()[range.end as usize..].starts_with(' '));

	state.change(EditKind::Typing, |tr| {
		tr.delete_text(block, range.clone())?;
		let mut end = Point::new(block, range.start + content.len());
		tr.step(Step::InsertInline {
			block,
			offset: range.start,
			content,
		})?;

		if spaced {
			end.offset += 1;
		} else {
			end = tr.insert_text(end, " ", marks)?;
		}

		tr.set_selection(Selection::caret(end));
		Ok(true)
	})
}
