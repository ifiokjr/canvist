//! Scene values as painting values (`peniko` and `kurbo`).

use canvist_scene::BlendMode;
use canvist_scene::Color;
use canvist_scene::FillRule;
use canvist_scene::GradientStop;
use canvist_scene::Paint;
use canvist_scene::PaintKind;
use canvist_scene::StrokeCap;
use canvist_scene::StrokeJoin;
use canvist_scene::StrokeStyle;
use kurbo::Affine;
use kurbo::Cap;
use kurbo::Join;
use kurbo::Stroke;
use kurbo::Vec2;
use peniko::Brush;
use peniko::ColorStop;
use peniko::Compose;
use peniko::Fill;
use peniko::Gradient;
use peniko::Mix;

/// A scene colour as a painting colour.
#[must_use]
pub fn color(color: Color) -> peniko::Color {
	peniko::Color::from_rgba8(color.r, color.g, color.b, color.a)
}

/// A layout colour as a painting colour.
#[must_use]
pub fn layout_color(color: canvist_layout::Color) -> peniko::Color {
	peniko::Color::from_rgba8(color.r, color.g, color.b, color.a)
}

/// A scene blend mode as a painting blend mode.
#[must_use]
pub fn blend(mode: BlendMode) -> peniko::BlendMode {
	let mix = match mode {
		BlendMode::Normal => Mix::Normal,
		BlendMode::Multiply => Mix::Multiply,
		BlendMode::Screen => Mix::Screen,
		BlendMode::Overlay => Mix::Overlay,
		BlendMode::Darken => Mix::Darken,
		BlendMode::Lighten => Mix::Lighten,
		BlendMode::ColorDodge => Mix::ColorDodge,
		BlendMode::ColorBurn => Mix::ColorBurn,
		BlendMode::HardLight => Mix::HardLight,
		BlendMode::SoftLight => Mix::SoftLight,
		BlendMode::Difference => Mix::Difference,
		BlendMode::Exclusion => Mix::Exclusion,
		BlendMode::Hue => Mix::Hue,
		BlendMode::Saturation => Mix::Saturation,
		BlendMode::Color => Mix::Color,
		BlendMode::Luminosity => Mix::Luminosity,
	};

	peniko::BlendMode::new(mix, Compose::SrcOver)
}

/// A scene fill rule as a painting fill rule.
#[must_use]
pub fn fill_rule(rule: FillRule) -> Fill {
	match rule {
		FillRule::NonZero => Fill::NonZero,
		FillRule::EvenOdd => Fill::EvenOdd,
	}
}

/// A scene stroke style as a kurbo stroke, `scale` times as wide (inside
/// and outside strokes are drawn twice as wide and clipped).
#[must_use]
pub fn stroke(style: &StrokeStyle, scale: f64) -> Stroke {
	let cap = match style.cap {
		StrokeCap::Butt => Cap::Butt,
		StrokeCap::Round => Cap::Round,
		StrokeCap::Square => Cap::Square,
	};

	let join = match style.join {
		StrokeJoin::Miter => Join::Miter,
		StrokeJoin::Round => Join::Round,
		StrokeJoin::Bevel => Join::Bevel,
	};

	Stroke::new(style.width * scale)
		.with_caps(cap)
		.with_join(join)
		.with_miter_limit(style.miter_limit)
		.with_dashes(0.0, style.dashes.iter().copied())
}

/// A paint as a brush and the transform from the node's unit box (where
/// gradient geometry lives) to its box of `size`. `None` for image paints,
/// which draw as images.
#[must_use]
pub fn brush(paint: &Paint, size: Vec2) -> Option<(Brush, Option<Affine>)> {
	let alpha = paint.opacity.clamp(0.0, 1.0) as f32;
	let unit = Some(Affine::scale_non_uniform(size.x, size.y));

	let brush = match &paint.kind {
		PaintKind::Solid(solid) => {
			return Some((Brush::Solid(color(*solid).multiply_alpha(alpha)), None));
		}
		PaintKind::Linear { from, to, stops } => {
			Gradient::new_linear(*from, *to).with_stops(color_stops(stops, alpha).as_slice())
		}
		PaintKind::Radial {
			center,
			radius,
			stops,
		} => {
			// peniko's radial gradients are circular; stretch a unit circle to
			// the requested radii.
			let stretch =
				Affine::translate(center.to_vec2()) * Affine::scale_non_uniform(radius.x, radius.y);
			let gradient = Gradient::new_radial((0.0, 0.0), 1.0)
				.with_stops(color_stops(stops, alpha).as_slice());
			return Some((Brush::Gradient(gradient), unit.map(|unit| unit * stretch)));
		}
		PaintKind::Sweep {
			center,
			angle,
			stops,
		} => {
			let start = angle.to_radians() as f32;
			Gradient::new_sweep(*center, start, start + std::f32::consts::TAU)
				.with_stops(color_stops(stops, alpha).as_slice())
		}
		PaintKind::Image { .. } => return None,
	};

	Some((Brush::Gradient(brush), unit))
}

fn color_stops(stops: &[GradientStop], alpha: f32) -> Vec<ColorStop> {
	stops
		.iter()
		.map(|stop| ColorStop::from((stop.offset as f32, color(stop.color).multiply_alpha(alpha))))
		.collect()
}

/// The colour a node shows at a glance: its first visible solid fill, or the
/// first stop of its first gradient. Used to draw nodes too small to detail.
#[must_use]
pub fn summary_color(paints: &[Paint]) -> Option<peniko::Color> {
	paints
		.iter()
		.filter(|paint| paint.visible)
		.find_map(|paint| {
			match &paint.kind {
				PaintKind::Solid(solid) => Some(*solid),
				PaintKind::Linear { stops, .. }
				| PaintKind::Radial { stops, .. }
				| PaintKind::Sweep { stops, .. } => stops.first().map(|stop| stop.color),
				PaintKind::Image { .. } => None,
			}
		})
		.map(color)
}
