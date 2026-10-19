# The SSR CSS contract

canvist renders a document on the server as semantic HTML (`to_html`), then the web host replaces it with the live canvas. The swap must not move anything: every line has to wrap at the same character in the HTML as on the canvas, and every block has to sit at the same height. This page lists what `canvist_layout` emits to make that true, what the web host must do, and where exact agreement is still at risk. The [pixel parity tests](../src/guides/rendering-parity.md) check the whole contract in Chromium, Firefox, and WebKit.

## How agreement works

The canvas and the HTML agree because the same browser measures both:

1. **Breaking.** `canvist_layout` finds break opportunities with ICU4X's line segmenter (UAX #14, `line-break: strict`, `word-break: normal`). Browsers implement the same algorithm, and the CSS pins the same options.
2. **Measuring.** On the web, the `TextMeasurer` is the browser's own `measureText`, with the same font strings the CSS uses. So a word is the same width in the canvas layout and in the HTML.
3. **Filling lines.** Both fill lines greedily with `white-space: pre-wrap` rules: trailing spaces hang, and a word wider than the line breaks between graphemes (`overflow-wrap: break-word`).
4. **Stacking blocks.** Both build the same CSS line boxes (CSS 2.1 §10.8.1), and the HTML spells out every vertical gap that layout uses, so margins never collapse differently.

## What `to_html` emits

`to_html(&doc, &theme, &HtmlOptions::new(width))` returns a fragment (`<div class="cv-doc">…</div>`) and a stylesheet scoped under `.cv-doc`. `HtmlDocument::page()` wraps both in a complete page.

### The pinned properties

`.cv-doc` sets every property that changes line breaking or glyph advances, so a host page's own CSS cannot leak in:

| Property                                    | Value                     | Why                                                                                                           |
| ------------------------------------------- | ------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `width`                                     | the layout width, in px   | Lines are broken to exactly this width.                                                                       |
| `box-sizing`, `margin`, `padding`           | `content-box`, `0`, `0`   | The content box must be the layout width.                                                                     |
| `white-space`                               | `pre-wrap`                | Preserves spaces and tabs, and makes trailing spaces hang, as layout does.                                    |
| `overflow-wrap`                             | `break-word`              | Long words break between graphemes, as layout's emergency split does.                                         |
| `word-break`, `line-break`                  | `normal`, `strict`        | The ICU4X segmenter options layout uses.                                                                      |
| `hyphens`                                   | `manual`                  | Layout never hyphenates.                                                                                      |
| `text-align`, `text-indent`, `word-spacing` | `start`, `0`, `0`         | Layout aligns to the paragraph's start edge and adds no extra spacing.                                        |
| `tab-size`                                  | the theme's tab size      | Tabs stop at multiples of this many space widths, skipping stops closer than half a space (CSS Text 3).       |
| `font-kerning`, `font-variant-ligatures`    | `normal`                  | Kerning and ligatures stay on, matching what `measureText` and `fillText` use.                                |
| `font-feature-settings`                     | `normal`                  | No extra OpenType features.                                                                                   |
| `text-transform`                            | `none`                    | Layout measures the document's own characters.                                                                |
| `text-size-adjust`                          | `100%`                    | Mobile Safari would otherwise inflate font sizes.                                                             |
| `font-*`, `line-height`, `letter-spacing`   | the resolved theme values | Every block class repeats the full resolved text style, so nothing depends on inheritance from the host page. |
| `background-color`                          | the theme's background    | The canvas is opaque and fills it, so both antialias text against the same colour.                            |
| `text-spacing-trim`, `text-autospace`       | `normal`, `no-autospace`  | Canvas text trims adjacent full-width punctuation and never autospaces, so the HTML does the same.            |

Inside `.cv-doc`, a zero-specificity reset (`:where(*)`) clears the browser's default margins, padding, borders, fonts, list styles, and backgrounds. That is why `<h2>`, `<pre>`, `<ul>`, and `<blockquote>` render with the theme's metrics and not the browser's.

### Blocks

- Every block is `display: flow-root`. It never collapses margins with its children, and its content box width is its parent's minus the theme's padding and border.
- **Vertical gaps** follow one rule, shared with layout: between siblings, the larger of the previous block's `space_after` and the next block's `space_before`; no gap before a first child or after a last child. `to_html` writes each gap as an inline `margin-top`. Blocks never get `margin-bottom`, so no margins can collapse.
- **Text styles** are one class per distinct resolved style (`.cv-s0`, `.cv-s1`, …). Each repeats the font family, size in px, weight, style, unitless line height, colour, and letter spacing.
- **Empty blocks**, and blocks ending in a line break, get a trailing `<br class="cv-br">`. Without it, the browser would give an empty `<p>` no height, or ignore a final `<br>`, while the editor shows an empty line there.
- **Text direction:** every text block gets `dir="auto"`, or its `dir` attribute. `dir="auto"` uses the first strong character, as UAX #9 rules P2 and P3 do in layout. `text-align: start` then aligns right-to-left paragraphs to the right, as layout does.

### Marks

Marks become nested elements (`<strong>`, `<em>`, `<code>`, `<a>`, …) whose classes carry the theme's mark patch. Relative sizes stay relative (`font-size: 0.85em`) and apply in the same order layout applies them (by mark type), so the computed sizes match. Values bound from mark attributes, such as a highlight colour, are validated and written as inline styles. Link targets must be `http`, `https`, `mailto`, or relative, so `to_html` never emits a `javascript:` URL.

### Inline embeds

Embeds are atomic inline-blocks (`display: inline-block; white-space: nowrap`):

- **For breaking and bidi,** CSS treats an atomic inline as U+FFFC. That is the character the model stores for an embed, so break opportunities and bidi resolution agree.
- **Boxes** (inline images, node views) have an explicit width and height. Their baseline is their bottom edge, so they rise `height` above the baseline, as layout assumes.
- **Labels** (mentions) have `dir="auto"`, which isolates their text. Their label text then cannot change the direction of the surrounding paragraph, just as the neutral U+FFFC cannot in layout.
- **Line breaks** are `<br>`.

### Line boxes

Layout reproduces the CSS line box exactly, so the CSS has to keep to the same model:

- Line heights are **unitless**. Each inline box is `font-size × line-height` tall, with half the leading above the ascent and half below the descent. A line's height is the union of the block's strut and every inline box on the line. A larger inline font makes its line taller in both.
- Line heights and baselines are rounded to 1/64 px, which is the `LayoutUnit` Chromium and WebKit use for line boxes.

### Lists

- **Markers** are generated content: `li::before { position: absolute; right: 100%; margin-right: <gap> }`. A marker's right edge sits `gap` px left of the item's padding box, as layout draws it, and it is out of flow, so it never affects wrapping.
- **Ordered lists** use their own counter (`counter-reset: cv-n <start - 1>` on the list, `counter-increment: cv-n` on each item). The `list-item` counter would stop counting once the items are no longer `display: list-item`.
- **Task items** contain a real, disabled `<input type="checkbox">` with `appearance: none`, positioned exactly where layout reserves the checkbox slot. Readers without JavaScript still get a checkbox.

## What the web host does

`canvist_web` and `@canvist/web` implement the host's side of the contract. See [Fonts and text rendering](../src/guides/fonts.md) for how.

1. **The content box is the layout width.** The editor element's content box is the document width, and resizing it lays the document out again. Pad a wrapper, never `.cv-doc` or the editor.
2. **Fonts load before they matter.** When layout first uses a font, the editor asks the browser to load the faces it needs for that text. When faces finish loading, only the fonts that list the family are measured again (`DocumentLayout::forget_fonts`).
3. **Text is measured the way the canvas paints and the DOM lays out.**
   - Advances come from `measureText` with `fontKerning = "normal"`, `fontVariantCaps = "normal"`, `textRendering = "auto"`, `letterSpacing = "0px"`, and `wordSpacing = "0px"`. Layout adds letter spacing itself.
   - The edge after each grapheme is placed as a shaper places it: a kerning pair's adjustment goes to the first glyph. Adjacent segments in one font kern across their boundary.
   - Line boxes come from the DOM: for each font and unitless line height, a hidden probe reports the space above and below the baseline. Canvas metrics are rounded differently from every engine's line layout.
   - Where canvas and DOM resolve a family list to different faces, or draw fractional sizes differently, or where text is cursive (Arabic), widths or edges come from DOM ranges.
4. **The display list is painted as given.** Each `TextRun` is drawn with `fillText` at `(x, baseline)` with `textBaseline = "alphabetic"` and `textAlign = "left"`, `direction` from `rtl`, and `letterSpacing` from the run. Baselines and background edges snap to the grid the browser snaps DOM content to, and border widths snap down to device pixels.
5. **Breaks are tailored to the engine.** On Chromium, layout applies Blink's tailoring (`canvist_layout::chromium_breaks`). Firefox and WebKit follow ICU's defaults, which layout uses without an override.
6. **Do not restyle `.cv-doc`.** That means no `text-wrap: balance` or `pretty`, no `text-autospace` or `text-spacing-trim` changes, no `font-variant` overrides, and no `zoom` on an ancestor.

## Known risks

These are the places where the HTML and the canvas can still differ. The pixel parity tests measure each one.

- **Kerning across a space.** Layout kerns adjacent segments that touch, but measures a word together with its trailing spaces and never kerns across them. A browser shapes the whole line, so a font that kerns a space against a letter shifts later glyphs by a fraction of a pixel. That doesn't change wrapping unless a line fits to within that fraction.
- **Contextual punctuation spacing.** Firefox applies OpenType `chws` to full-width punctuation depending on the characters on both sides. Layout measures Japanese one character at a time, so in Firefox a line holding 「。」 before Latin text can wrap one character differently. See [Rendering parity](../src/guides/rendering-parity.md).
- **Sub-pixel rounding.** Chromium keeps inline positions in 1/64 px units. A line whose width lands within 1/64 px of the available width could wrap differently. Layout already accepts widths up to 1/128 px over the limit, to absorb float error.
- **Complex scripts.** Layout uses ICU4X without its dictionary and LSTM models, so Thai, Lao, Khmer, and Burmese only break at spaces. Browsers break inside words in these scripts. Adding the `auto` segmenter models fixes this, at a large cost in wasm size.
- **New CSS properties.** The stylesheet pins every property known to change advances or wrapping today. A property that browsers add later, with a non-neutral default, would need pinning too; the pixel parity tests are the alarm for it.
- **Decorations.** Underline and strike positions come from the font size, not the font's own metrics. They can sit a pixel off the browser's, but they never move text.
- **Markers.** A marker sits on the first line's baseline in layout, but on its own line box in CSS. The two differ only when the first line holds something taller than the item's own font.

## Measured costs

Layout and both renderers add about **96 KB gzipped** to a wasm build (211 KB raw after `wasm-opt -Oz`). Of that, ICU4X's line and grapheme data is about 23 KB and `unicode-bidi` about 17 KB. The `parley` measurer would add another **283 KB gzipped** (685 KB raw). That is why the web uses the browser's measurer, and `parley` is a Cargo feature for desktop and SSR hosts.
