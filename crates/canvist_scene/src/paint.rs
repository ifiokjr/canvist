//! Paints, strokes, effects, and blend modes.

use std::sync::Arc;

use kurbo::Point;
use kurbo::Vec2;

/// An sRGB colour with straight (not premultiplied) alpha, eight bits per
/// channel. Formats as `#rrggbbaa`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color {
	/// Red.
	pub r: u8,
	/// Green.
	pub g: u8,
	/// Blue.
	pub b: u8,
	/// Alpha.
	pub a: u8,
}

impl Color {
	/// Opaque black.
	pub const BLACK: Self = Self::rgb(0, 0, 0);
	/// Fully transparent black.
	pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
	/// Opaque white.
	pub const WHITE: Self = Self::rgb(255, 255, 255);

	/// An opaque colour.
	#[must_use]
	pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
		Self { r, g, b, a: 255 }
	}

	/// A colour with alpha.
	#[must_use]
	pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
		Self { r, g, b, a }
	}

	/// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa`.
	#[must_use]
	pub fn parse_hex(text: &str) -> Option<Self> {
		let hex = text.strip_prefix('#')?;
		let channel = |at: usize, width: usize| -> Option<u8> {
			let value = u8::from_str_radix(hex.get(at..at + width)?, 16).ok()?;
			Some(if width == 1 { value * 17 } else { value })
		};

		match hex.len() {
			3 => Some(Self::rgb(channel(0, 1)?, channel(1, 1)?, channel(2, 1)?)),
			6 => Some(Self::rgb(channel(0, 2)?, channel(2, 2)?, channel(4, 2)?)),
			8 => {
				Some(Self::rgba(
					channel(0, 2)?,
					channel(2, 2)?,
					channel(4, 2)?,
					channel(6, 2)?,
				))
			}
			_ => None,
		}
	}

	/// `#rrggbbaa`.
	#[must_use]
	pub fn to_hex(self) -> String {
		format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
	}

	/// The colour with its alpha multiplied by `opacity` (0 to 1).
	#[must_use]
	pub fn with_opacity(self, opacity: f64) -> Self {
		let alpha = (f64::from(self.a) * opacity.clamp(0.0, 1.0)).round() as u8;
		Self { a: alpha, ..self }
	}
}

/// One colour stop of a gradient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
	/// Where the stop sits along the gradient, 0 to 1.
	pub offset: f64,
	/// The colour at the stop.
	pub color: Color,
}

/// How an image paint fills its node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageFit {
	/// Scale to cover the node, cropping the overflow.
	#[default]
	Fill,
	/// Scale to fit inside the node, letterboxing the rest.
	Fit,
	/// Stretch to the node's exact size.
	Stretch,
	/// Repeat at the image's natural size.
	Tile,
}

/// What a paint draws. Gradient geometry is in the node's unit box: `(0, 0)`
/// is its top-left corner and `(1, 1)` its bottom-right, so gradients follow
/// resizes.
#[derive(Clone, Debug, PartialEq)]
pub enum PaintKind {
	/// A flat colour.
	Solid(Color),
	/// A linear gradient from `from` to `to`.
	Linear {
		/// The start point, in the unit box.
		from: Point,
		/// The end point, in the unit box.
		to: Point,
		/// The colour stops, in increasing offset order.
		stops: Arc<[GradientStop]>,
	},
	/// A radial gradient.
	Radial {
		/// The centre, in the unit box.
		center: Point,
		/// The horizontal and vertical radii, in the unit box.
		radius: Vec2,
		/// The colour stops.
		stops: Arc<[GradientStop]>,
	},
	/// A sweep (conic) gradient.
	Sweep {
		/// The centre, in the unit box.
		center: Point,
		/// The start angle in degrees, clockwise from the positive x axis.
		angle: f64,
		/// The colour stops.
		stops: Arc<[GradientStop]>,
	},
	/// An image, by asset id (for example a content hash the host resolves).
	Image {
		/// The asset id.
		asset: Arc<str>,
		/// How it fills the node.
		fit: ImageFit,
	},
}

/// One entry in a node's fills or strokes.
#[derive(Clone, Debug, PartialEq)]
pub struct Paint {
	/// What to draw.
	pub kind: PaintKind,
	/// The paint's own opacity, 0 to 1.
	pub opacity: f64,
	/// Whether the paint is drawn. Hidden paints stay in the list.
	pub visible: bool,
	/// How the paint blends with the paints below it.
	pub blend: BlendMode,
}

impl Paint {
	/// A visible, opaque solid paint.
	#[must_use]
	pub fn solid(color: Color) -> Self {
		Self::new(PaintKind::Solid(color))
	}

	/// A visible, opaque paint of any kind.
	#[must_use]
	pub fn new(kind: PaintKind) -> Self {
		Self {
			kind,
			opacity: 1.0,
			visible: true,
			blend: BlendMode::Normal,
		}
	}
}

/// Where a stroke sits relative to the outline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StrokeAlign {
	/// Entirely inside the outline.
	Inside,
	/// Centred on the outline.
	#[default]
	Center,
	/// Entirely outside the outline.
	Outside,
}

/// How open stroke ends are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StrokeCap {
	/// Flat, ending at the endpoint.
	#[default]
	Butt,
	/// A semicircle past the endpoint.
	Round,
	/// A half square past the endpoint.
	Square,
}

/// How stroke corners are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StrokeJoin {
	/// Sharp, up to the miter limit.
	#[default]
	Miter,
	/// Rounded.
	Round,
	/// Cut off.
	Bevel,
}

/// The geometry of a node's strokes. The paints come from `strokes`.
#[derive(Clone, Debug, PartialEq)]
pub struct StrokeStyle {
	/// The stroke width.
	pub width: f64,
	/// Where the stroke sits.
	pub align: StrokeAlign,
	/// The end caps.
	pub cap: StrokeCap,
	/// The corner joins.
	pub join: StrokeJoin,
	/// The miter limit, as a ratio of the width.
	pub miter_limit: f64,
	/// Alternating dash and gap lengths; empty for a solid stroke.
	pub dashes: Arc<[f64]>,
}

impl Default for StrokeStyle {
	fn default() -> Self {
		Self {
			width: 1.0,
			align: StrokeAlign::Center,
			cap: StrokeCap::Butt,
			join: StrokeJoin::Miter,
			miter_limit: 4.0,
			dashes: Arc::new([]),
		}
	}
}

/// A shadow's parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
	/// The shadow colour.
	pub color: Color,
	/// The offset from the node.
	pub offset: Vec2,
	/// The blur radius.
	pub blur: f64,
	/// How far the shadow grows (positive) or shrinks before blurring.
	pub spread: f64,
}

/// A visual effect applied to a node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EffectKind {
	/// A shadow behind the node.
	DropShadow(Shadow),
	/// A shadow inside the node's outline.
	InnerShadow(Shadow),
	/// Blur the node itself.
	LayerBlur {
		/// The blur radius.
		radius: f64,
	},
	/// Blur what is behind the node, inside its outline.
	BackgroundBlur {
		/// The blur radius.
		radius: f64,
	},
}

/// One entry in a node's effects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effect {
	/// The effect.
	pub kind: EffectKind,
	/// Whether it is drawn.
	pub visible: bool,
}

/// How a node or paint composites with what is below it: the separable and
/// non-separable blend modes of the W3C compositing spec.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlendMode {
	/// Source over.
	#[default]
	Normal,
	/// Multiply.
	Multiply,
	/// Screen.
	Screen,
	/// Overlay.
	Overlay,
	/// Darken.
	Darken,
	/// Lighten.
	Lighten,
	/// Colour dodge.
	ColorDodge,
	/// Colour burn.
	ColorBurn,
	/// Hard light.
	HardLight,
	/// Soft light.
	SoftLight,
	/// Difference.
	Difference,
	/// Exclusion.
	Exclusion,
	/// Hue.
	Hue,
	/// Saturation.
	Saturation,
	/// Colour.
	Color,
	/// Luminosity.
	Luminosity,
}

impl BlendMode {
	/// Every blend mode, in declaration order.
	pub const ALL: [Self; 16] = [
		Self::Normal,
		Self::Multiply,
		Self::Screen,
		Self::Overlay,
		Self::Darken,
		Self::Lighten,
		Self::ColorDodge,
		Self::ColorBurn,
		Self::HardLight,
		Self::SoftLight,
		Self::Difference,
		Self::Exclusion,
		Self::Hue,
		Self::Saturation,
		Self::Color,
		Self::Luminosity,
	];

	/// The CSS `mix-blend-mode` name, which is also the JSON name.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::Normal => "normal",
			Self::Multiply => "multiply",
			Self::Screen => "screen",
			Self::Overlay => "overlay",
			Self::Darken => "darken",
			Self::Lighten => "lighten",
			Self::ColorDodge => "color-dodge",
			Self::ColorBurn => "color-burn",
			Self::HardLight => "hard-light",
			Self::SoftLight => "soft-light",
			Self::Difference => "difference",
			Self::Exclusion => "exclusion",
			Self::Hue => "hue",
			Self::Saturation => "saturation",
			Self::Color => "color",
			Self::Luminosity => "luminosity",
		}
	}

	/// The blend mode with this CSS name.
	#[must_use]
	pub fn from_name(name: &str) -> Option<Self> {
		Self::ALL.into_iter().find(|mode| mode.name() == name)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn hex_round_trips() {
		let color = Color::rgba(59, 44, 245, 128);

		assert_eq!(color.to_hex(), "#3b2cf580");
		assert_eq!(Color::parse_hex("#3b2cf580"), Some(color));
		assert_eq!(Color::parse_hex("#fff"), Some(Color::WHITE));
		assert_eq!(Color::parse_hex("#3b2cf5"), Some(Color::rgb(59, 44, 245)));
		assert_eq!(Color::parse_hex("3b2cf5"), None);
		assert_eq!(Color::parse_hex("#3b2cf"), None);
	}

	#[test]
	fn blend_names_round_trip() {
		for mode in BlendMode::ALL {
			assert_eq!(BlendMode::from_name(mode.name()), Some(mode));
		}
	}
}
