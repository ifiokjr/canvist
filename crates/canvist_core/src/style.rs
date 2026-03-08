//! Text styling primitives for the canvist editor.
//!
//! Provides a composable [`Style`] type that describes how text should be
//! rendered. Styles are designed to be built fluently:
//!
//! ```
//! use canvist_core::Style;
//!
//! let heading = Style::new()
//!     .bold()
//!     .font_size(24.0)
//!     .font_family("Inter")
//!     .color(0x1A, 0x1A, 0x2E, 0xFF);
//! ```

use serde::Deserialize;
use serde::Serialize;

/// Describes how a run of text should be rendered.
///
/// All fields are optional — [`None`] means "inherit from the parent context".
/// This allows styles to be composed and merged cleanly.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Style {
	/// Font family name (e.g. `"Inter"`, `"Georgia"`).
	pub font_family: Option<String>,

	/// Font size in logical pixels.
	pub font_size: Option<f32>,

	/// Font weight (400 = normal, 700 = bold).
	pub font_weight: Option<FontWeight>,

	/// Whether the text is italic.
	pub italic: Option<bool>,

	/// Whether the text has an underline.
	pub underline: Option<bool>,

	/// Whether the text has a strikethrough.
	pub strikethrough: Option<bool>,

	/// Text color as RGBA.
	pub color: Option<Color>,

	/// Background highlight color as RGBA.
	pub background: Option<Color>,

	/// Line height multiplier (e.g. `1.5` for 150%).
	pub line_height: Option<f32>,

	/// Letter spacing in logical pixels.
	pub letter_spacing: Option<f32>,

	/// Text alignment within the paragraph.
	pub text_align: Option<TextAlign>,
}

impl Style {
	/// Create a new empty style with all fields set to [`None`].
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the font family.
	#[must_use]
	pub fn font_family(mut self, family: impl Into<String>) -> Self {
		self.font_family = Some(family.into());
		self
	}

	/// Set the font size in logical pixels.
	#[must_use]
	pub fn font_size(mut self, size: f32) -> Self {
		self.font_size = Some(size);
		self
	}

	/// Set the font weight.
	#[must_use]
	pub fn font_weight(mut self, weight: FontWeight) -> Self {
		self.font_weight = Some(weight);
		self
	}

	/// Make the text bold (weight 700).
	#[must_use]
	pub fn bold(self) -> Self {
		self.font_weight(FontWeight::Bold)
	}

	/// Make the text italic.
	#[must_use]
	pub fn italic(mut self) -> Self {
		self.italic = Some(true);
		self
	}

	/// Add an underline decoration.
	#[must_use]
	pub fn underline(mut self) -> Self {
		self.underline = Some(true);
		self
	}

	/// Add a strikethrough decoration.
	#[must_use]
	pub fn strikethrough(mut self) -> Self {
		self.strikethrough = Some(true);
		self
	}

	/// Set the text color from RGBA components.
	#[must_use]
	pub fn color(mut self, r: u8, g: u8, b: u8, a: u8) -> Self {
		self.color = Some(Color { r, g, b, a });
		self
	}

	/// Set the background highlight color from RGBA components.
	#[must_use]
	pub fn background(mut self, r: u8, g: u8, b: u8, a: u8) -> Self {
		self.background = Some(Color { r, g, b, a });
		self
	}

	/// Set the line height multiplier.
	#[must_use]
	pub fn line_height(mut self, multiplier: f32) -> Self {
		self.line_height = Some(multiplier);
		self
	}

	/// Set the letter spacing in logical pixels.
	#[must_use]
	pub fn letter_spacing(mut self, spacing: f32) -> Self {
		self.letter_spacing = Some(spacing);
		self
	}

	/// Set the text alignment.
	#[must_use]
	pub fn text_align(mut self, align: TextAlign) -> Self {
		self.text_align = Some(align);
		self
	}

	/// Merge another style on top of this one.
	///
	/// Fields from `other` take priority when they are [`Some`].
	#[must_use]
	pub fn merge(&self, other: &Self) -> Self {
		Self {
			font_family: other.font_family.clone().or_else(|| self.font_family.clone()),
			font_size: other.font_size.or(self.font_size),
			font_weight: other.font_weight.or(self.font_weight),
			italic: other.italic.or(self.italic),
			underline: other.underline.or(self.underline),
			strikethrough: other.strikethrough.or(self.strikethrough),
			color: other.color.or(self.color),
			background: other.background.or(self.background),
			line_height: other.line_height.or(self.line_height),
			letter_spacing: other.letter_spacing.or(self.letter_spacing),
			text_align: other.text_align.or(self.text_align),
		}
	}

	/// Return a resolved style with sensible defaults for any unset fields.
	#[must_use]
	pub fn resolve(&self) -> ResolvedStyle {
		ResolvedStyle {
			font_family: self
				.font_family
				.clone()
				.unwrap_or_else(|| String::from("sans-serif")),
			font_size: self.font_size.unwrap_or(16.0),
			font_weight: self.font_weight.unwrap_or(FontWeight::Normal),
			italic: self.italic.unwrap_or(false),
			underline: self.underline.unwrap_or(false),
			strikethrough: self.strikethrough.unwrap_or(false),
			color: self.color.unwrap_or(Color::BLACK),
			background: self.background.unwrap_or(Color::TRANSPARENT),
			line_height: self.line_height.unwrap_or(1.5),
			letter_spacing: self.letter_spacing.unwrap_or(0.0),
			text_align: self.text_align.unwrap_or(TextAlign::Left),
		}
	}
}

/// A fully resolved style with no optional fields.
///
/// Created by calling [`Style::resolve`]. Every field has a concrete value
/// ready for rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedStyle {
	/// Font family name.
	pub font_family: String,
	/// Font size in logical pixels.
	pub font_size: f32,
	/// Font weight.
	pub font_weight: FontWeight,
	/// Whether italic.
	pub italic: bool,
	/// Whether underlined.
	pub underline: bool,
	/// Whether struck through.
	pub strikethrough: bool,
	/// Text color.
	pub color: Color,
	/// Background color.
	pub background: Color,
	/// Line height multiplier.
	pub line_height: f32,
	/// Letter spacing in logical pixels.
	pub letter_spacing: f32,
	/// Text alignment.
	pub text_align: TextAlign,
}

/// Font weight as a numeric value.
///
/// Follows the CSS `font-weight` convention where 400 is normal and 700 is
/// bold.
#[derive(
	Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum FontWeight {
	/// Thin (100).
	Thin,
	/// Extra Light (200).
	ExtraLight,
	/// Light (300).
	Light,
	/// Normal / Regular (400).
	#[default]
	Normal,
	/// Medium (500).
	Medium,
	/// Semi Bold (600).
	SemiBold,
	/// Bold (700).
	Bold,
	/// Extra Bold (800).
	ExtraBold,
	/// Black (900).
	Black,
}

impl FontWeight {
	/// Return the numeric CSS weight value.
	#[must_use]
	pub fn as_u16(self) -> u16 {
		match self {
			Self::Thin => 100,
			Self::ExtraLight => 200,
			Self::Light => 300,
			Self::Normal => 400,
			Self::Medium => 500,
			Self::SemiBold => 600,
			Self::Bold => 700,
			Self::ExtraBold => 800,
			Self::Black => 900,
		}
	}
}



/// An RGBA color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
	/// Red channel (0–255).
	pub r: u8,
	/// Green channel (0–255).
	pub g: u8,
	/// Blue channel (0–255).
	pub b: u8,
	/// Alpha channel (0–255, where 255 is fully opaque).
	pub a: u8,
}

impl Color {
	/// Fully opaque black.
	pub const BLACK: Self = Self {
		r: 0,
		g: 0,
		b: 0,
		a: 255,
	};

	/// Fully opaque white.
	pub const WHITE: Self = Self {
		r: 255,
		g: 255,
		b: 255,
		a: 255,
	};

	/// Fully transparent.
	pub const TRANSPARENT: Self = Self {
		r: 0,
		g: 0,
		b: 0,
		a: 0,
	};

	/// Create a new color from RGBA components.
	#[must_use]
	pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
		Self { r, g, b, a }
	}

	/// Create a fully opaque color from RGB components.
	#[must_use]
	pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
		Self { r, g, b, a: 255 }
	}

	/// Convert to a CSS-style `rgba(r, g, b, a)` string.
	#[must_use]
	pub fn to_css(&self) -> String {
		if self.a == 255 {
			format!("rgb({}, {}, {})", self.r, self.g, self.b)
		} else {
			let alpha = f64::from(self.a) / 255.0;
			format!("rgba({}, {}, {}, {alpha:.3})", self.r, self.g, self.b)
		}
	}
}

impl Default for Color {
	fn default() -> Self {
		Self::BLACK
	}
}

/// Text alignment within a paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TextAlign {
	/// Align text to the left edge.
	#[default]
	Left,
	/// Center text horizontally.
	Center,
	/// Align text to the right edge.
	Right,
	/// Justify text to fill the available width.
	Justify,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn style_builder_chain() {
		let style = Style::new()
			.bold()
			.italic()
			.font_size(24.0)
			.font_family("Georgia")
			.color(255, 0, 0, 255);

		assert_eq!(style.font_weight, Some(FontWeight::Bold));
		assert_eq!(style.italic, Some(true));
		assert_eq!(style.font_size, Some(24.0));
		assert_eq!(style.font_family.as_deref(), Some("Georgia"));
		assert_eq!(style.color, Some(Color::new(255, 0, 0, 255)));
	}

	#[test]
	fn style_merge_prefers_other() {
		let base = Style::new().font_size(16.0).bold();
		let overlay = Style::new().font_size(24.0).italic();
		let merged = base.merge(&overlay);

		assert_eq!(merged.font_size, Some(24.0));
		assert_eq!(merged.font_weight, Some(FontWeight::Bold));
		assert_eq!(merged.italic, Some(true));
	}

	#[test]
	fn resolve_provides_defaults() {
		let resolved = Style::new().resolve();
		assert_eq!(resolved.font_family, "sans-serif");
		assert!((resolved.font_size - 16.0).abs() < f32::EPSILON);
		assert_eq!(resolved.font_weight, FontWeight::Normal);
		assert!(!resolved.italic);
	}

	#[test]
	fn color_to_css() {
		assert_eq!(Color::BLACK.to_css(), "rgb(0, 0, 0)");
		assert_eq!(Color::new(255, 128, 0, 128).to_css(), "rgba(255, 128, 0, 0.502)");
	}
}
