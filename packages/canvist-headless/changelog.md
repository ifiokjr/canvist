All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.1.1](https://github.com/canvist/canvist/releases/tag/v0.1.1) (2026-10-20)

### 🚀 Feature

#### Load, transform, and export canvas scenes on a server

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #136](https://github.com/canvist/canvist/pull/136)

`canvist_headless`'s new `scene` feature adds `SceneEngine` and `SceneDoc`. They load canvas scenes from JSON (flat or nested) or Yjs state, run canvas commands, edit inspector fields, transform scenes in transactions with undo, and export the page or chosen nodes as SVG or PNG with the renderer the canvas draws with. With `collab`, a scene shares and merges Yjs updates. Scene failures carry the existing stable error codes.

`@canvist/headless/scene` exposes the same in TypeScript, with `SceneJson` types for every node, property, paint, effect, and shape. It is a separate WebAssembly module, so the document entry point's size is unchanged.

#### Add `@canvist/headless`, the document model for JavaScript runtimes with no DOM

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #110](https://github.com/canvist/canvist/pull/110)

Add `@canvist/headless` (`packages/canvist-headless`), canvist's document model for JavaScript runtimes with no DOM, over a new `canvist_headless_wasm` crate (wasm-bindgen, no `web-sys`).

- Loads in Node, Deno, and Bun from disk, and in Workers from an imported `WebAssembly.Module`. A `build` step emits JavaScript for npm, and CI installs the packed tarball as a dependency.
- Types come from the extension metadata (`src/extensions.generated.ts`, generated from the `canvist_extensions` snapshot): an engine's documents only offer the registered extensions' commands, queries, blocks, and marks, with argument types such as heading levels `1 | … | 6`. `tests/types.check.ts` pins this with `tsc --noEmit` and `@ts-expect-error`.
- The shared conformance fixtures pass in Node and Deno (30 of 30), alongside API tests that sync with JavaScript Yjs.
- `examples/server` is a Deno server that converts Markdown to JSON over HTTP and runs a y-websocket-compatible sync endpoint, tested end to end with real Yjs clients.
- `bench/overhead.ts` records per-call overhead and the wasm size against budgets.

#### Add `@canvist/web`, the canvas editor for the browser, with a fonts plugin

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #137](https://github.com/canvist/canvist/pull/137)

- `@canvist/web` and `canvist_web` paint the document on an opaque Canvas2D context with HiDPI backing stores, dirty-rectangle repaints, and baselines and backgrounds snapped to the browser's own pixel grid. `createEditor` types commands, queries, and plugin APIs from what you register.
- Text is measured by the browser: `measureText` advances with kerning placed as shapers place it, line boxes read from the DOM, family lists resolved to the face the DOM uses, and DOM ranges for Arabic and for Firefox's fractional sizes. The canvas wraps where server-rendered HTML does.
- `@canvist/web/fonts` loads faces from URLs or bytes with `unicode-range` and `font-display`, builds metric-compatible fallbacks, lists local fonts through `queryLocalFonts`, and writes the same faces as `@font-face` CSS for SSR.
- `canvist_layout`: `TextMeasurer` batches carry line-box requests that a measurer may answer; `DocumentLayout::forget_fonts` relays out only the blocks a loaded face affects; `measure_stats`, `line_ranges`, `fonts`, and `font_metrics`; kerning between adjacent segments; `chromium_breaks`; border widths snapped to device pixels; `Theme::starter_kit_with` for dark and custom-font themes, and `Chrome::background`. `to_html` now sets the background and `text-spacing-trim: normal`, the StarterKit code stack no longer names `ui-monospace`, and graphemes come from the editor's segmenter. `ParleyMeasurer` sets a variable font's optical size to the font size, as browsers do.
- `canvist_headless`: `Doc::edit` runs editor calls with a host's clock and line navigator, and `Doc::command_named` resolves a named command.
- `@canvist/headless` exports its generated extension descriptors at `@canvist/headless/extensions`, with no WebAssembly.

### 🐛 Fixed

- **Read the bundled WebAssembly module from a file path.** `load()` converts the bundled module's `file:` URL to a path before reading it, so the package type-checks under JSR's `deno publish` as well as with `@types/node`. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #125](https://github.com/canvist/canvist/pull/125)
- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)
