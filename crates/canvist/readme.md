# canvist

> A canvas-based text editor written in Rust — build your own Google Docs.

**canvist** gives you full control over the editing experience by rendering text through a custom canvas engine, just like Figma and Google Docs do. Instead of relying on `contenteditable` or platform text views, canvist owns every pixel.

## Features

- **Custom render engine** — platform-agnostic rendering via canvas APIs
- **Real-time collaboration** — built-in CRDT support via [Yjs](https://yjs.dev)
- **Full accessibility** — parallel hidden DOM for screen readers
- **Cross-platform** — native, WebAssembly, and server-side rendering
- **Beautiful API** — ergonomic builder patterns for documents, styles, and operations

## Quick start

```rust
use canvist::prelude::*;

// Create a new document
let mut doc = Document::new();

// Insert styled text
let style = Style::new().bold().font_size(24.0).font_family("Inter");
doc.insert_text(Position::zero(), "Hello, canvist!");
doc.apply_style(Selection::all(&doc), &style);
```

## Crate structure

| Crate                                                       | Description                                           |
| ----------------------------------------------------------- | ----------------------------------------------------- |
| [`canvist`](https://crates.io/crates/canvist)               | Umbrella re-export crate (this crate)                 |
| [`canvist_core`](https://crates.io/crates/canvist_core)     | Document model, operations, selections, collaboration |
| [`canvist_render`](https://crates.io/crates/canvist_render) | Platform-agnostic rendering traits and text layout    |
| [`canvist_wasm`](https://crates.io/crates/canvist_wasm)     | WebAssembly + Canvas2D rendering backend              |

## License

This project is licensed under the [Unlicense](https://unlicense.org/).
