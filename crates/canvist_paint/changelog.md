All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-24)

### 🚀 Feature

#### Add ink drawings and kinetic type, drawn by the canvas itself

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #179](https://github.com/canvist/canvist/pull/179)

Two canvas-native extensions, each behind its own Cargo feature and included in `full`.

**`Ink` (`ink`)** adds `ink` blocks: freehand drawings in a space 1000 units wide and `height` units tall, scaled with their column.

- Strokes are `points` lists of `x, y` pairs with an optional `color` and `width`.
- Commands: `insertInk`, `addInkStroke`, `undoInkStroke`, and `clearInk`. Query: `inks`.
- Each stroke is one undo step and one Yjs update.
- HTML writes a `<figure data-type="ink" role="img" aria-label="Drawing">` with an SVG of the drawing and the strokes as JSON. Markdown writes the same HTML, and both read it back.

**`Kinetic` (`kinetic`)** adds the `kinetic` mark, whose `effect` is `wave`, `shake`, `pulse`, or `rainbow`.

- Commands: `setKinetic`, `toggleKinetic` (another effect switches to it), and `unsetKinetic`. Query: `kineticEffect`.
- HTML writes `<span data-kinetic="…">`. Markdown keeps the text.

`canvist_layout`:

- `DisplayItem::Stroke` is new, and the SVG renderer, `canvist_web`'s Canvas2D painter, and `canvist_paint` draw it with round caps and joins. A stroke holds a range of the new `DisplayList::points`, read with `DisplayList::stroke_points`, and its bounds, so no item but a text run owns memory and lists of text stay as fast to build as before.
- `LeafStyle::Ink` lays out drawings. `Motion` and `AttrBinding::Motion` set how text moves.
- `DisplayOptions::time` moves kinetic text glyph by glyph, and `DisplayList::animated` says when a later frame would differ. Without a time, text is still, as it should be for readers who prefer reduced motion.
- The HTML renderer draws ink as inline SVG in a figure named as an image, and animates kinetic text with CSS, off under `prefers-reduced-motion`.
- New default features `ink` and `kinetic` hold this code. `canvist_web` builds without them, as it does without `tables`.

`canvist_core`: `ValueType::sample` gives lists two items, so lists of pairs get valid samples.

`canvist_canvas` and `canvist_canvas_wasm` turn on `canvist_layout`'s new `ink` and `kinetic` features.

#### Render the canvas, and edit text frames with the real text editor

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #134](https://github.com/canvist/canvist/pull/134)

`canvist_paint` (new) draws a canvas page through one `Painter` contract. Backends:

- vello on wgpu: WebGPU in browsers, and Metal, Vulkan, and DX12 natively;
- `vello_cpu`, for PNG export and the browser fallback;
- SVG, with text kept as text.

The scene walker culls to the viewport and has level of detail. Text is shaped with parley and the bundled Atkinson Hyperlegible font, so a frame wraps the same on the canvas, in a PNG, and in SVG. With 10,000 nodes on screen, encoding a GPU frame takes about 1 ms and a 1080p CPU frame about 4 ms.

`canvist_canvas` adds `TextSession` (feature `editor`). It opens a text frame in a `canvist_core::Editor`, places the caret where you click, draws the caret and selection in the overlay, and hands the document back on close. It also adds `Canvas::tool_named`.

`canvist_core` adds `Editor::into_state`, so a host that lent a document to an editor can take it back.

`canvist_canvas_wasm` (new, unpublished) runs the canvas in the browser on WebGPU with a CPU fallback, behind the `@canvist/canvas` demo.

#### Paint any display list, and shape with the system's fonts

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #161](https://github.com/canvist/canvist/pull/161)

- `paint_display_list` paints a `canvist_layout` display list through any `Painter`, with a `DisplayStyle` transform and optional fill. Canvas text frames now paint through it too, and `GREEK_PIXELS` moves with it.
- `Shaper::with_system_fonts` reads the fonts installed on the machine, for desktop hosts.
- `ShapedRun` gains `coords`, `embolden`, and `skew`, with `embolden_amount()` and `shear()`.

### 🐛 Fixed

- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)
- **Draw bold and italic text at the weight and slant layout measured.** The GPU and CPU painters drew every glyph from a font's default instance, so bold text in a variable font (such as the system UI font) painted at regular weight, and fonts without a bold or italic face drew neither. Glyph runs now carry the variation coordinates parley shaped with, and synthetic bold and italic when the font lacks them. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #161](https://github.com/canvist/canvist/pull/161)
