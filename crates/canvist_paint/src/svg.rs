//! SVG output: the same painter calls as the GPU and CPU backends, as an
//! SVG document. Text stays text (`<text>` elements in the frame's font),
//! so exported files remain searchable and editable.

use std::fmt::Write as _;

use kurbo::Affine;
use kurbo::BezPath;
use kurbo::Size;
use peniko::Brush;
use peniko::Color;
use peniko::Fill;
use peniko::Gradient;
use peniko::GradientKind;

use crate::painter::BoxShadow;
use crate::painter::Draw;
use crate::painter::Image;
use crate::painter::Layer;
use crate::painter::Painter;
use crate::painter::Text;

/// Builds an SVG document from painter calls.
#[derive(Clone, Debug)]
pub struct SvgPainter {
	size: Size,
	defs: String,
	body: String,
	next_id: usize,
	/// Returns an `href` for an image: a data URL or a URL the host serves.
	/// Images without one are left out.
	image_href: fn(&Image<'_>) -> Option<String>,
}

impl SvgPainter {
	/// A painter for an image of `size`, which leaves images out.
	#[must_use]
	pub fn new(size: Size) -> Self {
		Self {
			size,
			defs: String::new(),
			body: String::new(),
			next_id: 0,
			image_href: |_| None,
		}
	}

	/// Use `href` to reference images.
	#[must_use]
	pub fn with_images(mut self, href: fn(&Image<'_>) -> Option<String>) -> Self {
		self.image_href = href;
		self
	}

	/// The finished document.
	#[must_use]
	pub fn finish(self) -> String {
		let (width, height) = (self.size.width, self.size.height);
		format!(
			"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" \
			 viewBox=\"0 0 {width} {height}\"><defs>{}</defs>{}</svg>",
			self.defs, self.body
		)
	}

	fn id(&mut self, prefix: &str) -> String {
		self.next_id += 1;
		format!("{prefix}{}", self.next_id)
	}

	/// A `fill` or `stroke` attribute value for `brush`, adding a gradient to
	/// the definitions when needed.
	fn paint(
		&mut self,
		brush: &Brush,
		transform: Affine,
		brush_transform: Option<Affine>,
	) -> String {
		match brush {
			Brush::Solid(color) => solid(*color),
			Brush::Gradient(gradient) => {
				let id = self.id("g");
				let units = transform * brush_transform.unwrap_or(Affine::IDENTITY);
				self.defs.push_str(&gradient_def(&id, gradient, units));
				format!("url(#{id})")
			}
			Brush::Image(_) => "none".to_owned(),
		}
	}
}

impl Painter for SvgPainter {
	fn fill(&mut self, draw: &Draw<'_>, rule: Fill) {
		let fill = self.paint(draw.brush, draw.transform, draw.brush_transform);

		let rule = match rule {
			Fill::NonZero => "nonzero",
			Fill::EvenOdd => "evenodd",
		};

		let _ = write!(
			self.body,
			"<path d=\"{}\" fill=\"{fill}\" fill-rule=\"{rule}\"/>",
			path_data(draw.path, draw.transform)
		);
	}

	fn stroke(&mut self, draw: &Draw<'_>, style: &kurbo::Stroke) {
		let stroke = self.paint(draw.brush, draw.transform, draw.brush_transform);
		let scale = transform_scale(draw.transform);
		let dashes: Vec<String> = style
			.dash_pattern
			.iter()
			.map(|dash| number(dash * scale))
			.collect();
		let dash = if dashes.is_empty() {
			String::new()
		} else {
			format!(" stroke-dasharray=\"{}\"", dashes.join(" "))
		};

		let _ = write!(
			self.body,
			"<path d=\"{}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{}\"{dash}/>",
			path_data(draw.path, draw.transform),
			number(style.width * scale)
		);
	}

	fn push_layer(&mut self, layer: &Layer<'_>) {
		let mut attributes = String::new();

		if let Some(clip) = layer.clip {
			let id = self.id("c");
			let _ = write!(
				self.defs,
				"<clipPath id=\"{id}\"><path d=\"{}\"/></clipPath>",
				path_data(clip, layer.transform)
			);
			let _ = write!(attributes, " clip-path=\"url(#{id})\"");
		}

		if layer.alpha < 1.0 {
			let _ = write!(
				attributes,
				" opacity=\"{}\"",
				number(f64::from(layer.alpha))
			);
		}

		if let Some(mode) = blend_name(layer.blend) {
			let _ = write!(attributes, " style=\"mix-blend-mode:{mode}\"");
		}

		let _ = write!(self.body, "<g{attributes}>");
	}

	fn pop_layer(&mut self) {
		self.body.push_str("</g>");
	}

	fn image(&mut self, image: &Image<'_>) {
		let Some(href) = (self.image_href)(image) else {
			return;
		};
		let _ = write!(
			self.body,
			"<image href=\"{}\" width=\"{}\" height=\"{}\" transform=\"{}\" opacity=\"{}\" \
			 preserveAspectRatio=\"none\"/>",
			escape(&href),
			image.data.width,
			image.data.height,
			matrix(image.transform),
			number(f64::from(image.alpha))
		);
	}

	fn text(&mut self, text: &Text<'_>) {
		let run = text.run;
		let font = text.font;
		let style = if font.italic {
			" font-style=\"italic\""
		} else {
			""
		};

		let _ = write!(
			self.body,
			"<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\" \
			 font-weight=\"{}\"{style} fill=\"{}\" transform=\"{}\" \
			 xml:space=\"preserve\">{}</text>",
			number(f64::from(run.x)),
			number(f64::from(run.baseline)),
			escape(&font.family),
			number(f64::from(font.size)),
			font.weight,
			solid(text.color),
			matrix(text.transform),
			escape(run.text())
		);
	}

	fn shadow(&mut self, shadow: &BoxShadow) {
		let id = self.id("s");
		let deviation = number(shadow.blur / 2.0);
		let _ = write!(
			self.defs,
			"<filter id=\"{id}\" x=\"-50%\" y=\"-50%\" width=\"200%\" \
			 height=\"200%\"><feGaussianBlur stdDeviation=\"{deviation}\"/></filter>"
		);
		let rect = shadow.rect;
		let _ = write!(
			self.body,
			"<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\" \
			 transform=\"{}\" filter=\"url(#{id})\"/>",
			number(rect.x0),
			number(rect.y0),
			number(rect.width()),
			number(rect.height()),
			number(shadow.radius),
			solid(shadow.color),
			matrix(shadow.transform)
		);
	}
}

/// Path data with `transform` applied, so strokes keep their screen width.
fn path_data(path: &BezPath, transform: Affine) -> String {
	(transform * path.clone()).to_svg()
}

fn transform_scale(transform: Affine) -> f64 {
	let [a, b, ..] = transform.as_coeffs();
	a.hypot(b)
}

fn matrix(transform: Affine) -> String {
	let coeffs = transform.as_coeffs().map(number);
	format!("matrix({})", coeffs.join(" "))
}

fn solid(color: Color) -> String {
	let [r, g, b, a] = color.to_rgba8().to_u8_array();
	format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
}

fn gradient_def(id: &str, gradient: &Gradient, units: Affine) -> String {
	let stops = gradient.stops.iter().fold(String::new(), |mut out, stop| {
		let _ = write!(
			out,
			"<stop offset=\"{}\" stop-color=\"{}\"/>",
			number(f64::from(stop.offset)),
			solid(stop.color.to_alpha_color())
		);
		out
	});
	let transform = matrix(units);

	match gradient.kind {
		GradientKind::Linear(line) => {
			format!(
				"<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" \
				 gradientTransform=\"{transform}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" \
				 y2=\"{}\">{stops}</linearGradient>",
				number(line.start.x),
				number(line.start.y),
				number(line.end.x),
				number(line.end.y)
			)
		}
		GradientKind::Radial(radial) => {
			format!(
				"<radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" \
				 gradientTransform=\"{transform}\" cx=\"{}\" cy=\"{}\" \
				 r=\"{}\">{stops}</radialGradient>",
				number(radial.end_center.x),
				number(radial.end_center.y),
				number(f64::from(radial.end_radius))
			)
		}
		// SVG has no conic gradient; a linear gradient stands in.
		GradientKind::Sweep(_) => format!("<linearGradient id=\"{id}\">{stops}</linearGradient>"),
	}
}

fn blend_name(blend: peniko::BlendMode) -> Option<&'static str> {
	use peniko::Mix;

	Some(match blend.mix {
		Mix::Multiply => "multiply",
		Mix::Screen => "screen",
		Mix::Overlay => "overlay",
		Mix::Darken => "darken",
		Mix::Lighten => "lighten",
		Mix::ColorDodge => "color-dodge",
		Mix::ColorBurn => "color-burn",
		Mix::HardLight => "hard-light",
		Mix::SoftLight => "soft-light",
		Mix::Difference => "difference",
		Mix::Exclusion => "exclusion",
		Mix::Hue => "hue",
		Mix::Saturation => "saturation",
		Mix::Color => "color",
		Mix::Luminosity => "luminosity",
		Mix::Normal => return None,
	})
}

/// A number with at most three decimals, without trailing zeros.
fn number(value: f64) -> String {
	let rounded = (value * 1000.0).round() / 1000.0;
	let text = format!("{rounded}");

	if text == "-0" { "0".to_owned() } else { text }
}

fn escape(text: &str) -> String {
	text.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
}
