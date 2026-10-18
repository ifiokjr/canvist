//! A compact outline notation for documents and selections, used to write
//! command tests as before/after pairs.
//!
//! ```text
//! heading level=2: Ti|tle
//! bulletList
//!   listItem
//!     paragraph: {bold:one} {@mention id=ifiok}
//! horizontalRule @
//! ```
//!
//! - Two spaces of indentation per nesting level.
//! - `name key=value: text` declares a block; text blocks always have `:`.
//! - In text, `|` is the selection head and `^` the anchor of a range.
//! - `{mark key=value:text}` applies a mark; marks nest.
//! - `{@embed key=value}` is an inline embed.
//! - A trailing ` @` selects a block as an object.

#![allow(dead_code)]

use std::sync::Arc;

use canvist_editor::EditorState;
use canvist_editor::commands;
use canvist_model::Attrs;
use canvist_model::Block;
use canvist_model::BlockId;
use canvist_model::BlockSpec;
use canvist_model::BlockTree;
use canvist_model::Content;
use canvist_model::Document;
use canvist_model::Embed;
use canvist_model::EmbedSpec;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::MarkSpec;
use canvist_model::Point;
use canvist_model::Schema;
use canvist_model::Selection;
use canvist_model::TreeContent;
use canvist_model::Value;

/// A StarterKit-shaped schema with TipTap's names.
pub fn schema() -> Arc<Schema> {
	let item_content = &["paragraph", "bulletList", "orderedList", "taskList"];
	let schema = Schema::builder()
		.block(BlockSpec::text("paragraph"))
		.block(BlockSpec::text("heading").attrs(Attrs::new().with("level", 1)))
		.block(BlockSpec::text("codeBlock"))
		.block(BlockSpec::container("blockquote", &["block"]))
		.block(BlockSpec::container("bulletList", &["listItem"]))
		.block(
			BlockSpec::container("orderedList", &["listItem"]).attrs(Attrs::new().with("start", 1)),
		)
		.block(BlockSpec::container("taskList", &["taskItem"]))
		.block(BlockSpec::container("listItem", item_content).groups(&[]))
		.block(
			BlockSpec::container("taskItem", item_content)
				.groups(&[])
				.attrs(Attrs::new().with("checked", false)),
		)
		.block(BlockSpec::leaf("horizontalRule"))
		.mark(MarkSpec::new("bold"))
		.mark(MarkSpec::new("italic"))
		.mark(MarkSpec::new("code").excludes(&["bold", "italic"]))
		.mark(MarkSpec::new("link").inclusive(false))
		.embed(EmbedSpec::new("mention"))
		.embed(EmbedSpec::new("hardBreak"))
		.build();
	Arc::new(schema.expect("the test schema is valid"))
}

/// Parse an outline into an editor state with its selection.
pub fn state(outline: &str) -> EditorState {
	let schema = schema();
	let mut parser = Parser {
		schema: &schema,
		next_id: 0,
		anchor: None,
		head: None,
		selected: Vec::new(),
	};
	let roots = parser.blocks(&dedent(outline));
	let mut doc = Document::empty(schema.clone(), 1);
	doc.transaction()
		.insert_blocks(None, 0, roots)
		.expect("the outline is valid for the schema");
	let selection = parser.selection();
	let mut state = EditorState::new(doc).expect("outline state");

	if let Some(selection) = selection {
		commands::set_selection(&mut state, selection).expect("valid selection");
	}

	state
}

/// Render a state back into the outline notation.
pub fn render(state: &EditorState) -> String {
	let doc = state.doc();
	let mut out = Vec::new();

	for &root in doc.children(None) {
		render_block(state, root, 0, &mut out);
	}

	out.join("\n")
}

/// Run `command` on the state parsed from `before` and render the result.
pub fn after(
	before: &str,
	command: impl FnOnce(&mut EditorState) -> canvist_editor::CommandResult,
) -> String {
	let mut state = state(before);
	command(&mut state).expect("the command succeeds");
	render(&state)
}

/// Remove the common leading indentation and surrounding blank lines.
pub fn dedent(text: &str) -> String {
	let lines: Vec<&str> = text
		.lines()
		.filter(|line| !line.trim().is_empty())
		.collect();
	let indent = lines
		.iter()
		.map(|line| line.len() - line.trim_start().len())
		.min()
		.unwrap_or(0);

	lines
		.iter()
		.map(|line| line[indent..].trim_end())
		.collect::<Vec<_>>()
		.join("\n")
}

/// The block type named `name`.
pub fn kind(state: &EditorState, name: &str) -> canvist_model::BlockTypeId {
	state.schema().block_type(name).expect("block type exists")
}

/// A mark of the type named `name`.
pub fn mark(state: &EditorState, name: &str) -> Mark {
	Mark::new(state.schema().mark_type(name).expect("mark type exists"))
}

struct Parser<'a> {
	schema: &'a Schema,
	next_id: u32,
	anchor: Option<Point>,
	head: Option<Point>,
	selected: Vec<BlockId>,
}

impl Parser<'_> {
	fn id(&mut self) -> BlockId {
		self.next_id += 1;
		BlockId::new(1, self.next_id)
	}

	fn selection(&self) -> Option<Selection> {
		if let (Some(&first), Some(&last)) = (self.selected.first(), self.selected.last()) {
			return Some(Selection::Blocks {
				anchor: first,
				head: last,
			});
		}

		let head = self.head?;
		Some(Selection::text(self.anchor.unwrap_or(head), head))
	}

	fn blocks(&mut self, outline: &str) -> Vec<BlockTree> {
		let lines: Vec<(usize, &str)> = outline
			.lines()
			.map(|line| ((line.len() - line.trim_start().len()) / 2, line.trim()))
			.collect();
		let mut index = 0;
		self.level(&lines, &mut index, 0)
	}

	fn level(
		&mut self,
		lines: &[(usize, &str)],
		index: &mut usize,
		depth: usize,
	) -> Vec<BlockTree> {
		let mut trees = Vec::new();

		while let Some(&(line_depth, line)) = lines.get(*index) {
			if line_depth < depth {
				break;
			}

			*index += 1;
			let mut tree = self.block(line);

			if let TreeContent::Blocks(children) = &mut tree.content {
				*children = self.level(lines, index, depth + 1);
			}

			trees.push(tree);
		}

		trees
	}

	fn block(&mut self, line: &str) -> BlockTree {
		let id = self.id();

		let (line, selected) = match line.strip_suffix(" @") {
			Some(rest) => (rest, true),
			None => (line, false),
		};

		if selected {
			self.selected.push(id);
		}

		let (head, text) = match line.split_once(':') {
			Some((head, text)) => (head, Some(text.strip_prefix(' ').unwrap_or(text))),
			None => (line, None),
		};

		let mut words = head.split_whitespace();
		let name = words.next().expect("a block type name");
		let kind = self
			.schema
			.block_type(name)
			.unwrap_or_else(|| panic!("unknown block `{name}`"));
		let mut attrs = self.schema.block(kind).attrs.clone();

		for (key, value) in words.filter_map(|word| word.split_once('=')) {
			attrs.set(key, parse_value(value));
		}

		let content = match (text, &self.schema.block(kind).content) {
			(Some(text), _) => TreeContent::Inline(self.inline(id, text)),
			(None, canvist_model::ContentKind::Leaf) => TreeContent::Leaf,
			(None, canvist_model::ContentKind::Inline) => TreeContent::Inline(InlineText::new()),
			(None, canvist_model::ContentKind::Blocks(_)) => TreeContent::Blocks(Vec::new()),
		};

		BlockTree {
			id,
			kind,
			attrs,
			content,
		}
	}

	fn inline(&mut self, block: BlockId, source: &str) -> InlineText {
		let mut inline = InlineParser {
			schema: self.schema,
			text: InlineText::new(),
			marks: Vec::new(),
			opened: Vec::new(),
			buffer: String::new(),
		};
		let mut chars = source.chars().peekable();

		while let Some(next) = chars.next() {
			if !"|^{}".contains(next) {
				inline.buffer.push(next);
				continue;
			}

			inline.flush();
			let offset = Point::new(block, inline.text.len());

			match next {
				'|' => self.head = Some(offset),
				'^' => self.anchor = Some(offset),
				'}' => inline.close(),
				_ => inline.open(&mut chars),
			}
		}

		inline.flush();
		inline.text
	}
}

/// Builds inline text from the `{mark:...}` and `{@embed}` notation.
struct InlineParser<'a> {
	schema: &'a Schema,
	text: InlineText,
	marks: Vec<Mark>,
	/// For each open brace, whether it opened a mark (embeds close at once).
	opened: Vec<bool>,
	buffer: String,
}

impl InlineParser<'_> {
	fn flush(&mut self) {
		if self.buffer.is_empty() {
			return;
		}

		let marks = MarkSet::from_marks(self.marks.clone());
		self.text.append(InlineText::plain(&self.buffer, marks));
		self.buffer.clear();
	}

	fn open(&mut self, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
		let spec: String =
			std::iter::from_fn(|| chars.next_if(|&peek| peek != ':' && peek != '}')).collect();

		if let Some(embed) = spec.strip_prefix('@') {
			let (name, attrs) = parse_head(embed);
			let kind = self.schema.embed_type(name).expect("embed type exists");
			let marks = MarkSet::from_marks(self.marks.clone());
			self.text
				.append(InlineText::embed(Embed { kind, attrs }, marks));
			self.opened.push(false);

			return;
		}

		chars.next();
		let (name, attrs) = parse_head(&spec);
		let kind = self.schema.mark_type(name).expect("mark type exists");
		self.marks.push(Mark::with_attrs(kind, attrs));
		self.opened.push(true);
	}

	fn close(&mut self) {
		if self.opened.pop() == Some(true) {
			self.marks.pop();
		}
	}
}

fn parse_head(head: &str) -> (&str, Attrs) {
	let mut words = head.split_whitespace();
	let name = words.next().unwrap_or_default();
	let attrs = words
		.filter_map(|word| word.split_once('='))
		.map(|(key, value)| (key, parse_value(value)))
		.collect();
	(name, attrs)
}

fn parse_value(value: &str) -> Value {
	if let Ok(number) = value.parse::<i64>() {
		return Value::Int(number);
	}

	match value {
		"true" => Value::Bool(true),
		"false" => Value::Bool(false),
		_ => Value::from(value),
	}
}

fn render_value(value: &Value) -> String {
	match value {
		Value::Null => "null".to_owned(),
		Value::Bool(flag) => flag.to_string(),
		Value::Int(number) => number.to_string(),
		Value::Float(number) => number.to_string(),
		Value::String(text) => text.to_string(),
		Value::Array(items) => format!("{items:?}"),
	}
}

fn render_attrs(attrs: &Attrs, defaults: &Attrs) -> String {
	let mut out = String::new();

	for (key, value) in attrs
		.iter()
		.filter(|(key, value)| defaults.get(key) != Some(value))
	{
		out.push(' ');
		out.push_str(key);
		out.push('=');
		out.push_str(&render_value(value));
	}

	out
}

fn render_block(state: &EditorState, id: BlockId, depth: usize, out: &mut Vec<String>) {
	let doc = state.doc();
	let block = doc.block(id).expect("child ids resolve");
	let spec = state.schema().block(block.kind());
	let mut line = format!("{}{}", "  ".repeat(depth), spec.name);
	line.push_str(&render_attrs(block.attrs(), &spec.attrs));

	if let Content::Inline(text) = block.content() {
		line.push_str(": ");
		line.push_str(&render_inline(state, block, text));
	}

	if let Selection::Blocks { anchor, head } = state.selection() {
		let selected = [anchor, head].contains(&id) || between(doc, anchor, head, id);

		if selected {
			line.push_str(" @");
		}
	}

	out.push(line.trim_end().to_owned());

	for &child in block.children() {
		render_block(state, child, depth + 1, out);
	}
}

fn between(doc: &Document, anchor: BlockId, head: BlockId, id: BlockId) -> bool {
	let parent = doc.block(anchor).and_then(Block::parent);

	if doc.block(id).and_then(Block::parent) != parent {
		return false;
	}

	let (Some(a), Some(h), Some(i)) = (
		doc.index_in_parent(anchor),
		doc.index_in_parent(head),
		doc.index_in_parent(id),
	) else {
		return false;
	};

	a.min(h) <= i && i <= a.max(h)
}

type Markers = std::iter::Peekable<std::vec::IntoIter<(u32, char)>>;

/// Render text runs with mark braces, embeds, and selection markers. Markers
/// at a run boundary go outside the braces, so `^{bold:x}|` reads naturally.
fn render_inline(state: &EditorState, block: &Block, text: &InlineText) -> String {
	let schema = state.schema();
	let mut markers = selection_markers(state, block.id());
	let mut out = String::new();

	for run in text.runs() {
		flush_markers(&mut out, &mut markers, run.range.start);

		for mark in run.marks.as_slice() {
			let attrs = render_attrs(&mark.attrs, &Attrs::new());
			out.push('{');
			out.push_str(&schema.mark(mark.kind).name);
			out.push_str(&attrs);
			out.push(':');
		}

		match run.embed {
			Some(embed) => {
				out.push_str("{@");
				out.push_str(&schema.embed(embed.kind).name);
				out.push_str(&render_attrs(&embed.attrs, &Attrs::new()));
				out.push('}');
			}
			None => push_text(&mut out, &mut markers, run.range.start, run.text),
		}

		out.push_str(&"}".repeat(run.marks.as_slice().len()));
	}

	flush_markers(&mut out, &mut markers, u32::MAX);
	out
}

/// The anchor (`^`) and head (`|`) markers inside `block`, in order.
fn selection_markers(state: &EditorState, block: BlockId) -> Markers {
	let mut markers = Vec::new();

	if let Selection::Text { anchor, head } = state.selection() {
		if anchor != head && anchor.block == block {
			markers.push((anchor.offset, '^'));
		}

		if head.block == block {
			markers.push((head.offset, '|'));
		}
	}

	markers.sort_by_key(|&(offset, marker)| (offset, marker == '|'));
	markers.into_iter().peekable()
}

fn flush_markers(out: &mut String, markers: &mut Markers, upto: u32) {
	while let Some((_, marker)) = markers.next_if(|&(offset, _)| offset <= upto) {
		out.push(marker);
	}
}

fn push_text(out: &mut String, markers: &mut Markers, start: u32, text: &str) {
	for (offset, character) in text.char_indices() {
		flush_markers(out, markers, start + offset as u32);
		out.push(character);
	}
}
