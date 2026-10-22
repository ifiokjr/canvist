//! Ink drawings: the strokes of an `ink` block, read once when its row is
//! flattened and drawn by the display list.

use std::sync::Arc;

use canvist_model::Attrs;
use canvist_model::Value;

use crate::Color;

/// The strokes of one drawing, in drawing units (see
/// [`INK_WIDTH`](crate::INK_WIDTH)).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct InkDrawing {
	pub(crate) strokes: Vec<InkStroke>,
}

/// One stroke: `x, y` pairs, and the colour and width it sets, if any.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InkStroke {
	pub(crate) points: Vec<f32>,
	pub(crate) color: Option<Color>,
	pub(crate) width: Option<f32>,
}

impl InkDrawing {
	/// The drawing a block's `strokes` attribute holds. Malformed strokes,
	/// and coordinates that are not finite numbers, are skipped, so bad data
	/// never breaks a layout.
	pub(crate) fn from_attrs(attrs: &Attrs) -> Option<Arc<Self>> {
		let strokes = attrs.get("strokes")?.as_array()?;
		let strokes: Vec<InkStroke> = strokes.iter().filter_map(stroke).collect();

		Some(Arc::new(Self { strokes }))
	}
}

fn number(value: &Value) -> Option<f32> {
	let number = match value {
		Value::Int(number) => *number as f32,
		Value::Float(number) => *number as f32,
		_ => return None,
	};

	number.is_finite().then_some(number)
}

fn stroke(value: &Value) -> Option<InkStroke> {
	let map = value.as_map()?;
	let mut points: Vec<f32> = map
		.get("points")?
		.as_array()?
		.iter()
		.filter_map(number)
		.collect();
	points.truncate(points.len() / 2 * 2);

	if points.is_empty() {
		return None;
	}

	Some(InkStroke {
		points,
		color: map
			.get("color")
			.and_then(Value::as_str)
			.and_then(Color::parse_hex),
		width: map
			.get("width")
			.and_then(number)
			.filter(|width| *width > 0.0),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn reads_strokes_and_skips_malformed_ones() {
		let stroke = |points: Vec<Value>| {
			Value::from(Attrs::new().with("points", points).with("color", "#ff0000"))
		};
		let attrs = Attrs::new().with(
			"strokes",
			vec![
				stroke(vec![Value::Int(1), Value::Float(2.5), Value::Int(3)]),
				stroke(Vec::new()),
				Value::from("not a stroke"),
			],
		);
		let drawing = InkDrawing::from_attrs(&attrs).expect("a drawing");

		assert_eq!(drawing.strokes.len(), 1);
		assert_eq!(drawing.strokes[0].points, vec![1.0, 2.5]);
		assert_eq!(drawing.strokes[0].color, Color::parse_hex("#ff0000"));
	}
}
