//! Markdown inline syntax: emphasis, strikethrough, highlights, code spans,
//! links, `<sub>` and `<sup>`, escapes, and hard breaks.

use canvist_model::Attrs;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;

use super::tree::Builder;

/// Parse inline Markdown into formatted text. Line breaks are soft (a
/// space) unless the line ends in a backslash or two spaces.
pub(super) fn parse_inline(build: &Builder<'_>, source: &str) -> InlineText {
	let mut parser = InlineParser {
		build,
		text: InlineText::new(),
		marks: Vec::new(),
		buffer: String::new(),
	};
	parser.parse(source);
	parser.flush();
	parser.text
}

struct InlineParser<'b, 's> {
	build: &'b Builder<'s>,
	text: InlineText,
	marks: Vec<Mark>,
	buffer: String,
}

/// Emphasis delimiters, longest first, with the mark each one applies.
const DELIMITERS: [(&str, &str); 6] = [
	("**", "bold"),
	("__", "bold"),
	("~~", "strike"),
	("==", "highlight"),
	("*", "italic"),
	("_", "italic"),
];

/// Inline HTML elements read as marks, with the mark each one applies.
const TAGS: [(&str, &str); 2] = [("sub", "subscript"), ("sup", "superscript")];

impl InlineParser<'_, '_> {
	fn flush(&mut self) {
		if self.buffer.is_empty() {
			return;
		}

		let marks = MarkSet::from_marks(self.marks.clone());
		self.text.append(InlineText::plain(&self.buffer, marks));
		self.buffer.clear();
	}

	/// Parse `source`, consuming one construct at a time.
	fn parse(&mut self, source: &str) {
		let mut rest = source;

		while let Some(next) = rest.chars().next() {
			let consumed = match next {
				'\\' => self.escape(rest),
				'\n' => Some(self.newline()),
				'`' => self.code_span(rest),
				'*' | '_' | '~' | '=' => self.emphasis(rest, source),
				'[' => self.link(rest),
				'<' => self.tag(rest),
				_ => None,
			};

			let consumed = consumed.unwrap_or_else(|| {
				self.buffer.push(next);
				next.len_utf8()
			});
			rest = &rest[consumed..];
		}
	}

	fn hard_break(&mut self) {
		let marks = MarkSet::from_marks(self.marks.clone());
		self.flush();

		match self.build.hard_break(marks) {
			Some(embed) => self.text.append(embed),
			None => self.buffer.push(' '),
		}
	}

	fn escape(&mut self, rest: &str) -> Option<usize> {
		let next = rest[1..].chars().next()?;

		if next == '\n' {
			self.hard_break();

			return Some(2);
		}

		if !next.is_ascii_punctuation() {
			return None;
		}

		self.buffer.push(next);
		Some(1 + next.len_utf8())
	}

	fn newline(&mut self) -> usize {
		if self.buffer.ends_with("  ") {
			let trimmed = self.buffer.trim_end_matches(' ').len();
			self.buffer.truncate(trimmed);
			self.hard_break();
		} else {
			self.buffer.push(' ');
		}

		1
	}

	fn code_span(&mut self, rest: &str) -> Option<usize> {
		let ticks = rest
			.chars()
			.take_while(|&character| character == '`')
			.count();
		let fence = &rest[..ticks];
		let close = rest[ticks..].find(fence)? + ticks;
		let after = rest[close + ticks..].chars().next();

		if after == Some('`') {
			return None;
		}

		let content = &rest[ticks..close];
		let content = content
			.strip_prefix(' ')
			.and_then(|inner| inner.strip_suffix(' '))
			.filter(|inner| !inner.trim().is_empty())
			.unwrap_or(content);
		self.with_mark("code", &Attrs::new(), |parser| {
			parser.buffer.push_str(content);
		});
		Some(close + ticks)
	}

	fn emphasis(&mut self, rest: &str, source: &str) -> Option<usize> {
		// `==` only counts with the highlight extension compiled in. Other
		// delimiters always do, and their marks are dropped when the schema
		// lacks them, so unknown formatting becomes plain text.
		let (delimiter, mark) = DELIMITERS.iter().find(|(delimiter, mark)| {
			rest.starts_with(delimiter) && (cfg!(feature = "highlight") || *mark != "highlight")
		})?;
		let before = source[..source.len() - rest.len()].chars().next_back();
		let inner = &rest[delimiter.len()..];
		let opens = inner
			.chars()
			.next()
			.is_some_and(|first| !first.is_whitespace());
		let intraword = delimiter.starts_with('_') && before.is_some_and(char::is_alphanumeric);

		if !opens || intraword {
			return None;
		}

		let close = closing(inner, delimiter)?;
		self.with_mark(mark, &Attrs::new(), |parser| parser.parse(&inner[..close]));
		Some(delimiter.len() + close + delimiter.len())
	}

	fn link(&mut self, rest: &str) -> Option<usize> {
		let label_end = matching_bracket(rest)?;
		let after = &rest[label_end + 1..];
		let target = after.strip_prefix('(')?;
		let href_end = target.find(')')?;
		let href = target[..href_end].trim();
		let label = &rest[1..label_end];
		let attrs = Attrs::new().with("href", href);
		self.with_mark("link", &attrs, |parser| parser.parse(label));

		Some(label_end + 2 + href_end + 1)
	}

	/// An inline `<sub>` or `<sup>` element and its content.
	fn tag(&mut self, rest: &str) -> Option<usize> {
		let (name, mark) = TAGS.iter().find(|(name, _)| {
			rest[1..].starts_with(name) && rest[1 + name.len()..].starts_with('>')
		})?;
		let open = name.len() + 2;
		let close = format!("</{name}>");
		let end = rest[open..].find(&close)? + open;
		self.with_mark(mark, &Attrs::new(), |parser| parser.parse(&rest[open..end]));

		Some(end + close.len())
	}

	/// Run `body` with mark `name` applied, if the schema has it.
	fn with_mark(&mut self, name: &str, attrs: &Attrs, body: impl FnOnce(&mut Self)) {
		self.flush();
		let mark = self.build.mark(name, attrs);
		let pushed = mark.is_some();
		self.marks.extend(mark);
		body(self);
		self.flush();

		if pushed {
			self.marks.pop();
		}
	}
}

/// Where `delimiter` closes in `text`: not after whitespace, not escaped
/// with a backslash, and for `_` not inside a word.
fn closing(text: &str, delimiter: &str) -> Option<usize> {
	let mut from = 0;

	while let Some(found) = text[from..].find(delimiter) {
		let index = from + found;
		let before = text[..index].chars().next_back();
		let after = text[index + delimiter.len()..].chars().next();
		let flanked = before.is_some_and(|character| !character.is_whitespace());
		let word_after = delimiter.starts_with('_') && after.is_some_and(char::is_alphanumeric);
		let longer =
			after.is_some_and(|character| delimiter.len() == 1 && delimiter.starts_with(character));

		let backslashes = text[..index]
			.chars()
			.rev()
			.take_while(|&character| character == '\\')
			.count();
		let escaped = backslashes % 2 == 1;

		if index > 0 && flanked && !word_after && !longer && !escaped {
			return Some(index);
		}

		// Delimiters are ASCII, so stepping one byte stays on a boundary and
		// finds a closer overlapping a rejected one, as in `\***`.
		from = index + 1;
	}

	None
}

/// The index of the `]` closing the `[` at the start of `text`.
fn matching_bracket(text: &str) -> Option<usize> {
	let mut depth = 0;
	let mut escaped = false;

	for (index, character) in text.char_indices() {
		match (escaped, character) {
			(true, _) => escaped = false,
			(false, '\\') => escaped = true,
			(false, '[') => depth += 1,

			(false, ']') if depth == 1 => return Some(index),
			(false, ']') => depth -= 1,
			_ => {}
		}
	}

	None
}
