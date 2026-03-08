# canvist_wasm

> WebAssembly + Canvas2D rendering backend for the canvist canvas editor.

This crate provides the browser-side rendering backend that connects the
canvist editor to an HTML `<canvas>` element. It implements the
`canvist_render::Renderer` trait using the Canvas 2D API.

## Features

- **Canvas2D rendering** — draws text, selections, and UI via `CanvasRenderingContext2d`
- **DOM event bridge** — keyboard, mouse, and IME event handling
- **Accessibility** — generates a parallel hidden DOM for screen readers
- **WASM-first** — compiled to WebAssembly for near-native performance
