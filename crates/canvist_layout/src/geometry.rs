//! Rectangles, insets, colours, and the fixed-point unit for vertical
//! positions.

use std::fmt;

/// Fixed-point units per pixel for vertical positions.
///
/// Block tops and heights are summed in 1/64 px integers, like the
/// `LayoutUnit` that Chromium and WebKit use, so incremental and full layout
/// produce bit-identical positions whatever order heights are updated in.
pub(crate) const UNITS_PER_PX: f64 = 64.0;

/// Convert pixels to fixed-point units, rounding to the nearest unit.
pub(crate) fn to_units(px: f32) -> i64 {
	(f64::from(px) * UNITS_PER_PX).round() as i64
}

/// Convert fixed-point units back to pixels.
pub(crate) fn to_px(units: i64) -> f32 {
	(units as f64 / UNITS_PER_PX) as f32
}

/// Round `px` to the nearest fixed-point unit.
pub(crate) fn snap(px: f32) -> f32 {
	to_px(to_units(px))
}

/// An axis-aligned rectangle in document pixels.
///
/// Document coordinates start at the top-left of the document's content box.
/// They are `f32`, which keeps 1/64 px precision up to about 262,000 px and
/// sub-pixel precision past two million.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
	/// The left edge.
	pub x: f32,
	/// The top edge.
	pub y: f32,
	/// The width, never negative for rectangles built by this crate.
	pub width: f32,
	/// The height, never negative for rectangles built by this crate.
	pub height: f32,
}

impl Rect {
	/// A rectangle from its top-left corner and size.
	#[must_use]
	pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
		Self {
			x,
			y,
			width,
			height,
		}
	}

	/// The right edge.
	#[must_use]
	pub fn right(&self) -> f32 {
		self.x + self.width
	}

	/// The bottom edge.
	#[must_use]
	pub fn bottom(&self) -> f32 {
		self.y + self.height
	}

	/// Whether the rectangle covers no area.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.width <= 0.0 || self.height <= 0.0
	}

	/// The smallest rectangle containing both.
	#[must_use]
	pub fn union(&self, other: &Self) -> Self {
		let x = self.x.min(other.x);
		let y = self.y.min(other.y);

		Self::new(
			x,
			y,
			self.right().max(other.right()) - x,
			self.bottom().max(other.bottom()) - y,
		)
	}

	/// Whether the two rectangles overlap with positive area.
	#[must_use]
	pub fn intersects(&self, other: &Self) -> bool {
		self.x < other.right()
			&& other.x < self.right()
			&& self.y < other.bottom()
			&& other.y < self.bottom()
	}

	/// Whether the vertical span of the rectangle overlaps `top..bottom`.
	#[must_use]
	pub fn overlaps_rows(&self, top: f32, bottom: f32) -> bool {
		self.y < bottom && top < self.bottom()
	}
}

/// Space on each side of a box, in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
	/// Space above.
	pub top: f32,
	/// Space to the right.
	pub right: f32,
	/// Space below.
	pub bottom: f32,
	/// Space to the left.
	pub left: f32,
}

impl Insets {
	/// No space on any side.
	pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);

	/// Insets in CSS order: top, right, bottom, left.
	#[must_use]
	pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
		Self {
			top,
			right,
			bottom,
			left,
		}
	}

	/// Vertical and horizontal insets, like the two-value CSS shorthand.
	#[must_use]
	pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
		Self::new(vertical, horizontal, vertical, horizontal)
	}

	/// The sum of the left and right insets.
	#[must_use]
	pub fn horizontal(&self) -> f32 {
		self.left + self.right
	}

	/// The sum of the top and bottom insets.
	#[must_use]
	pub fn vertical(&self) -> f32 {
		self.top + self.bottom
	}
}

/// The digit count of `#rgb`.
const SHORT_HEX_LEN: usize = 3;
/// The digit count of `#rrggbb`.
const RGB_HEX_LEN: usize = 6;
/// The digit count of `#rrggbbaa`.
const RGBA_HEX_LEN: usize = 8;
/// Repeats a hex digit into both nibbles of a byte, so `#abc` means
/// `#aabbcc`: `0xa * 17 == 0xaa`.
const HEX_DIGIT_TO_BYTE: u8 = 17;

/// An sRGB colour with straight alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
	/// Red.
	pub r: u8,
	/// Green.
	pub g: u8,
	/// Blue.
	pub b: u8,
	/// Alpha, where 255 is opaque.
	pub a: u8,
}

impl Color {
	/// Fully transparent black.
	pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

	/// An opaque colour.
	#[must_use]
	pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
		Self::rgba(r, g, b, 255)
	}

	/// A colour with alpha.
	#[must_use]
	pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
		Self { r, g, b, a }
	}

	/// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa`. Returns `None` for anything
	/// else, including CSS colour names, which hosts must resolve themselves.
	#[must_use]
	pub fn parse_hex(input: &str) -> Option<Self> {
		let hex = input.strip_prefix('#')?;

		if !hex.is_ascii() {
			return None;
		}

		let channel = |range: std::ops::Range<usize>| u8::from_str_radix(hex.get(range)?, 16).ok();
		let short = |index: usize| channel(index..index + 1).map(|value| value * HEX_DIGIT_TO_BYTE);

		match hex.len() {
			SHORT_HEX_LEN => Some(Self::rgb(short(0)?, short(1)?, short(2)?)),
			RGB_HEX_LEN => Some(Self::rgb(channel(0..2)?, channel(2..4)?, channel(4..6)?)),
			RGBA_HEX_LEN => {
				Some(Self::rgba(
					channel(0..2)?,
					channel(2..4)?,
					channel(4..6)?,
					channel(6..8)?,
				))
			}
			_ => None,
		}
	}
}

/// Formats as CSS: `#rrggbb` when opaque, `rgba(r, g, b, a)` otherwise.
impl fmt::Display for Color {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.a == 255 {
			return write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b);
		}

		let alpha = f32::from(self.a) / 255.0;
		write!(f, "rgba({}, {}, {}, {alpha:.3})", self.r, self.g, self.b)
	}
}

#[cfg(test)]
#[allow(
	clippy::float_cmp,
	reason = "the values compared are exactly representable"
)]
mod tests {
	use super::*;

	#[test]
	fn units_round_trip_and_snap() {
		assert_eq!(to_units(1.0), 64);
		assert_eq!(to_px(96), 1.5);
		assert_eq!(snap(25.6), 1638.0 / 64.0);
	}

	#[test]
	fn rect_union_and_intersection() {
		const SIDE: f32 = 10.0;
		let half = SIDE / 2.0;
		let square = Rect::new(0.0, 0.0, SIDE, SIDE);
		let overlapping = Rect::new(half, half, SIDE, SIDE);
		let touching = Rect::new(SIDE, 0.0, half, half);

		assert_eq!(
			square.union(&overlapping),
			Rect::new(0.0, 0.0, SIDE + half, SIDE + half)
		);
		assert!(square.intersects(&overlapping));
		assert!(
			!square.intersects(&touching),
			"touching edges do not overlap"
		);
	}

	#[test]
	fn colors_parse_and_format_as_css() {
		// `0x80`, which formats as an alpha of 0.502.
		const HALF_ALPHA: u8 = 128;

		assert_eq!(Color::parse_hex("#fff"), Some(Color::rgb(255, 255, 255)));
		assert_eq!(Color::parse_hex("#2e2b29"), Some(Color::rgb(46, 43, 41)));
		assert_eq!(
			Color::parse_hex("#00000080"),
			Some(Color::rgba(0, 0, 0, HALF_ALPHA))
		);
		assert_eq!(Color::parse_hex("red"), None);
		assert_eq!(Color::parse_hex("#ééé"), None);
		assert_eq!(Color::rgb(46, 43, 41).to_string(), "#2e2b29");
		assert_eq!(
			Color::rgba(0, 0, 0, HALF_ALPHA).to_string(),
			"rgba(0, 0, 0, 0.502)"
		);
	}
}
