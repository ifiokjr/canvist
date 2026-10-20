//! The painting contract every backend implements.
//!
//! The scene walker turns a scene into calls on a [`Painter`]: fills,
//! strokes, layers, images, text runs, and box shadows, in screen space.
//! vello (GPU), `vello_cpu` (PNG and servers), and SVG each implement it, so
//! every backend draws the same thing.

use canvist_layout::FontSpec;
use canvist_layout::TextRun;
use kurbo::Affine;
use kurbo::BezPath;
use kurbo::Rect;
use kurbo::Stroke;
use peniko::BlendMode;
use peniko::Brush;
use peniko::Color;
use peniko::Fill;
use peniko::ImageData;

/// A path to fill or stroke, where it goes, and what paints it.
#[derive(Clone, Copy, Debug)]
pub struct Draw<'a> {
	/// The outline, in its own coordinates.
	pub path: &'a BezPath,
	/// Maps the outline to the screen.
	pub transform: Affine,
	/// What paints it.
	pub brush: &'a Brush,
	/// Maps the brush's coordinates (such as a gradient's unit box) into the
	/// outline's, before `transform`.
	pub brush_transform: Option<Affine>,
}

/// A compositing layer: everything drawn until the matching
/// [`Painter::pop_layer`] is clipped, blended, and faded together.
#[derive(Clone, Copy, Debug)]
pub struct Layer<'a> {
	/// The clip outline, or `None` for no clip.
	pub clip: Option<&'a BezPath>,
	/// Maps the clip outline to the screen.
	pub transform: Affine,
	/// How the layer blends with what is below it.
	pub blend: BlendMode,
	/// The layer's opacity.
	pub alpha: f32,
}

/// A blurred rounded rectangle: how shadows are drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxShadow {
	/// The rectangle, in its own coordinates.
	pub rect: Rect,
	/// Its corner radius.
	pub radius: f64,
	/// Maps it to the screen.
	pub transform: Affine,
	/// The shadow colour.
	pub color: Color,
	/// The blur radius in screen pixels.
	pub blur: f64,
}

/// A text run to draw.
#[derive(Clone, Copy, Debug)]
pub struct Text<'a> {
	/// The run, in its frame's coordinates.
	pub run: &'a TextRun,
	/// Its font.
	pub font: &'a FontSpec,
	/// Maps the frame to the screen.
	pub transform: Affine,
	/// The text colour.
	pub color: Color,
}

/// An image to draw: its pixels, its asset id, where it goes, and its
/// opacity. The image spans `(0, 0)` to its pixel size before `transform`.
#[derive(Clone, Copy, Debug)]
pub struct Image<'a> {
	/// The asset id the scene names it by.
	pub asset: &'a str,
	/// The pixels.
	pub data: &'a ImageData,
	/// Maps pixel coordinates to the screen.
	pub transform: Affine,
	/// The image's opacity.
	pub alpha: f32,
}

/// A rendering backend.
pub trait Painter {
	/// Fill `draw.path` with `rule`.
	fn fill(&mut self, draw: &Draw<'_>, rule: Fill);

	/// Stroke `draw.path` with `style`.
	fn stroke(&mut self, draw: &Draw<'_>, style: &Stroke);

	/// Start a layer.
	fn push_layer(&mut self, layer: &Layer<'_>);

	/// End the most recent layer.
	fn pop_layer(&mut self);

	/// Draw an image.
	fn image(&mut self, image: &Image<'_>);

	/// Draw a run of text.
	fn text(&mut self, text: &Text<'_>);

	/// Draw a blurred rounded rectangle.
	fn shadow(&mut self, shadow: &BoxShadow);
}
