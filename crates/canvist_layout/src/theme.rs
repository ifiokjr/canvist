//! Data-driven themes: how block types, marks, and embeds look.
//!
//! A [`Theme`] is a base [`TextStyle`] plus rules keyed by schema type
//! *names*, so an extension that adds a block, mark, or embed also adds a
//! rule for it. Rules are partial ([`TextPatch`], [`BlockPatch`]); later rules
//! override earlier ones, and block text styles inherit from their container
//! the way CSS inherits `color` and `font`.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use canvist_model::Value;

use crate::Color;
use crate::FontSpec;
use crate::Insets;

#[cfg(feature = "json")]
mod json;
mod starter_kit;

pub use starter_kit::ColorScheme;
pub use starter_kit::StarterKitOptions;

/// A font size, either absolute or relative to the inherited size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FontSize {
	/// An absolute size in pixels.
	Px(f32),
	/// A multiple of the inherited size, like CSS `em`.
	Em(f32),
}

/// Text decorations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Decorations {
	/// Draw an underline.
	pub underline: bool,
	/// Draw a line through the text.
	pub strike: bool,
	/// Draw a wavy underline, e.g. for spelling errors.
	pub squiggle: bool,
	/// The decoration colour, or the text colour when `None`.
	pub color: Option<Color>,
}

impl Decorations {
	/// Whether any decoration is drawn.
	#[must_use]
	pub fn any(&self) -> bool {
		self.underline || self.strike || self.squiggle
	}
}

/// A fully resolved text style.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
	/// The font.
	pub font: FontSpec,
	/// The line height as a multiple of the font size, like a unitless CSS
	/// `line-height`.
	pub line_height: f32,
	/// The text colour.
	pub color: Color,
	/// The background behind the text, e.g. for inline code or highlights.
	pub background: Option<Color>,
	/// Underline, strike, and squiggle.
	pub decorations: Decorations,
	/// Extra space after every grapheme cluster, in pixels, like CSS
	/// `letter-spacing`.
	pub letter_spacing: f32,
	/// How far the text's baseline sits above the line's, in pixels, like
	/// CSS `vertical-align` with a length. Superscripts are positive and
	/// subscripts negative.
	pub baseline_shift: f32,
	/// How the text moves when the host asks for motion.
	pub motion: Motion,
}

/// How kinetic text moves, glyph by glyph, when the host passes a time in
/// [`DisplayOptions::time`](crate::DisplayOptions::time). Without one, text
/// is still, as it is for readers who prefer reduced motion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Motion {
	/// Still.
	#[default]
	None,
	/// Each glyph rises and falls a little after the one before it.
	Wave,
	/// Glyphs jitter, as if shivering.
	Shake,
	/// Glyphs fade in and out in a ripple.
	Pulse,
	/// Glyphs cycle through the colour wheel.
	Rainbow,
}

impl Motion {
	/// Every motion that moves.
	pub const ALL: [Self; 4] = [Self::Wave, Self::Shake, Self::Pulse, Self::Rainbow];

	/// The attribute value naming the motion, such as `wave`.
	#[must_use]
	pub const fn name(self) -> &'static str {
		match self {
			Self::None => "none",
			Self::Wave => "wave",
			Self::Shake => "shake",
			Self::Pulse => "pulse",
			Self::Rainbow => "rainbow",
		}
	}

	/// The motion named `name`.
	#[must_use]
	pub fn from_name(name: &str) -> Option<Self> {
		Self::ALL.into_iter().find(|motion| motion.name() == name)
	}
}

impl TextStyle {
	/// The used line height in pixels.
	#[must_use]
	pub fn line_height_px(&self) -> f32 {
		self.font.size * self.line_height
	}
}

/// A partial [`TextStyle`]: every `Some` field overrides the inherited one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextPatch {
	/// The CSS `font-family` list.
	pub family: Option<Arc<str>>,
	/// The font size.
	pub size: Option<FontSize>,
	/// The CSS weight.
	pub weight: Option<u16>,
	/// Italic or upright.
	pub italic: Option<bool>,
	/// The unitless line height.
	pub line_height: Option<f32>,
	/// The text colour.
	pub color: Option<Color>,
	/// The text background.
	pub background: Option<Color>,
	/// Underline on or off.
	pub underline: Option<bool>,
	/// Strike-through on or off.
	pub strike: Option<bool>,
	/// Squiggle on or off.
	pub squiggle: Option<bool>,
	/// The decoration colour.
	pub decoration_color: Option<Color>,
	/// Letter spacing in pixels.
	pub letter_spacing: Option<f32>,
	/// Raise the baseline by this many ems of the patched font size, like
	/// CSS `vertical-align: 0.5em`. Negative lowers it. Shifts add up, as
	/// nested inline boxes do in CSS.
	pub baseline_shift: Option<f32>,
}

impl TextPatch {
	/// A patch that changes nothing.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the font family list.
	#[must_use]
	pub fn family(mut self, family: &str) -> Self {
		self.family = Some(Arc::from(family));
		self
	}

	/// Set the font size.
	#[must_use]
	pub fn size(mut self, size: FontSize) -> Self {
		self.size = Some(size);
		self
	}

	/// Set the font weight.
	#[must_use]
	pub fn weight(mut self, weight: u16) -> Self {
		self.weight = Some(weight);
		self
	}

	/// Set italic.
	#[must_use]
	pub fn italic(mut self, italic: bool) -> Self {
		self.italic = Some(italic);
		self
	}

	/// Set the unitless line height.
	#[must_use]
	pub fn line_height(mut self, line_height: f32) -> Self {
		self.line_height = Some(line_height);
		self
	}

	/// Set the text colour.
	#[must_use]
	pub fn color(mut self, color: Color) -> Self {
		self.color = Some(color);
		self
	}

	/// Set the text background.
	#[must_use]
	pub fn background(mut self, color: Color) -> Self {
		self.background = Some(color);
		self
	}

	/// Turn the underline on or off.
	#[must_use]
	pub fn underline(mut self, on: bool) -> Self {
		self.underline = Some(on);
		self
	}

	/// Turn strike-through on or off.
	#[must_use]
	pub fn strike(mut self, on: bool) -> Self {
		self.strike = Some(on);
		self
	}

	/// Turn the squiggle on or off.
	#[must_use]
	pub fn squiggle(mut self, on: bool) -> Self {
		self.squiggle = Some(on);
		self
	}

	/// Set letter spacing in pixels.
	#[must_use]
	pub fn letter_spacing(mut self, px: f32) -> Self {
		self.letter_spacing = Some(px);
		self
	}

	/// Raise the baseline by `em` of the patched font size; negative lowers
	/// it.
	#[must_use]
	pub fn baseline_shift(mut self, em: f32) -> Self {
		self.baseline_shift = Some(em);
		self
	}

	/// Apply this patch on top of `style`.
	pub fn apply(&self, style: &mut TextStyle) {
		if let Some(family) = &self.family {
			style.font.family = family.clone();
		}

		match self.size {
			Some(FontSize::Px(px)) => style.font.size = px,
			Some(FontSize::Em(em)) => style.font.size *= em,
			None => {}
		}

		set(&mut style.font.weight, self.weight);
		set(&mut style.font.italic, self.italic);
		set(&mut style.line_height, self.line_height);
		set(&mut style.color, self.color);
		set(&mut style.letter_spacing, self.letter_spacing);
		set(&mut style.decorations.underline, self.underline);
		set(&mut style.decorations.strike, self.strike);
		set(&mut style.decorations.squiggle, self.squiggle);

		if self.background.is_some() {
			style.background = self.background;
		}

		if self.decoration_color.is_some() {
			style.decorations.color = self.decoration_color;
		}

		if let Some(em) = self.baseline_shift {
			style.baseline_shift += em * style.font.size;
		}
	}
}

fn set<T: Copy>(target: &mut T, value: Option<T>) {
	if let Some(value) = value {
		*target = value;
	}
}

/// A left border, e.g. a blockquote bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
	/// The width in pixels.
	pub width: f32,
	/// The colour.
	pub color: Color,
}

/// The marker a list container draws beside each child.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkerKind {
	/// A bullet that cycles through [`MarkerStyle::bullets`] by depth.
	Bullet,
	/// `1.`, `2.`, … starting from the list's `start` attribute.
	Ordered,
	/// A checkbox node-view slot reflecting the item's `checked` attribute.
	Task,
	/// `▾` beside an open container's first child, or `▸` when its `open`
	/// attribute is `false`, as a `<details>` summary shows.
	Disclosure,
}

/// How a block takes part in a table's grid.
///
/// A table's rows are laid out together: each cell's content is placed in
/// the columns its `colspan` covers, from the widths in its `colwidth`
/// attribute (a list of pixels, one per column, as TipTap stores them) or an
/// even share of what is left, and each row is as tall as its tallest cell.
/// A cell's `rowspan` runs it down into the rows below.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TableRole {
	/// The table, with grid lines of `rule` around and between its cells.
	Table {
		/// The grid lines.
		rule: Border,
	},
	/// A row of cells.
	Row,
	/// A cell, or a header cell.
	Cell,
}

/// How a leaf block (one with no content) is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LeafStyle {
	/// A horizontal rule of the given thickness.
	Rule {
		/// The thickness in pixels, which is also the block's height.
		thickness: f32,
		/// The colour.
		color: Color,
	},
	/// A host-rendered node view. The block's `height` attribute, when it is a
	/// number, overrides `height`.
	Slot {
		/// The default height in pixels.
		height: f32,
	},
	/// A freehand drawing that layout draws itself, from the block's
	/// `strokes` attribute: each a `points` list of `x, y` pairs, with an
	/// optional `color` and `width`. Coordinates are in a space
	/// [`INK_WIDTH`] wide, so a drawing scales with its column; the block's
	/// `height` attribute, in the same units, overrides `height`.
	Ink {
		/// The default height, in drawing units.
		height: f32,
		/// The default stroke width, in drawing units.
		stroke: f32,
	},
}

/// The width of an ink drawing's coordinate space: a drawing this many
/// units wide fills its column.
pub const INK_WIDTH: f32 = 1000.0;

/// A partial block style contributed by a [`BlockRule`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockPatch {
	/// Text style changes, inherited by descendants.
	pub text: TextPatch,
	/// Space between the block's border and its content.
	pub padding: Option<Insets>,
	/// A left border.
	pub border_left: Option<Border>,
	/// A background behind the whole block.
	pub background: Option<Color>,
	/// The background's corner radius.
	pub radius: Option<f32>,
	/// Space above the block. Between siblings the larger of the previous
	/// block's `space_after` and this block's `space_before` is used, and a
	/// first child gets none.
	pub space_before: Option<f32>,
	/// Space below the block. A last child gets none.
	pub space_after: Option<f32>,
	/// The marker drawn beside each child of this container.
	pub marker: Option<MarkerKind>,
	/// How the block draws when it is a leaf.
	pub leaf: Option<LeafStyle>,
	/// Its part in a table grid, if any.
	pub table: Option<TableRole>,
	/// The HTML element used for server rendering, e.g. `p` or `h2`.
	pub tag: Option<Arc<str>>,
}

impl BlockPatch {
	/// A patch that changes nothing.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the text patch.
	#[must_use]
	pub fn text(mut self, text: TextPatch) -> Self {
		self.text = text;
		self
	}

	/// Set the padding.
	#[must_use]
	pub fn padding(mut self, padding: Insets) -> Self {
		self.padding = Some(padding);
		self
	}

	/// Set the left border.
	#[must_use]
	pub fn border_left(mut self, width: f32, color: Color) -> Self {
		self.border_left = Some(Border { width, color });
		self
	}

	/// Set the background and its corner radius.
	#[must_use]
	pub fn background(mut self, color: Color, radius: f32) -> Self {
		self.background = Some(color);
		self.radius = Some(radius);
		self
	}

	/// Set the space above and below.
	#[must_use]
	pub fn space(mut self, before: f32, after: f32) -> Self {
		self.space_before = Some(before);
		self.space_after = Some(after);
		self
	}

	/// Set the marker drawn beside children.
	#[must_use]
	pub fn marker(mut self, marker: MarkerKind) -> Self {
		self.marker = Some(marker);
		self
	}

	/// Set the leaf style.
	#[must_use]
	pub fn leaf(mut self, leaf: LeafStyle) -> Self {
		self.leaf = Some(leaf);
		self
	}

	/// Set the block's part in a table grid.
	#[must_use]
	pub fn table(mut self, role: TableRole) -> Self {
		self.table = Some(role);
		self
	}

	/// Set the HTML tag.
	#[must_use]
	pub fn tag(mut self, tag: &str) -> Self {
		self.tag = Some(Arc::from(tag));
		self
	}
}

/// Styles every block of one type, optionally only when an attribute has a
/// given value or when the block sits directly inside a given container.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockRule {
	/// The block type name.
	pub block: Arc<str>,
	/// Only match when this attribute equals this value. The schema's default
	/// attributes apply when the block does not set the attribute.
	pub when: Option<(Arc<str>, Value)>,
	/// Only match when the parent block has this type name.
	pub parent: Option<Arc<str>>,
	/// The style changes.
	pub patch: BlockPatch,
}

impl BlockRule {
	/// A rule for every block of type `block`.
	#[must_use]
	pub fn new(block: &str, patch: BlockPatch) -> Self {
		Self {
			block: Arc::from(block),
			when: None,
			parent: None,
			patch,
		}
	}

	/// Only match when attribute `key` equals `value`.
	#[must_use]
	pub fn when(mut self, key: &str, value: impl Into<Value>) -> Self {
		self.when = Some((Arc::from(key), value.into()));
		self
	}

	/// Only match directly inside a `parent` block.
	#[must_use]
	pub fn within(mut self, parent: &str) -> Self {
		self.parent = Some(Arc::from(parent));
		self
	}
}

/// Reads a style value from a mark attribute, e.g. a colour mark's `color`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttrBinding {
	/// A `#rrggbb` attribute sets the text colour.
	Color(Arc<str>),
	/// A `#rrggbb` attribute sets the background.
	Background(Arc<str>),
	/// A string attribute sets the font family.
	FontFamily(Arc<str>),
	/// A numeric attribute sets the font size in pixels.
	FontSize(Arc<str>),
	/// A unitless number, such as `1.5` or `"1.5"`, sets the line height as a
	/// multiple of the font size.
	LineHeight(Arc<str>),
	/// A [`Motion`] name, such as `wave`, sets how the text moves.
	Motion(Arc<str>),
}

/// Styles one mark type.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkRule {
	/// The mark type name.
	pub mark: Arc<str>,
	/// The style changes.
	pub patch: TextPatch,
	/// Style values read from the mark's attributes, applied after `patch`.
	pub bindings: Vec<AttrBinding>,
	/// The HTML element used for server rendering, e.g. `strong`.
	pub tag: Arc<str>,
}

impl MarkRule {
	/// A rule rendering `mark` as `<tag>` with `patch`.
	#[must_use]
	pub fn new(mark: &str, tag: &str, patch: TextPatch) -> Self {
		Self {
			mark: Arc::from(mark),
			patch,
			bindings: Vec::new(),
			tag: Arc::from(tag),
		}
	}

	/// Add an attribute binding.
	#[must_use]
	pub fn bind(mut self, binding: AttrBinding) -> Self {
		self.bindings.push(binding);
		self
	}
}

/// How an inline embed lays out.
#[derive(Clone, Debug, PartialEq)]
pub enum EmbedStyle {
	/// A forced line break, like `<br>`.
	LineBreak,
	/// A box of fixed size sitting on the baseline, reserved for a
	/// host-rendered node view. Numeric `width` and `height` attributes
	/// override the defaults.
	Box {
		/// The default width in pixels.
		width: f32,
		/// The default height in pixels.
		height: f32,
	},
	/// Unbreakable text read from an attribute, e.g. a mention's `@label`.
	Label {
		/// The attribute holding the text.
		attr: Arc<str>,
		/// Text drawn before the attribute value.
		prefix: Arc<str>,
		/// The label's style, on top of the surrounding text.
		patch: TextPatch,
		/// An attribute drawn when `attr` is missing: after the prefix, such
		/// as a mention's id when it has no label, or as `:value:` when there
		/// is no prefix, such as an emoji's shortcode when only its name is
		/// known.
		fallback: Option<Arc<str>>,
	},
}

impl EmbedStyle {
	/// The text a label embed with `attrs` draws, or `None` for other
	/// styles.
	#[must_use]
	pub fn label_text(&self, attrs: &canvist_model::Attrs) -> Option<String> {
		let Self::Label {
			attr,
			prefix,
			fallback,
			..
		} = self
		else {
			return None;
		};
		let text = |key: &str| attrs.get(key).and_then(Value::as_str);

		Some(match (text(attr), fallback.as_deref().and_then(text)) {
			(Some(value), _) => format!("{prefix}{value}"),
			(None, Some(fallback)) if prefix.is_empty() => format!(":{fallback}:"),
			(None, Some(fallback)) => format!("{prefix}{fallback}"),
			(None, None) => prefix.to_string(),
		})
	}
}

/// Styles one embed type.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbedRule {
	/// The embed type name.
	pub embed: Arc<str>,
	/// How it lays out.
	pub style: EmbedStyle,
}

impl EmbedRule {
	/// A rule for `embed`.
	#[must_use]
	pub fn new(embed: &str, style: EmbedStyle) -> Self {
		Self {
			embed: Arc::from(embed),
			style,
		}
	}
}

/// The page background, selection, and caret colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chrome {
	/// The background behind the document. The web canvas is opaque and
	/// fills it, and server-rendered HTML sets it on `.cv-doc`, so both
	/// antialias text against the same colour.
	pub background: Color,
	/// The selection highlight.
	pub selection: Color,
	/// The caret.
	pub caret: Color,
	/// The caret width in pixels.
	pub caret_width: f32,
}

/// How list markers look.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkerStyle {
	/// Space between a marker's right edge and the item content, in pixels.
	pub gap: f32,
	/// Bullet glyphs by nesting depth, cycling when lists nest deeper.
	pub bullets: Vec<Arc<str>>,
	/// The task checkbox's side length in pixels.
	pub checkbox: f32,
	/// The marker colour, or the item's text colour when `None`.
	pub color: Option<Color>,
}

/// The selection highlight of a new theme: a translucent blue that reads over
/// light and dark text alike.
const DEFAULT_SELECTION: Color = Color::rgba(0, 120, 215, 64);

/// Process-wide counter giving every built theme a distinct version.
static NEXT_VERSION: AtomicU64 = AtomicU64::new(1);

/// An immutable theme. Build one with [`Theme::builder`], or start from
/// [`Theme::starter_kit`] and extend it with [`Theme::to_builder`].
#[derive(Clone, Debug)]
pub struct Theme {
	base: TextStyle,
	tab_size: u32,
	chrome: Chrome,
	markers: MarkerStyle,
	blocks: Vec<BlockRule>,
	marks: Vec<MarkRule>,
	embeds: Vec<EmbedRule>,
	version: u64,
}

impl Theme {
	/// Start a theme from the root text style.
	#[must_use]
	pub fn builder(base: TextStyle) -> ThemeBuilder {
		ThemeBuilder {
			theme: Self {
				base,
				tab_size: 8,
				chrome: Chrome {
					background: Color::rgb(0xff, 0xff, 0xff),
					selection: DEFAULT_SELECTION,
					caret: Color::rgb(0, 0, 0),
					caret_width: 2.0,
				},
				markers: MarkerStyle {
					gap: 6.0,
					bullets: vec![Arc::from("•"), Arc::from("◦"), Arc::from("▪")],
					checkbox: 14.0,
					color: None,
				},
				blocks: Vec::new(),
				marks: Vec::new(),
				embeds: Vec::new(),
				version: 0,
			},
		}
	}

	/// Continue building from this theme. The result gets a new version.
	#[must_use]
	pub fn to_builder(&self) -> ThemeBuilder {
		ThemeBuilder {
			theme: self.clone(),
		}
	}

	/// A version unique to this theme. Layout caches are dropped whenever
	/// the version changes.
	#[must_use]
	pub fn version(&self) -> u64 {
		self.version
	}

	/// The root text style.
	#[must_use]
	pub fn base(&self) -> &TextStyle {
		&self.base
	}

	/// Tab stops every this many space widths, like CSS `tab-size`.
	#[must_use]
	pub fn tab_size(&self) -> u32 {
		self.tab_size
	}

	/// Selection and caret colours.
	#[must_use]
	pub fn chrome(&self) -> &Chrome {
		&self.chrome
	}

	/// List marker settings.
	#[must_use]
	pub fn markers(&self) -> &MarkerStyle {
		&self.markers
	}

	/// Block rules in the order they apply.
	#[must_use]
	pub fn block_rules(&self) -> &[BlockRule] {
		&self.blocks
	}

	/// Mark rules; a later rule for the same mark wins.
	#[must_use]
	pub fn mark_rules(&self) -> &[MarkRule] {
		&self.marks
	}

	/// Embed rules; a later rule for the same embed wins.
	#[must_use]
	pub fn embed_rules(&self) -> &[EmbedRule] {
		&self.embeds
	}
}

/// Builds a [`Theme`].
#[derive(Clone, Debug)]
pub struct ThemeBuilder {
	theme: Theme,
}

impl ThemeBuilder {
	/// Set the tab size in space widths.
	#[must_use]
	pub fn tab_size(mut self, spaces: u32) -> Self {
		self.theme.tab_size = spaces.max(1);
		self
	}

	/// Set the selection and caret colours.
	#[must_use]
	pub fn chrome(mut self, chrome: Chrome) -> Self {
		self.theme.chrome = chrome;
		self
	}

	/// Set the list marker style.
	#[must_use]
	pub fn markers(mut self, markers: MarkerStyle) -> Self {
		self.theme.markers = markers;
		self
	}

	/// Append a block rule.
	#[must_use]
	pub fn block(mut self, rule: BlockRule) -> Self {
		self.theme.blocks.push(rule);
		self
	}

	/// Append a mark rule.
	#[must_use]
	pub fn mark(mut self, rule: MarkRule) -> Self {
		self.theme.marks.push(rule);
		self
	}

	/// Append an embed rule.
	#[must_use]
	pub fn embed(mut self, rule: EmbedRule) -> Self {
		self.theme.embeds.push(rule);
		self
	}

	/// Finish the theme, giving it a fresh version.
	#[must_use]
	pub fn build(mut self) -> Theme {
		self.theme.version = NEXT_VERSION.fetch_add(1, Ordering::Relaxed);
		self.theme
	}
}

#[cfg(test)]
#[allow(
	clippy::float_cmp,
	reason = "the values compared are exactly representable"
)]
mod tests {
	use super::*;

	#[test]
	fn patches_override_and_scale() {
		const BOLD: u16 = 700;

		let mut style = Theme::starter_kit().base().clone();
		TextPatch::new()
			.size(FontSize::Em(0.5))
			.weight(BOLD)
			.underline(true)
			.apply(&mut style);

		assert_eq!(style.font.size, 8.0);
		assert_eq!(style.font.weight, BOLD);
		assert!(style.decorations.underline);
		assert!(!style.decorations.strike);
	}

	#[test]
	fn every_build_gets_a_new_version() {
		let theme = Theme::starter_kit();
		let extended = theme.to_builder().tab_size(4).build();

		assert_ne!(theme.version(), extended.version());
		assert_eq!(extended.tab_size(), 4);
	}
}
