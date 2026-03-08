# canvist

A canvas-based text editor written in Rust.

**canvist** renders text through a custom canvas engine — the same approach used by Google Docs and Figma. Instead of delegating to `contenteditable` or platform text views, canvist owns every pixel, giving you full control over the editing experience.

## Why?

Browser-native editing surfaces (`contenteditable`, `textarea`) are notoriously inconsistent across platforms and browsers. By taking full ownership of rendering, canvist can guarantee:

- Pixel-perfect cross-platform consistency
- First-class real-time collaboration via CRDTs
- Full accessibility through a parallel hidden DOM
- Custom rendering backends (Canvas2D, WebGL, Skia, native)
