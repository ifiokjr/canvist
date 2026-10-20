//! HTML to slices: a forgiving reader for pasted markup.
//!
//! It tokenizes tags and text, keeps the structure and formatting the
//! StarterKit understands, and treats everything else as transparent, so
//! markup from browsers, word processors, and TipTap all paste sensibly.

use canvist_editor::Slice;
use canvist_model::Attrs;
use canvist_model::BlockTree;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::MarkSet;
use canvist_model::Schema;
use canvist_model::Value;

use super::tree::Builder;

/// Parse pasted HTML into a slice for `schema`.
#[must_use]
pub fn from_html(schema: &Schema, html: &str) -> Slice {
	let mut reader = Reader::new(schema);

	for token in Tokenizer::new(html) {
		reader.token(token);
	}

	reader.finish()
}

/// One piece of markup.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
	Open {
		name: String,
		attrs: Vec<(String, String)>,
	},
	Close(String),
	Text(String),
}

/// Splits HTML into tags and decoded text, skipping comments, doctypes,
/// and the contents of `<script>` and `<style>`.
struct Tokenizer<'a> {
	rest: &'a str,
}

impl<'a> Tokenizer<'a> {
	fn new(html: &'a str) -> Self {
		Self { rest: html }
	}

	/// Skip past `end`, or to the end of the input.
	fn skip_past(&mut self, end: &str) {
		self.rest = self
			.rest
			.find(end)
			.map_or("", |index| &self.rest[index + end.len()..]);
	}

	/// Skip a comment, doctype, or processing instruction, returning
	/// whether there was one.
	fn skip_markup(&mut self) -> bool {
		if self.rest.starts_with("<!--") {
			self.skip_past("-->");

			return true;
		}

		if self.rest.starts_with("<!") || self.rest.starts_with("<?") {
			self.skip_past(">");

			return true;
		}

		false
	}

	fn tag(&mut self) -> Option<Token> {
		let closing = self.rest.starts_with("</");
		let body_start = if closing { 2 } else { 1 };
		let end = self.rest.find('>')?;
		let body = &self.rest[body_start..end];
		self.rest = &self.rest[end + 1..];
		let body = body.trim_end_matches('/');
		let name_end = body
			.find(|character: char| character.is_whitespace())
			.unwrap_or(body.len());
		let name = body[..name_end].to_ascii_lowercase();

		if closing {
			return Some(Token::Close(name));
		}

		if name == "script" || name == "style" {
			self.skip_past(&format!("</{name}"));
			self.skip_past(">");
		}

		Some(Token::Open {
			attrs: parse_attrs(&body[name_end..]),
			name,
		})
	}

	fn text(&mut self) -> Token {
		let first = self.rest.chars().next().map_or(0, char::len_utf8);
		let end = self.rest[first..]
			.find('<')
			.map_or(self.rest.len(), |index| index + first);
		let text = decode(&self.rest[..end]);
		self.rest = &self.rest[end..];
		Token::Text(text)
	}
}

impl Iterator for Tokenizer<'_> {
	type Item = Token;

	fn next(&mut self) -> Option<Token> {
		while self.skip_markup() {}

		if self.rest.is_empty() {
			return None;
		}

		let starts_tag = self.rest.starts_with("</")
			|| (self.rest.starts_with('<')
				&& self.rest[1..].starts_with(|next: char| next.is_ascii_alphabetic()));

		if starts_tag {
			return self.tag();
		}

		Some(self.text())
	}
}

/// Parse `name="value"` pairs, with single, double, or no quotes.
fn parse_attrs(source: &str) -> Vec<(String, String)> {
	let mut attrs = Vec::new();
	let mut rest = source.trim_start();

	while !rest.is_empty() {
		let name_end = rest
			.find(|character: char| character.is_whitespace() || character == '=')
			.unwrap_or(rest.len());
		let name = rest[..name_end].to_ascii_lowercase();
		rest = rest[name_end..].trim_start();

		let value = match rest.strip_prefix('=') {
			Some(after) => {
				let (value, remaining) = attr_value(after.trim_start());
				rest = remaining.trim_start();
				value
			}
			None => String::new(),
		};

		if !name.is_empty() {
			attrs.push((name, value));
		}
	}

	attrs
}

/// A quoted or bare attribute value and the input after it.
fn attr_value(source: &str) -> (String, &str) {
	let quote = source
		.chars()
		.next()
		.filter(|&quote| quote == '"' || quote == '\'');

	let Some(quote) = quote else {
		let end = source.find(char::is_whitespace).unwrap_or(source.len());

		return (decode(&source[..end]), &source[end..]);
	};
	let body = &source[1..];
	let end = body.find(quote).unwrap_or(body.len());
	let rest = body.get(end + 1..).unwrap_or("");

	(decode(&body[..end]), rest)
}

/// Decode character references.
fn decode(text: &str) -> String {
	let mut decoded = String::with_capacity(text.len());
	let mut rest = text;

	while let Some(start) = rest.find('&') {
		decoded.push_str(&rest[..start]);
		rest = &rest[start..];
		let end = rest.find(';').filter(|&end| end <= 10);
		let character = end.and_then(|end| entity(&rest[1..end]));

		if let (Some(end), Some(character)) = (end, character) {
			decoded.push(character);
			rest = &rest[end + 1..];
		} else {
			decoded.push('&');
			rest = &rest[1..];
		}
	}

	decoded.push_str(rest);
	decoded
}

fn entity(name: &str) -> Option<char> {
	let numeric = |digits: &str, radix| {
		u32::from_str_radix(digits, radix)
			.ok()
			.and_then(char::from_u32)
	};

	match name {
		"amp" => Some('&'),
		"lt" => Some('<'),
		"gt" => Some('>'),
		"quot" => Some('"'),
		"apos" => Some('\''),
		"nbsp" => Some('\u{a0}'),
		_ => {
			let digits = name.strip_prefix('#')?;
			match digits.strip_prefix(['x', 'X']) {
				Some(hex) => numeric(hex, 16),
				None => numeric(digits, 10),
			}
		}
	}
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
	attrs
		.iter()
		.find(|(key, _)| key == name)
		.map(|(_, value)| value.as_str())
}

/// What a tag means to the reader.
enum Role {
	/// Opens a text block of the named type.
	Text(&'static str, Attrs),
	/// Opens a container of the named type.
	Container(&'static str, Attrs),
	/// An atomic block and its attributes.
	Leaf(&'static str, Attrs),
	/// A line break.
	Break,
	/// An inline atom, such as a mention, whose text is read but not kept.
	Atom(&'static str, Attrs),
	/// Inline formatting: zero or more marks until the tag closes.
	Marks(Vec<(&'static str, Attrs)>),
	/// A block boundary with no type of its own, such as `<div>`.
	Boundary,
	/// Inline markup with no meaning here, such as `<span>`.
	Inline,
}

const BOUNDARIES: [&str; 20] = [
	"address",
	"article",
	"aside",
	"body",
	"dd",
	"div",
	"dl",
	"dt",
	"figcaption",
	"figure",
	"footer",
	"header",
	"html",
	"main",
	"nav",
	"section",
	"table",
	"tbody",
	"td",
	"tr",
];

/// A text block's attributes from its `style`: TipTap's text alignment.
fn text_block_attrs(attrs: &[(String, String)]) -> Attrs {
	let style = attr(attrs, "style").unwrap_or_default();
	// CSS keywords ignore ASCII case: `TEXT-ALIGN: Center` is centred.
	let align = cfg!(feature = "text-align")
		.then(|| css_value(style, "text-align").map(str::to_ascii_lowercase))
		.flatten();

	match align.as_deref() {
		Some(align @ ("left" | "center" | "right" | "justify")) if cfg!(feature = "text-align") => {
			Attrs::new().with("textAlign", align)
		}
		_ => Attrs::new(),
	}
}

fn role(name: &str, attrs: &[(String, String)], parent: Option<&'static str>) -> Role {
	let data_type = attr(attrs, "data-type");

	match name {
		"p" => Role::Text("paragraph", text_block_attrs(attrs)),
		"h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
			let level = i64::from(name.as_bytes()[1] - b'0');
			Role::Text("heading", text_block_attrs(attrs).with("level", level))
		}
		"pre" => Role::Text("codeBlock", Attrs::new()),
		"blockquote" => Role::Container("blockquote", Attrs::new()),
		"ul" if data_type == Some("taskList") => Role::Container("taskList", Attrs::new()),
		"ul" => Role::Container("bulletList", Attrs::new()),
		"ol" => {
			let start = attr(attrs, "start").and_then(|start| start.parse::<i64>().ok());
			Role::Container(
				"orderedList",
				Attrs::new().with("start", start.unwrap_or(1)),
			)
		}

		"li" if data_type == Some("taskItem") || parent == Some("taskList") => {
			let checked = attr(attrs, "data-checked") == Some("true");
			Role::Container("taskItem", Attrs::new().with("checked", checked))
		}
		"li" => Role::Container("listItem", Attrs::new()),
		"hr" => Role::Leaf("horizontalRule", Attrs::new()),
		"img" if cfg!(feature = "image") => {
			Role::Leaf("image", media_attrs(attrs, IMAGE_ATTRIBUTES))
		}
		"audio" if cfg!(feature = "audio") => Role::Leaf("audio", audio_attrs(attrs)),
		"iframe" if cfg!(any(feature = "youtube", feature = "twitch")) => iframe_role(attrs),
		"div" if cfg!(feature = "file") && (data_type == Some("file")) => {
			Role::Leaf("file", media_attrs(attrs, FILE_ATTRIBUTES))
		}
		"br" => Role::Break,
		"details" if cfg!(feature = "details") => {
			Role::Container(
				"details",
				Attrs::new().with("open", attr(attrs, "open").is_some()),
			)
		}
		"summary" => Role::Text("detailsSummary", Attrs::new()),
		"div" if data_type == Some("detailsContent") => {
			Role::Container("detailsContent", Attrs::new())
		}
		"div" if cfg!(feature = "callout") && (attr(attrs, "data-callout-type").is_some()) => {
			let kind = attr(attrs, "data-callout-type").unwrap_or("info");
			Role::Container("callout", Attrs::new().with("type", kind))
		}
		"div" if cfg!(feature = "mathematics") && (data_type == Some("block-math")) => {
			Role::Leaf("blockMath", atom_attrs(attrs))
		}
		"span" if cfg!(feature = "mathematics") && (data_type == Some("inline-math")) => {
			Role::Atom("inlineMath", atom_attrs(attrs))
		}
		"span" if cfg!(feature = "mention") && (data_type == Some("mention")) => {
			Role::Atom("mention", atom_attrs(attrs))
		}
		"span" if cfg!(feature = "emoji") && (data_type == Some("emoji")) => {
			Role::Atom("emoji", atom_attrs(attrs))
		}
		_ => inline_role(name, attrs),
	}
}

/// The HTML attributes of `<img>`, and the image attributes they hold.
const IMAGE_ATTRIBUTES: &[(&str, &str)] = &[
	("src", "src"),
	("alt", "alt"),
	("title", "title"),
	("width", "width"),
	("height", "height"),
];

/// The HTML attributes of a video `<iframe>` that videos keep.
const VIDEO_ATTRIBUTES: &[(&str, &str)] =
	&[("src", "src"), ("width", "width"), ("height", "height")];

/// The HTML attributes of a file card, and the file attributes they hold.
const FILE_ATTRIBUTES: &[(&str, &str)] = &[
	("data-url", "url"),
	("data-file-name", "fileName"),
	("data-file-type", "fileType"),
	("data-file-size", "fileSize"),
];

/// The boolean attributes of `<audio>`.
const AUDIO_FLAGS: [&str; 4] = ["controls", "autoplay", "loop", "muted"];

/// The attributes `mapping` names, with sizes read as numbers.
fn media_attrs(attrs: &[(String, String)], mapping: &[(&str, &'static str)]) -> Attrs {
	mapping
		.iter()
		.filter_map(|(html, key)| {
			let value = attr(attrs, html)?;
			let number = matches!(*key, "width" | "height" | "fileSize")
				.then(|| value.parse::<f64>().ok())
				.flatten();
			Some((*key, number.map_or_else(|| Value::from(value), Value::from)))
		})
		.collect()
}

/// An `<audio>` element's source and flags; a flag is on when present.
fn audio_attrs(attrs: &[(String, String)]) -> Attrs {
	let flags = AUDIO_FLAGS
		.iter()
		.map(|flag| (*flag, Value::Bool(attr(attrs, flag).is_some())));
	let src = attr(attrs, "src").map(|src| ("src", Value::from(src)));
	src.into_iter().chain(flags).collect()
}

/// An `<iframe>` showing a YouTube or Twitch video, as TipTap writes them;
/// other iframes carry nothing to keep.
fn iframe_role(attrs: &[(String, String)]) -> Role {
	let Some(video) = attr(attrs, "src").and_then(video_kind) else {
		return Role::Inline;
	};

	Role::Leaf(video, media_attrs(attrs, VIDEO_ATTRIBUTES))
}

#[cfg(any(feature = "youtube", feature = "twitch"))]
pub(super) use crate::media::video_kind;

/// Without the video extensions, no iframe is a video.
#[cfg(not(any(feature = "youtube", feature = "twitch")))]
pub(super) fn video_kind(_src: &str) -> Option<&'static str> {
	None
}

/// The `data-` attributes TipTap writes on inline atoms, and the attributes
/// they hold.
const ATOM_ATTRIBUTES: [(&str, &str); 6] = [
	("data-id", "id"),
	("data-label", "label"),
	("data-mention-suggestion-char", "mentionSuggestionChar"),
	("data-name", "name"),
	("data-emoji", "emoji"),
	("data-latex", "latex"),
];

fn atom_attrs(attrs: &[(String, String)]) -> Attrs {
	ATOM_ATTRIBUTES
		.iter()
		.filter_map(|(data, key)| Some((*key, attr(attrs, data)?)))
		.collect()
}

fn inline_role(name: &str, attrs: &[(String, String)]) -> Role {
	if BOUNDARIES.contains(&name) {
		return Role::Boundary;
	}

	let style = attr(attrs, "style").map(Style::parse).unwrap_or_default();

	let tagged = match name {
		"strong" | "b" => Some("bold"),
		"em" | "i" => Some("italic"),
		"u" => Some("underline"),
		"s" | "strike" | "del" => Some("strike"),
		"code" => Some("code"),
		"sub" if cfg!(feature = "subscript") => Some("subscript"),
		"sup" if cfg!(feature = "superscript") => Some("superscript"),
		_ => None,
	};

	let mut marks: Vec<(&'static str, Attrs)> = tagged
		.into_iter()
		.chain(style.marks())
		.filter(|&mark| !(mark == "bold" && style.bold == Some(false)))
		.map(|mark| (mark, Attrs::new()))
		.collect();

	if name == "a" {
		let href = attr(attrs, "href").unwrap_or_default();
		marks.push(("link", Attrs::new().with("href", href)));
	}

	if cfg!(feature = "hashtag") && attr(attrs, "data-type") == Some("hashtag") {
		let tag = attr(attrs, "data-tag").map_or(Value::Null, Value::from);
		marks.push(("hashtag", Attrs::new().with("tag", tag)));
	}

	let css = attr(attrs, "style").unwrap_or_default();

	if cfg!(feature = "highlight") && name == "mark" {
		let color = attr(attrs, "data-color").or_else(|| css_value(css, "background-color"));
		let attrs = color.map_or_else(Attrs::new, |color| Attrs::new().with("color", color));
		marks.push(("highlight", attrs));
	}

	let styles = if cfg!(feature = "text-style") {
		text_style_attrs(css)
	} else {
		Attrs::new()
	};

	if cfg!(feature = "text-style") && !styles.is_empty() && name != "mark" {
		marks.push(("textStyle", styles));
	}

	if marks.is_empty() && tagged.is_none() && name != "span" && name != "a" {
		return Role::Inline;
	}

	Role::Marks(marks)
}

/// The value of CSS `property` in a `style` attribute.
fn css_value<'a>(style: &'a str, property: &str) -> Option<&'a str> {
	style.split(';').find_map(|declaration| {
		let (name, value) = declaration.split_once(':')?;
		let value = value.trim();
		(name.trim().eq_ignore_ascii_case(property) && !value.is_empty()).then_some(value)
	})
}

/// `textStyle` attributes from a `style` attribute: colour, background,
/// font family, size, and line height. A transparent background is no
/// background.
fn text_style_attrs(style: &str) -> Attrs {
	let mut attrs = Attrs::new();

	for (attr, property) in super::html_write::TEXT_STYLE_PROPERTIES {
		let value = css_value(style, property).filter(|value| *value != "transparent");

		if let Some(value) = value {
			attrs.set(attr, value);
		}
	}

	attrs
}

/// Formatting carried by inline `style` attributes, which word processors
/// such as Google Docs use instead of `<b>` and `<i>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Style {
	/// `Some(false)` for an explicit normal weight, which cancels `<b>`.
	bold: Option<bool>,
	italic: bool,
	underline: bool,
	strike: bool,
}

impl Style {
	fn parse(style: &str) -> Self {
		let mut parsed = Self::default();

		for declaration in style.split(';') {
			let Some((property, value)) = declaration.split_once(':') else {
				continue;
			};
			let value = value.trim().to_ascii_lowercase();

			match property.trim().to_ascii_lowercase().as_str() {
				"font-weight" => parsed.bold = Some(heavy(&value)),
				"font-style" => parsed.italic = value == "italic" || value == "oblique",
				"text-decoration" | "text-decoration-line" => {
					parsed.underline = value.contains("underline");
					parsed.strike = value.contains("line-through");
				}
				_ => {}
			}
		}

		parsed
	}

	fn marks(self) -> impl Iterator<Item = &'static str> {
		[
			(self.bold == Some(true), "bold"),
			(self.italic, "italic"),
			(self.underline, "underline"),
			(self.strike, "strike"),
		]
		.into_iter()
		.filter_map(|(on, mark)| on.then_some(mark))
	}
}

/// The lightest CSS font weight that reads as bold (semibold).
const BOLD_WEIGHT: u16 = 600;

/// Whether a `font-weight` value is bold.
fn heavy(value: &str) -> bool {
	match value.parse::<u16>() {
		Ok(weight) => weight >= BOLD_WEIGHT,
		Err(_) => value == "bold" || value == "bolder",
	}
}

/// An open container.
struct Frame {
	tag: String,
	name: &'static str,
	attrs: Attrs,
	children: Vec<BlockTree>,
}

/// The text block being filled.
struct OpenText {
	tag: Option<String>,
	name: &'static str,
	attrs: Attrs,
	inline: InlineText,
	/// Whitespace seen but not yet written, with the marks it had, so
	/// blocks never end in spaces and a space before `<b>` stays unbolded.
	space: Option<MarkSet>,
}

/// An inline atom being read, such as a mention: the text inside its tag
/// is what it shows, not document text.
struct OpenAtom {
	tag: String,
	name: &'static str,
	attrs: Attrs,
	text: String,
}

/// Builds block trees from tokens.
struct Reader<'s> {
	build: Builder<'s>,
	frames: Vec<Frame>,
	text: Option<OpenText>,
	/// Open inline tags and the marks each applies.
	marks: Vec<(String, Vec<Mark>)>,
	atom: Option<OpenAtom>,
}

impl<'s> Reader<'s> {
	fn new(schema: &'s Schema) -> Self {
		let root = Frame {
			tag: String::new(),
			name: "",
			attrs: Attrs::new(),
			children: Vec::new(),
		};

		Self {
			build: Builder::new(schema),
			frames: vec![root],
			text: None,
			marks: Vec::new(),
			atom: None,
		}
	}

	fn in_code(&self) -> bool {
		self.text
			.as_ref()
			.is_some_and(|text| text.name == "codeBlock")
	}

	fn token(&mut self, token: Token) {
		match token {
			Token::Open { name, attrs } => self.open(name, &attrs),
			Token::Close(name) => self.close(&name),
			Token::Text(text) => self.push_text(&text),
		}
	}

	fn open(&mut self, tag: String, attrs: &[(String, String)]) {
		let parent = self.frames.last().map(|frame| frame.name);

		match role(&tag, attrs, parent) {
			Role::Text(name, block_attrs) => {
				self.end_text();
				self.text = Some(OpenText {
					tag: Some(tag),
					name,
					attrs: block_attrs,
					inline: InlineText::new(),
					space: None,
				});
			}
			Role::Container(name, block_attrs) => {
				self.end_text();
				self.frames.push(Frame {
					tag,
					name,
					attrs: block_attrs,
					children: Vec::new(),
				});
			}
			Role::Leaf(name, leaf_attrs) => {
				self.end_text();
				let leaf = self.build.leaf(name, &leaf_attrs);
				self.children().extend(leaf);
			}
			Role::Break => self.line_break(),
			Role::Atom(name, attrs) => {
				self.atom = Some(OpenAtom {
					tag,
					name,
					attrs,
					text: String::new(),
				});
			}
			Role::Marks(marks) => self.open_marks(tag, marks, attrs),
			Role::Boundary => self.end_text(),
			Role::Inline => {}
		}
	}

	fn open_marks(
		&mut self,
		tag: String,
		marks: Vec<(&'static str, Attrs)>,
		attrs: &[(String, String)],
	) {
		if self.in_code() {
			let language = attr(attrs, "class").and_then(|class| class.strip_prefix("language-"));

			if let (Some(language), Some(text)) = (language, self.text.as_mut()) {
				text.attrs.set("language", Value::from(language));
			}

			return;
		}

		let marks = marks
			.into_iter()
			.filter_map(|(name, mark_attrs)| {
				if name == "textStyle" {
					self.build.style_mark(name, &mark_attrs)
				} else {
					self.build.mark(name, &mark_attrs)
				}
			})
			.collect();
		self.marks.push((tag, marks));
	}

	fn close(&mut self, tag: &str) {
		if self.atom.as_ref().is_some_and(|atom| atom.tag == tag) {
			self.finish_atom();

			return;
		}

		let closes_text = self
			.text
			.as_ref()
			.is_some_and(|text| text.tag.as_deref() == Some(tag));

		if closes_text {
			self.end_text();

			return;
		}

		if let Some(index) = self.marks.iter().rposition(|(open, _)| open == tag) {
			self.marks.remove(index);

			return;
		}

		if let Some(index) = self
			.frames
			.iter()
			.rposition(|frame| frame.tag == tag && !tag.is_empty())
		{
			self.end_text();

			while self.frames.len() > index.max(1) {
				self.pop_frame();
			}

			return;
		}

		if BOUNDARIES.contains(&tag) {
			self.end_text();
		}
	}

	fn children(&mut self) -> &mut Vec<BlockTree> {
		let last = self.frames.len() - 1;
		&mut self.frames[last].children
	}

	fn pop_frame(&mut self) {
		let Some(frame) = self.frames.pop() else {
			return;
		};
		let trees = self
			.build
			.container(frame.name, &frame.attrs, frame.children);
		self.children().extend(trees);
	}

	fn marks(&self) -> MarkSet {
		MarkSet::from_marks(
			self.marks
				.iter()
				.flat_map(|(_, marks)| marks.iter().cloned()),
		)
	}

	/// The open text block, opening an implicit paragraph if needed.
	fn ensure_text(&mut self) -> &mut OpenText {
		self.text.get_or_insert_with(|| {
			OpenText {
				tag: None,
				name: "paragraph",
				attrs: Attrs::new(),
				inline: InlineText::new(),
				space: None,
			}
		})
	}

	/// Add text the way a browser renders it: runs of whitespace collapse
	/// to one space, and a space waits until more text follows, so blocks
	/// never start or end with one. Code keeps its text as is.
	fn push_text(&mut self, raw: &str) {
		if let Some(atom) = self.atom.as_mut() {
			atom.text.push_str(raw);

			return;
		}

		if self.in_code() {
			let marks = self.marks();
			self.ensure_text()
				.inline
				.append(InlineText::plain(raw, marks));

			return;
		}

		let words: Vec<&str> = raw.split_ascii_whitespace().collect();
		let leading = raw.starts_with(|character: char| character.is_ascii_whitespace());
		let trailing = raw.ends_with(|character: char| character.is_ascii_whitespace());
		let marks = self.marks();

		if words.is_empty() {
			if let Some(text) = self.text.as_mut().filter(|_| leading) {
				text.space.get_or_insert(marks);
			}

			return;
		}

		let text = self.ensure_text();
		let pending = text.space.take().or_else(|| leading.then(|| marks.clone()));

		if let Some(space_marks) = pending.filter(|_| !text.inline.is_empty()) {
			text.inline.append(InlineText::plain(" ", space_marks));
		}

		text.inline
			.append(InlineText::plain(&words.join(" "), marks.clone()));
		text.space = trailing.then_some(marks);
	}

	/// Add the atom just closed: as an embed, with what its text shows
	/// filling in a missing label or emoji, or as that text when the schema
	/// has no such embed.
	fn finish_atom(&mut self) {
		let Some(atom) = self.atom.take() else {
			return;
		};
		let shown = atom.text.trim();
		let mut attrs = atom.attrs;

		match atom.name {
			"emoji"
				if cfg!(feature = "emoji") && attrs.get("emoji").is_none() && !shown.is_empty() =>
			{
				attrs.set("emoji", shown);
			}
			"mention" if cfg!(feature = "mention") && attrs.get("label").is_none() => {
				let label: String = shown.chars().skip(1).collect();
				// A mention without a label shows its id, which is not a label.
				let id = attrs.get("id").and_then(Value::as_str);

				if id != Some(label.as_str()) {
					attrs.set("label", label);
				}
			}
			_ => {}
		}

		let marks = self.marks();
		let Some(embed) = self.build.embed(atom.name, &attrs, marks) else {
			self.push_text(&atom.text);

			return;
		};
		let text = self.ensure_text();

		if let Some(space) = text.space.take().filter(|_| !text.inline.is_empty()) {
			text.inline.append(InlineText::plain(" ", space));
		}

		text.inline.append(embed);
	}

	fn line_break(&mut self) {
		if self.in_code() {
			self.push_text("\n");

			return;
		}

		let marks = self.marks();

		match self.build.hard_break(marks) {
			Some(embed) => {
				let text = self.ensure_text();
				text.inline.append(embed);
				text.space = None;
			}
			None => self.end_text(),
		}
	}

	fn end_text(&mut self) {
		let Some(text) = self.text.take() else {
			return;
		};

		if text.tag.is_none() && text.inline.is_empty() {
			return;
		}

		let block = self.build.text(text.name, text.attrs, text.inline);
		self.children().push(block);
	}

	fn finish(mut self) -> Slice {
		self.end_text();

		while self.frames.len() > 1 {
			self.pop_frame();
		}

		let roots = self
			.frames
			.pop()
			.map(|root| root.children)
			.unwrap_or_default();
		self.build.finish(roots)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn tokenizes_tags_attributes_and_entities() {
		let tokens: Vec<Token> =
			Tokenizer::new("<!-- c --><a HREF='x&amp;y' data-x=1 hidden>&lt;3 &#x1F600;</a>")
				.collect();

		assert_eq!(
			tokens,
			[
				Token::Open {
					name: "a".to_owned(),
					attrs: vec![
						("href".to_owned(), "x&y".to_owned()),
						("data-x".to_owned(), "1".to_owned()),
						("hidden".to_owned(), String::new()),
					],
				},
				Token::Text("<3 😀".to_owned()),
				Token::Close("a".to_owned()),
			]
		);
	}

	#[test]
	fn skips_scripts_styles_and_doctypes() {
		let tokens: Vec<Token> =
			Tokenizer::new("<!DOCTYPE html><style>p{}</style><script>if (a < b) {}</script>x")
				.collect();

		assert_eq!(tokens.last(), Some(&Token::Text("x".to_owned())));
		assert_eq!(tokens.len(), 3);
	}

	#[test]
	fn keeps_unknown_entities_and_stray_ampersands() {
		assert_eq!(decode("a & b &bogus; &#xZZ;"), "a & b &bogus; &#xZZ;");
	}
}
