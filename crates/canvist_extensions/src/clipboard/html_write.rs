//! Slices to HTML, in the markup TipTap renders.

use canvist_editor::Slice;
use canvist_model::Attrs;
use canvist_model::BlockTree;
use canvist_model::InlineText;
use canvist_model::Mark;
use canvist_model::Schema;
use canvist_model::TreeContent;
use canvist_model::Value;

use super::tree::HARD_BREAK;

/// Serialize a slice to HTML: `<p>`, `<h1>`–`<h6>`, `<blockquote>`,
/// `<pre><code>`, `<ul>`, `<ol>`, `<li>`, `<hr>`, and inline `<strong>`,
/// `<em>`, `<u>`, `<s>`, `<code>`, `<a>`, `<sub>`, `<sup>`, `<mark>`,
/// `<span style>` for text styles, and `<br>`. Task lists use
/// TipTap's `data-type` and `data-checked` attributes. Unknown blocks are
/// written as `<div data-type>` around their content.
#[must_use]
pub fn to_html(schema: &Schema, slice: &Slice) -> String {
	let mut html = String::new();

	for tree in slice.blocks() {
		write_block(schema, tree, &mut html);
	}

	html
}

fn attr_int(attrs: &Attrs, key: &str) -> Option<i64> {
	attrs.get(key).and_then(Value::as_int)
}

fn write_block(schema: &Schema, tree: &BlockTree, html: &mut String) {
	let name = schema.block(tree.kind).name.as_ref();

	match &tree.content {
		TreeContent::Inline(text) => write_text_block(schema, name, &tree.attrs, text, html),
		TreeContent::Blocks(children) => {
			let (open, close) = container_tags(name, &tree.attrs);
			html.push_str(&open);

			for child in children {
				write_block(schema, child, html);
			}

			html.push_str(close);
		}

		TreeContent::Leaf if name == "horizontalRule" => html.push_str("<hr>"),
		TreeContent::Leaf => {
			html.push_str("<div data-type=\"");
			html.push_str(&escape(name));
			html.push_str("\"></div>");
		}
	}
}

fn container_tags(name: &str, attrs: &Attrs) -> (String, &'static str) {
	match name {
		"blockquote" => ("<blockquote>".to_owned(), "</blockquote>"),
		"bulletList" => ("<ul>".to_owned(), "</ul>"),
		"orderedList" => {
			match attr_int(attrs, "start").filter(|&start| start != 1) {
				Some(start) => (format!("<ol start=\"{start}\">"), "</ol>"),
				None => ("<ol>".to_owned(), "</ol>"),
			}
		}
		"listItem" => ("<li>".to_owned(), "</li>"),
		"taskList" => ("<ul data-type=\"taskList\">".to_owned(), "</ul>"),
		"taskItem" => {
			let checked = attrs.get("checked").and_then(Value::as_bool) == Some(true);
			let open = format!("<li data-type=\"taskItem\" data-checked=\"{checked}\">");
			(open, "</li>")
		}
		other => (format!("<div data-type=\"{}\">", escape(other)), "</div>"),
	}
}

fn write_text_block(
	schema: &Schema,
	name: &str,
	attrs: &Attrs,
	text: &InlineText,
	html: &mut String,
) {
	match name {
		"heading" => {
			let level = attr_int(attrs, "level")
				.unwrap_or(1)
				.clamp(1, 6)
				.to_string();
			html.push_str("<h");
			html.push_str(&level);
			if cfg!(feature = "text-align") {
				html.push_str(&align_style(attrs));
			}
			html.push('>');
			write_inline(schema, text, html);
			html.push_str("</h");
			html.push_str(&level);
			html.push('>');
		}
		"codeBlock" => {
			match attrs.get("language").and_then(Value::as_str) {
				Some(language) => {
					html.push_str("<pre><code class=\"language-");
					html.push_str(&escape(language));
					html.push_str("\">");
				}
				None => html.push_str("<pre><code>"),
			}

			html.push_str(&escape(&super::tree::plain_text(schema, text)));
			html.push_str("</code></pre>");
		}
		_ => {
			html.push_str("<p");
			if cfg!(feature = "text-align") {
				html.push_str(&align_style(attrs));
			}
			html.push('>');
			write_inline(schema, text, html);
			html.push_str("</p>");
		}
	}
}

/// A ` style="text-align: …"` attribute for an aligned block, as TipTap
/// writes it, or nothing.
fn align_style(attrs: &Attrs) -> String {
	attrs
		.get("textAlign")
		.and_then(Value::as_str)
		.map(|align| format!(" style=\"text-align: {}\"", escape(align)))
		.unwrap_or_default()
}

/// The opening and closing tags for one mark.
fn mark_tags(schema: &Schema, mark: &Mark) -> (String, &'static str) {
	match schema.mark(mark.kind).name.as_ref() {
		"bold" => ("<strong>".to_owned(), "</strong>"),
		"italic" => ("<em>".to_owned(), "</em>"),
		"underline" => ("<u>".to_owned(), "</u>"),
		"strike" => ("<s>".to_owned(), "</s>"),
		"code" => ("<code>".to_owned(), "</code>"),
		"link" => {
			let href = mark
				.attrs
				.get("href")
				.and_then(Value::as_str)
				.unwrap_or_default();
			(format!("<a href=\"{}\">", escape(href)), "</a>")
		}
		"subscript" => ("<sub>".to_owned(), "</sub>"),
		"superscript" => ("<sup>".to_owned(), "</sup>"),
		"highlight" => (highlight_tag(&mark.attrs), "</mark>"),
		"textStyle" => (text_style_tag(&mark.attrs), "</span>"),
		other => (format!("<span data-mark=\"{}\">", escape(other)), "</span>"),
	}
}

/// `<mark>`, with TipTap's `data-color` and inline background when the
/// highlight has a colour.
fn highlight_tag(attrs: &Attrs) -> String {
	match attrs.get("color").and_then(Value::as_str) {
		Some(color) => {
			let color = escape(color);
			format!(
				"<mark data-color=\"{color}\" style=\"background-color: {color}; color: inherit\">"
			)
		}
		None => "<mark>".to_owned(),
	}
}

/// The CSS properties of `textStyle` attributes, in the order TipTap writes
/// them.
pub(super) const TEXT_STYLE_PROPERTIES: [(&str, &str); 5] = [
	("color", "color"),
	("backgroundColor", "background-color"),
	("fontFamily", "font-family"),
	("fontSize", "font-size"),
	("lineHeight", "line-height"),
];

/// `<span>` with a `style` declaration per set `textStyle` attribute.
fn text_style_tag(attrs: &Attrs) -> String {
	let declarations: Vec<String> = TEXT_STYLE_PROPERTIES
		.iter()
		.filter_map(|(attr, property)| {
			let value = attrs.get(attr)?.as_str()?;
			Some(format!("{property}: {value}"))
		})
		.collect();

	if declarations.is_empty() {
		return "<span>".to_owned();
	}

	format!("<span style=\"{}\">", escape(&declarations.join("; ")))
}

fn write_inline(schema: &Schema, text: &InlineText, html: &mut String) {
	for run in text.runs() {
		let tags: Vec<_> = run
			.marks
			.as_slice()
			.iter()
			.map(|mark| mark_tags(schema, mark))
			.collect();

		for (open, _) in &tags {
			html.push_str(open);
		}

		match run.embed {
			Some(embed) if schema.embed(embed.kind).name.as_ref() == HARD_BREAK => {
				html.push_str("<br>");
			}

			Some(embed) => {
				html.push_str("<span data-type=\"");
				html.push_str(&escape(&schema.embed(embed.kind).name));
				html.push_str("\"></span>");
			}
			None => html.push_str(&escape(run.text)),
		}

		for (_, close) in tags.iter().rev() {
			html.push_str(close);
		}
	}
}

/// Escape text for HTML content and attribute values.
pub(super) fn escape(text: &str) -> String {
	let mut escaped = String::with_capacity(text.len());

	for character in text.chars() {
		match character {
			'&' => escaped.push_str("&amp;"),
			'<' => escaped.push_str("&lt;"),
			'>' => escaped.push_str("&gt;"),
			'"' => escaped.push_str("&quot;"),
			other => escaped.push(other),
		}
	}

	escaped
}
