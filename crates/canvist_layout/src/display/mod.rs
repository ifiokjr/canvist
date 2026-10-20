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
	/// A freehand stroke through `points`, drawn as straight segments with
	/// round caps and joins.
	Stroke {
		/// The points, as `x, y` pairs in document pixels.
		points: Arc<[f32]>,
		/// The line width in pixels.
		width: f32,
		/// The colour.
		color: Color,
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
			Self::Stroke { points, width, .. } => stroke_bounds(points, *width),
		}
	}
}

/// The box around a stroke's points, widened by half its width.
fn stroke_bounds(points: &[f32], width: f32) -> Rect {
	let (mut left, mut top, mut right, mut bottom) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);

	for point in points.chunks_exact(2) {
		left = left.min(point[0]);
		right = right.max(point[0]);
		top = top.min(point[1]);
		bottom = bottom.max(point[1]);
	}

	if left > right {
		return Rect::default();
	}

	let half = width / 2.0;
	Rect::new(
		left - half,
		top - half,
		right - left + width,
		bottom - top + width,
	)
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
}

impl DisplayList {
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

			if old.is_some_and(|old| old.items == group.items) {
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
