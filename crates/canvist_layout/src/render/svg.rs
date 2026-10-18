//! Painting a display list as an SVG string, for SSR, export, and
//! thumbnails.

use std::fmt::Write;

use super::attribute;
use super::escape_into;
use super::num;
use crate::Color;
use crate::DecorationKind;
use crate::DisplayItem;
use crate::DisplayList;
use crate::Rect;
use crate::SlotKind;
use crate::display::TextRun;

/// How far a squiggle swings above and below its centre line, as a multiple
/// of the decoration's thickness.
const SQUIGGLE_AMPLITUDE_PER_THICKNESS: f32 = 1.5;

/// The length of one full squiggle wave, as a multiple of its amplitude.
const SQUIGGLE_WAVELENGTH_PER_AMPLITUDE: f32 = 4.0;

/// The checkbox tick's start, corner, and end points, as fractions of the
/// checkbox's width and height.
const CHECK_TICK_POINTS: [(f32, f32); 3] = [(0.2, 0.5), (0.42, 0.72), (0.8, 0.28)];

/// Paint `list` as a standalone SVG document whose viewport is the display
/// list's bounds.
///
/// Text is emitted as `<text>` with `xml:space="preserve"` and the run's
/// font, so the SVG renders with the same faces the layout measured.
/// Right-to-left runs anchor at their right edge with `direction="rtl"`.
/// Slots are drawn as dashed placeholders carrying `data-block`, and checked
/// checkboxes get a tick.
#[must_use]
pub fn to_svg(list: &DisplayList) -> String {
	let bounds = list.bounds;
	let mut out = String::new();
	let _ = write!(
		out,
		"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} \
		 {}\">",
		num(bounds.width),
		num(bounds.height),
		num(bounds.x),
		num(bounds.y),
		num(bounds.width),
		num(bounds.height)
	);

	for item in &list.items {
		match item {
			DisplayItem::Rect {
				rect,
				color,
				radius,
				..
			} => rect_element(&mut out, rect, *color, *radius),
			DisplayItem::Text(run) => text_element(&mut out, list, run),
			DisplayItem::Decoration {
				kind: DecorationKind::Squiggle,
				x,
				y,
				width,
				thickness,
				color,
				..
			} => squiggle(&mut out, (*x, *y), *width, *thickness, *color),
			DisplayItem::Decoration {
				x,
				y,
				width,
				thickness,
				color,
				..
			} => {
				let line = Rect::new(*x, y - thickness / 2.0, *width, *thickness);
				rect_element(&mut out, &line, *color, 0.0);
			}
			DisplayItem::Slot {
				kind, block, rect, ..
			} => slot(&mut out, kind, &block.to_string(), rect),
		}
	}

	out.push_str("</svg>");
	out
}

fn fill(out: &mut String, color: Color) {
	let opaque = Color { a: 255, ..color };
	let _ = write!(out, " fill=\"{opaque}\"");

	if color.a != 255 {
		let _ = write!(out, " fill-opacity=\"{}\"", num(f32::from(color.a) / 255.0));
	}
}

fn rect_element(out: &mut String, rect: &Rect, color: Color, radius: f32) {
	let _ = write!(
		out,
		"<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
		num(rect.x),
		num(rect.y),
		num(rect.width),
		num(rect.height)
	);

	if radius > 0.0 {
		let _ = write!(out, " rx=\"{}\"", num(radius));
	}

	fill(out, color);
	out.push_str("/>");
}

/// Write a text run as `<text>`.
///
/// Each optional attribute (italic, letter spacing, right-to-left direction)
/// is emitted only when it differs from the SVG default, which keeps the
/// output small. A run whose font is missing from the list paints nothing
/// rather than guessing a face.
fn text_element(out: &mut String, list: &DisplayList, run: &TextRun) {
	let Some(font) = list.fonts.get(run.font.index()) else {
		return;
	};
	let x = if run.rtl { run.x + run.width } else { run.x };
	let _ = write!(out, "<text x=\"{}\" y=\"{}\"", num(x), num(run.baseline));
	attribute(out, "font-family", &font.family);
	let _ = write!(
		out,
		" font-size=\"{}\" font-weight=\"{}\"",
		num(font.size),
		font.weight
	);

	if font.italic {
		out.push_str(" font-style=\"italic\"");
	}

	if run.letter_spacing != 0.0 {
		let _ = write!(out, " letter-spacing=\"{}\"", num(run.letter_spacing));
	}

	if run.rtl {
		out.push_str(" direction=\"rtl\" unicode-bidi=\"embed\"");
	}

	fill(out, run.color);
	out.push_str(" xml:space=\"preserve\">");
	escape_into(out, run.text());
	out.push_str("</text>");
}

/// Draw a wavy line from `start` as a chain of quadratic half-waves, cutting
/// the last one short so the squiggle ends exactly at `width`.
fn squiggle(out: &mut String, start: (f32, f32), width: f32, thickness: f32, color: Color) {
	let (x, y) = start;
	let amplitude = thickness * SQUIGGLE_AMPLITUDE_PER_THICKNESS;
	let wave = amplitude * SQUIGGLE_WAVELENGTH_PER_AMPLITUDE;
	let _ = write!(out, "<path d=\"M{} {}", num(x), num(y));
	let mut position = 0.0;
	let mut up = true;

	while position < width {
		let step = (wave / 2.0).min(width - position);
		let peak = if up { -amplitude } else { amplitude };
		let _ = write!(out, " q{} {} {} 0", num(step / 2.0), num(peak), num(step));
		position += step;
		up = !up;
	}

	let opaque = Color { a: 255, ..color };
	let _ = write!(
		out,
		"\" fill=\"none\" stroke=\"{opaque}\" stroke-width=\"{}\"/>",
		num(thickness)
	);
}

/// Draw a slot as a placeholder outline tagged with its kind and block.
///
/// Checkboxes are the one slot kind with a real appearance, so they get a
/// solid rounded box and, when checked, a tick; every other kind is a dashed
/// square box that only marks where host content will go.
fn slot(out: &mut String, kind: &SlotKind, block: &str, rect: &Rect) {
	let name = match kind {
		SlotKind::Block => "block",
		SlotKind::Embed(_) => "embed",
		SlotKind::Checkbox { .. } => "checkbox",
	};

	let is_checkbox = matches!(kind, SlotKind::Checkbox { .. });
	let radius = if is_checkbox { "3" } else { "0" };
	let _ = write!(
		out,
		"<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{radius}\" fill=\"none\" \
		 stroke=\"#8a8580\"",
		num(rect.x),
		num(rect.y),
		num(rect.width),
		num(rect.height)
	);

	if !is_checkbox {
		out.push_str(" stroke-dasharray=\"4 2\"");
	}

	attribute(out, "data-slot", name);
	attribute(out, "data-block", block);
	out.push_str("/>");

	if matches!(kind, SlotKind::Checkbox { checked: true }) {
		check_tick(out, rect);
	}
}

fn check_tick(out: &mut String, rect: &Rect) {
	out.push_str("<path d=\"");

	for (index, (across, down)) in CHECK_TICK_POINTS.into_iter().enumerate() {
		let command = if index == 0 { "M" } else { " L" };
		let x = rect.x + rect.width * across;
		let y = rect.y + rect.height * down;
		let _ = write!(out, "{command}{} {}", num(x), num(y));
	}

	out.push_str("\" fill=\"none\" stroke=\"#2e2b29\" stroke-width=\"1.5\"/>");
}
