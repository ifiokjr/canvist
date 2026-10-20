//! Constraints and auto-layout as JSON.

use serde_json::Value as Json;
use serde_json::json;

use super::Decoded;
use super::boolean;
use super::field;
use super::named;
use super::number;
use super::numbers;
use super::object;
use crate::Align;
use crate::AutoLayout;
use crate::Constraint;
use crate::Constraints;
use crate::Direction;
use crate::LayoutChild;
use crate::Padding;
use crate::Sizing;

const DIRECTIONS: [Direction; 2] = [Direction::Row, Direction::Column];

fn direction_name(direction: Direction) -> &'static str {
	match direction {
		Direction::Row => "row",
		Direction::Column => "column",
	}
}

fn constraint(value: &Json) -> Decoded<Constraint> {
	named(value, Constraint::ALL, Constraint::name)
}

fn align(value: &Json) -> Decoded<Align> {
	named(value, Align::ALL, Align::name)
}

fn sizing(value: &Json) -> Decoded<Sizing> {
	named(value, Sizing::ALL, Sizing::name)
}

pub(super) fn constraints_to_json(constraints: Constraints) -> Json {
	json!({
		"horizontal": constraints.horizontal.name(),
		"vertical": constraints.vertical.name(),
	})
}

pub(super) fn constraints_from_json(value: &Json) -> Decoded<Constraints> {
	let map = object(value)?;

	Ok(Constraints {
		horizontal: field(map, "horizontal", Constraint::Start, constraint)?,
		vertical: field(map, "vertical", Constraint::Start, constraint)?,
	})
}

pub(super) fn auto_layout_to_json(layout: AutoLayout) -> Json {
	let padding = layout.padding;

	json!({
		"direction": direction_name(layout.direction),
		"gap": layout.gap,
		"padding": [padding.top, padding.right, padding.bottom, padding.left],
		"align": layout.align.name(),
		"crossAlign": layout.cross_align.name(),
		"wrap": layout.wrap,
	})
}

pub(super) fn auto_layout_from_json(value: &Json) -> Decoded<Option<AutoLayout>> {
	if value.is_null() {
		return Ok(None);
	}

	let map = object(value)?;

	Ok(Some(AutoLayout {
		direction: field(map, "direction", Direction::Row, |value| {
			named(value, DIRECTIONS, direction_name)
		})?,
		gap: field(map, "gap", 0.0, number)?,
		padding: field(map, "padding", Padding::default(), padding)?,
		align: field(map, "align", Align::Start, align)?,
		cross_align: field(map, "crossAlign", Align::Start, align)?,
		wrap: field(map, "wrap", false, boolean)?,
	}))
}

fn padding(value: &Json) -> Decoded<Padding> {
	if value.is_number() {
		return number(value).map(Padding::all);
	}

	let [top, right, bottom, left] = numbers(value)?;

	Ok(Padding {
		top,
		right,
		bottom,
		left,
	})
}

pub(super) fn layout_child_to_json(child: LayoutChild) -> Json {
	json!({
		"width": child.width.name(),
		"height": child.height.name(),
		"absolute": child.absolute,
	})
}

pub(super) fn layout_child_from_json(value: &Json) -> Decoded<LayoutChild> {
	let map = object(value)?;

	Ok(LayoutChild {
		width: field(map, "width", Sizing::Fixed, sizing)?,
		height: field(map, "height", Sizing::Fixed, sizing)?,
		absolute: field(map, "absolute", false, boolean)?,
	})
}
