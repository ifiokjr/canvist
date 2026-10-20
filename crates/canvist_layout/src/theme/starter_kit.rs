//! The default theme, matching the look of TipTap's StarterKit templates.

use std::sync::Arc;

use super::AttrBinding;
use super::BlockPatch;
use super::BlockRule;
use super::Chrome;
use super::Decorations;
use super::EmbedRule;
use super::EmbedStyle;
use super::FontSize;
use super::LeafStyle;
use super::MarkRule;
use super::MarkerKind;
use super::MarkerStyle;
use super::TextPatch;
use super::TextStyle;
use super::Theme;
use super::ThemeBuilder;
use crate::Color;
use crate::FontSpec;
use crate::Insets;

/// The sans-serif stack used for body text.
pub const SANS: &str = "Inter, ui-sans-serif, system-ui, sans-serif";
/// The monospace stack used for code. It names fonts rather than
/// `ui-monospace`: after a missing family, WebKit takes tab stops from the
/// `ui-monospace` face but draws glyphs from the `monospace` fallback, which
/// a canvas cannot reproduce.
pub const MONO: &str =
	"\"JetBrains Mono\", SFMono-Regular, Menlo, Consolas, \"Liberation Mono\", monospace";

/// Light or dark colours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColorScheme {
	/// Dark text on white, TipTap's look.
	#[default]
	Light,
	/// Light text on a warm near-black.
	Dark,
}

/// The colours one scheme uses.
struct Palette {
	page: Color,
	ink: Color,
	accent: Color,
	accent_wash: Color,
	rule: Color,
	quote_bar: Color,
	highlight: Color,
	code_page: Color,
	code_ink: Color,
	selection: Color,
}

const LIGHT: Palette = Palette {
	page: Color::rgb(0xff, 0xff, 0xff),
	ink: Color::rgb(0x2e, 0x2b, 0x29),
	accent: Color::rgb(0x6a, 0x00, 0xf5),
	accent_wash: Color::rgb(0xee, 0xee, 0xff),
	rule: Color::rgb(0xe5, 0xe3, 0xe1),
	quote_bar: Color::rgb(0xd6, 0xd3, 0xd1),
	highlight: Color::rgb(0xfa, 0xf5, 0x94),
	code_page: Color::rgb(0x2e, 0x2b, 0x29),
	code_ink: Color::rgb(0xff, 0xff, 0xff),
	selection: Color::rgba(0x6a, 0x00, 0xf5, 0x33),
};

const DARK: Palette = Palette {
	page: Color::rgb(0x1a, 0x18, 0x17),
	ink: Color::rgb(0xe9, 0xe6, 0xe3),
	accent: Color::rgb(0xb8, 0x96, 0xff),
	accent_wash: Color::rgb(0x33, 0x2a, 0x4d),
	rule: Color::rgb(0x3d, 0x39, 0x36),
	quote_bar: Color::rgb(0x57, 0x52, 0x4e),
	highlight: Color::rgb(0x5c, 0x55, 0x1a),
	code_page: Color::rgb(0x0d, 0x0c, 0x0b),
	code_ink: Color::rgb(0xe9, 0xe6, 0xe3),
	selection: Color::rgba(0xb8, 0x96, 0xff, 0x4d),
};

/// What [`Theme::starter_kit_with`] varies: the colour scheme, the font
/// stacks, and the body size. Spacing stays in the StarterKit's rem units.
#[derive(Clone, Debug, PartialEq)]
pub struct StarterKitOptions {
	/// Light or dark colours.
	pub scheme: ColorScheme,
	/// The CSS `font-family` list for body text and headings.
	pub body_family: Arc<str>,
	/// The CSS `font-family` list for inline code and code blocks.
	pub code_family: Arc<str>,
	/// The body font size in pixels.
	pub size: f32,
	/// The body line height, as a multiple of the font size.
	pub line_height: f32,
	/// Extra space after every grapheme of body text, in pixels, like CSS
	/// `letter-spacing`.
	pub letter_spacing: f32,
}

impl Default for StarterKitOptions {
	fn default() -> Self {
		Self {
			scheme: ColorScheme::Light,
			body_family: Arc::from(SANS),
			code_family: Arc::from(MONO),
			size: REM,
			line_height: BODY_LINE_HEIGHT,
			letter_spacing: 0.0,
		}
	}
}

/// One root em in pixels.
const REM: f32 = 16.0;
/// The body line height, as a multiple of the font size.
const BODY_LINE_HEIGHT: f32 = 1.5;

/// The weight of headings and bold text.
const BOLD: u16 = 700;
/// Headings set tighter than body text, as a multiple of their font size.
const HEADING_LINE_HEIGHT: f32 = 1.1;

/// Space between a list marker and the item content.
const MARKER_GAP: f32 = 0.4 * REM;
/// The side length of a task checkbox.
const CHECKBOX_SIZE: f32 = 0.875 * REM;

/// Space above and below a top-level paragraph.
const PARAGRAPH_SPACE: f32 = 0.75 * REM;
/// Space above and below block quotes and code blocks.
const FIGURE_SPACE: f32 = 1.5 * REM;
/// Space above and below a top-level list.
const LIST_SPACE: f32 = 1.25 * REM;
/// The left padding a list keeps for its markers.
const LIST_INDENT: f32 = 1.4 * REM;
/// Space between list items, and around the paragraphs and lists inside
/// them, so items read as one block.
const ITEM_SPACE: f32 = 0.25 * REM;

/// The font size of code blocks.
const CODE_BLOCK_SIZE: f32 = 0.8 * REM;
/// The vertical padding of code blocks.
const CODE_BLOCK_PADDING: f32 = 0.75 * REM;
/// The font size of inline code, relative to the surrounding text.
const INLINE_CODE_SCALE: f32 = 0.85;
/// The font size of subscripts and superscripts, relative to the text.
const SCRIPT_SCALE: f32 = 0.75;
/// How far a subscript's baseline drops, in ems of its own size.
const SUBSCRIPT_SHIFT: f32 = -0.25;
/// How far a superscript's baseline rises, in ems of its own size.
const SUPERSCRIPT_SHIFT: f32 = 0.5;
/// The default height of an image block.
const IMAGE_HEIGHT: f32 = 12.5 * REM;

/// How one heading level looks. Sizes and spacing are in rem.
struct HeadingStyle {
	level: i64,
	size: f32,
	space_before: f32,
	space_after: f32,
}

impl HeadingStyle {
	/// The rule for this level, refining the base heading rule.
	fn rule(&self) -> BlockRule {
		BlockRule::new(
			"heading",
			BlockPatch::new()
				.text(TextPatch::new().size(FontSize::Px(self.size * REM)))
				.space(self.space_before * REM, self.space_after * REM)
				.tag(&format!("h{}", self.level)),
		)
		.when("level", self.level)
	}
}

/// Heading font sizes and spacing, from level 1 to 6. The top two levels
/// open major sections, so they get more room above and below.
const HEADINGS: [HeadingStyle; 6] = [
	HeadingStyle {
		level: 1,
		size: 1.4,
		space_before: 3.5,
		space_after: 1.5,
	},
	HeadingStyle {
		level: 2,
		size: 1.2,
		space_before: 3.5,
		space_after: 1.5,
	},
	HeadingStyle {
		level: 3,
		size: 1.1,
		space_before: 2.5,
		space_after: 0.75,
	},
	HeadingStyle {
		level: 4,
		size: 1.0,
		space_before: 2.5,
		space_after: 0.75,
	},
	HeadingStyle {
		level: 5,
		size: 1.0,
		space_before: 2.5,
		space_after: 0.75,
	},
	HeadingStyle {
		level: 6,
		size: 1.0,
		space_before: 2.5,
		space_after: 0.75,
	},
];

impl Theme {
	/// The default theme. Block, mark, and embed names follow TipTap's
	/// StarterKit (`paragraph`, `bulletList`, `codeBlock`, `hardBreak`, …),
	/// so documents imported from TipTap JSON style correctly.
	#[must_use]
	pub fn starter_kit() -> Self {
		Self::starter_kit_with(&StarterKitOptions::default())
	}

	/// The StarterKit theme in another colour scheme, font stacks, or body
	/// size. Server rendering and the canvas must build the theme from the
	/// same options, or the HTML will not match the canvas.
	#[must_use]
	pub fn starter_kit_with(options: &StarterKitOptions) -> Self {
		let palette = match options.scheme {
			ColorScheme::Light => &LIGHT,
			ColorScheme::Dark => &DARK,
		};
		let builder = Theme::builder(base_style(options, palette))
			.chrome(Chrome {
				background: palette.page,
				selection: palette.selection,
				caret: palette.ink,
				caret_width: 2.0,
			})
			.markers(MarkerStyle {
				gap: MARKER_GAP,
				bullets: vec![Arc::from("•"), Arc::from("◦"), Arc::from("▪")],
				checkbox: CHECKBOX_SIZE,
				color: None,
			});
		let builder = block_rules(options, palette)
			.into_iter()
			.fold(builder, ThemeBuilder::block);
		let builder = mark_rules(options, palette)
			.into_iter()
			.fold(builder, ThemeBuilder::mark);

		embed_rules(palette)
			.into_iter()
			.fold(builder, ThemeBuilder::embed)
			.build()
	}
}

/// The root text style: body text in the body stack.
fn base_style(options: &StarterKitOptions, palette: &Palette) -> TextStyle {
	TextStyle {
		font: FontSpec {
			family: options.body_family.clone(),
			size: options.size,
			weight: 400,
			italic: false,
		},
		line_height: options.line_height,
		color: palette.ink,
		background: None,
		decorations: Decorations::default(),
		letter_spacing: options.letter_spacing,
		baseline_shift: 0.0,
	}
}

/// Every block rule, in the order later rules override earlier ones.
fn block_rules(options: &StarterKitOptions, palette: &Palette) -> Vec<BlockRule> {
	let mut rules = text_block_rules();

	rules.extend(container_rules(palette));
	rules.extend(code_and_leaf_rules(options, palette));
	rules.extend(nested_list_rules());
	rules
}

/// Paragraphs and headings.
fn text_block_rules() -> Vec<BlockRule> {
	let heading_text = TextPatch::new()
		.weight(BOLD)
		.line_height(HEADING_LINE_HEIGHT);
	let mut rules = vec![
		BlockRule::new(
			"paragraph",
			BlockPatch::new()
				.space(PARAGRAPH_SPACE, PARAGRAPH_SPACE)
				.tag("p"),
		),
		BlockRule::new("heading", BlockPatch::new().text(heading_text).tag("h1")),
	];

	rules.extend(HEADINGS.iter().map(HeadingStyle::rule));
	rules
}

/// Block quotes, lists, and list items with their paragraphs.
fn container_rules(palette: &Palette) -> Vec<BlockRule> {
	let list = |marker| {
		BlockPatch::new()
			.padding(Insets::new(0.0, REM, 0.0, LIST_INDENT))
			.space(LIST_SPACE, LIST_SPACE)
			.marker(marker)
	};
	let item = || BlockPatch::new().space(ITEM_SPACE, ITEM_SPACE);

	vec![
		BlockRule::new(
			"blockquote",
			BlockPatch::new()
				.border_left(3.0, palette.quote_bar)
				.padding(Insets::new(0.0, 0.0, 0.0, REM))
				.space(FIGURE_SPACE, FIGURE_SPACE)
				.tag("blockquote"),
		),
		BlockRule::new("bulletList", list(MarkerKind::Bullet).tag("ul")),
		BlockRule::new("orderedList", list(MarkerKind::Ordered).tag("ol")),
		BlockRule::new("taskList", list(MarkerKind::Task).tag("ul")),
		BlockRule::new("listItem", item().tag("li")),
		BlockRule::new("taskItem", item().tag("li")),
		BlockRule::new("paragraph", item()).within("listItem"),
		BlockRule::new("paragraph", item()).within("taskItem"),
	]
}

/// Code blocks, horizontal rules, and images.
fn code_and_leaf_rules(options: &StarterKitOptions, palette: &Palette) -> Vec<BlockRule> {
	let code_text = TextPatch::new()
		.family(&options.code_family)
		.size(FontSize::Px(CODE_BLOCK_SIZE))
		.color(palette.code_ink);

	vec![
		BlockRule::new(
			"codeBlock",
			BlockPatch::new()
				.text(code_text)
				.padding(Insets::symmetric(CODE_BLOCK_PADDING, REM))
				.background(palette.code_page, 0.5 * REM)
				.space(FIGURE_SPACE, FIGURE_SPACE)
				.tag("pre"),
		),
		BlockRule::new(
			"horizontalRule",
			BlockPatch::new()
				.leaf(LeafStyle::Rule {
					thickness: 1.0,
					color: palette.rule,
				})
				.space(2.0 * REM, 2.0 * REM)
				.tag("hr"),
		),
		BlockRule::new(
			"image",
			BlockPatch::new()
				.leaf(LeafStyle::Slot {
					height: IMAGE_HEIGHT,
				})
				.space(REM, REM)
				.tag("div"),
		),
	]
}

/// Lists nested in list items take item spacing instead of list spacing.
fn nested_list_rules() -> Vec<BlockRule> {
	let mut rules = Vec::new();

	for list in ["bulletList", "orderedList", "taskList"] {
		for item in ["listItem", "taskItem"] {
			rules.push(
				BlockRule::new(list, BlockPatch::new().space(ITEM_SPACE, ITEM_SPACE)).within(item),
			);
		}
	}

	rules
}

fn mark_rules(options: &StarterKitOptions, palette: &Palette) -> Vec<MarkRule> {
	let mut rules = vec![
		MarkRule::new("bold", "strong", TextPatch::new().weight(BOLD)),
		MarkRule::new("italic", "em", TextPatch::new().italic(true)),
		MarkRule::new("underline", "u", TextPatch::new().underline(true)),
		MarkRule::new("strike", "s", TextPatch::new().strike(true)),
		MarkRule::new(
			"code",
			"code",
			TextPatch::new()
				.family(&options.code_family)
				.size(FontSize::Em(INLINE_CODE_SCALE))
				.background(palette.accent_wash),
		),
		MarkRule::new(
			"link",
			"a",
			TextPatch::new().color(palette.accent).underline(true),
		),
		MarkRule::new(
			"highlight",
			"mark",
			TextPatch::new().background(palette.highlight),
		)
		.bind(AttrBinding::Background(Arc::from("color"))),
		MarkRule::new("textStyle", "span", TextPatch::new())
			.bind(AttrBinding::Color(Arc::from("color")))
			.bind(AttrBinding::Background(Arc::from("backgroundColor")))
			.bind(AttrBinding::FontFamily(Arc::from("fontFamily")))
			.bind(AttrBinding::FontSize(Arc::from("fontSize")))
			.bind(AttrBinding::LineHeight(Arc::from("lineHeight"))),
	];

	if cfg!(feature = "theme-extensions") {
		rules.extend([
			MarkRule::new(
				"subscript",
				"sub",
				TextPatch::new()
					.size(FontSize::Em(SCRIPT_SCALE))
					.baseline_shift(SUBSCRIPT_SHIFT),
			),
			MarkRule::new(
				"superscript",
				"sup",
				TextPatch::new()
					.size(FontSize::Em(SCRIPT_SCALE))
					.baseline_shift(SUPERSCRIPT_SHIFT),
			),
		]);
	}

	rules
}

fn embed_rules(palette: &Palette) -> Vec<EmbedRule> {
	let mut rules = vec![
		EmbedRule::new("hardBreak", EmbedStyle::LineBreak),
		EmbedRule::new(
			"mention",
			EmbedStyle::Label {
				attr: Arc::from("label"),
				prefix: Arc::from("@"),
				patch: TextPatch::new()
					.color(palette.accent)
					.background(palette.accent_wash),
				fallback: None,
			},
		),
		EmbedRule::new(
			"inlineImage",
			EmbedStyle::Box {
				width: REM,
				height: REM,
			},
		),
	];

	if cfg!(feature = "theme-extensions") {
		rules.extend([EmbedRule::new(
			"emoji",
			EmbedStyle::Label {
				attr: Arc::from("emoji"),
				prefix: Arc::from(""),
				patch: TextPatch::new(),
				fallback: Some(Arc::from("name")),
			},
		)]);
	}

	rules
}
