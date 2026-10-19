---
"canvist_headless": minor
"canvist_headless_wasm": minor
"@canvist/headless": minor
---

# Load, transform, and export canvas scenes on a server

`canvist_headless`'s new `scene` feature adds `SceneEngine` and `SceneDoc`. They load canvas scenes from JSON (flat or nested) or Yjs state, run canvas commands, edit inspector fields, transform scenes in transactions with undo, and export the page or chosen nodes as SVG or PNG with the renderer the canvas draws with. With `collab`, a scene shares and merges Yjs updates. Scene failures carry the existing stable error codes.

`@canvist/headless/scene` exposes the same in TypeScript, with `SceneJson` types for every node, property, paint, effect, and shape. It is a separate WebAssembly module, so the document entry point's size is unchanged.
