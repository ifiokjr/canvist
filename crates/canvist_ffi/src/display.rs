//! What the platform paints: layout's display list as plain records.
//!
//! Items are in paint order and in document coordinates. A painter
//! translates by the scroll position and draws each one: Core Graphics on
//! iOS and `Canvas` on Android, with the same fonts the platform measured.

use canvist_headless::canvist_model::BlockId;
use canvist_headless::canvist_model::Document;
use canvist_layout::DecorationKind;
use canvist_layout::DisplayItem as LayoutItem;
use canvist_layout::DisplayList;
use canvist_layout::Rect;
use canvist_layout::RectKind;
use canvist_layout::SlotKind;
use canvist_layout::TextRun;

use crate::Bounds;
use crate::Rgba;

/// What a filled rectangle is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum FillKind {
	/// A block background, such as a code block's.
	Background,
	/// A container's left border, such as a block quote's bar.
	Border,
	/// A horizontal rule.
	Rule,
	/// An inline background, such as inline code or a highlight.
	Highlight,
	/// The marked text of an IME composition.
	Composition,
	/// The selection highlight.
	Selection,
	/// The caret.
	Caret,
}

impl From<RectKind> for FillKind {
	fn from(kind: RectKind) -> Self {
		match kind {
			RectKind::Background => Self::Background,
			RectKind::Border => Self::Border,
			RectKind::Rule => Self::Rule,
			RectKind::Highlight | RectKind::Range => Self::Highlight,
			RectKind::Selection => Self::Selection,
			RectKind::Caret => Self::Caret,
		}
	}
}

/// A text decoration line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum LineKind {
	/// Under the text.
	Underline,
	/// Through the text.
	Strike,
	/// A wavy line under the text.
	Squiggle,
}

impl From<DecorationKind> for LineKind {
	fn from(kind: DecorationKind) -> Self {
		match kind {
			DecorationKind::Underline => Self::Underline,
			DecorationKind::Strike => Self::Strike,
			DecorationKind::Squiggle => Self::Squiggle,
		}
	}
}

/// One thing to paint.
#[derive(Clone, Debug, PartialEq, uniffi::Enum)]
pub enum DisplayItem {
	/// A filled, optionally rounded, rectangle.
	Fill {
		/// The rectangle.
		bounds: Bounds,
		/// The fill.
		color: Rgba,
		/// The corner radius.
		radius: f32,
		/// What it is for.
		kind: FillKind,
	},
	/// A run of text in one font and direction.
	Text {
		/// The characters, in logical order.
		text: String,
		/// The font's id, as the measurer was sent it.
		font: u32,
		/// The run's left edge.
		x: f32,
		/// The baseline.
		baseline: f32,
		/// The advance width layout measured for the run.
		width: f32,
		/// The fill colour.
		color: Rgba,
		/// Extra space after each grapheme, like CSS `letter-spacing`.
		letter_spacing: f32,
		/// Whether the run is right to left: draw it with the platform's
		/// RTL direction, anchored at `x` on the left.
		rtl: bool,
		/// The line box the run sits in. Glyphs may reach a little outside
		/// it (accents, descenders, italic overhangs).
		bounds: Bounds,
	},
	/// A decoration line from `x` to `x + width`, centred on `y`.
	Line {
		/// The kind of line.
		kind: LineKind,
		/// The left end.
		x: f32,
		/// The vertical centre.
		y: f32,
		/// The length.
		width: f32,
		/// The thickness.
		thickness: f32,
		/// The colour.
		color: Rgba,
	},
	/// A freehand stroke through `points`, drawn as straight segments with
	/// round caps and joins. One point draws a dot.
	Stroke {
		/// The points, as `x, y` pairs in document coordinates.
		points: Vec<f32>,
		/// The line width.
		width: f32,
		/// The colour.
		color: Rgba,
		/// The area the stroke paints, its round caps included.
		bounds: Bounds,
	},
	/// A task list item's checkbox.
	Checkbox {
		/// The box.
		bounds: Bounds,
		/// Whether the task is done.
		checked: bool,
	},
	/// Space reserved for a view the host draws itself, such as an image.
	Slot {
		/// The space.
		bounds: Bounds,
		/// The node's name in the schema, such as `image` or `mention`.
		name: String,
	},
}

/// One frame of painting.
#[derive(Clone, Debug, Default, PartialEq, uniffi::Record)]
pub struct Frame {
	/// The document's height, including estimates for blocks not laid out
	/// yet. The host sizes its scrolling content to it.
	pub height: f32,
	/// How far content at the top of the viewport moved because blocks
	/// above it were measured. Add it to the scroll position to keep the
	/// content still.
	pub shift: f32,
	/// The page colour behind the document.
	pub background: Rgba,
	/// Everything in the viewport, in paint order, or `None` when nothing
	/// changed since the last frame: keep painting the last list.
	pub items: Option<Vec<DisplayItem>>,
	/// The areas that changed, in document coordinates. Empty when `full`.
	pub damage: Vec<Bounds>,
	/// Whether the whole viewport must be repainted.
	pub full: bool,
	/// Blocks laid out for this frame.
	pub laid_out: u32,
	/// Calls to the platform measurer for this frame: 0 when every segment
	/// was cached.
	pub measured: u32,
}

/// The display list as records, naming slots from `doc`'s schema.
pub(crate) fn items(list: &DisplayList, doc: &Document) -> Vec<DisplayItem> {
	list.items
		.iter()
		.map(|item| record(list, item, doc))
		.collect()
}

fn record(list: &DisplayList, item: &LayoutItem, doc: &Document) -> DisplayItem {
	match item {
		LayoutItem::Rect {
			rect,
			color,
			radius,
			kind,
			..
		} => {
			DisplayItem::Fill {
				bounds: (*rect).into(),
				color: (*color).into(),
				radius: *radius,
				kind: (*kind).into(),
			}
		}
		LayoutItem::Text(run) => text(run),
		LayoutItem::Decoration {
			kind,
			x,
			y,
			width,
			thickness,
			color,
			..
		} => {
			DisplayItem::Line {
				kind: (*kind).into(),
				x: *x,
				y: *y,
				width: *width,
				thickness: *thickness,
				color: (*color).into(),
			}
		}
		LayoutItem::Stroke {
			points,
			width,
			color,
			bounds,
			..
		} => {
			DisplayItem::Stroke {
				points: list.stroke_points(points).to_vec(),
				width: *width,
				color: (*color).into(),
				bounds: (*bounds).into(),
			}
		}
		LayoutItem::Slot {
			kind, rect, block, ..
		} => slot(doc, kind, *block, *rect),
	}
}

fn text(run: &TextRun) -> DisplayItem {
	DisplayItem::Text {
		text: run.text().to_owned(),
		font: run.font.index() as u32,
		x: run.x,
		baseline: run.baseline,
		width: run.width,
		color: run.color.into(),
		letter_spacing: run.letter_spacing,
		rtl: run.rtl,
		bounds: run.bounds.into(),
	}
}

/// A checkbox, or a slot named after what it holds in `doc`'s schema.
fn slot(doc: &Document, kind: &SlotKind, block: BlockId, rect: Rect) -> DisplayItem {
	let schema = doc.schema();

	let name = match kind {
		SlotKind::Checkbox { checked } => {
			return DisplayItem::Checkbox {
				bounds: rect.into(),
				checked: *checked,
			};
		}
		SlotKind::Embed(embed) => schema.embed(*embed).name.to_string(),
		SlotKind::Block => {
			doc.block(block)
				.map(|block| schema.block(block.kind()).name.to_string())
				.unwrap_or_default()
		}
	};

	DisplayItem::Slot {
		bounds: rect.into(),
		name,
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use canvist_headless::canvist_model::BlockSpec;
	use canvist_headless::canvist_model::Schema;
	use canvist_layout::Color;

	use super::*;

	#[test]
	fn an_ink_stroke_keeps_its_points_width_and_colour() {
		let schema = Schema::builder()
			.block(BlockSpec::text("paragraph"))
			.build()
			.expect("valid schema");
		let doc = Document::new(Arc::new(schema), 1);
		let bounds = Rect::new(-0.25, 0.75, 4.5, 4.5);
		let item = LayoutItem::Stroke {
			points: 2..6,
			width: 2.5,
			color: Color::rgb(10, 20, 30),
			bounds,
			block: BlockId::new(1, 0),
		};
		let list = DisplayList {
			fonts: Vec::new(),
			items: vec![item.clone()],
			bounds,
			animated: false,
			points: vec![9.0, 9.0, 1.0, 2.0, 3.0, 4.0],
		};

		assert_eq!(
			record(&list, &item, &doc),
			DisplayItem::Stroke {
				points: vec![1.0, 2.0, 3.0, 4.0],
				width: 2.5,
				color: Color::rgb(10, 20, 30).into(),
				bounds: bounds.into(),
			}
		);
	}
}
