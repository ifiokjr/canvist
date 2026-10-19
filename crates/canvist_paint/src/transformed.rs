//! A painter adapter that maps everything through one more transform, for
//! example device pixels: hosts paint in CSS pixels and scale by the
//! device pixel ratio here.

use kurbo::Affine;
use peniko::Fill;

use crate::painter::BoxShadow;
use crate::painter::Draw;
use crate::painter::Image;
use crate::painter::Layer;
use crate::painter::Painter;
use crate::painter::Text;

/// Forwards every call to `inner` with `transform` applied first.
#[derive(Debug)]
pub struct Transformed<'a, P: ?Sized> {
	inner: &'a mut P,
	transform: Affine,
}

impl<'a, P: Painter + ?Sized> Transformed<'a, P> {
	/// Paint into `inner` through `transform`.
	pub fn new(inner: &'a mut P, transform: Affine) -> Self {
		Self { inner, transform }
	}
}

impl<P: Painter + ?Sized> Painter for Transformed<'_, P> {
	fn fill(&mut self, draw: &Draw<'_>, rule: Fill) {
		self.inner.fill(
			&Draw {
				transform: self.transform * draw.transform,
				..*draw
			},
			rule,
		);
	}

	fn stroke(&mut self, draw: &Draw<'_>, style: &kurbo::Stroke) {
		self.inner.stroke(
			&Draw {
				transform: self.transform * draw.transform,
				..*draw
			},
			style,
		);
	}

	fn push_layer(&mut self, layer: &Layer<'_>) {
		self.inner.push_layer(&Layer {
			transform: self.transform * layer.transform,
			..*layer
		});
	}

	fn pop_layer(&mut self) {
		self.inner.pop_layer();
	}

	fn image(&mut self, image: &Image<'_>) {
		self.inner.image(&Image {
			transform: self.transform * image.transform,
			..*image
		});
	}

	fn text(&mut self, text: &Text<'_>) {
		self.inner.text(&Text {
			transform: self.transform * text.transform,
			..*text
		});
	}

	fn shadow(&mut self, shadow: &BoxShadow) {
		self.inner.shadow(&BoxShadow {
			transform: self.transform * shadow.transform,
			..*shadow
		});
	}
}
