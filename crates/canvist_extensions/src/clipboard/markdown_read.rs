//! Markdown to slices: the CommonMark block structure clipboards carry,
//! plus GitHub's task lists and strikethrough.

use canvist_editor::Slice;
use canvist_model::Attrs;
use canvist_model::BlockTree;
use canvist_model::InlineText;
use canvist_model::MarkSet;
use canvist_model::Schema;
use canvist_model::Value;

use super::markdown_inline::parse_inline;
use super::tree::Builder;

/// Parse Markdown into a slice for `schema`: ATX headings, paragraphs,
/// block quotes, fenced code, thematic breaks, and bullet, ordered, and
/// task lists, with inline emphasis, code, links, and hard breaks.
#[must_use]
pub fn from_markdown(schema: &Schema, markdown: &str) -> Slice {
	let normalized = markdown.replace("\r\n", "\n").replace('\r', "\n");
	let lines: Vec<&str> = normalized.lines().collect();
	let mut build = Builder::new(schema);
	let trees = blocks(&mut build, &lines);
	build.finish(trees)
}

/// The width of leading spaces, if it is small enough for a block marker.
fn marker_indent(line: &str) -> Option<usize> {
	let indent = line.len() - line.trim_start_matches(' ').len();
	(indent <= 3).then_some(indent)
}

fn heading(line: &str) -> Option<(usize, &str)> {
	let trimmed = &line[marker_indent(line)?..];
	let level = trimmed
		.chars()
		.take_while(|&character| character == '#')
		.count();
	let rest = &trimmed[level..];

	if !(1..=6).contains(&level) || !(rest.is_empty() || rest.starts_with(' ')) {
		return None;
	}

	let text = rest.trim();
	let without_closing = text.trim_end_matches('#');
	let closes = without_closing.is_empty() || without_closing.ends_with(' ');
	let text = if closes {
		without_closing.trim_end()
	} else {
		text
	};

	Some((level, text))
}

fn is_rule(line: &str) -> bool {
	let Some(indent) = marker_indent(line) else {
		return false;
	};
	let compact: String = line[indent..]
		.chars()
		.filter(|character| *character != ' ')
		.collect();
	let first = compact.chars().next();

	compact.len() >= 3
		&& first.is_some_and(|first| matches!(first, '-' | '*' | '_'))
		&& compact.chars().all(|character| Some(character) == first)
}

/// An opening code fence: its marker and info string.
fn fence(line: &str) -> Option<(String, &str)> {
	let trimmed = &line[marker_indent(line)?..];
	let marker = trimmed
		.chars()
		.next()
		.filter(|&marker| marker == '`' || marker == '~')?;
	let length = trimmed
		.chars()
		.take_while(|&character| character == marker)
		.count();

	let info = trimmed[length..].trim();

	// A backtick fence's info string cannot contain backticks; such a line
	// is an inline code span instead.
	let valid = length >= 3 && !(marker == '`' && info.contains('`'));
	valid.then(|| (marker.to_string().repeat(length), info))
}

fn quote(line: &str) -> Option<&str> {
	let rest = line[marker_indent(line)?..].strip_prefix('>')?;
	Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// A list item's opening line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Item<'a> {
	/// Whether the item is numbered.
	ordered: bool,
	/// The bullet, or an ordered item's delimiter (`.` or `)`). A different
	/// marker starts a new list.
	marker: char,
	/// The ordered item's number.
	number: i64,
	/// Where the item's content starts.
	offset: usize,
	/// The checkbox state, for task items.
	checked: Option<bool>,
	/// The first line's content.
	content: &'a str,
}

fn item(line: &str) -> Option<Item<'_>> {
	let indent = marker_indent(line)?;
	let trimmed = &line[indent..];
	let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
	let (ordered, marker, number, width) = if digits > 0 && digits <= 9 {
		let delimiter = trimmed[digits..]
			.chars()
			.next()
			.filter(|&next| next == '.' || next == ')')?;
		(true, delimiter, trimmed[..digits].parse().ok()?, digits + 1)
	} else {
		let bullet = trimmed
			.chars()
			.next()
			.filter(|&next| matches!(next, '-' | '*' | '+'))?;

		(false, bullet, 1, 1)
	};

	let after = &trimmed[width..];

	if !(after.is_empty() || after.starts_with(' ')) {
		return None;
	}

	let content = after.strip_prefix(' ').unwrap_or(after);
	let (checked, content) = task(content);

	Some(Item {
		ordered,
		marker,
		number,
		offset: indent + width + 1,
		checked,
		content,
	})
}

fn task(content: &str) -> (Option<bool>, &str) {
	for (prefix, checked) in [("[ ] ", false), ("[x] ", true), ("[X] ", true)] {
		if let Some(rest) = content.strip_prefix(prefix) {
			return (Some(checked), rest);
		}
	}

	(None, content)
}

/// Whether `line` would open a block other than a paragraph.
pub(super) fn starts_block(line: &str) -> bool {
	fence(line).is_some()
		|| heading(line).is_some()
		|| is_rule(line)
		|| quote(line).is_some()
		|| item(line).is_some()
}

/// Parse a run of lines into blocks.
fn blocks(build: &mut Builder<'_>, lines: &[&str]) -> Vec<BlockTree> {
	let mut trees = Vec::new();
	let mut paragraph: Vec<&str> = Vec::new();
	let mut index = 0;

	while index < lines.len() {
		let line = lines[index];

		if line.trim().is_empty() {
			flush_paragraph(build, &mut paragraph, &mut trees);
			index += 1;
			continue;
		}

		if !starts_block(line) {
			paragraph.push(line.trim());
			index += 1;
			continue;
		}

		flush_paragraph(build, &mut paragraph, &mut trees);
		let (consumed, block) = block_at(build, &lines[index..]);
		trees.extend(block);
		index += consumed;
	}

	flush_paragraph(build, &mut paragraph, &mut trees);
	trees
}

fn flush_paragraph(build: &mut Builder<'_>, paragraph: &mut Vec<&str>, trees: &mut Vec<BlockTree>) {
	if paragraph.is_empty() {
		return;
	}

	let inline = parse_inline(build, &paragraph.join("\n"));
	trees.push(build.text("paragraph", Attrs::new(), inline));
	paragraph.clear();
}

/// Parse the block starting at `lines[0]`, returning how many lines it
/// used.
fn block_at(build: &mut Builder<'_>, lines: &[&str]) -> (usize, Vec<BlockTree>) {
	let line = lines[0];

	if let Some((marker, info)) = fence(line) {
		return code_block(build, lines, &marker, info);
	}

	if let Some((level, text)) = heading(line) {
		let inline = parse_inline(build, text);
		let attrs = Attrs::new().with("level", level as i64);

		return (1, vec![build.text("heading", attrs, inline)]);
	}

	if is_rule(line) {
		return (1, build.leaf("horizontalRule").into_iter().collect());
	}

	if quote(line).is_some() {
		let inner: Vec<&str> = lines.iter().map_while(|line| quote(line)).collect();
		let children = blocks(build, &inner);
		return (
			inner.len(),
			build.container("blockquote", &Attrs::new(), children),
		);
	}

	list(build, lines)
}

fn code_block(
	build: &mut Builder<'_>,
	lines: &[&str],
	marker: &str,
	info: &str,
) -> (usize, Vec<BlockTree>) {
	let body: Vec<&str> = lines[1..]
		.iter()
		.take_while(|line| line.trim() != marker)
		.copied()
		.collect();
	let closed = lines.len() > body.len() + 1;
	let language = info
		.split_whitespace()
		.next()
		.map_or(Value::Null, Value::from);
	let inline = InlineText::plain(&body.join("\n"), MarkSet::empty());
	let attrs = Attrs::new().with("language", language);
	let block = build.text("codeBlock", attrs, inline);

	(body.len() + 1 + usize::from(closed), vec![block])
}

/// Parse consecutive items of one list.
fn list(build: &mut Builder<'_>, lines: &[&str]) -> (usize, Vec<BlockTree>) {
	let Some(first) = item(lines[0]) else {
		return (1, Vec::new());
	};
	let mut index = 0;
	let mut items = Vec::new();

	while let Some(opening) = lines.get(index).and_then(|line| item(line)) {
		if (opening.ordered, opening.marker) != (first.ordered, first.marker) {
			break;
		}

		let body = item_lines(&lines[index..], opening);
		index += body.len();
		let mut content = vec![opening.content];
		content.extend(
			body.iter()
				.skip(1)
				.map(|line| strip_indent(line, opening.offset)),
		);
		let children = blocks(build, &content);

		let (name, attrs) = match (first.checked, opening.checked) {
			(Some(_), checked) => {
				(
					"taskItem",
					Attrs::new().with("checked", checked.unwrap_or(false)),
				)
			}
			(None, _) => ("listItem", Attrs::new()),
		};

		items.extend(build.container(name, &attrs, children));

		while lines.get(index).is_some_and(|line| line.trim().is_empty()) {
			index += 1;
		}
	}

	let (name, attrs) = match (first.ordered, first.checked) {
		(_, Some(_)) => ("taskList", Attrs::new()),
		(false, None) => ("bulletList", Attrs::new()),
		(true, None) => ("orderedList", Attrs::new().with("start", first.number)),
	};

	(index.max(1), build.container(name, &attrs, items))
}

/// The lines belonging to the item that opens `lines`: indented
/// continuations and blank lines between them, up to the next item or
/// unindented block.
fn item_lines<'a>(lines: &[&'a str], opening: Item<'_>) -> Vec<&'a str> {
	let mut taken = vec![lines[0]];

	for (offset, line) in lines.iter().enumerate().skip(1) {
		let blank = line.trim().is_empty();
		let indent = line.len() - line.trim_start_matches(' ').len();
		let continues = blank || indent >= opening.offset;
		let lazy = !blank
			&& item(line).is_none()
			&& !lines[offset - 1].trim().is_empty()
			&& quote(line).is_none();

		if !(continues || lazy) {
			break;
		}

		taken.push(line);
	}

	while taken.len() > 1 && taken.last().is_some_and(|line| line.trim().is_empty()) {
		taken.pop();
	}

	taken
}

fn strip_indent(line: &str, indent: usize) -> &str {
	let spaces = line.len() - line.trim_start_matches(' ').len();
	&line[spaces.min(indent)..]
}
