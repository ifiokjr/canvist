//! Slices to Markdown (CommonMark with GitHub's strikethrough and task
//! lists).

use canvist_editor::Slice;
use canvist_model::Attrs;
use canvist_model::BlockTree;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::Schema;
use canvist_model::TreeContent;
use canvist_model::Value;

use super::tree::HARD_BREAK;

/// Serialize a slice to Markdown. Highlights use `==`, and subscripts and
/// superscripts the inline `<sub>` and `<sup>` tags GitHub renders.
/// Underline and text styles have no Markdown form, so their text is
/// written plain; unknown blocks are written as their content.
#[must_use]
pub fn to_markdown(schema: &Schema, slice: &Slice) -> String {
	blocks(schema, slice.blocks(), "\n\n")
}

fn blocks(schema: &Schema, trees: &[BlockTree], separator: &str) -> String {
	rendered(schema, trees)
		.into_iter()
		.map(|(text, _)| text)
		.collect::<Vec<_>>()
		.join(separator)
}

/// The non-empty Markdown of each block. Adjacent lists of one family
/// alternate their marker (`-` and `*`, `.` and `)`), which is how
/// CommonMark keeps them apart instead of merging them into one list.
fn rendered<'t>(schema: &Schema, trees: &'t [BlockTree]) -> Vec<(String, &'t BlockTree)> {
	let mut out = Vec::new();
	let mut previous = None;

	for tree in trees {
		let marker =
			list_markers(schema.block(tree.kind).name.as_ref()).map(|(main, alternate)| {
				if previous == Some(main) {
					alternate
				} else {
					main
				}
			});
		let text = block(schema, tree, marker.unwrap_or('-'));

		if !text.is_empty() {
			previous = marker;
			out.push((text, tree));
		}
	}

	out
}

/// The default and alternate marker for a list type.
fn list_markers(name: &str) -> Option<(char, char)> {
	match name {
		"bulletList" | "taskList" => Some(('-', '*')),
		"orderedList" => Some(('.', ')')),
		_ => None,
	}
}

fn block(schema: &Schema, tree: &BlockTree, marker: char) -> String {
	let name = schema.block(tree.kind).name.as_ref();

	match &tree.content {
		TreeContent::Inline(text) => text_block(schema, name, &tree.attrs, text),
		TreeContent::Blocks(children) => container(schema, name, &tree.attrs, children, marker),

		TreeContent::Leaf if name == "horizontalRule" => "---".to_owned(),
		TreeContent::Leaf => String::new(),
	}
}

/// The deepest heading Markdown has.
const MAX_HEADING: i64 = 6;

/// Headings with their `#`s, code blocks fenced (with `~~~` when the code
/// itself holds a backtick fence), and paragraphs escaped line by line.
fn text_block(schema: &Schema, name: &str, attrs: &Attrs, text: &InlineText) -> String {
	match name {
		"heading" => {
			let level = attrs
				.get("level")
				.and_then(Value::as_int)
				.unwrap_or(1)
				.clamp(1, MAX_HEADING);
			let content = inline(schema, text);
			// A trailing `#` would read as the optional closing sequence.
			let content = match content.strip_suffix('#') {
				Some(head) => format!("{head}\\#"),
				None => content,
			};

			format!("{} {content}", "#".repeat(level as usize))
		}
		"codeBlock" => {
			let language = attrs
				.get("language")
				.and_then(Value::as_str)
				.unwrap_or_default();
			let code = super::tree::plain_text(schema, text);
			let fence = if code.contains("```") { "~~~" } else { "```" };
			format!("{fence}{language}\n{code}\n{fence}")
		}
		// Hard breaks start new lines, and each must not read as a block.
		_ => {
			inline(schema, text)
				.split('\n')
				.map(escape_line_start)
				.collect::<Vec<_>>()
				.join("\n")
		}
	}
}

fn container(
	schema: &Schema,
	name: &str,
	attrs: &Attrs,
	children: &[BlockTree],
	marker: char,
) -> String {
	match name {
		"blockquote" => prefix_lines(&blocks(schema, children, "\n\n"), "> ", ">"),
		"bulletList" => list(schema, children, |_| format!("{marker} ")),
		"taskList" => {
			list(schema, children, |item| {
				let checked = item.attrs.get("checked").and_then(Value::as_bool) == Some(true);
				let box_ = if checked { "[x]" } else { "[ ]" };
				format!("{marker} {box_} ")
			})
		}
		"orderedList" => {
			let start = attrs.get("start").and_then(Value::as_int).unwrap_or(1);
			let mut number = start;
			list(schema, children, move |_| {
				let item_marker = format!("{number}{marker} ");
				number += 1;
				item_marker
			})
		}
		_ => blocks(schema, children, "\n\n"),
	}
}

/// Write list items, each starting with its marker and with continuation
/// lines indented to line up under the item's text.
fn list(
	schema: &Schema,
	items: &[BlockTree],
	mut marker: impl FnMut(&BlockTree) -> String,
) -> String {
	let mut lines = Vec::new();

	for item in items {
		let marker = marker(item);
		// Continuation lines align with the item's content, which starts
		// before a task checkbox: the checkbox is part of the content.
		let width = marker.find('[').unwrap_or(marker.len());
		let indent = " ".repeat(width);

		let content = match &item.content {
			TreeContent::Blocks(children) => item_content(schema, children),
			TreeContent::Inline(_) | TreeContent::Leaf => block(schema, item, '-'),
		};

		let mut content_lines = content.lines();
		let first = content_lines.next().unwrap_or_default();
		lines.push(format!("{marker}{first}"));

		lines.extend(content_lines.map(|line| prefix_line(line, &indent, "")));
	}

	lines.join("\n")
}

/// An item's blocks: a nested list follows its paragraph on the next
/// line, while further paragraphs need a blank line between them.
fn item_content(schema: &Schema, children: &[BlockTree]) -> String {
	let mut content = String::new();

	for (index, (text, child)) in rendered(schema, children).into_iter().enumerate() {
		if index > 0 {
			let nested = matches!(child.content, TreeContent::Blocks(_));

			content.push_str(if nested { "\n" } else { "\n\n" });
		}

		content.push_str(&text);
	}

	content
}

/// Prefix every line, using `empty` for blank lines.
fn prefix_lines(text: &str, prefix: &str, empty: &str) -> String {
	text.lines()
		.map(|line| prefix_line(line, prefix, empty))
		.collect::<Vec<_>>()
		.join("\n")
}

fn prefix_line(line: &str, prefix: &str, empty: &str) -> String {
	if line.is_empty() {
		return empty.to_owned();
	}

	format!("{prefix}{line}")
}

/// Escape text that would otherwise open a heading, quote, list, fence,
/// or rule: the marker's first character, or an ordered list's delimiter.
fn escape_line_start(text: &str) -> String {
	if !super::markdown_read::starts_block(text) {
		return text.to_owned();
	}

	let indent = text.len() - text.trim_start_matches(' ').len();
	let digits = text[indent..]
		.chars()
		.take_while(char::is_ascii_digit)
		.count();
	let marker_end = indent + digits;

	format!("{}\\{}", &text[..marker_end], &text[marker_end..])
}

/// The inline HTML tags the Markdown reader understands, which literal text
/// must not spell out.
const INLINE_TAGS: [&str; 4] = ["<sub>", "</sub>", "<sup>", "</sup>"];

/// Escape characters with inline meaning: emphasis and code delimiters,
/// brackets, `==` (highlight), and `<` when it would open a tag the reader
/// understands.
fn escape(text: &str) -> String {
	let mut escaped = String::with_capacity(text.len());

	for (index, character) in text.char_indices() {
		let rest = &text[index..];
		let delimiter = matches!(character, '\\' | '*' | '_' | '`' | '[' | ']' | '~');
		let highlight = cfg!(feature = "highlight") && rest.starts_with("==");
		let tag = cfg!(any(feature = "subscript", feature = "superscript"))
			&& INLINE_TAGS.iter().any(|tag| rest.starts_with(tag));

		if delimiter || highlight || tag {
			escaped.push('\\');
		}

		escaped.push(character);
	}

	escaped
}

/// Whether Markdown writes `a` and `b` the same way: the same kind, and
/// for a link the same `href`. Any other attribute, such as a highlight's
/// colour, has no Markdown form, so marks that differ only in it share one
/// span: `==a b==`, not `==a====b==`, which reads back as one highlight
/// anyway.
fn same_in_markdown(schema: &Schema, a: &Mark, b: &Mark) -> bool {
	a.kind == b.kind && (a.attrs == b.attrs || schema.mark(a.kind).name.as_ref() != "link")
}

/// The delimiters around text with `mark`, or `None` for marks Markdown
/// cannot express.
fn delimiters(schema: &Schema, mark: &Mark) -> Option<(String, String)> {
	let pair = |delimiter: &str| Some((delimiter.to_owned(), delimiter.to_owned()));

	match schema.mark(mark.kind).name.as_ref() {
		"bold" => pair("**"),
		"italic" => pair("*"),
		"strike" => pair("~~"),
		"code" => pair("`"),
		"highlight" if cfg!(feature = "highlight") => pair("=="),
		"subscript" if cfg!(feature = "subscript") => {
			Some(("<sub>".to_owned(), "</sub>".to_owned()))
		}
		"superscript" if cfg!(feature = "superscript") => {
			Some(("<sup>".to_owned(), "</sup>".to_owned()))
		}
		"link" => {
			let href = mark
				.attrs
				.get("href")
				.and_then(Value::as_str)
				.unwrap_or_default();
			Some(("[".to_owned(), format!("]({href})")))
		}
		_ => None,
	}
}

/// A run's text split into leading whitespace, content, and trailing
/// whitespace, so whitespace can sit outside emphasis delimiters, where
/// Markdown requires it.
fn split_whitespace(text: &str) -> (&str, &str, &str) {
	let core_start = text.len() - text.trim_start().len();
	let core_end = text.trim_end().len().max(core_start);
	(
		&text[..core_start],
		&text[core_start..core_end],
		&text[core_end..],
	)
}

/// Builds inline Markdown run by run. Marks that continue across runs
/// stay open, marks closing out of order close (and reopen) the marks
/// opened after them so delimiters always nest, and whitespace at the edge
/// of formatted text moves outside the delimiters, where Markdown needs it.
struct InlineWriter<'s> {
	schema: &'s Schema,
	out: String,
	open: Vec<(Mark, String)>,
	/// Trailing whitespace held back until the marks around it close.
	pending: String,
}

impl InlineWriter<'_> {
	/// Close every open mark that `wanted` does not keep, innermost first.
	fn close_unwanted(&mut self, wanted: &[&Mark]) {
		let keep = self
			.open
			.iter()
			.take_while(|(mark, _)| {
				wanted
					.iter()
					.any(|wanted| same_in_markdown(self.schema, mark, wanted))
			})
			.count();

		for (_, close) in self.open.drain(keep..).rev() {
			self.out.push_str(&close);
		}
	}

	fn in_code(&self) -> bool {
		self.open
			.iter()
			.any(|(mark, _)| self.schema.mark(mark.kind).name.as_ref() == "code")
	}

	/// Write one run's text with `marks`. `literal` text (hard breaks) is
	/// written as is and never sits inside a mark.
	fn write(&mut self, text: &str, marks: &[Mark], literal: bool) {
		let (lead, core, trail) = split_whitespace(text);

		if core.is_empty() {
			self.pending.push_str(lead);
			self.pending.push_str(trail);

			return;
		}

		let wanted: Vec<&Mark> = marks
			.iter()
			.filter(|mark| !literal && delimiters(self.schema, mark).is_some())
			.collect();
		self.close_unwanted(&wanted);
		self.out.push_str(&self.pending);
		self.out.push_str(lead);
		trail.clone_into(&mut self.pending);
		open_marks(self.schema, &wanted, core, &mut self.open, &mut self.out);

		let escaped = if literal || self.in_code() {
			core.to_owned()
		} else {
			escape(core)
		};

		self.out.push_str(&escaped);
	}

	fn finish(mut self) -> String {
		self.close_unwanted(&[]);
		self.out.push_str(&self.pending);
		self.out
	}
}

/// Inline text as Markdown. Markdown cannot break a line inside emphasis
/// or code, so a hard break closes every mark and the text after it
/// reopens them.
fn inline(schema: &Schema, text: &InlineText) -> String {
	let mut writer = InlineWriter {
		schema,
		out: String::new(),
		open: Vec::new(),
		pending: String::new(),
	};

	for run in text.runs() {
		match run.embed {
			Some(embed) if schema.embed(embed.kind).name.as_ref() == HARD_BREAK => {
				writer.write("\\\n", &[], true);
			}

			Some(_) => {}
			None => writer.write(run.text, run.marks.as_slice(), false),
		}
	}

	writer.finish()
}

/// Open the delimiters for every wanted mark that is not open yet. Code
/// delimiters are sized to `text`, so code containing backticks survives.
fn open_marks(
	schema: &Schema,
	wanted: &[&Mark],
	text: &str,
	open: &mut Vec<(Mark, String)>,
	out: &mut String,
) {
	for mark in wanted {
		if open
			.iter()
			.any(|(existing, _)| same_in_markdown(schema, existing, mark))
		{
			continue;
		}

		let delimiters = if schema.mark(mark.kind).name.as_ref() == "code" {
			Some(code_fence(text))
		} else {
			delimiters(schema, mark)
		};

		if let Some((start, close)) = delimiters {
			out.push_str(&start);
			open.push(((*mark).clone(), close));
		}
	}
}

/// Backticks longer than any run inside `code`, padded with a space when
/// the code itself starts or ends with a backtick.
fn code_fence(code: &str) -> (String, String) {
	let longest = code
		.split(|character| character != '`')
		.map(str::len)
		.max()
		.unwrap_or(0);
	let fence = "`".repeat(longest + 1);
	let pad = if code.starts_with('`') || code.ends_with('`') {
		" "
	} else {
		""
	};

	(format!("{fence}{pad}"), format!("{pad}{fence}"))
}
