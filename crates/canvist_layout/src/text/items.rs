//! Splitting a block's inline text into measurable items.
//!
//! An item is the unit of measurement and line breaking: a run of text with
//! one font and one bidi level that ends at the latest at a line break
//! opportunity, or a single atomic tab, line break, or embed.

use std::ops::Range;
use std::sync::Arc;

use canvist_model::EmbedTypeId;
use canvist_model::InlineText;
use unicode_bidi::BidiInfo;
use unicode_bidi::Level;
use unicode_segmentation::UnicodeSegmentation;

use super::RunStyle;
use super::paragraph_level;
use crate::BreakOverride;
use crate::FontId;
use crate::FontMetrics;
use crate::LayoutError;
use crate::LineBox;
use crate::measure::MeasureCache;
use crate::measure::Measured;
use crate::style::Styles;
use crate::style::attr_px;
use crate::theme::EmbedStyle;
use crate::theme::TextStyle;

/// Whether a line may or must end after an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BreakAfter {
	/// No break opportunity.
	None,
	/// A soft wrap opportunity.
	Soft,
	/// A forced break.
	Hard,
}

/// What an item holds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ItemKind {
	/// Text in one font.
	Text {
		/// The font.
		font: FontId,
		/// Per-character advances including letter spacing.
		measured: Arc<Measured>,
	},
	/// A tab, whose width depends on where it lands.
	Tab,
	/// A forced line break: `\n` or a line-break embed.
	Break,
	/// An atomic inline embed.
	Embed(Box<EmbedBox>),
}

/// A laid-out inline embed.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EmbedBox {
	/// The embed's type.
	pub(crate) kind: EmbedTypeId,
	/// The width in pixels.
	pub(crate) width: f32,
	/// Height above the baseline.
	pub(crate) ascent: f32,
	/// Depth below the baseline.
	pub(crate) descent: f32,
	/// The label, for text-rendered embeds such as mentions.
	pub(crate) label: Option<Label>,
}

/// The text drawn for a label embed.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Label {
	/// The text.
	pub(crate) text: Arc<str>,
	/// Index into the block's run styles.
	pub(crate) style: u16,
}

/// One item of a block's text.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Item {
	/// Byte range in the block text.
	pub(crate) range: Range<u32>,
	/// Index into the block's run styles.
	pub(crate) style: u16,
	/// The resolved bidi embedding level.
	pub(crate) level: u8,
	/// What the item holds.
	pub(crate) kind: ItemKind,
	/// The advance in pixels. Tabs get theirs during line breaking.
	pub(crate) width: f32,
	/// The kerning adjustment with the next item, already included in
	/// `width` and in the last grapheme's advance.
	pub(crate) kern: f32,
	/// Whether a line may end after the item.
	pub(crate) break_after: BreakAfter,
}

impl Item {
	/// Whether the item is entirely white space that hangs at a line end.
	pub(crate) fn is_space(&self, text: &str) -> bool {
		match self.kind {
			ItemKind::Tab => true,
			ItemKind::Text { .. } => slice(text, &self.range).bytes().all(|b| b == b' '),
			ItemKind::Break | ItemKind::Embed(_) => false,
		}
	}

	/// The width of the part of the item from byte `from` to byte `to`.
	pub(crate) fn span(&self, text: &str, from: u32, to: u32) -> f32 {
		let from = from.max(self.range.start);
		let to = to.min(self.range.end);

		if from >= to {
			return 0.0;
		}

		if from == self.range.start && to == self.range.end {
			return self.width;
		}

		// Atomic items cannot be split, so a partial span covers nothing.
		let ItemKind::Text { measured, .. } = &self.kind else {
			return 0.0;
		};

		let local = from - self.range.start..to - self.range.start;

		advances_within(slice(text, &self.range), &measured.advances, local)
	}

	/// The width of trailing spaces from byte `from` to the item end.
	pub(crate) fn trailing_space(&self, text: &str, from: u32) -> f32 {
		match &self.kind {
			ItemKind::Tab => self.width,
			ItemKind::Text { measured, .. } => {
				let item = slice(text, &self.range);
				let trimmed = item.trim_end_matches(' ');
				let spaces = item.len() - trimmed.len();
				let start = (self.range.start + trimmed.len() as u32).max(from);
				let count = (self.range.end - start).min(spaces as u32) as usize;
				measured.advances.iter().rev().take(count).sum()
			}
			ItemKind::Break | ItemKind::Embed(_) => 0.0,
		}
	}
}

/// The summed advances of the chars of `text` that start inside the byte
/// range `range`.
fn advances_within(text: &str, advances: &[f32], range: Range<u32>) -> f32 {
	let mut offset = 0;
	let mut width = 0.0;

	for (character, advance) in text.chars().zip(advances) {
		if offset >= range.end {
			break;
		}

		if offset >= range.start {
			width += advance;
		}

		offset += character.len_utf8() as u32;
	}

	width
}

/// The byte range `range` of `text`, or `""` if it is not on char
/// boundaries (which the model's invariants rule out).
pub(crate) fn slice<'a>(text: &'a str, range: &Range<u32>) -> &'a str {
	text.get(range.start as usize..range.end as usize)
		.unwrap_or_default()
}

/// An item whose text has not been measured yet.
#[derive(Clone, Debug)]
pub(crate) struct DraftItem {
	pub(crate) range: Range<u32>,
	pub(crate) style: u16,
	pub(crate) level: u8,
	pub(crate) kind: DraftKind,
	pub(crate) break_after: BreakAfter,
}

/// What a draft item holds.
#[derive(Clone, Debug)]
pub(crate) enum DraftKind {
	Text(FontId),
	Tab,
	Break,
	Embed(DraftEmbed),
}

/// An embed whose label has not been measured yet.
#[derive(Clone, Debug)]
pub(crate) struct DraftEmbed {
	pub(crate) kind: EmbedTypeId,
	pub(crate) size: Option<(f32, f32)>,
	pub(crate) label: Option<Label>,
}

/// Inputs shared by every block in one layout pass.
pub(crate) struct ItemContext<'a> {
	pub(crate) styles: &'a Styles,
	pub(crate) cache: &'a mut MeasureCache,
	pub(crate) break_override: Option<BreakOverride>,
}

/// Run styles and draft items for one block, plus its paragraph direction.
pub(crate) struct Itemized {
	pub(crate) styles: Vec<RunStyle>,
	pub(crate) items: Vec<DraftItem>,
	pub(crate) rtl: bool,
	pub(crate) bidi: bool,
}

/// Itemize `inline`, queueing every segment the cache lacks.
pub(crate) fn itemize(
	context: &mut ItemContext<'_>,
	base: &TextStyle,
	inline: &InlineText,
	direction: Direction,
) -> Itemized {
	let text = inline.as_str();
	let rtl = direction.is_rtl(text);
	let bidi = rtl || text.chars().any(is_bidi_sensitive);
	let levels = bidi_levels(text, rtl, bidi);
	let breaks = super::breaks::soft_breaks(text, context.break_override);
	let styles = vec![RunStyle::new(base.clone(), context.cache)];
	let window = Window {
		range: 0..text.len() as u32,
		levels: &levels,
		breaks: &breaks,
	};
	let (styles, items) = itemize_window(context, base, inline, styles, &window);

	Itemized {
		styles,
		items,
		rtl,
		bidi,
	}
}

/// The part of a block's text to itemize, with what itemizing it reads.
pub(crate) struct Window<'a> {
	/// The byte range to itemize. Items are cut at both ends.
	pub(crate) range: Range<u32>,
	/// Bidi levels per byte of the whole text, or empty when all are zero.
	pub(crate) levels: &'a [u8],
	/// Sorted soft wrap opportunities covering `range`.
	pub(crate) breaks: &'a [u32],
}

/// Itemize the runs of `inline` inside `window`, adding to `styles` and
/// queueing every segment the cache lacks.
pub(crate) fn itemize_window(
	context: &mut ItemContext<'_>,
	base: &TextStyle,
	inline: &InlineText,
	styles: Vec<RunStyle>,
	window: &Window<'_>,
) -> (Vec<RunStyle>, Vec<DraftItem>) {
	let mut builder = Builder {
		text: inline.as_str(),
		levels: window.levels,
		breaks: window.breaks,
		next_break: 0,
		styles,
		items: Vec::new(),
	};
	let runs = inline
		.runs()
		.skip_while(|run| run.range.end <= window.range.start)
		.take_while(|run| run.range.start < window.range.end);

	for run in runs {
		let style = context.styles.run_style(base, run.marks);
		let range = run.range.start.max(window.range.start)..run.range.end.min(window.range.end);

		if let Some(embed) = run.embed {
			builder.embed(context, range, &style, embed);
		} else {
			let index = builder.style(style, context.cache);
			builder.text_run(range, index);
		}
	}

	builder.request_measurements(context.cache);

	(builder.styles, builder.items)
}

/// The resolved bidi embedding level of every byte of `text`, or none when
/// the text is not `bidi`, since every level is then zero.
fn bidi_levels(text: &str, rtl: bool, bidi: bool) -> Vec<u8> {
	if !bidi {
		return Vec::new();
	}

	let info = BidiInfo::new(text, Some(paragraph_level(rtl)));

	info.levels.iter().map(Level::number).collect()
}

/// The paragraph direction requested by a block's `dir` attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
	Ltr,
	Rtl,
	Auto,
}

impl Direction {
	/// Whether the paragraph runs right to left. `Auto` follows the first
	/// strong character, per UAX #9 rules P2 and P3.
	fn is_rtl(self, text: &str) -> bool {
		match self {
			Self::Ltr => false,
			Self::Rtl => true,
			Self::Auto => unicode_bidi::get_base_direction(text) == unicode_bidi::Direction::Rtl,
		}
	}
}

struct Builder<'a> {
	text: &'a str,
	levels: &'a [u8],
	breaks: &'a [u32],
	next_break: usize,
	styles: Vec<RunStyle>,
	items: Vec<DraftItem>,
}

impl Builder<'_> {
	fn style(&mut self, style: TextStyle, cache: &mut MeasureCache) -> u16 {
		if let Some(index) = self.styles.iter().position(|run| run.style == style) {
			return index as u16;
		}

		self.styles.push(RunStyle::new(style, cache));
		(self.styles.len() - 1) as u16
	}

	fn level(&self, offset: u32) -> u8 {
		self.levels.get(offset as usize).copied().unwrap_or(0)
	}

	/// Whether `offset` is a soft break opportunity. Offsets must be queried
	/// in increasing order.
	fn is_break(&mut self, offset: u32) -> bool {
		while self
			.breaks
			.get(self.next_break)
			.is_some_and(|b| *b < offset)
		{
			self.next_break += 1;
		}

		self.breaks.get(self.next_break) == Some(&offset)
	}

	/// Push a non-empty item. Forced breaks always end the line; anything
	/// else may end it where a soft wrap opportunity follows.
	fn push(&mut self, range: Range<u32>, style: u16, kind: DraftKind) {
		if range.is_empty() {
			return;
		}

		let break_after = if matches!(kind, DraftKind::Break) {
			BreakAfter::Hard
		} else if self.is_break(range.end) {
			BreakAfter::Soft
		} else {
			BreakAfter::None
		};

		self.items.push(DraftItem {
			level: self.level(range.start),
			range,
			style,
			kind,
			break_after,
		});
	}

	/// Split a run of one style into items. Tabs and forced breaks become
	/// atomic items, and text splits wherever the bidi level changes or a
	/// soft wrap opportunity falls, so every item measures in one direction
	/// and lines only end between items.
	fn text_run(&mut self, range: Range<u32>, style: u16) {
		let font = self.styles[style as usize].font;
		let mut start = range.start;

		for (index, character) in slice(self.text, &range).char_indices() {
			let offset = range.start + index as u32;

			if let Some(kind) = atomic_kind(character) {
				let next = offset + character.len_utf8() as u32;
				self.push(start..offset, style, DraftKind::Text(font));
				self.push(offset..next, style, kind);
				start = next;
				continue;
			}

			if offset > start && self.splits_text(start, offset) {
				self.push(start..offset, style, DraftKind::Text(font));
				start = offset;
			}
		}

		self.push(start..range.end, style, DraftKind::Text(font));
	}

	/// Whether text starting at `start` must end before `offset`. Offsets
	/// must increase between calls, as for [`Self::is_break`].
	fn splits_text(&mut self, start: u32, offset: u32) -> bool {
		self.level(offset) != self.level(start) || self.is_break(offset)
	}

	fn embed(
		&mut self,
		context: &mut ItemContext<'_>,
		range: Range<u32>,
		style: &TextStyle,
		embed: &canvist_model::Embed,
	) {
		let index = self.style(style.clone(), context.cache);

		let draft = match context.styles.embed_style(embed.kind) {
			Some(EmbedStyle::LineBreak) => {
				self.push(range, index, DraftKind::Break);
				return;
			}
			Some(EmbedStyle::Box { width, height }) => {
				let width = attr_px(&embed.attrs, "width").unwrap_or(*width);
				let height = attr_px(&embed.attrs, "height").unwrap_or(*height);
				DraftEmbed::sized(embed.kind, (width, height))
			}
			Some(label @ EmbedStyle::Label { patch, .. }) => {
				let mut label_style = style.clone();
				patch.apply(&mut label_style);

				let label = Label {
					style: self.style(label_style, context.cache),
					text: Arc::from(label.label_text(&embed.attrs).unwrap_or_default()),
				};
				DraftEmbed::labelled(embed.kind, label)
			}
			None => DraftEmbed::sized(embed.kind, (style.font.size, style.font.size)),
		};

		self.push(range, index, DraftKind::Embed(draft));
	}

	/// Queue every text item and embed label for measurement, and every
	/// kerning pair between touching items in one font.
	fn request_measurements(&self, cache: &mut MeasureCache) {
		let mut joined = String::new();

		for pair in self.items.windows(2) {
			if let Some((font, pair)) = kerning_pair(self.text, &pair[0], &pair[1]) {
				super::kerning::request(cache, font, pair, &mut joined);
			}
		}

		for item in &self.items {
			match &item.kind {
				DraftKind::Text(font) => cache.request(*font, slice(self.text, &item.range)),
				DraftKind::Embed(DraftEmbed {
					label: Some(label), ..
				}) => {
					let font = self.styles[label.style as usize].font;
					cache.request(font, &label.text);
				}
				DraftKind::Tab | DraftKind::Break | DraftKind::Embed(_) => {}
			}
		}
	}
}

/// The atomic item `character` forms on its own, if any.
fn atomic_kind(character: char) -> Option<DraftKind> {
	if character == '\t' {
		return Some(DraftKind::Tab);
	}

	is_newline(character).then_some(DraftKind::Break)
}

impl DraftEmbed {
	/// A box embed with a fixed `(width, height)`.
	fn sized(kind: EmbedTypeId, size: (f32, f32)) -> Self {
		Self {
			kind,
			size: Some(size),
			label: None,
		}
	}

	/// A text embed sized by its measured label.
	fn labelled(kind: EmbedTypeId, label: Label) -> Self {
		Self {
			kind,
			size: None,
			label: Some(label),
		}
	}
}

impl RunStyle {
	fn new(style: TextStyle, cache: &mut MeasureCache) -> Self {
		let font = cache.font(&style.font);
		cache.request_line_box(font, style.line_height);

		Self {
			style,
			font,
			metrics: FontMetrics::default(),
			line_box: LineBox::default(),
		}
	}
}

/// The font and graphemes of the kerning pair between two draft items, if
/// they touch in one font and direction.
pub(crate) fn kerning_pair<'t>(
	text: &'t str,
	left: &DraftItem,
	right: &DraftItem,
) -> Option<(FontId, (&'t str, &'t str))> {
	match (&left.kind, &right.kind) {
		(DraftKind::Text(a), DraftKind::Text(b)) if a == b && left.level == right.level => {
			super::kerning::touching(text, &left.range, &right.range).map(|pair| (*a, pair))
		}
		_ => None,
	}
}

/// Turn draft items into measured items. Fails only if the cache lacks a
/// segment, which means it was not flushed after itemizing.
pub(crate) fn measure_items(
	text: &str,
	drafts: Vec<DraftItem>,
	styles: &mut [RunStyle],
	cache: &MeasureCache,
) -> Result<Vec<Item>, LayoutError> {
	for style in styles.iter_mut() {
		style.metrics = cache
			.metrics(style.font)
			.ok_or(LayoutError::MissingMeasurement)?;
		style.line_box = cache
			.line_box(style.font, style.style.line_height)
			.ok_or(LayoutError::MissingMeasurement)?;
	}

	let measurer = DraftMeasurer {
		text,
		styles,
		cache,
	};

	drafts
		.into_iter()
		.map(|draft| measurer.item(draft))
		.collect()
}

/// What measuring the draft items of one block reads from.
struct DraftMeasurer<'a> {
	text: &'a str,
	styles: &'a [RunStyle],
	cache: &'a MeasureCache,
}

impl DraftMeasurer<'_> {
	fn item(&self, draft: DraftItem) -> Result<Item, LayoutError> {
		let (kind, width) = match draft.kind {
			DraftKind::Text(font) => {
				let spacing = self.styles[draft.style as usize].style.letter_spacing;
				let measured = self.spaced(font, slice(self.text, &draft.range), spacing)?;
				let width = measured.width;
				(ItemKind::Text { font, measured }, width)
			}
			DraftKind::Tab => (ItemKind::Tab, 0.0),
			DraftKind::Break => (ItemKind::Break, 0.0),
			DraftKind::Embed(embed) => {
				let embed = self.embed(embed)?;
				let width = embed.width;
				(ItemKind::Embed(Box::new(embed)), width)
			}
		};

		Ok(Item {
			range: draft.range,
			style: draft.style,
			level: draft.level,
			kind,
			width,
			kern: 0.0,
			break_after: draft.break_after,
		})
	}

	/// The cached measurement of `text` in `font`, letter spaced.
	fn spaced(&self, font: FontId, text: &str, spacing: f32) -> Result<Arc<Measured>, LayoutError> {
		let measured = self
			.cache
			.get(font, text)
			.ok_or(LayoutError::MissingMeasurement)?;

		Ok(letter_spaced(text, measured, spacing))
	}

	fn embed(&self, embed: DraftEmbed) -> Result<EmbedBox, LayoutError> {
		if let Some((width, height)) = embed.size {
			return Ok(EmbedBox {
				kind: embed.kind,
				width,
				ascent: height,
				descent: 0.0,
				label: None,
			});
		}

		let Some(label) = embed.label else {
			return Ok(EmbedBox {
				kind: embed.kind,
				width: 0.0,
				ascent: 0.0,
				descent: 0.0,
				label: None,
			});
		};
		let style = &self.styles[label.style as usize];
		let width = if label.text.is_empty() {
			0.0
		} else {
			self.spaced(style.font, &label.text, style.style.letter_spacing)?
				.width
		};

		let (above, below) = style.line_box();

		Ok(EmbedBox {
			kind: embed.kind,
			width,
			ascent: above,
			descent: below,
			label: Some(label),
		})
	}
}

/// Add `spacing` after every grapheme cluster, sharing the cached
/// measurement when there is none.
fn letter_spaced(text: &str, measured: &Arc<Measured>, spacing: f32) -> Arc<Measured> {
	if spacing == 0.0 {
		return measured.clone();
	}

	let mut advances = measured.advances.to_vec();
	let starts = grapheme_boundaries(text);
	let mut boundary = starts.iter().peekable();

	for (char_index, (offset, _)) in text.char_indices().enumerate() {
		if boundary.peek().is_some_and(|b| **b as usize == offset) {
			boundary.next();

			if let Some(advance) = advances.get_mut(char_index) {
				*advance += spacing;
			}
		}
	}

	let width = advances.iter().sum();
	Arc::new(Measured {
		advances: advances.into_boxed_slice(),
		width,
	})
}

/// Grapheme cluster boundaries of `text`, including 0 and `text.len()`.
///
/// Extended grapheme clusters (UAX #29, Unicode 16), found with the same
/// segmenter the editor moves the caret with, so a caret never stops where
/// layout has no edge.
pub(crate) fn grapheme_boundaries(text: &str) -> Vec<u32> {
	text.grapheme_indices(true)
		.map(|(offset, _)| offset as u32)
		.chain(std::iter::once(text.len() as u32))
		.collect()
}

/// Characters that force a line break (UAX #14 classes BK, CR, LF, NL).
pub(crate) fn is_newline(c: char) -> bool {
	matches!(
		c,
		'\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
	)
}

/// Whether `c` can change bidi levels in otherwise left-to-right text:
/// right-to-left scripts and explicit directional formatting characters.
pub(crate) fn is_bidi_sensitive(c: char) -> bool {
	matches!(
		c,
		'\u{0590}'..='\u{08FF}'
			| '\u{200E}'..='\u{200F}'
			| '\u{202A}'..='\u{202E}'
			| '\u{2066}'..='\u{2069}'
			| '\u{FB1D}'..='\u{FDFF}'
			| '\u{FE70}'..='\u{FEFF}'
			| '\u{10800}'..='\u{10FFF}'
			| '\u{1E800}'..='\u{1EFFF}'
	)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn graphemes_keep_clusters_together() {
		let family = "👨\u{200D}👩\u{200D}👧";
		let text = format!("e\u{301}{family}x");

		assert_eq!(
			grapheme_boundaries(&text),
			vec![0, 3, 3 + family.len() as u32, 4 + family.len() as u32]
		);
	}
}
