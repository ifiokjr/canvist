//! Shapes as JSON. Paths use SVG path data.

use std::sync::Arc;

use kurbo::BezPath;
use serde_json::Value as Json;
use serde_json::json;

use super::Decoded;
use super::field;
use super::named;
use super::number;
use super::object;
use super::required;
use super::string;
use crate::BoolOp;
use crate::FillRule;
use crate::Shape;
use crate::TextSizing;

const RULES: [FillRule; 2] = [FillRule::NonZero, FillRule::EvenOdd];

fn rule_name(rule: FillRule) -> &'static str {
	match rule {
		FillRule::NonZero => "nonzero",
		FillRule::EvenOdd => "evenodd",
	}
}

pub(super) fn shape_to_json(shape: &Shape) -> Json {
	match shape {
		Shape::None => json!({ "type": "none" }),
		Shape::Rect => json!({ "type": "rect" }),
		Shape::Ellipse {
			start,
			sweep,
			inner,
		} => {
			json!({ "type": "ellipse", "start": start, "sweep": sweep, "inner": inner })
		}
		Shape::Polygon { points, ratio } => {
			json!({ "type": "polygon", "points": points, "ratio": ratio })
		}
		Shape::Path { path, rule } => {
			json!({ "type": "path", "d": path.to_svg(), "rule": rule_name(*rule) })
		}
		Shape::Boolean(op) => json!({ "type": "boolean", "op": op.name() }),
		Shape::Text(sizing) => json!({ "type": "text", "sizing": sizing.name() }),
		Shape::Instance { component } => {
			json!({ "type": "instance", "component": component.to_string() })
		}
	}
}

pub(super) fn shape_from_json(value: &Json) -> Decoded<Shape> {
	let map = object(value)?;

	Ok(match required(map, "type", string)? {
		"none" => Shape::None,
		"rect" => Shape::Rect,
		"ellipse" => {
			Shape::Ellipse {
				start: field(map, "start", 0.0, number)?,
				sweep: field(map, "sweep", 360.0, number)?,
				inner: field(map, "inner", 0.0, number)?,
			}
		}
		"polygon" => {
			Shape::Polygon {
				points: field(map, "points", 3, points)?,
				ratio: field(map, "ratio", 1.0, number)?,
			}
		}
		"path" => {
			Shape::Path {
				path: Arc::new(required(map, "d", path)?),
				rule: field(map, "rule", FillRule::NonZero, |value| {
					named(value, RULES, rule_name)
				})?,
			}
		}
		"boolean" => {
			Shape::Boolean(field(map, "op", BoolOp::Union, |value| {
				named(value, BoolOp::ALL, BoolOp::name)
			})?)
		}
		"text" => {
			Shape::Text(field(map, "sizing", TextSizing::AutoHeight, |value| {
				named(value, TextSizing::ALL, TextSizing::name)
			})?)
		}
		"instance" => {
			Shape::Instance {
				component: required(map, "component", |value| {
					string(value)?.parse().map_err(|error| format!("{error}"))
				})?,
			}
		}
		other => return Err(format!("unknown shape type `{other}`")),
	})
}

fn points(value: &Json) -> Decoded<u32> {
	value
		.as_u64()
		.and_then(|points| u32::try_from(points).ok())
		.ok_or_else(|| format!("expected a whole number, found {value}"))
}

fn path(value: &Json) -> Decoded<BezPath> {
	let data = string(value)?;
	BezPath::from_svg(data).map_err(|error| format!("invalid path data: {error}"))
}
