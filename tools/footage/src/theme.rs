//! The Wet Caret document theme: the brand's Ink, Paint, and Paper on
//! the bundled Atkinson Hyperlegible faces, scaled from one base size.
//!
//! Every length is a multiple of the base size (`em`), so the same theme
//! reads the same on a 1080p page and a phone.

use std::sync::Arc;

use canvist_layout::BlockPatch;
use canvist_layout::BlockRule;
use canvist_layout::Chrome;
use canvist_layout::Color;
use canvist_layout::Decorations;
use canvist_layout::EmbedRule;
use canvist_layout::EmbedStyle;
use canvist_layout::FontSize;
use canvist_layout::FontSpec;
use canvist_layout::Insets;
use canvist_layout::LeafStyle;
use canvist_layout::MarkRule;
use canvist_layout::MarkerKind;
use canvist_layout::MarkerStyle;
use canvist_layout::TextPatch;
use canvist_layout::TextStyle;
use canvist_layout::Theme;
use canvist_layout::ThemeBuilder;

/// The family name the bundled faces register, and the name the video
/// declares with `@font-face`, so the SVG paints with the measured faces.
pub const FAMILY: &str = "Atkinson Hyperlegible";

/// Body text and headings on light backgrounds.
pub const INK: Color = Color::rgb(0x17, 0x14, 0x1f);
/// The caret colour on light backgrounds; links and quote bars.
pub const PAINT: Color = Color::rgb(0x3b, 0x2c, 0xf5);
/// Text on dark backgrounds, such as code blocks.
pub const PAPER: Color = Color::rgb(0xf5, 0xf3, 0xff);
/// Inline code backgrounds: Paint thinned with Paper.
const WASH: Color = Color::rgb(0xe4, 0xe0, 0xff);
/// The selection: Paint at a fifth of its strength.
const SELECTION: Color = Color::rgba(0x3b, 0x2c, 0xf5, 0x38);
/// Muted rules and borders.
const RULE: Color = Color::rgb(0xd9, 0xd5, 0xea);

const REGULAR: u16 = 400;
const BOLD: u16 = 700;

/// Body line height, as a multiple of the font size.
const LINE_HEIGHT: f32 = 1.45;
/// Headings set tighter than body text.
const HEADING_LINE_HEIGHT: f32 = 1.1;
/// The caret's width in em, and its narrowest in pixels.
const CARET_WIDTH: (f32, f32) = (0.11, 2.0);

/// Heading levels: (level, font size in em, space above in em).
const HEADINGS: [(i64, f32, f32); 3] = [(1, 1.9, 0.0), (2, 1.35, 0.5), (3, 1.15, 0.4)];
/// Space below every heading.
const HEADING_AFTER: f32 = 0.35;

/// Space above and below paragraphs.
const PARAGRAPH_SPACE: f32 = 0.35;
/// Space above and below quotes, lists, and code blocks.
const FIGURE_SPACE: f32 = 0.45;
/// Space above and below list items, so items read as one block.
const ITEM_SPACE: f32 = 0.1;

/// The width of a quote's bar, and the gap after it.
const QUOTE_BAR: (f32, f32) = (0.18, 0.8);
/// The left padding a list keeps for its markers.
const LIST_INDENT: f32 = 1.4;
/// The gap between a list marker and its text.
const MARKER_GAP: f32 = 0.45;
/// The side of a task checkbox.
const CHECKBOX: f32 = 0.8;

/// Code blocks: font size, vertical and horizontal padding, corner radius.
const CODE_SIZE: f32 = 0.85;
const CODE_PADDING: (f32, f32) = (0.55, 0.8);
const CODE_RADIUS: f32 = 0.35;

/// A horizontal rule's thickness in pixels, and its space above and below.
const RULE_THICKNESS: f32 = 2.0;
const RULE_SPACE: f32 = 0.8;

/// The theme at a base font size of `em` pixels.
pub fn wet_caret(em: f32) -> Arc<Theme> {
	let base = TextStyle {
		font: FontSpec {
			family: Arc::from(FAMILY),
			size: em,
			weight: REGULAR,
			italic: false,
		},
		line_height: LINE_HEIGHT,
		color: INK,
		background: None,
		decorations: Decorations::default(),
		letter_spacing: 0.0,
		baseline_shift: 0.0,
		motion: canvist_layout::Motion::None,
	};
	let builder = Theme::builder(base)
		.chrome(Chrome {
			// The film draws its own backdrop; this only fills exported HTML.
			background: Color::rgb(0xff, 0xff, 0xff),
			selection: SELECTION,
			caret: PAINT,
			caret_width: (CARET_WIDTH.0 * em).max(CARET_WIDTH.1),
		})
		.markers(MarkerStyle {
			gap: MARKER_GAP * em,
			bullets: vec![Arc::from("•"), Arc::from("◦"), Arc::from("▪")],
			checkbox: CHECKBOX * em,
			color: Some(PAINT),
		});
	let rules = text_rules(em)
		.into_iter()
		.chain(container_rules(em))
		.chain(leaf_rules(em));
	let builder = rules.fold(builder, ThemeBuilder::block);
	let builder = mark_rules().into_iter().fold(builder, ThemeBuilder::mark);

	Arc::new(
		builder
			.embed(EmbedRule::new("hardBreak", EmbedStyle::LineBreak))
			.build(),
	)
}

/// Paragraphs and headings.
fn text_rules(em: f32) -> Vec<BlockRule> {
	let paragraph = BlockPatch::new()
		.space(PARAGRAPH_SPACE * em, PARAGRAPH_SPACE * em)
		.tag("p");
	let headings = HEADINGS.iter().map(|&(level, size, before)| {
		let text = TextPatch::new()
			.size(FontSize::Em(size))
			.weight(BOLD)
			.line_height(HEADING_LINE_HEIGHT);
		let patch = BlockPatch::new()
			.text(text)
			.space(before * em, HEADING_AFTER * em)
			.tag(&format!("h{level}"));

		BlockRule::new("heading", patch).when("level", level)
	});

	std::iter::once(BlockRule::new("paragraph", paragraph))
		.chain(headings)
		.collect()
}

/// Quotes, lists, and list items with their paragraphs.
fn container_rules(em: f32) -> Vec<BlockRule> {
	let list = |marker| {
		BlockPatch::new()
			.padding(Insets::new(0.0, 0.0, 0.0, LIST_INDENT * em))
			.space(FIGURE_SPACE * em, FIGURE_SPACE * em)
			.marker(marker)
	};
	let item = || BlockPatch::new().space(ITEM_SPACE * em, ITEM_SPACE * em);
	let quote = BlockPatch::new()
		.border_left(QUOTE_BAR.0 * em, PAINT)
		.padding(Insets::new(0.0, 0.0, 0.0, QUOTE_BAR.1 * em))
		.space(FIGURE_SPACE * em, FIGURE_SPACE * em)
		.tag("blockquote");

	vec![
		BlockRule::new("blockquote", quote),
		BlockRule::new("bulletList", list(MarkerKind::Bullet).tag("ul")),
		BlockRule::new("orderedList", list(MarkerKind::Ordered).tag("ol")),
		BlockRule::new("taskList", list(MarkerKind::Task).tag("ul")),
		BlockRule::new("listItem", item().tag("li")),
		BlockRule::new("taskItem", item().tag("li")),
		BlockRule::new("paragraph", item()).within("listItem"),
		BlockRule::new("paragraph", item()).within("taskItem"),
	]
}

/// Code blocks and horizontal rules.
fn leaf_rules(em: f32) -> Vec<BlockRule> {
	let code = BlockPatch::new()
		.text(TextPatch::new().size(FontSize::Em(CODE_SIZE)).color(PAPER))
		.padding(Insets::symmetric(CODE_PADDING.0 * em, CODE_PADDING.1 * em))
		.background(INK, CODE_RADIUS * em)
		.space(FIGURE_SPACE * em, FIGURE_SPACE * em)
		.tag("pre");
	let rule = BlockPatch::new()
		.leaf(LeafStyle::Rule {
			thickness: RULE_THICKNESS,
			color: RULE,
		})
		.space(RULE_SPACE * em, RULE_SPACE * em)
		.tag("hr");

	vec![
		BlockRule::new("codeBlock", code),
		BlockRule::new("horizontalRule", rule),
	]
}

fn mark_rules() -> Vec<MarkRule> {
	vec![
		MarkRule::new("bold", "strong", TextPatch::new().weight(BOLD)),
		MarkRule::new("italic", "em", TextPatch::new().italic(true)),
		MarkRule::new("underline", "u", TextPatch::new().underline(true)),
		MarkRule::new("strike", "s", TextPatch::new().strike(true)),
		MarkRule::new(
			"code",
			"code",
			TextPatch::new().color(PAINT).background(WASH),
		),
		MarkRule::new("link", "a", TextPatch::new().color(PAINT).underline(true)),
		MarkRule::new("highlight", "mark", TextPatch::new().background(WASH)),
	]
}
