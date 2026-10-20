All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.1.1](https://github.com/canvist/canvist/releases/tag/v0.1.1) (2026-10-20)

### 🚀 Feature

#### Add `canvist_layout`, the v2 layout engine

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #100](https://github.com/canvist/canvist/pull/100)

- Data-driven `Theme` rules for blocks, marks, and embeds, with a default matching TipTap's StarterKit.
- A batched `TextMeasurer` trait with a per-(font, segment) cache, a deterministic `FixedMeasurer`, and a `ParleyMeasurer` behind the `parley` feature.
- UAX #14 line breaking with CSS `pre-wrap` semantics, UAX #9 bidi, tabs, embeds, and hard breaks.
- `DocumentLayout` with container blocks, list markers, viewport virtualization with estimated heights, and scroll anchoring.
- Incremental relayout from a `Commit`. A keystroke re-breaks only the lines around the edit, even in a 100 KB paragraph, and a structural edit re-flattens only the root subtrees it touched.
- Hit testing, caret and selection rectangles, selection bounds for anchoring host UI, and a `LayoutNavigator` for vertical caret motion that implements `canvist_editor::LineNavigator` behind the `editor` feature.
- A `DisplayList` with dirty-rectangle damage, plus `to_svg` and `to_html` renderers whose CSS reproduces the canvas line breaks.
- Release-mode performance budget tests and `divan` benchmarks at 100, 10k, and 100k blocks, plus long and heavily formatted paragraphs.

#### Fit the web editor in its 350 KB budget

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #137](https://github.com/canvist/canvist/pull/137)

- `@canvist/web` now ships 349.8 KB gzipped, measured as an app bundles it (`npm run size`, enforced in CI). The WebAssembly module is optimized without inlining, which gzips smaller.
- `canvist_layout::css_number` formats CSS numbers without `core`'s float formatting, which no longer links into the module.
- `EditorState::change` and `InputRule::new` keep one copy of their logic instead of one per command and rule, and `Engine::metadata` is built on first use.

#### Add `@canvist/web`, the canvas editor for the browser, with a fonts plugin

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #137](https://github.com/canvist/canvist/pull/137)

- `@canvist/web` and `canvist_web` paint the document on an opaque Canvas2D context with HiDPI backing stores, dirty-rectangle repaints, and baselines and backgrounds snapped to the browser's own pixel grid. `createEditor` types commands, queries, and plugin APIs from what you register.
- Text is measured by the browser: `measureText` advances with kerning placed as shapers place it, line boxes read from the DOM, family lists resolved to the face the DOM uses, and DOM ranges for Arabic and for Firefox's fractional sizes. The canvas wraps where server-rendered HTML does.
- `@canvist/web/fonts` loads faces from URLs or bytes with `unicode-range` and `font-display`, builds metric-compatible fallbacks, lists local fonts through `queryLocalFonts`, and writes the same faces as `@font-face` CSS for SSR.
- `canvist_layout`: `TextMeasurer` batches carry line-box requests that a measurer may answer; `DocumentLayout::forget_fonts` relays out only the blocks a loaded face affects; `measure_stats`, `line_ranges`, `fonts`, and `font_metrics`; kerning between adjacent segments; `chromium_breaks`; border widths snapped to device pixels; `Theme::starter_kit_with` for dark and custom-font themes, and `Chrome::background`. `to_html` now sets the background and `text-spacing-trim: normal`, the StarterKit code stack no longer names `ui-monospace`, and graphemes come from the editor's segmenter. `ParleyMeasurer` sets a variable font's optical size to the font size, as browsers do.
- `canvist_headless`: `Doc::edit` runs editor calls with a host's clock and line navigator, and `Doc::command_named` resolves a named command.
- `@canvist/headless` exports its generated extension descriptors at `@canvist/headless/extensions`, with no WebAssembly.

### 🐛 Fixed

- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)

<details>
<summary><strong>🧪 Testing</strong></summary>

#### Test canvas and server-rendered HTML for pixel parity in every browser

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #138](https://github.com/canvist/canvist/pull/138)

- Playwright renders a fixture matrix (system and web fonts, a variable font, CJK with `unicode-range` subsets, Arabic and Hebrew, Devanagari, three emoji formats, letter spacing, sizes from 10 to 64 px) as the canvas, the SSR HTML, and SVG, at device pixel ratios 1 to 3, fractional zoom, and light and dark themes, in Chromium, Firefox, and WebKit on macOS and Linux.
- Line boxes, runs, and caret stops are compared exactly; pixels with pixelmatch and SSIM against per-browser limits, with every known difference documented. Each run uploads a side-by-side report.
- A Rust test holds `parley`, with the same Inter file, to Chromium's line breaks.

</details>
