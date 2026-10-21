//! Input rules (rewrites triggered while typing) and paste rules
//! (rewrites applied to pasted text).
//!
//! Patterns use [`regex_lite`]: input and paste rules are authored by
//! extensions as ordinary regular expressions, and `regex-lite` keeps the
//! WebAssembly bundle small (it carries no Unicode tables) at the cost of
//! speed that rules do not need. Input rules only run when the typed text
//! ends in whitespace or punctuation, so ordinary typing never pays for
//! them.

use std::ops::Range;
use std::sync::Arc;

use canvist_model::Attrs;
use canvist_model::BlockId;
use canvist_model::BlockTypeId;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkTypeId;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Selection;
use canvist_model::StepError;
use regex_lite::Regex;

use crate::CommandResult;
use crate::EditKind;
use crate::EditorError;
use crate::EditorState;
use crate::commands;
use crate::position::parent_of;
use crate::structure;

/// The most text before the caret an input rule sees, as in TipTap.
pub const RULE_CONTEXT: u32 = 500;

/// Applies an input rule that matched.
pub type RuleHandler = Arc<dyn Fn(&mut EditorState, &RuleMatch) -> CommandResult + Send + Sync>;

/// Where an input rule matched.
///
/// Offsets are byte offsets in the block, counting the typed text as if it
/// had been inserted at the caret. It never is: when a rule applies, the
/// rule decides what the keystroke produces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleMatch {
	/// The text block holding the caret.
	pub block: BlockId,
	/// Where the match starts.
	pub start: u32,
	/// The caret, where the typed text would go.
	pub caret: u32,
	/// The text being typed.
	pub typed: String,
	/// The matched text, including the typed text.
	pub text: String,
	/// The capture groups' ranges; group 0 is the whole match.
	pub groups: Vec<Option<Range<u32>>>,
}

impl RuleMatch {
	/// The text of capture group `index`.
	#[must_use]
	pub fn group(&self, index: usize) -> Option<&str> {
		let range = self.groups.get(index)?.as_ref()?;
		let start = (range.start - self.start) as usize;
		let end = (range.end - self.start) as usize;
		self.text.get(start..end)
	}

	/// Where the match starts once leading whitespace (matched by patterns
	/// like `(?:^|\s)`) is skipped.
	#[must_use]
	pub fn content_start(&self) -> u32 {
		let leading = self.text.len() - self.text.trim_start().len();
		self.start + leading as u32
	}
}

/// What input rules match against: up to [`RULE_CONTEXT`] bytes of the
/// caret's block before the caret, followed by the typed text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleInput {
	/// The text block holding the caret.
	pub block: BlockId,
	/// The block offset where `haystack` starts.
	pub window: u32,
	/// The caret offset.
	pub caret: u32,
	/// The typed text.
	pub typed: String,
	/// The text before the caret followed by the typed text.
	pub haystack: String,
}

impl RuleInput {
	/// The rule input for typing `typed` at `caret`, or `None` when no rule
	/// should run: outside text blocks, and for typed text that ends in a
	/// letter or digit.
	#[must_use]
	pub fn new(state: &EditorState, caret: Point, typed: &str) -> Option<Self> {
		let triggers = typed
			.chars()
			.next_back()
			.is_some_and(|last| !last.is_alphanumeric());
		let text = state.doc().inline(caret.block)?.as_str();

		if !triggers {
			return None;
		}

		let mut window = caret.offset.saturating_sub(RULE_CONTEXT) as usize;

		while !text.is_char_boundary(window) {
			window += 1;
		}

		let before = text.get(window..caret.offset as usize)?;

		Some(Self {
			block: caret.block,
			window: window as u32,
			caret: caret.offset,
			typed: typed.to_owned(),
			haystack: format!("{before}{typed}"),
		})
	}
}

/// Rewrites text as it is typed, e.g. `## ` into a heading.
#[derive(Clone)]
pub struct InputRule {
	pattern: Regex,
	trigger: Trigger,
	/// The typed characters the rule declared it runs on, which override
	/// the trigger read from its pattern.
	on: Option<&'static str>,
	/// Whether the pattern starts with `^`: it can only match from the
	/// start of the block.
	anchored: bool,
	/// Whether the pattern mentions `^` anywhere, as in `(?:^|\s)`: a
	/// shortened haystack must then start at a word boundary.
	uses_start: bool,
	lookback: Lookback,
	handler: RuleHandler,
}

/// How much text before the caret an unanchored rule searches. Regular
/// expressions cost time per byte, so a rule that runs on a keystroke must
/// look at a bounded amount of text whatever the paragraph's length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lookback {
	/// At most this many bytes, widened to the nearest word boundary.
	Bytes(u32),
	/// The word before the caret, for matches that never contain spaces
	/// (such as URLs), however long.
	Word,
}

/// The default [`Lookback`]: enough for Markdown emphasis around a phrase.
pub const DEFAULT_LOOKBACK: Lookback = Lookback::Bytes(64);

/// The typed characters that can complete a rule's match, read from the
/// end of its pattern. Skipping rules that cannot match keeps a keystroke
/// from scanning hundreds of bytes per rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trigger {
	/// The pattern's end is too complex to read; always try it.
	Any,
	/// The pattern ends with `\s`.
	Whitespace,
	/// The pattern ends with this literal character.
	Char(char),
}

impl Trigger {
	/// Read the final atom of a `$`-anchored pattern.
	fn of(pattern: &str) -> Self {
		let body = pattern.strip_suffix('$').unwrap_or(pattern);

		let whitespace = body.ends_with(r"\s") && !body.ends_with(r"\\s");

		if whitespace || body.ends_with(r"[\s\n]") {
			return Self::Whitespace;
		}

		let mut tail = body.chars().rev();
		let (Some(last), before) = (tail.next(), tail.next()) else {
			return Self::Any;
		};
		let escaped = before == Some('\\');

		match (escaped, last) {
			(true, last) if last.is_ascii_punctuation() => Self::Char(last),

			(false, last) if !"\\.^$*+?()[]{}|".contains(last) => Self::Char(last),
			_ => Self::Any,
		}
	}

	fn accepts(self, typed: char) -> bool {
		match self {
			Self::Any => true,
			Self::Whitespace => typed.is_whitespace(),
			Self::Char(expected) => typed == expected,
		}
	}
}

impl std::fmt::Debug for InputRule {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("InputRule")
			.field("pattern", &self.pattern.as_str())
			.finish_non_exhaustive()
	}
}

fn compile(pattern: &str) -> Result<Regex, EditorError> {
	Regex::new(pattern).map_err(|error| {
		EditorError::InvalidPattern {
			pattern: pattern.to_owned(),
			reason: error.to_string(),
		}
	})
}

impl InputRule {
	/// A rule that runs `handler` when `pattern` matches the text before the
	/// caret plus the typed text. The pattern must end with `$`.
	pub fn new(
		pattern: &str,
		handler: impl Fn(&mut EditorState, &RuleMatch) -> CommandResult + Send + Sync + 'static,
	) -> Result<Self, EditorError> {
		Self::with_handler(pattern, Arc::new(handler))
	}

	/// [`InputRule::new`] with the handler already boxed, so every rule
	/// shares one copy of the pattern checks in a WebAssembly build.
	fn with_handler(pattern: &str, handler: RuleHandler) -> Result<Self, EditorError> {
		if !pattern.ends_with('$') {
			return Err(EditorError::InvalidPattern {
				pattern: pattern.to_owned(),
				reason: "input rules must be anchored to the caret with `$`".to_owned(),
			});
		}

		Ok(Self {
			pattern: compile(pattern)?,
			trigger: Trigger::of(pattern),
			on: None,
			anchored: pattern.starts_with('^'),
			uses_start: pattern.contains('^'),
			lookback: DEFAULT_LOOKBACK,
			handler,
		})
	}

	/// Search a different amount of text before the caret. Rules anchored
	/// at the start of the block (`^...$`) ignore this.
	#[must_use]
	pub fn lookback(mut self, lookback: Lookback) -> Self {
		self.lookback = lookback;
		self
	}

	/// Only try the rule when the typed character is one of `chars`. A
	/// pattern ending in a group or class, such as `(")$`, cannot say this
	/// itself, so without it the rule runs on every punctuation keystroke.
	#[must_use]
	pub fn on(mut self, chars: &'static str) -> Self {
		self.on = Some(chars);
		self
	}

	/// Where in the haystack this rule's search starts, or `None` if the
	/// rule cannot match there at all.
	fn search_start(&self, input: &RuleInput) -> Option<usize> {
		if self.anchored {
			return (input.window == 0).then_some(0);
		}

		let before = &input.haystack[..input.haystack.len() - input.typed.len()];

		let mut cut = match self.lookback {
			Lookback::Word => before.rfind(char::is_whitespace).unwrap_or(0),
			Lookback::Bytes(bytes) => before.len().saturating_sub(bytes as usize),
		};

		// A byte count can land inside a multi-byte character.
		while !before.is_char_boundary(cut) {
			cut += 1;
		}

		let at_block_start = cut == 0 && input.window == 0;

		if at_block_start || !self.uses_start {
			return Some(cut);
		}

		// Start on a whitespace character so `^` cannot match mid-word.
		before[cut..]
			.find(char::is_whitespace)
			.map(|offset| cut + offset)
	}

	/// Replace the match (after any leading whitespace) with `replacement`,
	/// e.g. `->` into `→`.
	pub fn text(pattern: &str, replacement: &'static str) -> Result<Self, EditorError> {
		Self::new(pattern, move |state, found| {
			replace_text(state, found, replacement)
		})
	}

	/// Turn the text block into `kind`, removing the matched text, e.g.
	/// `^(#{1,6})\s$` into a heading whose level `attrs` reads from the match.
	pub fn block_type(
		pattern: &str,
		kind: BlockTypeId,
		attrs: impl Fn(&RuleMatch) -> Attrs + Send + Sync + 'static,
	) -> Result<Self, EditorError> {
		Self::new(pattern, move |state, found| {
			set_type(state, found, kind, attrs(found))
		})
	}

	/// Run `wrap` (for example wrapping the block in a list), then remove the
	/// matched text. If `wrap` does not apply, neither does the rule.
	pub fn wrap(
		pattern: &str,
		wrap: impl Fn(&mut EditorState, &RuleMatch) -> CommandResult + Send + Sync + 'static,
	) -> Result<Self, EditorError> {
		Self::new(pattern, move |state, found| {
			let Some(wrapped) = wrap(state, found)? else {
				return Ok(None);
			};
			let removed = remove_match(state, found)?;

			Ok(Some(match removed {
				Some(removed) => wrapped.then(removed),
				None => wrapped,
			}))
		})
	}

	/// Apply mark `kind` to capture group 1 and remove the delimiters around
	/// it, e.g. `**bold**`. Typing continues without the mark.
	pub fn mark(pattern: &str, kind: MarkTypeId) -> Result<Self, EditorError> {
		Self::new(pattern, move |state, found| apply_mark(state, found, kind))
	}

	/// Replace the matched text with an atomic block of `kind`, e.g. `---`
	/// into a horizontal rule.
	pub fn leaf(pattern: &str, kind: BlockTypeId) -> Result<Self, EditorError> {
		Self::new(pattern, move |state, found| insert_leaf(state, found, kind))
	}

	/// Match against the text before the caret plus the typed text.
	#[must_use]
	pub fn find(&self, input: &RuleInput) -> Option<RuleMatch> {
		let last = input.typed.chars().next_back()?;

		let triggered = match self.on {
			Some(chars) => chars.contains(last),
			None => self.trigger.accepts(last),
		};

		if !triggered {
			return None;
		}

		let start = self.search_start(input)?;
		let haystack = &input.haystack[start..];

		// Most rules do not match; answering that is cheaper than capturing.
		if !self.pattern.is_match(haystack) {
			return None;
		}

		let captures = self.pattern.captures(haystack)?;
		let whole = captures.get(0)?;
		let offset = |at: usize| input.window + (start + at) as u32;
		let groups = (0..captures.len())
			.map(|index| {
				captures
					.get(index)
					.map(|group| offset(group.start())..offset(group.end()))
			})
			.collect();

		Some(RuleMatch {
			block: input.block,
			start: offset(whole.start()),
			caret: input.caret,
			typed: input.typed.clone(),
			text: whole.as_str().to_owned(),
			groups,
		})
	}

	/// Run the rule's handler for a match.
	pub fn apply(&self, state: &mut EditorState, found: &RuleMatch) -> CommandResult {
		(self.handler)(state, found)
	}
}

fn caret_at(block: BlockId, offset: u32) -> Selection {
	Selection::caret(Point::new(block, offset))
}

fn replace_text(state: &mut EditorState, found: &RuleMatch, replacement: &str) -> CommandResult {
	let from = found.content_start().min(found.caret);
	let marks = commands::marks_at(state.doc(), Point::new(found.block, from));

	state.change(EditKind::InputRule, |tr| {
		tr.delete_text(found.block, from..found.caret)?;
		let end = tr.insert_text(Point::new(found.block, from), replacement, marks)?;

		tr.set_selection(Selection::caret(end));
		Ok(true)
	})
}

fn set_type(
	state: &mut EditorState,
	found: &RuleMatch,
	kind: BlockTypeId,
	attrs: Attrs,
) -> CommandResult {
	let doc = state.doc();

	if !structure::allows(doc, parent_of(doc, found.block), kind) {
		return Ok(None);
	}

	state.change(EditKind::InputRule, |tr| {
		tr.delete_text(found.block, found.start..found.caret)?;
		tr.set_block(found.block, kind, attrs)?;
		tr.set_selection(caret_at(found.block, found.start));
		Ok(true)
	})
}

fn remove_match(state: &mut EditorState, found: &RuleMatch) -> CommandResult {
	if found.start == found.caret {
		return Ok(None);
	}

	state.change(EditKind::InputRule, |tr| {
		tr.delete_text(found.block, found.start..found.caret)?;
		tr.set_selection(caret_at(found.block, found.start));
		Ok(true)
	})
}

fn apply_mark(state: &mut EditorState, found: &RuleMatch, kind: MarkTypeId) -> CommandResult {
	let Some(content) = found.groups.get(1).cloned().flatten() else {
		return Ok(None);
	};
	let open = found.content_start();

	if content.end > found.caret || content.is_empty() {
		return Ok(None);
	}

	state.change(EditKind::InputRule, |tr| {
		tr.delete_text(found.block, content.end..found.caret)?;
		tr.add_mark(found.block, content.clone(), Mark::new(kind))?;
		tr.delete_text(found.block, open..content.start)?;
		let end = Point::new(found.block, content.end - (content.start - open));
		let marks = commands::marks_at(tr.doc(), end).without(kind);

		tr.set_selection(Selection::caret(end));
		tr.set_stored_marks(Some(marks));
		Ok(true)
	})
}

fn insert_leaf(state: &mut EditorState, found: &RuleMatch, kind: BlockTypeId) -> CommandResult {
	let doc = state.doc();

	if !structure::allows(doc, parent_of(doc, found.block), kind) {
		return Ok(None);
	}

	let removed = remove_match(state, found)?;
	let inserted = commands::insert_leaf(state, kind, Attrs::new())?;

	Ok(match (removed, inserted) {
		(Some(removed), Some(inserted)) => Some(removed.then(inserted)),
		(removed, inserted) => removed.or(inserted),
	})
}

/// Computes a mark's attributes from the matched text, e.g. a link's href.
pub type MarkAttrs = Arc<dyn Fn(&str) -> Attrs + Send + Sync>;

/// Formats matching text in pasted content, e.g. URLs into links or
/// `**bold**` into bold text.
#[derive(Clone)]
pub struct PasteRule {
	pattern: Regex,
	kind: MarkTypeId,
	attrs: MarkAttrs,
}

impl std::fmt::Debug for PasteRule {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("PasteRule")
			.field("pattern", &self.pattern.as_str())
			.field("kind", &self.kind)
			.finish_non_exhaustive()
	}
}

impl PasteRule {
	/// Apply mark `kind` to every match. With a capture group, only group 1
	/// is marked and the rest of the match (its delimiters) is removed.
	pub fn mark(pattern: &str, kind: MarkTypeId) -> Result<Self, EditorError> {
		Self::mark_with(pattern, kind, |_| Attrs::new())
	}

	/// Like [`PasteRule::mark`], with attributes computed from the marked
	/// text.
	pub fn mark_with(
		pattern: &str,
		kind: MarkTypeId,
		attrs: impl Fn(&str) -> Attrs + Send + Sync + 'static,
	) -> Result<Self, EditorError> {
		Ok(Self {
			pattern: compile(pattern)?,
			kind,
			attrs: Arc::new(attrs),
		})
	}

	/// Format every match in `text`.
	pub fn apply(&self, text: &mut InlineText, schema: &Schema) -> Result<(), EditorError> {
		let source = text.as_str().to_owned();
		let matches: Vec<(Range<u32>, Range<u32>)> = self
			.pattern
			.captures_iter(&source)
			.filter_map(|captures| {
				let whole = captures.get(0)?;
				let content = captures.get(1).unwrap_or(whole);
				let leading = whole.as_str().len() - whole.as_str().trim_start().len();
				let open = (whole.start() + leading) as u32;
				Some((
					open..whole.end() as u32,
					content.start() as u32..content.end() as u32,
				))
			})
			.collect();

		for (whole, content) in matches.into_iter().rev() {
			let marked = &source[content.start as usize..content.end as usize];
			let mark = Mark::with_attrs(self.kind, (self.attrs)(marked));
			text.add_mark(content.clone(), &mark, schema)
				.map_err(StepError::from)?;
			text.remove(content.end..whole.end.max(content.end))
				.map_err(StepError::from)?;
			text.remove(whole.start.min(content.start)..content.start)
				.map_err(StepError::from)?;
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn reads_triggers_from_the_end_of_patterns() {
		assert_eq!(Trigger::of(r"^(#{1,6})\s$"), Trigger::Whitespace);
		assert_eq!(Trigger::of(r"^```([a-z]+)?[\s\n]$"), Trigger::Whitespace);
		assert_eq!(Trigger::of(r"(?:^|\s)\*\*([^*]+)\*\*$"), Trigger::Char('*'));
		assert_eq!(Trigger::of(r"(?:^|\s)_([^_]+)_$"), Trigger::Char('_'));
		assert_eq!(Trigger::of(r"->$"), Trigger::Char('>'));
		assert_eq!(Trigger::of(r"^(?:---|___\s)$"), Trigger::Any);
		assert_eq!(Trigger::of(r"a.$"), Trigger::Any);
	}
}
