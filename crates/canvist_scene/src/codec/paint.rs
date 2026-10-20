//! Paints, strokes, and effects as JSON.

use std::sync::Arc;

use serde_json::Map;
use serde_json::Value as Json;
use serde_json::json;

use super::Decoded;
use super::array;
use super::boolean;
use super::field;
use super::named;
use super::number;
use super::object;
use super::point;
use super::required;
use super::string;
use super::vec2;
use crate::BlendMode;
use crate::Color;
use crate::Effect;
use crate::EffectKind;
use crate::GradientStop;
use crate::ImageFit;
use crate::Paint;
use crate::PaintKind;
use crate::Shadow;
use crate::StrokeAlign;
use crate::StrokeCap;
use crate::StrokeJoin;
use crate::StrokeStyle;

const FITS: [ImageFit; 4] = [
	ImageFit::Fill,
	ImageFit::Fit,
	ImageFit::Stretch,
	ImageFit::Tile,
];
const ALIGNS: [StrokeAlign; 3] = [
	StrokeAlign::Inside,
	StrokeAlign::Center,
	StrokeAlign::Outside,
];
const CAPS: [StrokeCap; 3] = [StrokeCap::Butt, StrokeCap::Round, StrokeCap::Square];
const JOINS: [StrokeJoin; 3] = [StrokeJoin::Miter, StrokeJoin::Round, StrokeJoin::Bevel];

fn fit_name(fit: ImageFit) -> &'static str {
	match fit {
		ImageFit::Fill => "fill",
		ImageFit::Fit => "fit",
		ImageFit::Stretch => "stretch",
		ImageFit::Tile => "tile",
	}
}

fn align_name(align: StrokeAlign) -> &'static str {
	match align {
		StrokeAlign::Inside => "inside",
		StrokeAlign::Center => "center",
		StrokeAlign::Outside => "outside",
	}
}

fn cap_name(cap: StrokeCap) -> &'static str {
	match cap {
		StrokeCap::Butt => "butt",
		StrokeCap::Round => "round",
		StrokeCap::Square => "square",
	}
}

fn join_name(join: StrokeJoin) -> &'static str {
	match join {
		StrokeJoin::Miter => "miter",
		StrokeJoin::Round => "round",
		StrokeJoin::Bevel => "bevel",
	}
}

pub(super) fn color(value: &Json) -> Decoded<Color> {
	let text = string(value)?;
	Color::parse_hex(text).ok_or_else(|| format!("expected a #rrggbbaa colour, found {value}"))
}

pub(super) fn paints_to_json(paints: &[Paint]) -> Json {
	Json::Array(paints.iter().map(paint_to_json).collect())
}

fn paint_to_json(paint: &Paint) -> Json {
	let mut out = kind_to_json(&paint.kind);

	if (paint.opacity - 1.0).abs() > f64::EPSILON {
		out.insert("opacity".into(), json!(paint.opacity));
	}

	if !paint.visible {
		out.insert("visible".into(), json!(false));
	}

	if paint.blend != BlendMode::Normal {
		out.insert("blend".into(), json!(paint.blend.name()));
	}

	Json::Object(out)
}

fn kind_to_json(kind: &PaintKind) -> Map<String, Json> {
	let value = match kind {
		PaintKind::Solid(color) => json!({ "type": "solid", "color": color.to_hex() }),
		PaintKind::Linear { from, to, stops } => {
			json!({ "type": "linear", "from": [from.x, from.y], "to": [to.x, to.y], "stops": stops_to_json(stops) })
		}
		PaintKind::Radial {
			center,
			radius,
			stops,
		} => {
			json!({ "type": "radial", "center": [center.x, center.y], "radius": [radius.x, radius.y], "stops": stops_to_json(stops) })
		}
		PaintKind::Sweep {
			center,
			angle,
			stops,
		} => {
			json!({ "type": "sweep", "center": [center.x, center.y], "angle": angle, "stops": stops_to_json(stops) })
		}
		PaintKind::Image { asset, fit } => {
			json!({ "type": "image", "asset": &**asset, "fit": fit_name(*fit) })
		}
	};

	match value {
		Json::Object(map) => map,
		_ => Map::new(),
	}
}

fn stops_to_json(stops: &[GradientStop]) -> Json {
	stops
		.iter()
		.map(|stop| json!({ "offset": stop.offset, "color": stop.color.to_hex() }))
		.collect()
}

pub(super) fn paints_from_json(value: &Json) -> Decoded<Arc<[Paint]>> {
	array(value)?
		.iter()
		.enumerate()
		.map(|(index, paint)| {
			paint_from_json(paint).map_err(|reason| format!("[{index}]: {reason}"))
		})
		.collect()
}

fn paint_from_json(value: &Json) -> Decoded<Paint> {
	let paint = object(value)?;

	Ok(Paint {
		kind: kind_from_json(paint)?,
		opacity: field(paint, "opacity", 1.0, number)?,
		visible: field(paint, "visible", true, boolean)?,
		blend: field(paint, "blend", BlendMode::Normal, |value| {
			named(value, BlendMode::ALL, BlendMode::name)
		})?,
	})
}

fn kind_from_json(paint: &Map<String, Json>) -> Decoded<PaintKind> {
	Ok(match required(paint, "type", string)? {
		"solid" => PaintKind::Solid(required(paint, "color", color)?),
		"linear" => {
			PaintKind::Linear {
				from: required(paint, "from", point)?,
				to: required(paint, "to", point)?,
				stops: required(paint, "stops", stops_from_json)?,
			}
		}
		"radial" => {
			PaintKind::Radial {
				center: required(paint, "center", point)?,
				radius: required(paint, "radius", vec2)?,
				stops: required(paint, "stops", stops_from_json)?,
			}
		}
		"sweep" => {
			PaintKind::Sweep {
				center: required(paint, "center", point)?,
				angle: field(paint, "angle", 0.0, number)?,
				stops: required(paint, "stops", stops_from_json)?,
			}
		}
		"image" => {
			PaintKind::Image {
				asset: required(paint, "asset", string)?.into(),
				fit: field(paint, "fit", ImageFit::Fill, |value| {
					named(value, FITS, fit_name)
				})?,
			}
		}
		other => return Err(format!("unknown paint type `{other}`")),
	})
}

fn stops_from_json(value: &Json) -> Decoded<Arc<[GradientStop]>> {
	array(value)?
		.iter()
		.map(|stop| {
			let stop = object(stop)?;
			Ok(GradientStop {
				offset: required(stop, "offset", number)?,
				color: required(stop, "color", color)?,
			})
		})
		.collect()
}

pub(super) fn stroke_to_json(stroke: &StrokeStyle) -> Json {
	json!({
		"width": stroke.width,
		"align": align_name(stroke.align),
		"cap": cap_name(stroke.cap),
		"join": join_name(stroke.join),
		"miterLimit": stroke.miter_limit,
		"dashes": &*stroke.dashes,
	})
}

pub(super) fn stroke_from_json(value: &Json) -> Decoded<StrokeStyle> {
	let stroke = object(value)?;
	let defaults = StrokeStyle::default();

	Ok(StrokeStyle {
		width: field(stroke, "width", defaults.width, number)?,
		align: field(stroke, "align", defaults.align, |value| {
			named(value, ALIGNS, align_name)
		})?,
		cap: field(stroke, "cap", defaults.cap, |value| {
			named(value, CAPS, cap_name)
		})?,
		join: field(stroke, "join", defaults.join, |value| {
			named(value, JOINS, join_name)
		})?,
		miter_limit: field(stroke, "miterLimit", defaults.miter_limit, number)?,
		dashes: field(stroke, "dashes", defaults.dashes, |value| {
			array(value)?.iter().map(number).collect()
		})?,
	})
}

pub(super) fn effects_to_json(effects: &[Effect]) -> Json {
	Json::Array(effects.iter().map(effect_to_json).collect())
}

fn effect_to_json(effect: &Effect) -> Json {
	let mut value = match effect.kind {
		EffectKind::DropShadow(shadow) => shadow_to_json("drop-shadow", shadow),
		EffectKind::InnerShadow(shadow) => shadow_to_json("inner-shadow", shadow),
		EffectKind::LayerBlur { radius } => json!({ "type": "layer-blur", "radius": radius }),
		EffectKind::BackgroundBlur { radius } => {
			json!({ "type": "background-blur", "radius": radius })
		}
	};

	if !effect.visible
		&& let Some(map) = value.as_object_mut()
	{
		map.insert("visible".into(), json!(false));
	}

	value
}

fn shadow_to_json(kind: &str, shadow: Shadow) -> Json {
	json!({
		"type": kind,
		"color": shadow.color.to_hex(),
		"offset": [shadow.offset.x, shadow.offset.y],
		"blur": shadow.blur,
		"spread": shadow.spread,
	})
}

pub(super) fn effects_from_json(value: &Json) -> Decoded<Arc<[Effect]>> {
	array(value)?
		.iter()
		.enumerate()
		.map(|(index, effect)| {
			effect_from_json(effect).map_err(|reason| format!("[{index}]: {reason}"))
		})
		.collect()
}

fn effect_from_json(value: &Json) -> Decoded<Effect> {
	let effect = object(value)?;
	let kind = match required(effect, "type", string)? {
		"drop-shadow" => EffectKind::DropShadow(shadow_from_json(effect)?),
		"inner-shadow" => EffectKind::InnerShadow(shadow_from_json(effect)?),
		"layer-blur" => {
			EffectKind::LayerBlur {
				radius: required(effect, "radius", number)?,
			}
		}
		"background-blur" => {
			EffectKind::BackgroundBlur {
				radius: required(effect, "radius", number)?,
			}
		}
		other => return Err(format!("unknown effect type `{other}`")),
	};

	Ok(Effect {
		kind,
		visible: field(effect, "visible", true, boolean)?,
	})
}

fn shadow_from_json(effect: &Map<String, Json>) -> Decoded<Shadow> {
	Ok(Shadow {
		color: required(effect, "color", color)?,
		offset: field(effect, "offset", kurbo::Vec2::ZERO, vec2)?,
		blur: field(effect, "blur", 0.0, number)?,
		spread: field(effect, "spread", 0.0, number)?,
	})
}
