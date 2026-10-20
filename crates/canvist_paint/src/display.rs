//! Painting a `canvist_layout` display list: what canvas text frames and
//! the desktop editor draw.

use canvist_layout::DisplayItem;
use canvist_layout::DisplayList;
use canvist_layout::TextRun;
use kurbo::Affine;
use kurbo::BezPath;
use kurbo::Rect;
use kurbo::Shape as _;
use peniko::Brush;
use peniko::Color;
use peniko::Fill;

use crate::convert;
use crate::painter::Draw;
use crate::painter::Painter;
use crate::painter::Text;

/// Text whose font is smaller than this many screen pixels draws as bars.
pub const GREEK_PIXELS: f64 = 4.0;
/// The opacity of greeked text bars.
const GREEK_ALPHA: f32 = 0.35;
/// A greeked bar's height, as a fraction of the font size: about an
/// x-height.
const GREEK_HEIGHT: f64 = 0.3;
/// How far above the baseline a greeked bar sits, in bar heights.
const GREEK_RISE: f64 = 1.5;

/// How [`paint_display_list`] draws a list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayStyle {
	/// Maps the list's document coordinates to the screen.
	pub transform: Affine,
	/// One colour for every run and decoration, overriding the list's own,
	/// as a canvas text frame's fill does. `None` keeps the list's colours.
	pub fill: Option<Color>,
}

impl DisplayStyle {
	/// Draw with `transform` and the list's own colours.
	#[must_use]
	pub const fn new(transform: Affine) -> Self {
		Self {
			transform,
			fill: None,
		}
	}

	/// Screen pixels per document unit, to decide when text is too small to
	/// read.
	fn scale(&self) -> f64 {
		let [a, b, ..] = self.transform.as_coeffs();
		a.hypot(b)
	}
}

/// Paint every item of `list` in order: rectangles, text runs (as bars
/// where they would be too small to read), decoration lines, and ink
/// strokes. Slots are left empty for the host's own views.
pub fn paint_display_list<P: Painter + ?Sized>(
	painter: &mut P,
	list: &DisplayList,
	style: &DisplayStyle,
) {
	for item in &list.items {
		paint_item(painter, list, item, style);
	}
}

fn paint_item<P: Painter + ?Sized>(
	painter: &mut P,
	list: &DisplayList,
	item: &DisplayItem,
	style: &DisplayStyle,
) {
	match item {
		DisplayItem::Text(run) => paint_run(painter, list, run, style),
		DisplayItem::Rect {
			rect,
			color,
			radius,
			..
		} => {
			let shape = kurbo::RoundedRect::from_rect(layout_rect(*rect), f64::from(*radius))
				.to_path(tolerance());
			fill(painter, &shape, convert::layout_color(*color), style);
		}
		DisplayItem::Decoration {
			x,
			y,
			width,
			thickness,
			color,
			..
		} => {
			let half = f64::from(*thickness) / 2.0;
			let line = Rect::new(
				f64::from(*x),
				f64::from(*y) - half,
				f64::from(*x + *width),
				f64::from(*y) + half,
			);
			let color = style.fill.unwrap_or_else(|| convert::layout_color(*color));
			fill(painter, &line.to_path(tolerance()), color, style);
		}
		DisplayItem::Stroke {
			points,
			width,
			color,
			..
		} => {
			// A drawing keeps its own ink, whatever fill a text frame sets.
			paint_stroke(
				painter,
				points,
				*width,
				convert::layout_color(*color),
				style,
			);
		}
		DisplayItem::Slot { .. } => {}
	}
}

/// An ink stroke: a polyline through `points`, as `x, y` pairs, with round
/// caps and joins.
fn paint_stroke<P: Painter + ?Sized>(
	painter: &mut P,
	points: &[f32],
	width: f32,
	color: Color,
	style: &DisplayStyle,
) {
	let mut path = BezPath::new();

	for (index, point) in points.chunks_exact(2).enumerate() {
		let point = kurbo::Point::new(f64::from(point[0]), f64::from(point[1]));

		if index == 0 {
			path.move_to(point);
		}

		path.line_to(point);
	}

	let pen = kurbo::Stroke::new(f64::from(width))
		.with_caps(kurbo::Cap::Round)
		.with_join(kurbo::Join::Round);
	let brush = Brush::Solid(color);
	painter.stroke(
		&Draw {
			path: &path,
			transform: style.transform,
			brush: &brush,
			brush_transform: None,
		},
		&pen,
	);
}

/// One text run, or a bar where it is too small to read.
fn paint_run<P: Painter + ?Sized>(
	painter: &mut P,
	list: &DisplayList,
	run: &TextRun,
	style: &DisplayStyle,
) {
	let Some(font) = list.fonts.get(run.font.index()) else {
		return;
	};
	let color = style
		.fill
		.unwrap_or_else(|| convert::layout_color(run.color));

	if f64::from(font.size) * style.scale() >= GREEK_PIXELS {
		painter.text(&Text {
			run,
			font,
			transform: style.transform,
			color,
		});

		return;
	}

	let height = f64::from(font.size) * GREEK_HEIGHT;
	let top = f64::from(run.baseline) - height * GREEK_RISE;
	let bar = Rect::new(
		f64::from(run.x),
		top,
		f64::from(run.x + run.width),
		top + height,
	);
	fill(
		painter,
		&bar.to_path(tolerance()),
		color.multiply_alpha(GREEK_ALPHA),
		style,
	);
}

fn fill<P: Painter + ?Sized>(painter: &mut P, path: &BezPath, color: Color, style: &DisplayStyle) {
	let brush = Brush::Solid(color);
	painter.fill(
		&Draw {
			path,
			transform: style.transform,
			brush: &brush,
			brush_transform: None,
		},
		Fill::NonZero,
	);
}

/// A layout rectangle as a kurbo rectangle.
fn layout_rect(rect: canvist_layout::Rect) -> Rect {
	Rect::new(
		f64::from(rect.x),
		f64::from(rect.y),
		f64::from(rect.x + rect.width),
		f64::from(rect.y + rect.height),
	)
}

fn tolerance() -> f64 {
	canvist_canvas::geometry::TOLERANCE
}
