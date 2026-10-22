//! The display list: the only thing painters see.
//!
//! Items are in paint order and in document coordinates. A painter
//! translates by the scroll offset, then draws each item: `fillText` per
//! [`DisplayItem::Text`] on the web, `<text>` in [`to_svg`](crate::to_svg),
//! glyph runs in vello, and so on. Slots are rectangles reserved for views the
//! host renders itself, such as images and checkboxes.

use std::ops::Range;
use std::sync::Arc;

use canvist_model::BlockId;
use canvist_model::EmbedTypeId;
use canvist_model::Selection;
use rustc_hash::FxHashMap;

use crate::Caret;
use crate::Color;
use crate::FontId;
use crate::FontSpec;
use crate::Rect;
use crate::text::slice;

mod build;
mod motion;

/// What a rectangle is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RectKind {
	/// A block or container background, such as a code block.
	Background,
	/// A container's border, such as a blockquote bar or a table's grid
	/// lines.
	Border,
	/// A horizontal rule.
	Rule,
	/// An inline background, such as inline code or a highlight mark.
	Highlight,
	/// A caller-supplied highlight range, such as a search match.
	Range,
	/// The selection.
	Selection,
	/// The caret.
	Caret,
}

/// A text decoration line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecorationKind {
	/// A straight line below the baseline.
	Underline,
	/// A straight line through the middle of lowercase letters.
	Strike,
	/// A wavy line below the baseline.
	Squiggle,
}

/// What a host-rendered slot holds.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SlotKind {
	/// A leaf block's node view, such as an image.
	Block,
	/// An inline embed's node view.
	Embed(EmbedTypeId),
	/// A task list item's checkbox.
	Checkbox {
		/// Whether the item is checked.
		checked: bool,
	},
}

/// A run of text in one font and direction.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
	/// The block the text belongs to.
	pub block: BlockId,
	/// The block's text (shared, not copied); the run covers `range` of it.
	pub source: Arc<str>,
	/// The byte range of `source` to draw.
	pub range: Range<u32>,
	/// The font, indexed into [`DisplayList::fonts`].
	pub font: FontId,
	/// The left edge of the run.
	pub x: f32,
	/// The baseline.
	pub baseline: f32,
	/// The advance width layout used.
	pub width: f32,
	/// The fill colour.
	pub color: Color,
	/// CSS `letter-spacing` to apply while drawing.
	pub letter_spacing: f32,
	/// Whether the run is right to left. Its characters are in logical order;
	/// draw it with the platform's RTL direction, anchored at `x` on the left.
	pub rtl: bool,
	/// The line box the run sits in.
	pub bounds: Rect,
}

impl TextRun {
	/// The text to draw.
	#[must_use]
	pub fn text(&self) -> &str {
		slice(&self.source, &self.range)
	}
}

/// One thing to paint.
#[derive(Clone, Debug, PartialEq)]
pub enum DisplayItem {
	/// A filled rectangle.
	Rect {
		/// The rectangle.
		rect: Rect,
		/// The fill.
		color: Color,
		/// The corner radius.
		radius: f32,
		/// What the rectangle is for.
		kind: RectKind,
		/// The block it belongs to, or `None` for the selection and caret.
		block: Option<BlockId>,
	},
	/// Text.
	Text(TextRun),
	/// A decoration line from `x` to `x + width`, centred on `y`.
	Decoration {
		/// The kind of line.
		kind: DecorationKind,
		/// The left end.
		x: f32,
		/// The vertical centre of the line.
		y: f32,
		/// The length.
		width: f32,
		/// The line thickness.
		thickness: f32,
		/// The colour.
		color: Color,
		/// The block it belongs to.
		block: BlockId,
	},
	/// A freehand stroke, drawn as straight segments with round caps and
	/// joins through its points: `x, y` pairs in document pixels, read with
	/// [`DisplayList::stroke_points`]. The points live in the list rather
	/// than the item, so no item but a text run owns memory and the bulk
	/// of every list stays cheap to build and drop.
	Stroke {
		/// The stroke's range of [`DisplayList::points`].
		points: Range<u32>,
		/// The line width in pixels.
		width: f32,
		/// The colour.
		color: Color,
		/// The area the stroke paints, its round caps included.
		bounds: Rect,
		/// The block it belongs to.
		block: BlockId,
	},
	/// A rectangle reserved for a host-rendered view.
	Slot {
		/// What the slot holds.
		kind: SlotKind,
		/// The block it belongs to.
		block: BlockId,
		/// The byte offset of an inline embed within the block.
		offset: Option<u32>,
		/// The rectangle.
		rect: Rect,
	},
}

impl DisplayItem {
	/// The block the item belongs to, if any.
	#[must_use]
	pub fn block(&self) -> Option<BlockId> {
		match self {
			Self::Rect { block, .. } => *block,
			Self::Text(run) => Some(run.block),
			Self::Decoration { block, .. }
			| Self::Stroke { block, .. }
			| Self::Slot { block, .. } => Some(*block),
		}
	}

	/// The area the item paints.
	#[must_use]
	pub fn bounds(&self) -> Rect {
		match self {
			Self::Rect { rect, .. } | Self::Slot { rect, .. } => *rect,
			Self::Text(run) => run.bounds,
			Self::Decoration {
				x,
				y,
				width,
				thickness,
				..
			} => Rect::new(*x, y - thickness * 2.0, *width, thickness * 4.0),
			Self::Stroke { bounds, .. } => *bounds,
		}
	}
}

/// What to put in a display list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayOptions {
	/// The top of the area to paint.
	pub top: f32,
	/// The bottom of the area to paint.
	pub bottom: f32,
	/// The selection, if it is not collapsed.
	pub selection: Option<Selection>,
	/// The caret, if it is visible.
	pub caret: Option<Caret>,
	/// Extra highlighted ranges, such as search results, with their colours.
	pub highlights: Vec<(Selection, Color)>,
	/// Placeholder text for empty text blocks, such as "Write something …",
	/// drawn at the block's start in its text colour, faded.
	pub placeholders: Vec<(BlockId, Arc<str>)>,
	/// The animation clock in seconds, for kinetic text. `None` keeps all
	/// text still: pass it only while animating, and never to readers who
	/// prefer reduced motion. [`DisplayList::animated`] says whether a
	/// later frame would differ.
	pub time: Option<f32>,
}

impl DisplayOptions {
	/// Paint the area from `top` to `bottom`, with no selection or caret.
	#[must_use]
	pub fn viewport(top: f32, bottom: f32) -> Self {
		Self {
			top,
			bottom,
			..Self::default()
		}
	}

	/// Add a selection.
	#[must_use]
	pub fn selection(mut self, selection: Selection) -> Self {
		self.selection = Some(selection);
		self
	}

	/// Add a caret.
	#[must_use]
	pub fn caret(mut self, caret: Caret) -> Self {
		self.caret = Some(caret);
		self
	}

	/// Add a highlighted range.
	#[must_use]
	pub fn highlight(mut self, range: Selection, color: Color) -> Self {
		self.highlights.push((range, color));
		self
	}

	/// Move kinetic text as it is at `seconds` on the animation clock.
	#[must_use]
	pub fn time(mut self, seconds: f32) -> Self {
		self.time = Some(seconds);
		self
	}

	/// Show `text` in `block` while it is empty.
	#[must_use]
	pub fn placeholder(mut self, block: BlockId, text: &str) -> Self {
		self.placeholders.push((block, Arc::from(text)));
		self
	}
}

/// Everything to paint for one area of the document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList {
	/// Fonts referenced by text runs, indexed by [`FontId::index`]. Ids are
	/// stable for the life of the layout, so painters can cache per id.
	pub fonts: Vec<FontSpec>,
	/// Items in paint order.
	pub items: Vec<DisplayItem>,
	/// The painted area: the full document width between the requested top
	/// and bottom.
	pub bounds: Rect,
	/// Whether kinetic text is in view, so a host animating it should keep
	/// asking for frames, and one that is not knows it could.
	pub animated: bool,
	/// The points of every stroke, as `x, y` pairs, each stroke's at the
	/// range it holds.
	pub points: Vec<f32>,
}

impl DisplayList {
	/// A stroke's points, as `x, y` pairs, from the range it holds; empty
	/// for a range outside this list.
	#[must_use]
	pub fn stroke_points(&self, range: &Range<u32>) -> &[f32] {
		self.points
			.get(range.start as usize..range.end as usize)
			.unwrap_or_default()
	}

	/// The areas that differ between `previous` and `self`.
	///
	/// Items are grouped by block (plus one group for the selection and
	/// caret). Every group that changed contributes its bounds in both lists,
	/// so a painter that clears and redraws these rectangles reproduces
	/// `self` exactly.
	#[must_use]
	pub fn damage(&self, previous: &Self) -> Vec<Rect> {
		let now = groups(self);
		let before = groups(previous);
		let mut rects = Vec::new();

		for (key, group) in &now {
			let old = before.get(key);

			if old.is_some_and(|old| same_items(previous, &old.items, self, &group.items)) {
				continue;
			}

			rects.extend(old.and_then(|old| old.bounds));
			rects.extend(group.bounds);
		}

		for (key, old) in &before {
			if !now.contains_key(key) {
				rects.extend(old.bounds);
			}
		}

		rects
	}
}

/// Whether two groups paint the same, comparing strokes by their points
/// rather than their ranges, which differ from list to list.
fn same_items(
	previous: &DisplayList,
	old: &[&DisplayItem],
	list: &DisplayList,
	new: &[&DisplayItem],
) -> bool {
	old.len() == new.len()
		&& old.iter().zip(new).all(|(old, new)| {
			match (old, new) {
				(
					DisplayItem::Stroke {
						points: old_points,
						width: old_width,
						color: old_color,
						bounds: old_bounds,
						block: old_block,
					},
					DisplayItem::Stroke {
						points,
						width,
						color,
						bounds,
						block,
					},
				) => {
					(old_width, old_color, old_bounds, old_block) == (width, color, bounds, block)
						&& previous.stroke_points(old_points) == list.stroke_points(points)
				}
				_ => old == new,
			}
		})
}

struct Group<'a> {
	items: Vec<&'a DisplayItem>,
	bounds: Option<Rect>,
}

fn groups(list: &DisplayList) -> FxHashMap<Option<BlockId>, Group<'_>> {
	let mut groups: FxHashMap<Option<BlockId>, Group<'_>> = FxHashMap::default();

	for item in &list.items {
		let group = groups.entry(item.block()).or_insert(Group {
			items: Vec::new(),
			bounds: None,
		});
		let bounds = item.bounds();
		group.items.push(item);
		group.bounds = Some(group.bounds.map_or(bounds, |b| b.union(&bounds)));
	}

	groups
}

#[cfg(test)]
mod tests {
	use super::*;

	fn list_with_stroke(points: Vec<f32>) -> DisplayList {
		let bounds = Rect::new(0.0, 0.0, 10.0, 10.0);
		DisplayList {
			fonts: Vec::new(),
			items: vec![DisplayItem::Stroke {
				points: 0..points.len() as u32,
				width: 2.0,
				color: Color::rgb(0, 0, 0),
				bounds,
				block: BlockId::new(1, 0),
			}],
			bounds,
			animated: false,
			points,
		}
	}

	#[test]
	fn strokes_compare_by_their_points_not_their_ranges() {
		let before = list_with_stroke(vec![1.0, 2.0, 3.0, 4.0]);
		let moved = list_with_stroke(vec![1.0, 2.0, 5.0, 6.0]);

		assert!(
			list_with_stroke(vec![1.0, 2.0, 3.0, 4.0])
				.damage(&before)
				.is_empty()
		);
		assert_eq!(
			moved.damage(&before),
			[
				Rect::new(0.0, 0.0, 10.0, 10.0),
				Rect::new(0.0, 0.0, 10.0, 10.0)
			]
		);
	}

	#[test]
	fn a_range_outside_the_list_reads_as_no_points() {
		assert_eq!(
			list_with_stroke(vec![1.0, 2.0]).stroke_points(&(2..6)),
			[0.0; 0]
		);
	}
}
