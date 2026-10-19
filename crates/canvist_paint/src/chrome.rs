//! The default chrome painter: draws a canvas overlay (selection outlines,
//! handles, marquee, guides, pen previews, labels) with any [`Painter`].
//! Hosts that want their own look draw the overlay themselves.

use canvist_canvas::OutlineKind;
use canvist_canvas::OverlayItem;
use canvist_canvas::PointKind;
use kurbo::Affine;
use kurbo::BezPath;
use kurbo::Circle;
use kurbo::Line;
use kurbo::Shape as _;
use kurbo::Stroke;
use peniko::Brush;
use peniko::Color;
use peniko::Fill;

use crate::painter::Draw;
use crate::painter::Painter;

/// Colours and widths for the default chrome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChromeStyle {
	/// Outlines, handles, and the marquee border: canvist's paint blue.
	pub accent: Color,
	/// Snapping guides.
	pub guide: Color,
	/// Handle fill.
	pub handle_fill: Color,
	/// Outline width in pixels.
	pub width: f64,
}

/// canvist's paint blue, from the brand.
const PAINT_BLUE: Color = Color::from_rgba8(59, 44, 245, 255);
/// A red that stands out from most designs, for guides.
const GUIDE_RED: Color = Color::from_rgba8(240, 82, 82, 255);

impl Default for ChromeStyle {
	fn default() -> Self {
		Self {
			accent: PAINT_BLUE,
			guide: GUIDE_RED,
			handle_fill: Color::WHITE,
			width: 1.0,
		}
	}
}

/// The marquee's fill: the accent at this opacity.
const MARQUEE_ALPHA: f32 = 0.08;
/// The radius of pen anchors and control points, in pixels.
const POINT_RADIUS: f64 = 3.5;
/// Shape flattening tolerance for chrome, in pixels.
const TOLERANCE: f64 = 0.1;

/// Draw `items` (screen space) with `style`.
pub fn paint_overlay<P: Painter + ?Sized>(
	painter: &mut P,
	items: &[OverlayItem],
	style: &ChromeStyle,
) {
	for item in items {
		paint_item(painter, item, style);
	}
}

fn paint_item<P: Painter + ?Sized>(painter: &mut P, item: &OverlayItem, style: &ChromeStyle) {
	let line = Stroke::new(style.width);
	let accent = Brush::Solid(style.accent);

	match item {
		OverlayItem::Outline { path, kind } => {
			let width = if *kind == OutlineKind::Hover {
				style.width * 2.0
			} else {
				style.width
			};

			stroke(painter, path, &accent, &Stroke::new(width));
		}
		OverlayItem::Handle { rect, .. } => {
			let square = rect.to_path(TOLERANCE);
			fill(painter, &square, &Brush::Solid(style.handle_fill));
			stroke(painter, &square, &accent, &line);
		}
		OverlayItem::Marquee { rect } => {
			let area = rect.to_path(TOLERANCE);
			fill(
				painter,
				&area,
				&Brush::Solid(style.accent.multiply_alpha(MARQUEE_ALPHA)),
			);
			stroke(painter, &area, &accent, &line);
		}
		OverlayItem::Guide { from, to } => {
			stroke(
				painter,
				&Line::new(*from, *to).to_path(TOLERANCE),
				&Brush::Solid(style.guide),
				&line,
			);
		}
		OverlayItem::Path { path } => stroke(painter, path, &accent, &line),
		OverlayItem::Point { at, kind } => {
			let dot = Circle::new(*at, POINT_RADIUS).to_path(TOLERANCE);
			let inside = if *kind == PointKind::Anchor {
				style.handle_fill
			} else {
				style.accent
			};

			fill(painter, &dot, &Brush::Solid(inside));
			stroke(painter, &dot, &accent, &line);
		}
		// Labels are text in the host's UI font; hosts draw them.
		OverlayItem::Label { .. } => {}
	}
}

fn fill<P: Painter + ?Sized>(painter: &mut P, path: &BezPath, brush: &Brush) {
	painter.fill(
		&Draw {
			path,
			transform: Affine::IDENTITY,
			brush,
			brush_transform: None,
		},
		Fill::NonZero,
	);
}

fn stroke<P: Painter + ?Sized>(painter: &mut P, path: &BezPath, brush: &Brush, style: &Stroke) {
	painter.stroke(
		&Draw {
			path,
			transform: Affine::IDENTITY,
			brush,
			brush_transform: None,
		},
		style,
	);
}
