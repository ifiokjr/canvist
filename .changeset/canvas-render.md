---
"canvist_paint": minor
"canvist_canvas": minor
"canvist_editor": minor
"canvist_canvas_wasm": minor
---

# Render the canvas, and edit text frames with the real text editor

`canvist_paint` (new) draws a canvas page through one `Painter` contract. Backends:

- vello on wgpu: WebGPU in browsers, and Metal, Vulkan, and DX12 natively;
- `vello_cpu`, for PNG export and the browser fallback;
- SVG, with text kept as text.

The scene walker culls to the viewport and has level of detail. Text is shaped with parley and the bundled Atkinson Hyperlegible font, so a frame wraps the same on the canvas, in a PNG, and in SVG. With 10,000 nodes on screen, encoding a GPU frame takes about 1 ms and a 1080p CPU frame about 4 ms.

`canvist_canvas` adds `TextSession` (feature `editor`). It opens a text frame in a `canvist_editor::Editor`, places the caret where you click, draws the caret and selection in the overlay, and hands the document back on close. It also adds `Canvas::tool_named`.

`canvist_editor` adds `Editor::into_state`, so a host that lent a document to an editor can take it back.

`canvist_canvas_wasm` (new, unpublished) runs the canvas in the browser on WebGPU with a CPU fallback, behind the `@canvist/canvas` demo.
