//! Laying out the inline content of one text block.
//!
//! Layout runs in two phases so a whole relayout pass shares one measurer
//! call: [`prepare`] itemizes the text and queues missing measurements, the
//! caller flushes the cache, then [`finish`] breaks lines, computes CSS line
//! boxes, and reorders each line for bidi.

use std::ops::Range;
use std::sync::Arc;

use canvist_model::InlineText;
use unicode_bidi::BidiInfo;
use unicode_bidi::Level;

use self::breaking::RawLine;
pub(crate) use self::items::Direction;
use self::items::Item;
pub(crate) use self::items::ItemContext;
pub(crate) use self::items::ItemKind;
use self::items::Itemized;
pub(crate) use self::items::is_newline;
pub(crate) use self::items::slice;
use crate::FontId;
use crate::FontMetrics;
use crate::LayoutError;
use crate::LineBox;
use crate::geometry::snap;
use crate::measure::MeasureCache;
use crate::theme::TextStyle;

mod breaking;
mod breaks;
mod incremental;
mod items;
mod kerning;
mod query;

pub(crate) use self::incremental::Previous;
pub(crate) use self::incremental::TextEdit;

/// A text style used in one block, with its interned font and metrics.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunStyle {
	pub(crate) style: TextStyle,
	pub(crate) font: FontId,
	pub(crate) metrics: FontMetrics,
	pub(crate) line_box: LineBox,
}

impl RunStyle {
	/// Height above and below the baseline of an inline box in this style.
	pub(crate) fn line_box(&self) -> (f32, f32) {
		(self.line_box.above, self.line_box.below)
	}
}

/// A run of one item on one line, positioned in visual order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Piece {
	/// Byte range in the block text.
	pub(crate) range: Range<u32>,
	/// Index of the item the piece belongs to.
	pub(crate) item: u32,
	/// Left edge relative to the block's content box.
	pub(crate) x: f32,
	/// Advance width.
	pub(crate) width: f32,
	/// Whether the piece runs right to left.
	pub(crate) rtl: bool,
}

/// One laid-out line.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Line {
	/// Byte range including trailing spaces and any forced break.
	pub(crate) range: Range<u32>,
	/// The last caret offset on the line.
	pub(crate) caret_end: u32,
	/// Whether the line ended at a soft wrap.
	pub(crate) soft: bool,
	/// Top edge relative to the block's content box.
	pub(crate) top: f32,
	/// Line box height.
	pub(crate) height: f32,
	/// Baseline position relative to `top`.
	pub(crate) baseline: f32,
	/// Width of the content, excluding hanging white space.
	pub(crate) width: f32,
	/// Pieces in visual order, left to right.
	pub(crate) pieces: Vec<Piece>,
}

/// The layout of one text block's inline content.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TextLayout {
	pub(crate) text: Arc<str>,
	pub(crate) styles: Vec<RunStyle>,
	pub(crate) items: Vec<Item>,
	pub(crate) lines: Vec<Line>,
	/// The width lines were broken to.
	pub(crate) width: f32,
	/// The sum of line heights.
	pub(crate) height: f32,
	/// Whether the paragraph direction is right to left.
	pub(crate) rtl: bool,
	/// Whether lines were reordered for bidi.
	pub(crate) bidi: bool,
}

/// A block whose measurements have been queued.
pub(crate) enum Draft {
	/// The whole block is laid out from scratch.
	Full { text: Arc<str>, itemized: Itemized },
	/// Only the part around an edit is laid out again.
	Partial(incremental::Partial),
}

/// What phase one needs to know about a block.
pub(crate) struct Block<'a> {
	pub(crate) base: &'a TextStyle,
	pub(crate) inline: &'a InlineText,
	pub(crate) direction: Direction,
	/// The content width lines will be broken to.
	pub(crate) width: f32,
	/// The block's previous layout and the edit since, when it has one.
	pub(crate) previous: Option<Previous>,
}

/// Phase one: itemize the block and queue its measurements. A block with a
/// previous layout re-itemizes only around its edit when it can.
pub(crate) fn prepare(context: &mut ItemContext<'_>, block: Block<'_>) -> Draft {
	let Block {
		base,
		inline,
		direction,
		width,
		previous,
	} = block;
	let partial =
		previous.and_then(|previous| incremental::prepare(context, base, inline, previous, width));

	if let Some(partial) = partial {
		return partial;
	}

	let itemized = items::itemize(context, base, inline, direction);

	if inline.as_str().contains('\t') {
		context.cache.request(itemized.styles[0].font, " ");
	}

	Draft::Full {
		text: Arc::from(inline.as_str()),
		itemized,
	}
}

/// Phase two: lay out a prepared block at `width` once the cache is flushed.
pub(crate) fn finish(
	draft: Draft,
	cache: &MeasureCache,
	width: f32,
	tab_size: u32,
) -> Result<TextLayout, LayoutError> {
	let (text, itemized) = match draft {
		Draft::Full { text, itemized } => (text, itemized),
		Draft::Partial(partial) => return incremental::finish(partial, cache, tab_size),
	};

	let Itemized {
		mut styles,
		items: drafts,
		rtl,
		bidi,
	} = itemized;
	let mut items = items::measure_items(&text, drafts, &mut styles, cache)?;
	let count = items.len();
	kerning::apply(&text, &mut items, 0..count, cache)?;
	let space = tab_space(&text, &styles[0], cache)?;
	let tabs = breaking::TabStops {
		interval: space * tab_size as f32,
		space,
	};
	let raw = breaking::break_lines(&text, &mut items, width, tabs);
	let mut layout = TextLayout {
		text,
		styles,
		items,
		lines: Vec::new(),
		width,
		height: 0.0,
		rtl,
		bidi,
	};

	layout.stack_lines(raw, bidi);

	Ok(layout)
}

/// The base embedding level of a paragraph.
fn paragraph_level(rtl: bool) -> Level {
	if rtl { Level::rtl() } else { Level::ltr() }
}

/// How the lines of `layout` are filled.
fn line_settings(
	layout: &TextLayout,
	cache: &MeasureCache,
	tab_size: u32,
) -> Result<breaking::Lines, LayoutError> {
	let space = tab_space(&layout.text, &layout.styles[0], cache)?;

	Ok(breaking::Lines {
		available: layout.width,
		tabs: breaking::TabStops {
			interval: space * tab_size as f32,
			space,
		},
	})
}

/// The advance of a space in the base style, which spaces tab stops. Text
/// without tabs never queued that measurement, so it gets zero.
fn tab_space(text: &str, base: &RunStyle, cache: &MeasureCache) -> Result<f32, LayoutError> {
	if !text.contains('\t') {
		return Ok(0.0);
	}

	let space = cache
		.get(base.font, " ")
		.ok_or(LayoutError::MissingMeasurement)?;

	Ok(space.width + base.style.letter_spacing)
}

impl TextLayout {
	/// Turn `raw` lines into positioned lines stacked from the top, reordering
	/// them for bidi when the block needs it, and set the block height.
	fn stack_lines(&mut self, raw: Vec<RawLine>, bidi: bool) {
		let info = bidi.then(|| BidiInfo::new(&self.text, Some(paragraph_level(self.rtl))));

		self.lines = self.position_lines(raw, 0.0, info.as_ref());
		self.height = self.lines.last().map_or(0.0, |line| line.top + line.height);
	}

	/// Position left-to-right `raw` lines, stacked from `top`.
	fn build_lines(&self, raw: Vec<RawLine>, top: f32) -> Vec<Line> {
		self.position_lines(raw, top, None)
	}

	/// Position `raw` lines stacked from `top`, reordering them with `info`
	/// when it is given.
	fn position_lines(
		&self,
		raw: Vec<RawLine>,
		top: f32,
		info: Option<&BidiInfo<'_>>,
	) -> Vec<Line> {
		let strut = self.styles[0].line_box();
		let mut top = top;

		raw.into_iter()
			.map(|raw| {
				let mut line = Line {
					pieces: self.pieces(&raw.range, info),
					range: raw.range,
					caret_end: raw.caret_end,
					soft: raw.soft,
					top,
					height: 0.0,
					baseline: 0.0,
					width: 0.0,
				};

				self.place(strut, &mut line);
				top += line.height;
				line
			})
			.collect()
	}

	/// The pieces of the line covering `range`, in visual order when the
	/// block needs bidi reordering.
	fn pieces(&self, range: &Range<u32>, info: Option<&BidiInfo<'_>>) -> Vec<Piece> {
		let logical = logical_pieces(&self.text, &self.items, range);

		match info {
			Some(info) => visual_pieces(&self.text, &self.items, info, range, &logical),
			None => logical,
		}
	}

	/// Position `line`'s pieces and compute its line box, which is at least
	/// as tall as `strut`.
	fn place(&self, strut: (f32, f32), line: &mut Line) {
		let total: f32 = line.pieces.iter().map(|piece| piece.width).sum();
		let hang = hanging_width(&self.text, &self.items, line);
		let mut x = if self.rtl { self.width - total } else { 0.0 };
		let (mut above, mut below) = strut;

		for piece in &mut line.pieces {
			piece.x = x;
			x += piece.width;

			let item = &self.items[piece.item as usize];

			let (item_above, item_below) = match &item.kind {
				ItemKind::Text { .. } => self.styles[item.style as usize].line_box(),
				ItemKind::Embed(embed) => (embed.ascent, embed.descent),
				ItemKind::Tab | ItemKind::Break => continue,
			};

			above = above.max(item_above);
			below = below.max(item_below);
		}

		line.width = (total - hang).max(0.0) + 0.0;
		line.baseline = snap(above);
		line.height = snap(above + below);
	}
}

/// Pieces of every item intersecting `range`, in logical order.
fn logical_pieces(text: &str, items: &[Item], range: &Range<u32>) -> Vec<Piece> {
	let first = items.partition_point(|item| item.range.end <= range.start);

	items[first..]
		.iter()
		.enumerate()
		.take_while(|(_, item)| item.range.start < range.end)
		.map(|(index, item)| {
			let start = item.range.start.max(range.start);
			let end = item.range.end.min(range.end);
			Piece {
				range: start..end,
				item: (first + index) as u32,
				x: 0.0,
				width: item.span(text, start, end),
				rtl: false,
			}
		})
		.collect()
}

/// Reorder `logical` pieces of one line into visual order with UAX #9,
/// splitting pieces whose characters resolve to different levels.
fn visual_pieces(
	text: &str,
	items: &[Item],
	info: &BidiInfo<'_>,
	line: &Range<u32>,
	logical: &[Piece],
) -> Vec<Piece> {
	let mut visual = Vec::with_capacity(logical.len());
	let line_range = line.start as usize..line.end as usize;

	for paragraph in &info.paragraphs {
		let start = paragraph.range.start.max(line_range.start);
		let end = paragraph.range.end.min(line_range.end);

		if start >= end {
			continue;
		}

		let (levels, runs) = info.visual_runs(paragraph, start..end);

		for run in runs {
			let rtl = levels.get(run.start).is_some_and(Level::is_rtl);
			let run = run.start as u32..run.end as u32;
			let mut parts: Vec<Piece> = logical
				.iter()
				.filter_map(|piece| {
					let start = piece.range.start.max(run.start);
					let end = piece.range.end.min(run.end);
					(start < end).then(|| {
						Piece {
							range: start..end,
							item: piece.item,
							x: 0.0,
							width: items[piece.item as usize].span(text, start, end),
							rtl,
						}
					})
				})
				.collect();

			if rtl {
				parts.reverse();
			}

			visual.extend(parts);
		}
	}

	visual
}

/// The width of white space hanging at the logical end of `line`.
fn hanging_width(text: &str, items: &[Item], line: &Line) -> f32 {
	let mut ordered: Vec<&Piece> = line.pieces.iter().collect();
	ordered.sort_by_key(|piece| piece.range.start);
	let mut hang = 0.0;

	for piece in ordered.into_iter().rev() {
		let item = &items[piece.item as usize];

		if item.kind == ItemKind::Break {
			continue;
		}

		if item.is_space(text) {
			hang += piece.width;
			continue;
		}

		let trailing = item.trailing_space(text, piece.range.start);
		let beyond = item.span(text, piece.range.end, item.range.end);
		hang += (trailing - beyond).max(0.0);
		break;
	}

	hang
}
