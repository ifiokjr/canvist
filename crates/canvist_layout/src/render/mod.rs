//! Server-side renderers: SVG from a display list, and semantic HTML plus
//! CSS from a document.

use std::fmt::Write;

mod html;
mod svg;

pub use html::HtmlDocument;
pub use html::HtmlOptions;
pub use html::to_html;
pub use svg::to_svg;

/// Format a CSS number compactly: at most three decimals, no trailing
/// zeros, and no `-0`.
#[must_use]
pub fn css_number(value: f32) -> String {
	let mut out = fixed(f64::from(value), 3);

	while out.ends_with('0') {
		out.pop();
	}

	if out.ends_with('.') {
		out.pop();
	}

	if out == "-0" { "0".to_owned() } else { out }
}

pub(crate) use css_number as num;

/// `value` with exactly `decimals` digits after the point, rounded half
/// away from zero, like `format!("{value:.3}")` for three decimals.
///
/// Written with integer arithmetic: `core`'s float formatting costs every
/// WebAssembly build of canvist about 20 KB, and this is the only float
/// formatting layout needs.
pub(crate) fn fixed(value: f64, decimals: u32) -> String {
	let scale = 10_u64.pow(decimals);
	// Pixel values are far below 2^53 / 1000, so the product is exact enough.
	#[allow(
		clippy::cast_possible_truncation,
		clippy::cast_sign_loss,
		clippy::cast_precision_loss,
		reason = "rounded, non-negative, and in range for CSS lengths"
	)]
	let scaled = (value.abs() * scale as f64).round() as u64;
	let sign = if value < 0.0 && scaled != 0 { "-" } else { "" };
	let whole = scaled / scale;

	if decimals == 0 {
		return format!("{sign}{whole}");
	}

	let fraction = scaled % scale;
	let width = decimals as usize;

	format!("{sign}{whole}.{fraction:0width$}")
}

#[cfg(test)]
mod number_tests {
	use super::*;

	#[test]
	fn formats_numbers_without_float_formatting() {
		assert_eq!(css_number(16.0), "16");
		assert_eq!(css_number(13.5), "13.5");
		assert_eq!(css_number(0.0625), "0.063");
		assert_eq!(css_number(-0.0001), "0");
		assert_eq!(css_number(-1.25), "-1.25");
		assert_eq!(fixed(0.5, 3), "0.500");
		assert_eq!(fixed(2.0 / 3.0, 3), "0.667");
		assert_eq!(fixed(12.0, 0), "12");
	}
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

/// An SVG path through `points`, `x, y` pairs, as straight segments. A
/// single point is a zero-length segment, which round caps draw as a dot.
pub(crate) fn stroke_path(points: &[f32]) -> String {
	use std::fmt::Write as _;

	let mut path = String::new();

	for (index, point) in points.chunks_exact(2).enumerate() {
		let command = if index == 0 { 'M' } else { 'L' };
		let _ = write!(path, "{command}{} {}", num(point[0]), num(point[1]));
	}

	if points.len() == 2 {
		let _ = write!(path, "L{} {}", num(points[0]), num(points[1]));
	}

	path
}
