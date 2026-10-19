# Theming and styling

In this guide you change how documents look: fonts, colours, and spacing for each block and mark. Because canvist paints the document itself, you style it with a theme, not CSS. The same theme drives the live editor and [server-rendered HTML and SVG](../quick-start/ssr.md), so they always match.

**Status:** themes work today for server-side rendering, in Rust. The editor hosts take the same themes when they ship.

## What a theme is

A theme is a base text style plus rules:

- **Block rules** style a block type, such as `heading` or `codeBlock`, optionally only when an attribute matches, such as `level = 1`, or only inside another block.
- **Mark rules** style a mark, such as `bold` or `link`.
- **Embed rules** style inline embeds, such as `hardBreak`.
- **Chrome** sets the selection colour, caret colour, and caret width.

Rules are keyed by type name, so an extension's blocks are styled by name whether they come from the StarterKit or your own extension. Later rules override earlier ones, and text styles inherit from their container, the way CSS inherits `color` and `font`.

## Start from the StarterKit theme

`Theme::starter_kit()` styles every StarterKit block and mark. Extend it with your own rules:

```rust
use std::sync::Arc;

use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::Renderer;
use canvist_layout::BlockPatch;
use canvist_layout::BlockRule;
use canvist_layout::Color;
use canvist_layout::TextPatch;
use canvist_layout::Theme;

fn main() -> Result<(), canvist_headless::HeadlessError> {
	let brand = Color::rgb(0x6a, 0x00, 0xf5);
	let theme = Theme::starter_kit()
		.to_builder()
		// Every heading in the brand colour.
		.block(BlockRule::new(
			"heading",
			BlockPatch::new().text(TextPatch::new().color(brand)),
		))
		// Level-one headings in a serif face, too.
		.block(
			BlockRule::new(
				"heading",
				BlockPatch::new().text(TextPatch::new().family("Georgia, serif")),
			)
			.when("level", 1),
		)
		.build();

	let doc = Engine::starter_kit().parse(Format::Markdown, "# Launch\n\n## Details")?;
	let renderer = Renderer::with_theme(Arc::new(theme));
	let html = renderer.html(&doc, 640.0);

	assert!(html.css.contains("Georgia"));
	Ok(())
}
```

Add `canvist_layout` next to `canvist_headless` to use themes:

```sh
cargo add canvist_layout --git https://github.com/ifiokjr/canvist
```

## What you can set

| Rule part    | Settings                                                                                                                          |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| `TextPatch`  | family, size (pixels or `em`), weight, italic, line height, colour, background, underline, strike, wavy underline, letter spacing |
| `BlockPatch` | text, padding, a left border (for quotes), background and corner radius (for code blocks), space before and after, list markers   |
| `Chrome`     | selection colour, caret colour, caret width                                                                                       |

## Fonts

Name fonts in the theme as you would in CSS, with fallbacks. On a server, register each font's bytes with `Renderer::register_font` so text is measured with the real font. Until then, the bundled Atkinson Hyperlegible stands in. See [Server-side rendering](../quick-start/ssr.md#how-the-html-matches-the-editor).

## Next

- [Server-side rendering](../quick-start/ssr.md): render HTML and SVG with your theme.
- [Performance](./performance.md): what rendering costs.
