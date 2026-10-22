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
		|| cfg!(feature = "mathematics") && line.trim() == "$$"
		|| cfg!(feature = "details") && details_open(line).is_some()
		|| html_block(line).is_some()
}

/// The closing tag of an HTML block `line` opens, as Markdown writes a
/// table a pipe table cannot hold and an ink drawing.
fn html_block(line: &str) -> Option<&'static str> {
	let line = line.trim_start();

	if cfg!(feature = "table") && line.starts_with("<table") {
		Some("</table>")
	} else if cfg!(feature = "ink") && line.starts_with("<figure data-type=\"ink\"") {
		Some("</figure>")
	} else {
		None
	}
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

		if let Some((consumed, table)) = cfg!(feature = "table")
			.then(|| pipe_table(build, &lines[index..]))
			.flatten()
		{
			flush_paragraph(build, &mut paragraph, &mut trees);
			trees.extend(table);
			index += consumed;
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

	let source = paragraph.join("\n");
	let image = cfg!(feature = "image")
		.then(|| image(&source))
		.flatten()
		.and_then(|attrs| build.leaf("image", &attrs));
	let tree = image.unwrap_or_else(|| {
		let inline = parse_inline(build, &source);
		build.text("paragraph", Attrs::new(), inline)
	});

	trees.push(tree);
	paragraph.clear();
}

/// The attributes of a paragraph that is only an image, `![alt](src
/// "title")`, which reads as an image block.
fn image(source: &str) -> Option<Attrs> {
	let rest = source.trim().strip_prefix("![")?;
	let (alt, rest) = rest.split_once("](")?;
	let target = rest.strip_suffix(')')?;
	let (src, title) = match target.split_once(char::is_whitespace) {
		Some((src, title)) => (src, Some(title.trim().trim_matches(['"', '\'']))),
		None => (target, None),
	};

	if src.is_empty() || src.contains(char::is_whitespace) || alt.contains(']') {
		return None;
	}

	let text = |text: Option<&str>| {
		text.filter(|text| !text.is_empty())
			.map_or(Value::Null, Value::from)
	};
	Some(
		Attrs::new()
			.with("src", src)
			.with("alt", text(Some(alt)))
			.with("title", text(title)),
	)
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
		return (
			1,
			build
				.leaf("horizontalRule", &Attrs::new())
				.into_iter()
				.collect(),
		);
	}

	if quote(line).is_some() {
		let inner: Vec<&str> = lines.iter().map_while(|line| quote(line)).collect();

		if let Some(callout) = cfg!(feature = "callout")
			.then(|| callout(build, &inner))
			.flatten()
		{
			return (inner.len(), callout);
		}

		let children = blocks(build, &inner);
		return (
			inner.len(),
			build.container("blockquote", &Attrs::new(), children),
		);
	}

	if cfg!(feature = "mathematics") && line.trim() == "$$" {
		return block_math(build, lines);
	}

	if let Some(open) = cfg!(feature = "details")
		.then(|| details_open(line))
		.flatten()
	{
		return details(build, lines, open);
	}

	if let Some(close) = html_block(line) {
		return block_html(build, lines, close);
	}

	list(build, lines)
}

/// A quote that starts with a GitHub alert such as `[!WARNING]`, as a
/// callout of that type, when the schema has callouts.
fn callout(build: &mut Builder<'_>, inner: &[&str]) -> Option<Vec<BlockTree>> {
	build.schema.block_type("callout")?;
	let alert = inner
		.first()?
		.trim()
		.strip_prefix("[!")?
		.strip_suffix(']')?;
	let (kind, _) = super::markdown_write::ALERTS
		.iter()
		.find(|(_, name)| name.eq_ignore_ascii_case(alert))?;
	let children = blocks(build, &inner[1..]);

	Some(build.container("callout", &Attrs::new().with("type", *kind), children))
}

/// A `$$` block, through the next `$$` line, as a block formula.
fn block_math(build: &mut Builder<'_>, lines: &[&str]) -> (usize, Vec<BlockTree>) {
	let close = lines[1..]
		.iter()
		.position(|line| line.trim() == "$$")
		.map_or(lines.len(), |index| index + 1);
	let latex = lines[1..close.min(lines.len())].join("\n");
	let used = (close + 1).min(lines.len());
	let attrs = Attrs::new().with("latex", latex.as_str());
	let tree = build.leaf("blockMath", &attrs).unwrap_or_else(|| {
		let inline = parse_inline(build, &format!("$${latex}$$"));
		build.text("paragraph", Attrs::new(), inline)
	});

	(used, vec![tree])
}

/// An HTML block, through the line holding its `close` tag, read as HTML.
fn block_html(build: &mut Builder<'_>, lines: &[&str], close: &str) -> (usize, Vec<BlockTree>) {
	let used = lines
		.iter()
		.position(|line| line.contains(close))
		.map_or(lines.len(), |index| index + 1);
	let html = lines[..used].join("\n");
	let trees = super::html_read::from_html(build.schema, &html)
		.into_blocks()
		.into_iter()
		.map(|tree| build.adopt(tree))
		.collect();

	(used, trees)
}

/// A GitHub pipe table: a header row, a delimiter row of `---` cells with
/// `:` marking alignment, then body rows until a blank line or another
/// block. Short rows are padded and long ones cut to the header's width,
/// and a row without pipes is one cell, as GitHub reads them.
fn pipe_table(build: &mut Builder<'_>, lines: &[&str]) -> Option<(usize, Vec<BlockTree>)> {
	build.schema.block_type("table")?;
	let header = pipe_cells(lines.first()?)?;
	let alignments = pipe_cells(lines.get(1)?)?
		.into_iter()
		.map(alignment)
		.collect::<Option<Vec<_>>>()?;

	if alignments.len() != header.len() {
		return None;
	}

	let body = lines[2..]
		.iter()
		.take_while(|line| !line.trim().is_empty() && !starts_block(line))
		.count();
	let mut rows = table_row(build, &header, "tableHeader", &alignments);

	for line in &lines[2..2 + body] {
		let mut cells = pipe_cells(line).unwrap_or_else(|| vec![line.trim()]);
		cells.resize(header.len(), "");
		rows.extend(table_row(build, &cells, "tableCell", &alignments));
	}

	Some((2 + body, build.container("table", &Attrs::new(), rows)))
}

/// The cells of a pipe table row, split at unescaped pipes, without the
/// optional pipes at either end. `None` for a line with no pipe.
fn pipe_cells(line: &str) -> Option<Vec<&str>> {
	let trimmed = line.trim();

	if !trimmed.contains('|') {
		return None;
	}

	let inner = trimmed.strip_prefix('|').unwrap_or(trimmed);
	let inner = match inner.strip_suffix('|') {
		Some(rest) if !rest.ends_with('\\') => rest,
		_ => inner,
	};
	let mut cells = Vec::new();
	let mut start = 0;
	let mut escaped = false;

	for (index, character) in inner.char_indices() {
		match (escaped, character) {
			(false, '\\') => escaped = true,
			(false, '|') => {
				cells.push(inner[start..index].trim());
				start = index + 1;
			}
			_ => escaped = false,
		}
	}

	cells.push(inner[start..].trim());
	Some(cells)
}

/// A pipe table column's alignment, from its delimiter cell: the
/// paragraphs' `textAlign`, or `None` for the default.
#[derive(Clone, Copy)]
struct ColumnAlign(Option<&'static str>);

/// A delimiter cell's alignment, or `None` when the cell is not a
/// delimiter.
fn alignment(cell: &str) -> Option<ColumnAlign> {
	let left = cell.starts_with(':');
	let right = cell.ends_with(':') && cell.len() > 1;
	let dashes = cell.trim_start_matches(':').trim_end_matches(':');

	if dashes.is_empty() || !dashes.chars().all(|character| character == '-') {
		return None;
	}

	Some(ColumnAlign(match (left, right) {
		(true, true) => Some("center"),
		(true, false) => Some("left"),
		(false, true) => Some("right"),
		(false, false) => None,
	}))
}

/// One table row of cells of type `kind`, each a paragraph aligned as its
/// column says.
fn table_row(
	build: &mut Builder<'_>,
	cells: &[&str],
	kind: &str,
	alignments: &[ColumnAlign],
) -> Vec<BlockTree> {
	let cells: Vec<BlockTree> = cells
		.iter()
		.zip(alignments)
		.flat_map(|(text, align)| {
			let attrs = align
				.0
				.map_or_else(Attrs::new, |align| Attrs::new().with("textAlign", align));
			let inline = parse_inline(build, text);
			let paragraph = build.text("paragraph", attrs, inline);
			build.container(kind, &Attrs::new(), vec![paragraph])
		})
		.collect();

	build.container("tableRow", &Attrs::new(), cells)
}

/// Whether `line` opens a `<details>` block, and if so whether it is open.
fn details_open(line: &str) -> Option<bool> {
	match line.trim() {
		"<details>" => Some(false),
		"<details open>" => Some(true),
		_ => None,
	}
}

/// A `<details>` block as GitHub writes it: an optional `<summary>` line,
/// then Markdown content, through the `</details>` that closes it, past
/// those of details nested inside.
fn details(build: &mut Builder<'_>, lines: &[&str], open: bool) -> (usize, Vec<BlockTree>) {
	let mut depth = 0_usize;
	let close = lines[1..]
		.iter()
		.position(|line| {
			if details_open(line).is_some() {
				depth += 1;
			} else if line.trim() == "</details>" {
				if depth == 0 {
					return true;
				}

				depth -= 1;
			}

			false
		})
		.map_or(lines.len(), |index| index + 1);
	let inner = &lines[1..close.min(lines.len())];
	let summary_text = inner.first().and_then(|line| {
		line.trim()
			.strip_prefix("<summary>")?
			.strip_suffix("</summary>")
	});
	let body = if summary_text.is_some() {
		&inner[1..]
	} else {
		inner
	};
	// The summary line is HTML, as the writer escapes it, not Markdown.
	let summary_text = super::html_read::decode(summary_text.unwrap_or_default());
	let summary_inline = InlineText::plain(&summary_text, MarkSet::empty());
	let summary = build.text("detailsSummary", Attrs::new(), summary_inline);
	let mut content = blocks(build, body);

	if content.is_empty() {
		content.push(build.text("paragraph", Attrs::new(), InlineText::new()));
	}

	let content = build.container("detailsContent", &Attrs::new(), content);
	let children = std::iter::once(summary).chain(content).collect();
	let used = (close + 1).min(lines.len());

	(
		used,
		build.container("details", &Attrs::new().with("open", open), children),
	)
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
