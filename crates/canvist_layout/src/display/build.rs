//! Building a display list from a laid-out document, in paint order.

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use canvist_model::BlockId;
use canvist_model::Selection;

use super::DecorationKind;
use super::DisplayItem;
use super::DisplayList;
use super::DisplayOptions;
use super::RectKind;
use super::SlotKind;
use super::TextRun;
use crate::BlockStyle;
use crate::Caret;
use crate::Color;
use crate::DocumentLayout;
use crate::Rect;
use crate::TableRole;
use crate::document::Marker;
use crate::document::Origin;
use crate::document::Row;
use crate::document::RowKind;
use crate::document::Span;
use crate::style::StyleId;
use crate::text::ItemKind;
use crate::text::Line;
use crate::text::Piece;
use crate::text::RunStyle;
use crate::text::TextLayout;
use crate::text::slice;
use crate::theme::LeafStyle;
use crate::theme::MarkerKind;

impl DocumentLayout {
	/// Build the display list for `options.top..options.bottom`.
	///
	/// Only measured blocks are painted: call [`DocumentLayout::ensure`] for
	/// the same range first.
	#[must_use]
	pub fn display_list(&self, options: &DisplayOptions) -> DisplayList {
		let mut builder = Builder::new(self, options);
		let rows: Vec<usize> = self.rows_between(options.top, options.bottom).collect();

		builder.containers(&rows);

		for &index in &rows {
			builder.row_background(index);
		}

		for &index in &rows {
			builder.inline_backgrounds(index);
		}

		for (range, color) in &options.highlights {
			builder.selection(range, *color, RectKind::Range);
		}

		if let Some(selection) = &options.selection {
			builder.selection(
				selection,
				self.theme().chrome().selection,
				RectKind::Selection,
			);
		}

		for &index in &rows {
			builder.row_content(index);
		}

		builder.placeholders(&rows, &options.placeholders);
		builder.finish(options.caret)
	}
}

/// How opaque placeholder text is, relative to the block's text colour.
const PLACEHOLDER_ALPHA: f32 = 0.4;

/// A decoration line's thickness as a fraction of the font size, before
/// rounding up to at least one pixel.
const DECORATION_THICKNESS: f32 = 1.0 / 16.0;

/// How far below the baseline an underline sits, as a fraction of the font
/// size.
const UNDERLINE_OFFSET: f32 = 0.1;

/// How far above the baseline a strikethrough sits, as a fraction of the
/// font size: roughly the middle of lowercase letters.
const STRIKE_OFFSET: f32 = 0.3;

/// How far below the baseline a squiggle sits, as a fraction of the font
/// size. It sits lower than an underline to leave room for its waves.
const SQUIGGLE_OFFSET: f32 = 0.15;

/// The vertical extent of one line of text.
#[derive(Clone, Copy)]
struct LineBox {
	top: f32,
	height: f32,
	baseline: f32,
}

struct Builder<'a> {
	layout: &'a DocumentLayout,
	list: DisplayList,
	/// Decorations and slots, painted above all text.
	foreground: Vec<DisplayItem>,
}

impl<'a> Builder<'a> {
	/// An empty list carrying `layout`'s fonts, bounded by the full document
	/// width between `options.top` and `options.bottom`.
	fn new(layout: &'a DocumentLayout, options: &DisplayOptions) -> Self {
		let height = options.bottom - options.top;
		let bounds = Rect::new(0.0, options.top, layout.options().width, height);

		Self {
			layout,
			list: DisplayList {
				fonts: layout.cache().fonts().fonts().to_vec(),
				items: Vec::new(),
				bounds,
			},
			foreground: Vec::new(),
		}
	}
}

impl Builder<'_> {
	/// Paint decorations and slots above all text, then the caret on top.
	fn finish(mut self, caret: Option<Caret>) -> DisplayList {
		self.list.items.append(&mut self.foreground);

		if let Some(rect) = caret.and_then(|caret| self.layout.caret_rect(caret)) {
			self.list.items.push(DisplayItem::Rect {
				rect,
				color: self.layout.theme().chrome().caret,
				radius: 0.0,
				kind: RectKind::Caret,
				block: None,
			});
		}

		self.list
	}

	fn push_rect(&mut self, rect: Rect, color: Color, radius: f32, kind: RectKind, block: BlockId) {
		self.list.items.push(DisplayItem::Rect {
			rect,
			color,
			radius,
			kind,
			block: Some(block),
		});
	}

	/// The background and left border `style` gives the box `rect`.
	fn push_box(&mut self, rect: Rect, style: &BlockStyle, block: BlockId) {
		if let Some(color) = style.background {
			self.push_rect(rect, color, style.radius, RectKind::Background, block);
		}

		if let Some(border) = style.border_left {
			let bar = Rect::new(rect.x, rect.y, border.width, rect.height);
			self.push_rect(bar, border.color, 0.0, RectKind::Border, block);
		}
	}

	/// Backgrounds and borders of every container enclosing a visible row,
	/// outermost first.
	fn containers(&mut self, rows: &[usize]) {
		let layout = self.layout;
		let mut tables = Vec::new();

		for index in visible_spans(layout, rows) {
			let span = &layout.spans()[index as usize];
			let style = layout.styles().get(span.style);

			self.push_box(layout.span_rect(span), style, span.block);

			if let Some(TableRole::Table { rule }) = style.table {
				tables.push((index, rule));
			}
		}

		for (index, rule) in tables {
			let block = layout.spans()[index as usize].block;

			for line in layout.grid_lines(index, self.list.bounds) {
				self.push_rect(line, rule.color, 0.0, RectKind::Border, block);
			}
		}
	}

	fn row_background(&mut self, index: usize) {
		let layout = self.layout;
		let row = &layout.rows()[index];
		let style = layout.style_of(row);
		let rect = layout.row_rect(index);

		self.push_box(rect, style, row.block);

		if row.kind != RowKind::Leaf {
			return;
		}

		let origin = layout.origin(index);
		let content = Rect::new(
			origin.x,
			origin.y,
			crate::document::content_width(style, row.width),
			rect.height - style.padding.vertical(),
		);

		match style.leaf {
			LeafStyle::Rule { color, .. } => {
				self.push_rect(content, color, 0.0, RectKind::Rule, row.block);
			}
			LeafStyle::Slot { .. } => {
				self.foreground.push(DisplayItem::Slot {
					kind: SlotKind::Block,
					block: row.block,
					offset: None,
					rect: content,
				});
			}
		}
	}

	/// Inline backgrounds of row `index`, such as inline code, painted below
	/// all text so neighbouring pieces never cover each other's glyphs.
	fn inline_backgrounds(&mut self, index: usize) {
		let layout = self.layout;
		let row = &layout.rows()[index];
		let Some(text) = &row.layout else {
			return;
		};
		let origin = layout.origin(index);

		for line in &text.lines {
			let backgrounds = line
				.pieces
				.iter()
				.filter_map(|piece| piece_background(text, line, piece, origin));

			for (rect, color) in backgrounds {
				self.push_rect(rect, color, 0.0, RectKind::Highlight, row.block);
			}
		}
	}

	fn selection(&mut self, selection: &Selection, color: Color, kind: RectKind) {
		for rect in self.layout.selection_rects(selection) {
			self.list.items.push(DisplayItem::Rect {
				rect,
				color,
				radius: 0.0,
				kind,
				block: None,
			});
		}
	}

	fn row_content(&mut self, index: usize) {
		let layout = self.layout;
		let row = &layout.rows()[index];

		self.markers(index, row);

		let Some(text) = &row.layout else {
			return;
		};
		let origin = layout.origin(index);

		for line in &text.lines {
			for piece in &line.pieces {
				self.piece(row.block, text, line, piece, origin);
			}
		}
	}

	/// Draw one piece: its text run and decorations, or a slot for an
	/// unlabelled embed.
	fn piece(
		&mut self,
		block: BlockId,
		text: &TextLayout,
		line: &Line,
		piece: &Piece,
		origin: Origin,
	) {
		let item = &text.items[piece.item as usize];
		let baseline = origin.y + line.top + line.baseline;
		let x = origin.x + piece.x;

		if let ItemKind::Embed(embed) = &item.kind
			&& embed.label.is_none()
		{
			self.foreground.push(DisplayItem::Slot {
				kind: SlotKind::Embed(embed.kind),
				block,
				offset: Some(item.range.start),
				rect: Rect::new(
					x,
					baseline - embed.ascent,
					embed.width,
					embed.ascent + embed.descent,
				),
			});

			return;
		}

		let Some((source, range, style)) = run_source(text, piece) else {
			return;
		};
		let baseline = baseline - style.style.baseline_shift;

		self.list.items.push(DisplayItem::Text(TextRun {
			block,
			source,
			range,
			font: style.font,
			x,
			baseline,
			width: piece.width,
			color: style.style.color,
			letter_spacing: style.style.letter_spacing,
			rtl: piece.rtl,
			bounds: Rect::new(x, origin.y + line.top, piece.width, line.height),
		}));

		self.decorations(block, style, x, baseline, piece.width);
	}

	/// Draw each placeholder in its block, if the block is visible, empty,
	/// and laid out. The text starts at the block's left edge on its first
	/// line, in the block's text colour at [`PLACEHOLDER_ALPHA`]. It is not
	/// measured, so its width is the block's.
	fn placeholders(&mut self, rows: &[usize], placeholders: &[(BlockId, Arc<str>)]) {
		if placeholders.is_empty() {
			return;
		}

		let layout = self.layout;

		for &index in rows {
			let row = &layout.rows()[index];
			let Some((_, text)) = placeholders.iter().find(|(block, _)| *block == row.block) else {
				continue;
			};
			let Some(empty) = row.layout.as_ref().filter(|layout| layout.text.is_empty()) else {
				continue;
			};
			let style = &empty.styles[0];
			let line = self.first_line_box(index, layout.style_of(row));
			let origin = layout.origin(index);
			let width = crate::document::content_width(layout.style_of(row), row.width);
			let color = style.style.color;
			let alpha = (f32::from(color.a) * PLACEHOLDER_ALPHA).round() as u8;

			self.list.items.push(DisplayItem::Text(TextRun {
				block: row.block,
				source: text.clone(),
				range: 0..text.len() as u32,
				font: style.font,
				x: origin.x,
				baseline: line.baseline,
				width,
				color: Color::rgba(color.r, color.g, color.b, alpha),
				letter_spacing: style.style.letter_spacing,
				rtl: false,
				bounds: Rect::new(origin.x, line.top, width, line.height),
			}));
		}
	}

	/// Push the decoration lines `style` asks for under a run of `width` at
	/// `x`.
	fn decorations(&mut self, block: BlockId, style: &RunStyle, x: f32, baseline: f32, width: f32) {
		let decorations = style.style.decorations;
		let size = style.style.font.size;
		let thickness = (size * DECORATION_THICKNESS).max(1.0);
		let color = decorations.color.unwrap_or(style.style.color);
		let lines = [
			(
				decorations.underline,
				DecorationKind::Underline,
				baseline + size * UNDERLINE_OFFSET,
			),
			(
				decorations.strike,
				DecorationKind::Strike,
				baseline - size * STRIKE_OFFSET,
			),
			(
				decorations.squiggle,
				DecorationKind::Squiggle,
				baseline + size * SQUIGGLE_OFFSET,
			),
		];

		for (_, kind, y) in lines.into_iter().filter(|line| line.0) {
			self.foreground.push(DisplayItem::Decoration {
				kind,
				x,
				y,
				width,
				thickness,
				color,
				block,
			});
		}
	}

	/// Markers of list items whose first row is `index`.
	fn markers(&mut self, index: usize, row: &Row) {
		let layout = self.layout;

		if let Some(marker) = &row.marker {
			let edge = row.x + border_width(layout.style_of(row));
			self.marker(index, row.block, row.style, edge, marker);
		}

		let mut span = row.span;

		while let Some(current) = span {
			let candidate: &Span = &layout.spans()[current as usize];

			if candidate.first_row as usize != index {
				break;
			}

			if let Some(marker) = &candidate.marker {
				let edge = candidate.x + border_width(layout.styles().get(candidate.style));
				self.marker(index, candidate.block, candidate.style, edge, marker);
			}

			span = candidate.parent;
		}
	}

	/// Draw `marker` to the left of `edge`, the list item's padding-box left
	/// edge (where CSS places `right: 100%`), on the first line of row
	/// `index`.
	fn marker(&mut self, index: usize, block: BlockId, style: StyleId, edge: f32, marker: &Marker) {
		let layout = self.layout;
		let style = layout.styles().get(style);
		let line = self.first_line_box(index, style);
		let right = edge - layout.theme().markers().gap;

		if marker.kind == MarkerKind::Task {
			self.checkbox(block, marker.checked, right, line);
		} else {
			self.marker_label(block, marker, style, right, line);
		}
	}

	/// The first line of row `index`, or one line of `style`'s text at the
	/// top of its content box while the row is unmeasured.
	fn first_line_box(&self, index: usize, style: &BlockStyle) -> LineBox {
		let layout = self.layout;
		let origin = layout.origin(index);
		let first = layout.rows()[index]
			.layout
			.as_ref()
			.and_then(|text| text.lines.first());

		match first {
			Some(line) => {
				LineBox {
					top: origin.y + line.top,
					height: line.height,
					baseline: origin.y + line.top + line.baseline,
				}
			}
			None => {
				LineBox {
					top: origin.y,
					height: style.text.line_height_px(),
					baseline: origin.y + style.text.font.size,
				}
			}
		}
	}

	/// Reserve a task item's checkbox slot, ending at `right` and centred on
	/// `line`.
	fn checkbox(&mut self, block: BlockId, checked: bool, right: f32, line: LineBox) {
		let size = self.layout.theme().markers().checkbox;

		self.foreground.push(DisplayItem::Slot {
			kind: SlotKind::Checkbox { checked },
			block,
			offset: None,
			rect: Rect::new(
				right - size,
				line.top + (line.height - size) / 2.0,
				size,
				size,
			),
		});
	}

	/// Draw a marker's text ending at `right` on `line`'s baseline. Markers
	/// with no text, or whose text has not been measured, are skipped.
	fn marker_label(
		&mut self,
		block: BlockId,
		marker: &Marker,
		style: &BlockStyle,
		right: f32,
		line: LineBox,
	) {
		let layout = self.layout;
		let Some(text) = layout.marker_text(marker) else {
			return;
		};

		let cache = layout.cache();
		let Some(font) = cache.fonts().find(&style.text.font) else {
			return;
		};
		let Some(width) = cache.get(font, &text).map(|measured| measured.width) else {
			return;
		};
		let color = layout.theme().markers().color.unwrap_or(style.text.color);

		self.list.items.push(DisplayItem::Text(TextRun {
			block,
			range: 0..text.len() as u32,
			source: text,
			font,
			x: right - width,
			baseline: line.baseline,
			width,
			color,
			letter_spacing: 0.0,
			rtl: false,
			bounds: Rect::new(right - width, line.top, width, line.height),
		}));
	}
}

/// Every container enclosing one of `rows`, by span index. Spans are
/// numbered in document order, so ascending order puts outer containers
/// first.
fn visible_spans(layout: &DocumentLayout, rows: &[usize]) -> BTreeSet<u32> {
	let mut visible = BTreeSet::new();

	for &index in rows {
		let mut span = layout.rows()[index].span;

		// Stop at the first container already seen: its ancestors are too.
		while let Some(current) = span
			&& visible.insert(current)
		{
			span = layout.spans()[current as usize].parent;
		}
	}

	visible
}

/// The inline background behind `piece`, such as inline code or a
/// highlight, with its colour.
fn piece_background(
	text: &TextLayout,
	line: &Line,
	piece: &Piece,
	origin: Origin,
) -> Option<(Rect, Color)> {
	let item = &text.items[piece.item as usize];

	// A labelled embed draws its label, so its background follows the
	// label's style.
	let style = match &item.kind {
		ItemKind::Embed(embed) => embed.label.as_ref().map_or(item.style, |label| label.style),
		_ => item.style,
	};

	let run = &text.styles[style as usize];
	let color = run.style.background?;

	if piece.width <= 0.0 || item.kind == ItemKind::Break {
		return None;
	}

	let baseline = origin.y + line.top + line.baseline - run.style.baseline_shift;
	let rect = Rect::new(
		origin.x + piece.x,
		baseline - run.metrics.ascent,
		piece.width,
		run.metrics.ascent + run.metrics.descent,
	);

	Some((rect, color))
}

/// The text `piece` draws, as a `range` of a shared `source`, and its style.
/// Embeds draw their label. Whitespace-only text, tabs, breaks, and
/// unlabelled embeds draw no text.
fn run_source<'a>(
	text: &'a TextLayout,
	piece: &Piece,
) -> Option<(Arc<str>, Range<u32>, &'a RunStyle)> {
	let item = &text.items[piece.item as usize];

	match &item.kind {
		ItemKind::Text { .. } => {
			let blank = slice(&text.text, &piece.range)
				.bytes()
				.all(|byte| byte == b' ');
			let style = &text.styles[item.style as usize];

			(!blank).then(|| (text.text.clone(), piece.range.clone(), style))
		}
		ItemKind::Embed(embed) => {
			embed.label.as_ref().map(|label| {
				let range = 0..label.text.len() as u32;

				(
					label.text.clone(),
					range,
					&text.styles[label.style as usize],
				)
			})
		}
		ItemKind::Tab | ItemKind::Break => None,
	}
}

/// The width of a style's left border.
fn border_width(style: &BlockStyle) -> f32 {
	style.border_left.map_or(0.0, |border| border.width)
}
