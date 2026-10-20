//! The JSON form of every property, shared by scene JSON and the Yjs
//! binding (Yjs values are JSON values). See the
//! [crate docs](crate#properties) for the format.

mod layout;
mod paint;
mod shape;

use kurbo::Affine;
use kurbo::Point;
use kurbo::Vec2;
use serde_json::Map;
use serde_json::Value as Json;
use serde_json::json;

use crate::BlendMode;
use crate::Prop;
use crate::PropKey;

/// A decoding failure: what was wrong, without the path (callers add it).
pub(crate) type Decoded<T> = Result<T, String>;

impl Prop {
	/// The property's value as JSON.
	#[must_use]
	pub fn to_json(&self) -> Json {
		match self {
			Self::Name(name) => json!(&**name),
			Self::Visible(value) | Self::Locked(value) | Self::Clip(value) => json!(value),
			Self::Transform(affine) => json!(affine.as_coeffs()),
			Self::Size(size) => json!([size.x, size.y]),
			Self::Opacity(value) => json!(value),
			Self::Blend(mode) => json!(mode.name()),
			Self::Fills(paints) | Self::Strokes(paints) => paint::paints_to_json(paints),
			Self::Stroke(stroke) => paint::stroke_to_json(stroke),
			Self::Effects(effects) => paint::effects_to_json(effects),
			Self::Radius(radii) => json!(radii),
			Self::Constraints(constraints) => layout::constraints_to_json(*constraints),
			Self::Layout(auto) => auto.map_or(Json::Null, layout::auto_layout_to_json),
			Self::LayoutChild(child) => layout::layout_child_to_json(*child),
			Self::Shape(shape) => shape::shape_to_json(shape),
			Self::Custom(_, value) => value.clone().unwrap_or(Json::Null),
			Self::Override(_, prop) => prop.to_json(),
			Self::ClearOverride(..) => Json::Null,
		}
	}

	/// Decode the value of `key` from JSON. `null` removes custom values
	/// and overrides.
	pub fn from_json(key: &PropKey, value: &Json) -> Result<Self, crate::SceneError> {
		decode(key, value).map_err(|reason| crate::SceneError::json(key.to_string(), reason))
	}
}

fn decode(key: &PropKey, value: &Json) -> Decoded<Prop> {
	Ok(match key {
		PropKey::Name => Prop::Name(string(value)?.into()),
		PropKey::Visible => Prop::Visible(boolean(value)?),
		PropKey::Locked => Prop::Locked(boolean(value)?),
		PropKey::Clip => Prop::Clip(boolean(value)?),
		PropKey::Transform => Prop::Transform(affine(value)?),
		PropKey::Size => Prop::Size(vec2(value)?),
		PropKey::Opacity => Prop::Opacity(number(value)?),
		PropKey::Blend => Prop::Blend(blend(value)?),
		PropKey::Fills => Prop::Fills(paint::paints_from_json(value)?),
		PropKey::Strokes => Prop::Strokes(paint::paints_from_json(value)?),
		PropKey::Stroke => Prop::Stroke(paint::stroke_from_json(value)?),
		PropKey::Effects => Prop::Effects(paint::effects_from_json(value)?),
		PropKey::Radius => Prop::Radius(radius(value)?),
		PropKey::Constraints => Prop::Constraints(layout::constraints_from_json(value)?),
		PropKey::Layout => Prop::Layout(layout::auto_layout_from_json(value)?),
		PropKey::LayoutChild => Prop::LayoutChild(layout::layout_child_from_json(value)?),
		PropKey::Shape => Prop::Shape(shape::shape_from_json(value)?),
		PropKey::Custom(name) => {
			Prop::Custom(name.clone(), (!value.is_null()).then(|| value.clone()))
		}
		PropKey::Override(target, inner) if value.is_null() => {
			Prop::ClearOverride(*target, inner.clone())
		}
		PropKey::Override(target, inner) => {
			Prop::Override(*target, Box::new(decode(inner, value)?))
		}
	})
}

pub(crate) fn number(value: &Json) -> Decoded<f64> {
	value
		.as_f64()
		.filter(|number| number.is_finite())
		.ok_or_else(|| format!("expected a finite number, found {value}"))
}

pub(crate) fn string(value: &Json) -> Decoded<&str> {
	value
		.as_str()
		.ok_or_else(|| format!("expected a string, found {value}"))
}

pub(crate) fn boolean(value: &Json) -> Decoded<bool> {
	value
		.as_bool()
		.ok_or_else(|| format!("expected a boolean, found {value}"))
}

pub(crate) fn numbers<const N: usize>(value: &Json) -> Decoded<[f64; N]> {
	let items = value
		.as_array()
		.filter(|items| items.len() == N)
		.ok_or_else(|| format!("expected {N} numbers, found {value}"))?;
	let mut out = [0.0; N];

	for (slot, item) in out.iter_mut().zip(items) {
		*slot = number(item)?;
	}

	Ok(out)
}

pub(crate) fn vec2(value: &Json) -> Decoded<Vec2> {
	let [x, y] = numbers(value)?;
	Ok(Vec2::new(x, y))
}

pub(crate) fn point(value: &Json) -> Decoded<Point> {
	vec2(value).map(Vec2::to_point)
}

fn affine(value: &Json) -> Decoded<Affine> {
	numbers(value).map(Affine::new)
}

fn radius(value: &Json) -> Decoded<[f64; 4]> {
	match value.as_f64() {
		Some(all) if all.is_finite() => Ok([all; 4]),
		_ => numbers(value),
	}
}

fn blend(value: &Json) -> Decoded<BlendMode> {
	BlendMode::from_name(string(value)?).ok_or_else(|| format!("unknown blend mode {value}"))
}

/// The variant of a `Copy` enum whose JSON name is `value`.
pub(crate) fn named<T: Copy, const N: usize>(
	value: &Json,
	all: [T; N],
	name: fn(T) -> &'static str,
) -> Decoded<T> {
	let text = string(value)?;
	all.into_iter()
		.find(|&item| name(item) == text)
		.ok_or_else(|| format!("unknown value {value}"))
}

/// Field `key` of an object, or `default` when absent.
pub(crate) fn field<T>(
	object: &Map<String, Json>,
	key: &str,
	default: T,
	decode: impl FnOnce(&Json) -> Decoded<T>,
) -> Decoded<T> {
	match object.get(key) {
		None => Ok(default),
		Some(value) => decode(value).map_err(|reason| format!("{key}: {reason}")),
	}
}

/// Field `key` of an object, which must be present.
pub(crate) fn required<'json, T>(
	object: &'json Map<String, Json>,
	key: &str,
	decode: impl FnOnce(&'json Json) -> Decoded<T>,
) -> Decoded<T> {
	let value = object.get(key).ok_or_else(|| format!("missing `{key}`"))?;
	decode(value).map_err(|reason| format!("{key}: {reason}"))
}

pub(crate) fn object(value: &Json) -> Decoded<&Map<String, Json>> {
	value
		.as_object()
		.ok_or_else(|| format!("expected an object, found {value}"))
}

pub(crate) fn array(value: &Json) -> Decoded<&Vec<Json>> {
	value
		.as_array()
		.ok_or_else(|| format!("expected an array, found {value}"))
}
