//! The Wet Caret document theme: the brand's Ink, Paint, and Paper on
//! the bundled Atkinson Hyperlegible face, scaled from one base size.

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

/// The family name the bundled font registers, and the name the video
/// declares with `@font-face`, so the SVG paints with the measured face.
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

const BOLD: u16 = 700;

/// The theme at a base font size of `rem` pixels.
pub fn wet_caret(rem: f32) -> Arc<Theme> {
	let base = TextStyle {
		font: FontSpec {
			family: Arc::from(FAMILY),
			size: rem,
			weight: 400,
			italic: false,
		},
		line_height: 1.45,
		color: INK,
		background: None,
		decorations: Decorations::default(),
		letter_spacing: 0.0,
	};
	let builder = Theme::builder(base)
		.chrome(Chrome {
			selection: SELECTION,
			caret: PAINT,
			caret_width: (rem / 9.0).max(2.0),
		})
		.markers(MarkerStyle {
			gap: 0.45 * rem,
			bullets: vec![Arc::from("•"), Arc::from("◦"), Arc::from("▪")],
			checkbox: 0.8 * rem,
			color: Some(PAINT),
		});
	let builder = block_rules(rem).into_iter().fold(builder, |builder, rule| builder.block(rule));
	let builder = mark_rules().into_iter().fold(builder, |builder, rule| builder.mark(rule));

	Arc::new(
		builder
			.embed(EmbedRule::new("hardBreak", EmbedStyle::LineBreak))
			.build(),
	)
}

fn block_rules(rem: f32) -> Vec<BlockRule> {
	let heading = |level: i64, size: f32, before: f32| {
		BlockRule::new(
			"heading",
			BlockPatch::new()
				.text(
					TextPatch::new()
						.size(FontSize::Em(size))
						.weight(BOLD)
						.line_height(1.1),
				)
				.space(before * rem, 0.35 * rem)
				.tag(&format!("h{level}")),
		)
		.when("level", level)
	};
	let list = |marker| {
		BlockPatch::new()
			.padding(Insets::new(0.0, 0.0, 0.0, 1.4 * rem))
			.space(0.4 * rem, 0.4 * rem)
			.marker(marker)
	};
	let item = || BlockPatch::new().space(0.1 * rem, 0.1 * rem);

	vec![
		BlockRule::new("paragraph", BlockPatch::new().space(0.35 * rem, 0.35 * rem).tag("p")),
		heading(1, 1.9, 0.0),
		heading(2, 1.35, 0.5),
		heading(3, 1.15, 0.4),
		BlockRule::new(
			"blockquote",
			BlockPatch::new()
				.border_left(0.18 * rem, PAINT)
				.padding(Insets::new(0.0, 0.0, 0.0, 0.8 * rem))
				.space(0.5 * rem, 0.5 * rem)
				.tag("blockquote"),
		),
		BlockRule::new("bulletList", list(MarkerKind::Bullet).tag("ul")),
		BlockRule::new("orderedList", list(MarkerKind::Ordered).tag("ol")),
		BlockRule::new("taskList", list(MarkerKind::Task).tag("ul")),
		BlockRule::new("listItem", item().tag("li")),
		BlockRule::new("taskItem", item().tag("li")),
		BlockRule::new("paragraph", item()).within("listItem"),
		BlockRule::new("paragraph", item()).within("taskItem"),
		BlockRule::new(
			"codeBlock",
			BlockPatch::new()
				.text(TextPatch::new().size(FontSize::Em(0.85)).color(PAPER))
				.padding(Insets::symmetric(0.55 * rem, 0.8 * rem))
				.background(INK, 0.35 * rem)
				.space(0.5 * rem, 0.5 * rem)
				.tag("pre"),
		),
		BlockRule::new(
			"horizontalRule",
			BlockPatch::new()
				.leaf(LeafStyle::Rule {
					thickness: 2.0,
					color: RULE,
				})
				.space(0.8 * rem, 0.8 * rem)
				.tag("hr"),
		),
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
