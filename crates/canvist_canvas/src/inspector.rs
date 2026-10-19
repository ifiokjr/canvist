//! Inspectors as data: what can be edited about the selection, described
//! so any host can render a panel and any agent can learn what is
//! editable, with no UI in the core.
//!
//! A field's key is a property name ([`PropKey`]'s names, such as
//! `opacity`, `fills`, or `c:seed`) or one of the geometry keys `x`, `y`,
//! `width`, `height`, and `rotation`, which read and write the node's
//! transform and size in its parent's coordinates.

use canvist_scene::NodeId;
use canvist_scene::NodeKind;
use canvist_scene::Prop;
use canvist_scene::PropKey;
use canvist_scene::Scene;
use canvist_scene::Step;
use kurbo::Affine;
use kurbo::Point;
use kurbo::Vec2;
use serde_json::Value as Json;
use serde_json::json;

use crate::CanvasError;
use crate::CanvasState;

/// How a host might edit a field.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
	/// A number, optionally bounded, in `unit` (such as `px` or `%`).
	Number {
		/// The smallest value.
		min: Option<f64>,
		/// The largest value.
		max: Option<f64>,
		/// The step between values.
		step: f64,
		/// The unit shown beside it.
		unit: &'static str,
	},
	/// An angle in degrees.
	Angle,
	/// On or off.
	Toggle,
	/// One of a fixed set of strings.
	Select(Vec<&'static str>),
	/// Free text.
	Text,
	/// A list of paints.
	Paints,
	/// Any JSON value matching the property's format.
	Json,
}

impl Control {
	/// A number control with a step and unit.
	#[must_use]
	pub fn number(step: f64, unit: &'static str) -> Self {
		Self::Number {
			min: None,
			max: None,
			step,
			unit,
		}
	}

	/// The control as JSON metadata.
	#[must_use]
	pub fn meta(&self) -> Json {
		match self {
			Self::Number {
				min,
				max,
				step,
				unit,
			} => {
				json!({ "type": "number", "min": min, "max": max, "step": step, "unit": unit })
			}
			Self::Angle => json!({ "type": "angle" }),
			Self::Toggle => json!({ "type": "toggle" }),
			Self::Select(options) => json!({ "type": "select", "options": options }),
			Self::Text => json!({ "type": "text" }),
			Self::Paints => json!({ "type": "paints" }),
			Self::Json => json!({ "type": "json" }),
		}
	}
}

/// One editable field.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
	/// The property name or geometry key.
	pub key: String,
	/// A label for people.
	pub label: String,
	/// How to edit it.
	pub control: Control,
}

impl Field {
	/// A field.
	#[must_use]
	pub fn new(key: &str, label: &str, control: Control) -> Self {
		Self {
			key: key.to_owned(),
			label: label.to_owned(),
			control,
		}
	}

	/// The field as JSON metadata.
	#[must_use]
	pub fn meta(&self) -> Json {
		json!({ "key": self.key, "label": self.label, "control": self.control.meta() })
	}
}

/// A group of fields shown for some node kinds.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
	/// A stable id, such as `layer`.
	pub id: String,
	/// A title for people.
	pub title: String,
	/// The node kinds it applies to; `None` for every kind.
	pub kinds: Option<Vec<NodeKind>>,
	/// The fields.
	pub fields: Vec<Field>,
}

impl Section {
	/// A section for every node kind.
	#[must_use]
	pub fn new(id: &str, title: &str, fields: Vec<Field>) -> Self {
		Self {
			id: id.to_owned(),
			title: title.to_owned(),
			kinds: None,
			fields,
		}
	}

	/// Limit the section to `kinds`.
	#[must_use]
	pub fn for_kinds(mut self, kinds: &[NodeKind]) -> Self {
		self.kinds = Some(kinds.to_vec());
		self
	}

	fn applies(&self, kind: NodeKind) -> bool {
		self.kinds
			.as_ref()
			.is_none_or(|kinds| kinds.contains(&kind))
	}
}

/// Node kinds with a box of their own, which the geometry, fill, stroke, and
/// effects sections apply to.
const SHAPED: [NodeKind; 11] = [
	NodeKind::Frame,
	NodeKind::Rectangle,
	NodeKind::Ellipse,
	NodeKind::Polygon,
	NodeKind::Path,
	NodeKind::Boolean,
	NodeKind::Image,
	NodeKind::Text,
	NodeKind::Component,
	NodeKind::Instance,
	NodeKind::Custom,
];

/// The sections every canvas has: layer, geometry, fill, stroke, effects.
#[must_use]
pub fn core_sections() -> Vec<Section> {
	vec![
		layer_section(),
		Section::new(
			"geometry",
			"Geometry",
			vec![
				Field::new("x", "X", Control::number(1.0, "px")),
				Field::new("y", "Y", Control::number(1.0, "px")),
				Field::new("width", "W", Control::number(1.0, "px")),
				Field::new("height", "H", Control::number(1.0, "px")),
				Field::new("rotation", "Rotation", Control::Angle),
			],
		)
		.for_kinds(&SHAPED),
		Section::new(
			"fill",
			"Fill",
			vec![Field::new("fills", "Fills", Control::Paints)],
		)
		.for_kinds(&SHAPED),
		Section::new(
			"stroke",
			"Stroke",
			vec![
				Field::new("strokes", "Strokes", Control::Paints),
				Field::new("stroke", "Style", Control::Json),
			],
		)
		.for_kinds(&SHAPED),
		Section::new(
			"effects",
			"Effects",
			vec![Field::new("effects", "Effects", Control::Json)],
		)
		.for_kinds(&SHAPED),
	]
}

/// The layer section: name, visibility, lock, opacity, and blend mode.
fn layer_section() -> Section {
	let blends: Vec<&'static str> = canvist_scene::BlendMode::ALL
		.iter()
		.map(|mode| mode.name())
		.collect();

	Section::new(
		"layer",
		"Layer",
		vec![
			Field::new("name", "Name", Control::Text),
			Field::new("visible", "Visible", Control::Toggle),
			Field::new("locked", "Locked", Control::Toggle),
			Field::new(
				"opacity",
				"Opacity",
				Control::Number {
					min: Some(0.0),
					max: Some(1.0),
					step: 0.01,
					unit: "",
				},
			),
			Field::new("blend", "Blend", Control::Select(blends)),
		],
	)
}

/// The inspector for the current selection: each section that applies to
/// every selected node, with each field's shared value, or `"mixed": true`
/// when the nodes differ.
#[must_use]
pub fn inspect(state: &CanvasState, sections: &[Section]) -> Json {
	let scene = state.scene();
	let nodes = state.selection().nodes();
	let kinds: Vec<NodeKind> = nodes.iter().filter_map(|&id| scene.kind(id)).collect();

	if kinds.is_empty() {
		return json!({ "sections": [] });
	}

	let sections: Vec<Json> = sections
		.iter()
		.filter(|section| kinds.iter().all(|&kind| section.applies(kind)))
		.map(|section| {
			let fields: Vec<Json> = section
				.fields
				.iter()
				.map(|field| field_value(scene, nodes, field))
				.collect();
			json!({ "id": section.id, "title": section.title, "fields": fields })
		})
		.collect();

	json!({ "sections": sections })
}

fn field_value(scene: &Scene, nodes: &[NodeId], field: &Field) -> Json {
	let mut values = nodes.iter().map(|&id| read(scene, id, &field.key));
	let first = values.next().flatten();
	let mixed = values.any(|value| value != first);
	let mut out = field.meta();

	if let Some(object) = out.as_object_mut() {
		if mixed {
			object.insert("mixed".into(), Json::Bool(true));
		} else {
			object.insert("value".into(), first.unwrap_or(Json::Null));
		}
	}

	out
}

/// The value of `key` on node `id`.
#[must_use]
pub fn read(scene: &Scene, id: NodeId, key: &str) -> Option<Json> {
	let props = scene.node(id)?.props();
	let geometry = Geometry::from_transform(props.transform, props.size);

	Some(match key {
		"x" => json!(geometry.origin.x),
		"y" => json!(geometry.origin.y),
		"width" => json!(props.size.x),
		"height" => json!(props.size.y),
		"rotation" => json!(geometry.rotation.to_degrees()),
		_ => props.get(&PropKey::parse(key)?).to_json(),
	})
}

/// Set field `key` to `value` on every selected node, as one commit.
pub fn set_field(state: &mut CanvasState, key: &str, value: &Json) -> Result<(), CanvasError> {
	let nodes = state.selection().nodes().to_vec();
	let steps = nodes
		.iter()
		.map(|&id| write(state.scene(), id, key, value))
		.collect::<Result<Vec<_>, _>>()?;

	state.apply(steps.into_iter().flatten().collect(), false)?;
	Ok(())
}

fn write(scene: &Scene, id: NodeId, key: &str, value: &Json) -> Result<Vec<Step>, CanvasError> {
	let Some(node) = scene.node(id) else {
		return Ok(Vec::new());
	};
	let props = node.props();
	let number = || {
		value
			.as_f64()
			.filter(|number| number.is_finite())
			.ok_or_else(|| CanvasError::InvalidArgs(format!("`{key}` needs a number")))
	};
	let set = |prop: Prop| Step::SetProp { node: id, prop };
	let geometry = Geometry::from_transform(props.transform, props.size);

	Ok(match key {
		"x" => {
			vec![set(Prop::Transform(
				geometry.moved(Point::new(number()?, geometry.origin.y)),
			))]
		}
		"y" => {
			vec![set(Prop::Transform(
				geometry.moved(Point::new(geometry.origin.x, number()?)),
			))]
		}
		"width" => vec![set(Prop::Size(Vec2::new(number()?.max(0.0), props.size.y)))],
		"height" => vec![set(Prop::Size(Vec2::new(props.size.x, number()?.max(0.0))))],
		"rotation" => {
			vec![set(Prop::Transform(
				geometry.rotated(number()?.to_radians()),
			))]
		}
		_ => {
			let key =
				PropKey::parse(key).ok_or_else(|| CanvasError::UnknownField(key.to_owned()))?;
			vec![set(Prop::from_json(&key, value)?)]
		}
	})
}

/// A transform read as a position and rotation, for the inspector.
struct Geometry {
	transform: Affine,
	size: Vec2,
	origin: Point,
	rotation: f64,
}

impl Geometry {
	fn from_transform(transform: Affine, size: Vec2) -> Self {
		let [a, b, ..] = transform.as_coeffs();

		Self {
			transform,
			size,
			origin: transform * Point::ORIGIN,
			rotation: b.atan2(a),
		}
	}

	/// The transform with its origin at `origin`.
	fn moved(&self, origin: Point) -> Affine {
		self.transform.with_translation(origin.to_vec2())
	}

	/// The transform rotated to `angle`, about the box's centre.
	fn rotated(&self, angle: f64) -> Affine {
		let center = self.transform * (self.size / 2.0).to_point();
		let turn = Affine::rotate_about(angle - self.rotation, center);
		turn * self.transform
	}
}
