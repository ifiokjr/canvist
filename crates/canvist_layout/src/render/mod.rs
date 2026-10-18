//! Server-side renderers: SVG from a display list, and semantic HTML plus
//! CSS from a document.

use std::fmt::Write;

mod html;
mod svg;

pub use html::HtmlDocument;
pub use html::HtmlOptions;
pub use html::to_html;
pub use svg::to_svg;

/// Format a pixel value compactly: at most three decimals, no trailing
/// zeros, and no `-0`.
pub(crate) fn num(value: f32) -> String {
	let rounded = (f64::from(value) * 1000.0).round() / 1000.0;

	if rounded == 0.0 {
		return "0".to_owned();
	}

	let mut out = format!("{rounded:.3}");

	while out.ends_with('0') {
		out.pop();
	}

	if out.ends_with('.') {
		out.pop();
	}

	out
}

/// Append `text` escaped for HTML and XML text content and attribute values.
pub(crate) fn escape_into(out: &mut String, text: &str) {
	for character in text.chars() {
		match character {
			'&' => out.push_str("&amp;"),
			'<' => out.push_str("&lt;"),
			'>' => out.push_str("&gt;"),
			'"' => out.push_str("&quot;"),
			'\'' => out.push_str("&#39;"),
			_ => out.push(character),
		}
	}
}

/// Write ` name="value"` with `value` escaped.
pub(crate) fn attribute(out: &mut String, name: &str, value: &str) {
	let _ = write!(out, " {name}=\"");
	escape_into(out, value);
	out.push('"');
}

#[cfg(test)]
#[allow(
	clippy::float_cmp,
	reason = "the values compared are exactly representable"
)]
mod tests {
	use super::*;

	#[test]
	fn numbers_are_compact() {
		assert_eq!(num(16.0), "16");
		assert_eq!(num(25.59375), "25.594");
		assert_eq!(num(-0.0001), "0");
		assert_eq!(num(0.5), "0.5");
	}

	#[test]
	fn escapes_markup() {
		let mut out = String::new();
		escape_into(&mut out, "<a href=\"x\">&'");
		assert_eq!(out, "&lt;a href=&quot;x&quot;&gt;&amp;&#39;");
	}
}
