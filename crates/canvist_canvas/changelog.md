All notable changes to this project will be documented in this file. This changelog is managed by [monochange](https://github.com/monochange/monochange).

## [0.2.0](https://github.com/canvist/canvist/releases/tag/v0.2.0) (2026-10-24)

### 🚀 Feature

#### Add `canvist_canvas`, the canvas's headless interaction core

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #133](https://github.com/canvist/canvist/pull/133)

`canvist_canvas` turns pointer, wheel, and key input into scene edits, with no UI of its own. Hosts feed it input and draw the scene and its overlay.

- A camera with zoom about a point, momentum, and pinch; an R-tree spatial index; and exact hit testing by fill, stroke, and text layout.
- Selection, marquee, transform handles that keep scale out of the transform, and snapping with guides.
- Tools for selection, panning, shapes, frames, the pen, and text, registered as typed extensions alongside commands and inspector fields. Machine-readable metadata describes them for bindings and agents.
- Auto-layout, constraints, and text frames laid out with `canvist_layout`.

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

### 🐛 Fixed

- **Move the repository to canvist/canvist.** canvist now lives at [github.com/canvist/canvist](https://github.com/canvist/canvist). Every repository, documentation, and badge link points there, the npm packages declare their repository, homepage, and issue tracker (with support@canvist.dev), and the release workflows run for the `canvist` organisation. The website footer has a Contact link to support@canvist.dev. _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/canvist/canvist/pull/148)
