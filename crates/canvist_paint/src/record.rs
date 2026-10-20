//! A painter that records calls, for tests and for counting what a frame
//! draws.

use kurbo::Affine;
use kurbo::Rect;
use kurbo::Shape as _;
use peniko::Brush;
use peniko::Fill;

use crate::painter::BoxShadow;
use crate::painter::Draw;
use crate::painter::Image;
use crate::painter::Layer;
use crate::painter::Painter;
use crate::painter::Text;

/// One recorded painter call, with its screen bounds.
#[derive(Clone, Debug, PartialEq)]
pub enum PaintOp {
	/// A fill.
	Fill {
		/// The filled outline's screen bounds.
		bounds: Rect,
		/// The brush.
		brush: Brush,
		/// The fill rule.
		rule: Fill,
	},
	/// A stroke.
	Stroke {
		/// The stroked outline's screen bounds, before stroke width.
		bounds: Rect,
		/// The brush.
		brush: Brush,
		/// The stroke width.
		width: f64,
	},
	/// A layer started.
	PushLayer {
		/// Whether it clips.
		clips: bool,
		/// Its opacity.
		alpha: f32,
	},
	/// A layer ended.
	PopLayer,
	/// An image.
	Image {
		/// Its asset id.
		asset: String,
		/// Its transform.
		transform: Affine,
	},
	/// A text run.
	Text {
		/// The text.
		text: String,
		/// The font size.
		size: f32,
		/// The transform.
		transform: Affine,
	},
	/// A shadow.
	Shadow(BoxShadow),
}

/// Records painter calls as [`PaintOp`]s.
#[derive(Clone, Debug, Default)]
pub struct Recorder {
	/// The calls, in order.
	pub ops: Vec<PaintOp>,
}

impl Recorder {
	/// An empty recorder.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}
}

fn screen_bounds(draw: &Draw<'_>) -> Rect {
	(draw.transform * draw.path.clone()).bounding_box()
}

impl Painter for Recorder {
	fn fill(&mut self, draw: &Draw<'_>, rule: Fill) {
		self.ops.push(PaintOp::Fill {
			bounds: screen_bounds(draw),
			brush: draw.brush.clone(),
			rule,
		});
	}

	fn stroke(&mut self, draw: &Draw<'_>, style: &kurbo::Stroke) {
		self.ops.push(PaintOp::Stroke {
			bounds: screen_bounds(draw),
			brush: draw.brush.clone(),
			width: style.width,
		});
	}

	fn push_layer(&mut self, layer: &Layer<'_>) {
		self.ops.push(PaintOp::PushLayer {
			clips: layer.clip.is_some(),
			alpha: layer.alpha,
		});
	}

	fn pop_layer(&mut self) {
		self.ops.push(PaintOp::PopLayer);
	}

	fn image(&mut self, image: &Image<'_>) {
		self.ops.push(PaintOp::Image {
			asset: image.asset.to_owned(),
			transform: image.transform,
		});
	}

	fn text(&mut self, text: &Text<'_>) {
		self.ops.push(PaintOp::Text {
			text: text.run.text().to_owned(),
			size: text.font.size,
			transform: text.transform,
		});
	}

	fn shadow(&mut self, shadow: &BoxShadow) {
		self.ops.push(PaintOp::Shadow(*shadow));
	}
}
