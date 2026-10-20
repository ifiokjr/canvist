//! Semantic HTML plus CSS that wraps exactly where the canvas does.
//!
//! The HTML is ordinary flow content (`<p>`, `<h2>`, `<ul><li>`, `<strong>`,
//! …) for no-JavaScript readers and search engines. The CSS is generated from
//! the same resolved styles layout uses and pins every property that affects
//! line breaking, so a browser rendering the HTML at the layout width breaks
//! lines at the same places as [`DocumentLayout`](crate::DocumentLayout)
//! with the browser's own measurer. The full contract is in
//! `docs/internal/ssr-css-contract.md`.

use std::fmt::Write;
use std::sync::Arc;

use canvist_model::Block;
use canvist_model::Content;
use canvist_model::Document;
use canvist_model::Embed;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::RunRef;
use canvist_model::Value;

use super::attribute;
use super::escape_into;
use super::num;
use crate::Color;
use crate::style::BlockStyle;
use crate::style::StyleId;
use crate::style::Styles;
use crate::style::attr;
use crate::style::attr_px;
use crate::style::line_height;
use crate::text::slice;
use crate::theme::AttrBinding;
use crate::theme::EmbedStyle;
use crate::theme::FontSize;
use crate::theme::LeafStyle;
use crate::theme::MarkerKind;
use crate::theme::TextPatch;
use crate::theme::TextStyle;
use crate::theme::Theme;

/// The link schemes [`is_safe_url`] lets through; anything else, such as
/// `javascript:`, could run code when clicked.
const SAFE_URL_SCHEMES: [&str; 3] = ["http:", "https:", "mailto:"];

/// The `textAlign` values layout understands, which are also CSS
/// `text-align` keywords.
const ALIGNMENTS: [&str; 4] = ["left", "center", "right", "justify"];

/// Options for [`to_html`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HtmlOptions {
	/// The content width, which must equal the layout width.
	pub width: f32,
}

impl HtmlOptions {
	/// Options for a document `width` pixels wide.
	#[must_use]
	pub const fn new(width: f32) -> Self {
		Self { width }
	}
}

/// Server-rendered HTML and the CSS it needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HtmlDocument {
	/// A `<div class="cv-doc">` fragment.
	pub html: String,
	/// The stylesheet, scoped under `.cv-doc`.
	pub css: String,
}

impl HtmlDocument {
	/// A complete HTML page holding the fragment and its stylesheet, with no
	/// body margin.
	#[must_use]
	pub fn page(&self) -> String {
		format!(
			"<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
			 content=\"width=device-width\"><style>{}</style></head><body \
			 style=\"margin:0\">{}</body></html>",
			self.css, self.html
		)
	}
}

/// Render `doc` with `theme` as semantic HTML and CSS.
#[must_use]
pub fn to_html(doc: &Document, theme: &Arc<Theme>, options: &HtmlOptions) -> HtmlDocument {
	let mut writer = Writer {
		doc,
		styles: Styles::new(theme.clone(), doc.schema().clone()),
		out: String::new(),
	};
	writer.out.push_str("<div class=\"cv-doc\">");
	writer.children(None, None, 0);
	writer.out.push_str("</div>");

	let css = stylesheet(&writer.styles, options.width);
	HtmlDocument {
		html: writer.out,
		css,
	}
}

struct Writer<'a> {
	doc: &'a Document,
	styles: Styles,
	out: String,
}

impl Writer<'_> {
	fn children(&mut self, parent: Option<&Block>, parent_style: Option<StyleId>, lists: usize) {
		let doc = self.doc;
		let marker = parent_style.and_then(|style| self.styles.get(style).marker);
		let open = parent.is_none_or(|parent| crate::document::is_open(doc, parent));
		let shown = if open { usize::MAX } else { 1 };
		let mut previous_after = None;

		for (index, &id) in doc
			.children(parent.map(Block::id))
			.iter()
			.enumerate()
			.take(shown)
		{
			let Some(block) = doc.block(id) else {
				continue;
			};
			let marker = marker.filter(|&kind| kind != MarkerKind::Disclosure || index == 0);

			let style = self
				.styles
				.resolve(block, parent.map(Block::kind), parent_style);
			let resolved = self.styles.get(style);
			let margin = previous_after.map_or(0.0, |after: f32| after.max(resolved.space_before));
			previous_after = Some(resolved.space_after);
			let marker = marker.map(|kind| {
				let depth = if kind == MarkerKind::Disclosure {
					usize::from(open)
				} else {
					lists.saturating_sub(1)
				};
				(kind, depth)
			});
			self.block(block, style, margin, marker, lists);
		}
	}

	/// Write `block` as one element, then its text or child blocks.
	///
	/// Void elements such as `<hr>` stop after the opening tag, task items
	/// get their checkbox before the content, and nested lists count their
	/// depth so bullets cycle through the theme's markers.
	fn block(
		&mut self,
		block: &Block,
		style: StyleId,
		margin: f32,
		marker: Option<(MarkerKind, usize)>,
		lists: usize,
	) {
		let resolved = self.styles.get(style).clone();
		let tag = tag_name(&resolved.tag);
		self.open_tag(block, style, tag, marker);

		// A host draws a slot's picture into the empty element, so the
		// element carries the picture's text alternative.
		if let (Content::Leaf, LeafStyle::Slot { .. }) = (block.content(), resolved.leaf)
			&& let Some(alt) = block
				.attrs()
				.get("alt")
				.and_then(Value::as_str)
				.filter(|alt| !alt.is_empty())
		{
			attribute(&mut self.out, "role", "img");
			attribute(&mut self.out, "aria-label", alt);
		}

		let inline_style = self.inline_style(block, &resolved, margin);

		if !inline_style.is_empty() {
			attribute(&mut self.out, "style", &inline_style);
		}

		self.out.push('>');

		if is_void(tag) {
			return;
		}

		if marker.is_some_and(|(kind, _)| kind == MarkerKind::Task) {
			self.checkbox(block, &resolved);
		}

		// The HTML parser drops a newline straight after `<pre>`, so a leading
		// newline in the text needs a sacrificial one in front of it.
		if let Content::Inline(inline) = block.content()
			&& tag == "pre"
			&& inline.as_str().starts_with('\n')
		{
			self.out.push('\n');
		}

		match block.content() {
			Content::Inline(inline) => self.inline(inline, &resolved.text),
			Content::Blocks(_) => {
				let lists = lists + usize::from(resolved.marker.is_some());
				self.children(Some(block), Some(style), lists);
			}
			Content::Leaf => {}
		}

		let _ = write!(self.out, "</{tag}>");
	}

	/// Write the start of `block`'s opening tag: its classes, `data-id`, and,
	/// for text blocks, `dir`. The tag is left open for the inline style.
	fn open_tag(
		&mut self,
		block: &Block,
		style: StyleId,
		tag: &str,
		marker: Option<(MarkerKind, usize)>,
	) {
		let defaults = &self.doc.schema().block(block.kind()).attrs;
		let type_name = class_name(&self.doc.schema().block(block.kind()).name);
		let _ = write!(
			self.out,
			"<{tag} class=\"cv-b cv-s{} cv-t-{type_name}",
			style.index()
		);

		if let Some((kind, depth)) = marker {
			let bullets = self.styles.theme().markers().bullets.len().max(1);

			let _ = match kind {
				MarkerKind::Bullet => write!(self.out, " cv-mk cv-mk-b{}", depth % bullets),
				MarkerKind::Ordered => write!(self.out, " cv-mk cv-mk-o"),
				MarkerKind::Task => write!(self.out, " cv-mk-t"),
				// For a disclosure, the depth slot carries whether it is open.
				MarkerKind::Disclosure => write!(self.out, " cv-mk cv-mk-d{depth}"),
			};
		}

		self.out.push('"');
		attribute(&mut self.out, "data-id", &block.id().to_string());

		if matches!(block.content(), Content::Inline(_)) {
			let dir = attr(block.attrs(), defaults, "dir")
				.and_then(Value::as_str)
				.filter(|dir| matches!(*dir, "ltr" | "rtl"))
				.unwrap_or("auto");
			attribute(&mut self.out, "dir", dir);
		}
	}

	/// The per-block declarations a shared class cannot carry: the collapsed
	/// top margin, an ordered list's start number, text alignment, and a
	/// slot's height.
	fn inline_style(&self, block: &Block, resolved: &BlockStyle, margin: f32) -> String {
		let defaults = &self.doc.schema().block(block.kind()).attrs;
		let mut inline_style = String::new();

		if margin > 0.0 {
			let _ = write!(inline_style, "margin-top:{}px;", num(margin));
		}

		if resolved.marker == Some(MarkerKind::Ordered) {
			let start = attr(block.attrs(), defaults, "start")
				.and_then(Value::as_int)
				.unwrap_or(1);
			let _ = write!(inline_style, "counter-reset:cv-n {};", start - 1);
		}

		let align = attr(block.attrs(), defaults, "textAlign").and_then(Value::as_str);

		if let Some(align) = align.filter(|align| ALIGNMENTS.contains(align))
			&& matches!(block.content(), Content::Inline(_))
		{
			let _ = write!(inline_style, "text-align:{align};");
		}

		if let (Content::Leaf, LeafStyle::Slot { height }) = (block.content(), resolved.leaf) {
			let height = attr_px(block.attrs(), "height").unwrap_or(height);
			let _ = write!(inline_style, "height:{}px;", num(height));
		}

		inline_style
	}

	fn checkbox(&mut self, block: &Block, style: &BlockStyle) {
		let defaults = &self.doc.schema().block(block.kind()).attrs;
		let checked = attr(block.attrs(), defaults, "checked")
			.and_then(Value::as_bool)
			.unwrap_or(false);
		let size = self.styles.theme().markers().checkbox;
		let top = (style.text.line_height_px() - size) / 2.0;
		let _ = write!(
			self.out,
			"<input type=\"checkbox\" class=\"cv-check\" disabled{} style=\"top:{}px\">",
			if checked { " checked" } else { "" },
			num(top)
		);
	}

	fn inline(&mut self, inline: &InlineText, base: &TextStyle) {
		let text = inline.as_str();
		let mut open: Vec<Mark> = Vec::new();
		let mut ends_with_break = text.is_empty();

		for run in inline.runs() {
			self.sync_marks(&mut open, run.marks.as_slice());
			ends_with_break = self.run_content(text, &run, base);
		}

		self.sync_marks(&mut open, &[]);

		if ends_with_break {
			self.out.push_str("<br class=\"cv-br\">");
		}
	}

	/// Write one run of `text`, returning whether it ends in a line break.
	fn run_content(&mut self, text: &str, run: &RunRef<'_>, base: &TextStyle) -> bool {
		if let Some(embed) = run.embed {
			return self.embed(embed, base, run.marks);
		}

		escape_into(&mut self.out, slice(text, &run.range));
		run.text.ends_with(crate::text::is_newline)
	}

	/// Move from the `open` mark elements to `marks`, closing only the
	/// elements past their shared prefix so adjacent runs that share marks
	/// share elements too.
	fn sync_marks(&mut self, open: &mut Vec<Mark>, marks: &[Mark]) {
		let common = open
			.iter()
			.zip(marks)
			.take_while(|(open_mark, mark)| open_mark == mark)
			.count();

		for mark in open.drain(common..).rev() {
			let _ = write!(self.out, "</{}>", self.mark_tag(&mark));
		}

		for mark in &marks[common..] {
			self.open_mark(mark);
			open.push(mark.clone());
		}
	}

	fn mark_tag(&self, mark: &Mark) -> String {
		self.styles
			.mark_tag(mark.kind)
			.map_or("span", |tag| tag_name(tag))
			.to_owned()
	}

	fn open_mark(&mut self, mark: &Mark) {
		let tag = self.mark_tag(mark);
		let name = class_name(&self.doc.schema().mark(mark.kind).name);
		let _ = write!(self.out, "<{tag} class=\"cv-m cv-m-{name}\"");

		let href = mark.attrs.get("href").and_then(Value::as_str);

		if let Some(href) = href.filter(|href| tag == "a" && is_safe_url(href)) {
			attribute(&mut self.out, "href", href);
		}

		let style = bindings_css(&self.styles, mark);

		if !style.is_empty() {
			attribute(&mut self.out, "style", &style);
		}

		self.out.push('>');
	}

	/// Write an embed, returning whether it is a line break.
	fn embed(&mut self, embed: &Embed, base: &TextStyle, marks: &canvist_model::MarkSet) -> bool {
		let name = class_name(&self.doc.schema().embed(embed.kind).name);

		let (width, height) = match self.styles.embed_style(embed.kind) {
			Some(EmbedStyle::LineBreak) => {
				self.out.push_str("<br>");

				return true;
			}
			Some(label @ EmbedStyle::Label { .. }) => {
				let text = label.label_text(&embed.attrs).unwrap_or_default();
				self.label(embed, &name, &text);

				return false;
			}
			Some(EmbedStyle::Box { width, height }) => (*width, *height),
			None => {
				let size = self.styles.run_style(base, marks).font.size;
				(size, size)
			}
		};

		let width = attr_px(&embed.attrs, "width").unwrap_or(width);
		let height = attr_px(&embed.attrs, "height").unwrap_or(height);
		let _ = write!(
			self.out,
			"<span class=\"cv-e cv-e-{name}\" style=\"width:{}px;height:{}px\"",
			num(width),
			num(height)
		);
		attribute(
			&mut self.out,
			"data-type",
			&self.doc.schema().embed(embed.kind).name,
		);
		self.out.push_str("></span>");
		false
	}

	/// Write a label embed as a text span of class `name` holding `text`, the
	/// theme's prefix followed by the embed's label attribute.
	fn label(&mut self, embed: &Embed, name: &str, text: &str) {
		let _ = write!(self.out, "<span class=\"cv-e cv-e-{name}\" dir=\"auto\"");
		attribute(
			&mut self.out,
			"data-type",
			&self.doc.schema().embed(embed.kind).name,
		);
		self.out.push('>');
		escape_into(&mut self.out, text);
		self.out.push_str("</span>");
	}
}

/// Inline CSS for the attribute bindings of `mark`, such as a colour mark's
/// `color`. Values from document content are validated before they reach a
/// style attribute.
fn bindings_css(styles: &Styles, mark: &Mark) -> String {
	let Some(rule) = styles.mark_rule(mark.kind) else {
		return String::new();
	};

	rule.bindings
		.iter()
		.filter_map(|binding| binding_css(binding, mark))
		.collect()
}

/// The declaration `binding` produces for `mark`, or `None` when the
/// attribute is missing or fails validation.
fn binding_css(binding: &AttrBinding, mark: &Mark) -> Option<String> {
	let string = |key: &str| mark.attrs.get(key).and_then(Value::as_str);

	match binding {
		AttrBinding::Color(key) => {
			string(key)
				.and_then(Color::parse_hex)
				.map(|color| format!("color:{color};"))
		}
		AttrBinding::Background(key) => {
			string(key)
				.and_then(Color::parse_hex)
				.map(|color| format!("background-color:{color};"))
		}
		AttrBinding::FontFamily(key) => {
			string(key)
				.filter(|family| is_safe_family(family))
				.map(|family| format!("font-family:{family};"))
		}
		AttrBinding::FontSize(key) => {
			attr_px(&mark.attrs, key)
				.filter(|size| *size > 0.0)
				.map(|size| format!("font-size:{}px;", num(size)))
		}
		AttrBinding::LineHeight(key) => {
			line_height(&mark.attrs, key).map(|height| format!("line-height:{};", num(height)))
		}
	}
}

/// The stylesheet for every style `styles` resolved, plus the fixed rules of
/// the CSS contract.
fn stylesheet(styles: &Styles, width: f32) -> String {
	let theme = styles.theme();
	let base = theme.base();
	let mut css = String::new();

	let _ = write!(
		css,
		".cv-doc{{box-sizing:content-box;width:{}px;margin:0;padding:0;background-color:{};\
		 {}white-space:pre-wrap;overflow-wrap:break-word;word-break:normal;line-break:strict;\
		 hyphens:manual;text-align:start;text-indent:0;text-transform:none;word-spacing:0;\
		 tab-size:{};font-kerning:normal;font-variant:normal;font-variant-ligatures:normal;\
		 font-feature-settings:normal;font-variation-settings:normal;font-size-adjust:none;\
		 font-stretch:normal;text-rendering:auto;text-wrap:wrap;text-autospace:no-autospace;\
		 text-spacing-trim:normal;-webkit-text-size-adjust:100%;text-size-adjust:100%}}",
		num(width),
		theme.chrome().background,
		text_css(base),
		theme.tab_size()
	);
	css.push_str(
		".cv-doc :where(*){margin:0;padding:0;border:0 solid \
		 transparent;font:inherit;color:inherit;background:none;text-decoration:none;list-style:\
		 none;vertical-align:baseline;white-space:inherit;letter-spacing:inherit;box-sizing:\
		 content-box}",
	);
	css.push_str(".cv-doc :where(.cv-b){display:flow-root}");
	css.push_str(".cv-doc :where(.cv-e){display:inline-block;white-space:nowrap;text-indent:0}");

	for (id, style) in styles.all() {
		let _ = write!(
			css,
			".cv-doc .cv-s{}{{{}",
			id.index(),
			text_css(&style.text)
		);
		box_css(&mut css, style);
		css.push('}');
	}

	marker_css(&mut css, theme);

	for rule in theme.mark_rules() {
		let _ = write!(
			css,
			".cv-doc .cv-m-{}{{{}}}",
			class_name(&rule.mark),
			patch_css(&rule.patch)
		);
	}

	for rule in theme.embed_rules() {
		if let EmbedStyle::Label { patch, .. } = &rule.style {
			let _ = write!(
				css,
				".cv-doc .cv-e-{}{{{}}}",
				class_name(&rule.embed),
				patch_css(patch)
			);
		}
	}

	css
}

/// Every text property, resolved to absolute values.
fn text_css(style: &TextStyle) -> String {
	let font = &style.font;
	let mut css = format!(
		"font-family:{};font-size:{}px;font-weight:{};font-style:{};line-height:{};color:{};\
		 letter-spacing:{}px;",
		font.family,
		num(font.size),
		font.weight,
		font_style(font.italic),
		num(style.line_height),
		style.color,
		num(style.letter_spacing)
	);
	let decorations = style.decorations;

	if decorations.any() {
		css.push_str(&decoration_css(
			decorations.underline || decorations.squiggle,
			decorations.strike,
			decorations.squiggle,
		));
	}

	css
}

/// The CSS `font-style` keyword for an italic or upright face.
fn font_style(italic: bool) -> &'static str {
	if italic { "italic" } else { "normal" }
}

fn decoration_css(underline: bool, strike: bool, wavy: bool) -> String {
	let lines: Vec<&str> = [(underline, "underline"), (strike, "line-through")]
		.into_iter()
		.filter_map(|(on, name)| on.then_some(name))
		.collect();

	if lines.is_empty() {
		return String::new();
	}

	format!(
		"text-decoration-line:{};text-decoration-style:{};",
		lines.join(" "),
		if wavy { "wavy" } else { "solid" }
	)
}

/// Append the box properties of `style`: padding, left border, background,
/// corner radius, and a rule's line.
///
/// Each property is written only when the style sets it, so plain blocks
/// keep the reset from the contract's base rules and get no box
/// declarations at all.
fn box_css(css: &mut String, style: &BlockStyle) {
	let padding = style.padding;

	if padding != crate::Insets::ZERO {
		let _ = write!(
			css,
			"padding:{}px {}px {}px {}px;",
			num(padding.top),
			num(padding.right),
			num(padding.bottom),
			num(padding.left)
		);
	}

	if let Some(border) = style.border_left {
		let _ = write!(
			css,
			"border-left:{}px solid {};",
			num(border.width),
			border.color
		);
	}

	if let Some(background) = style.background {
		let _ = write!(css, "background-color:{background};");
	}

	if style.radius > 0.0 {
		let _ = write!(css, "border-radius:{}px;", num(style.radius));
	}

	if let LeafStyle::Rule { thickness, color } = style.leaf {
		let _ = write!(css, "height:{}px;background-color:{color};", num(thickness));
	}
}

fn marker_css(css: &mut String, theme: &Theme) {
	let markers = theme.markers();
	let color = markers
		.color
		.map(|color| format!("color:{color};"))
		.unwrap_or_default();
	let gap = num(markers.gap);
	let size = num(markers.checkbox);

	let _ = write!(
		css,
		".cv-doc .cv-mk,.cv-doc .cv-mk-t{{position:relative}}.cv-doc \
		 .cv-mk::before{{position:absolute;top:0;right:100%;margin-right:{gap}px;white-space:\
		 nowrap;{color}}}.cv-doc .cv-mk-o{{counter-increment:cv-n}}.cv-doc \
		 .cv-mk-o::before{{content:counter(cv-n) \".\"}}"
	);

	for open in [false, true] {
		let _ = write!(
			css,
			".cv-doc .cv-mk-d{}::before{{content:\"{}\"}}",
			usize::from(open),
			crate::document::disclosure(open)
		);
	}

	for (index, bullet) in markers.bullets.iter().enumerate() {
		let _ = write!(
			css,
			".cv-doc .cv-mk-b{index}::before{{content:\"{}\"}}",
			css_string(bullet)
		);
	}

	let _ = write!(
		css,
		".cv-doc .cv-check{{appearance:none;-webkit-appearance:none;position:absolute;right:100%;\
		 margin:0 {gap}px 0 0;width:{size}px;height:{size}px;box-sizing:border-box;border:1px \
		 solid #8a8580;border-radius:3px;background:none}}.cv-doc \
		 .cv-check:checked{{background:#2e2b29}}"
	);
}

/// CSS for a relative patch, as applied by a mark or label.
fn patch_css(patch: &TextPatch) -> String {
	let declarations = [
		patch
			.family
			.as_ref()
			.map(|family| format!("font-family:{family};")),
		patch.size.map(|size| {
			match size {
				FontSize::Px(px) => format!("font-size:{}px;", num(px)),
				FontSize::Em(em) => format!("font-size:{}em;", num(em)),
			}
		}),
		patch.weight.map(|weight| format!("font-weight:{weight};")),
		patch
			.italic
			.map(|italic| format!("font-style:{};", font_style(italic))),
		patch
			.line_height
			.map(|line_height| format!("line-height:{};", num(line_height))),
		patch.color.map(|color| format!("color:{color};")),
		patch
			.background
			.map(|background| format!("background-color:{background};")),
		patch
			.letter_spacing
			.map(|spacing| format!("letter-spacing:{}px;", num(spacing))),
		patch
			.baseline_shift
			.map(|em| format!("vertical-align:{}em;", num(em))),
	];
	let mut css: String = declarations.into_iter().flatten().collect();

	let underline = patch.underline.unwrap_or(false) || patch.squiggle.unwrap_or(false);
	css.push_str(&decoration_css(
		underline,
		patch.strike.unwrap_or(false),
		patch.squiggle.unwrap_or(false),
	));

	css
}

/// `tag` if it is a plain lowercase element name, otherwise `div`.
fn tag_name(tag: &str) -> &str {
	let valid = !tag.is_empty()
		&& tag
			.bytes()
			.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
		&& tag.as_bytes()[0].is_ascii_lowercase();

	if valid { tag } else { "div" }
}

fn is_void(tag: &str) -> bool {
	matches!(tag, "hr" | "br" | "img" | "input")
}

/// A type name reduced to characters safe in a class name.
fn class_name(name: &str) -> String {
	name.chars()
		.map(|character| {
			if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
				character
			} else {
				'_'
			}
		})
		.collect()
}

fn css_string(value: &str) -> String {
	value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Whether a link target is safe to emit: relative, or http(s) or mailto.
fn is_safe_url(href: &str) -> bool {
	let lower = href.trim_start().to_ascii_lowercase();
	let path_start = lower.find(['/', '?', '#']).unwrap_or(lower.len());
	// A colon after the path starts is part of the path, not a scheme.
	let has_scheme = lower.find(':').is_some_and(|end| end < path_start);

	!has_scheme
		|| SAFE_URL_SCHEMES
			.iter()
			.any(|scheme| lower.starts_with(scheme))
}

/// Whether a font family from document content is safe inside a style
/// attribute.
fn is_safe_family(family: &str) -> bool {
	family.chars().all(|character| {
		character.is_alphanumeric() || matches!(character, ' ' | ',' | '-' | '_' | '"' | '\'')
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn rejects_script_urls() {
		assert!(is_safe_url("https://example.com"));
		assert!(is_safe_url("/docs#top"));
		assert!(is_safe_url("mailto:a@b.c"));
		assert!(!is_safe_url("javascript:alert(1)"));
		assert!(!is_safe_url(" JavaScript:alert(1)"));
		assert!(!is_safe_url("data:text/html,x"));
	}

	#[test]
	fn sanitizes_names() {
		assert_eq!(tag_name("h2"), "h2");
		assert_eq!(tag_name("script onload"), "div");
		assert_eq!(class_name("bullet list"), "bullet_list");
		assert!(!is_safe_family("x;background:url(evil)"));
	}
}
