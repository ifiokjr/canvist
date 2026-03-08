# canvist

> A canvas-based text editor written in Rust — build your own Google Docs.

[![ci](https://github.com/ifiokjr/canvist/actions/workflows/ci.yml/badge.svg)](https://github.com/ifiokjr/canvist/actions/workflows/ci.yml)

**canvist** gives you full control over the editing experience by rendering text through a custom canvas engine, just like [Figma](https://www.figma.com) and [Google Docs](https://docs.google.com) do. Instead of relying on `contenteditable` or platform text views, canvist owns every pixel.

## Features

- 🎨 **Custom render engine** — platform-agnostic rendering via canvas APIs
- 🤝 **Real-time collaboration** — built-in CRDT support via [Yjs](https://yjs.dev)
- ♿ **Full accessibility** — parallel hidden DOM for screen readers (like Google Docs)
- 🌐 **Cross-platform** — WebAssembly, native, and server-side rendering
- ✨ **Beautiful API** — ergonomic builder patterns for documents, styles, and operations

## Architecture

```text
┌─────────────────────────────────────────────┐
│                  canvist                     │  ← Umbrella crate
├──────────────┬──────────────┬───────────────┤
│ canvist_core │canvist_render│ canvist_wasm  │
│              │              │               │
│ • Document   │ • Canvas     │ • Canvas2D    │
│ • Selection  │ • Renderer   │ • DOM events  │
│ • Operations │ • Viewport   │ • A11y DOM    │
│ • Style      │ • FontCache  │ • JS bridge   │
│ • Layout     │              │               │
│ • CRDT sync  │              │               │
└──────────────┴──────────────┴───────────────┘
```

| Crate | Description |
|-------|-------------|
| [`canvist`](./crates/canvist) | Umbrella re-export crate |
| [`canvist_core`](./crates/canvist_core) | Document model, operations, selections, CRDT collaboration |
| [`canvist_render`](./crates/canvist_render) | Platform-agnostic rendering traits and text layout |
| [`canvist_wasm`](./crates/canvist_wasm) | WebAssembly + Canvas2D rendering backend |
| [`canvist_test`](./crates/canvist_test) | Playwright browser integration tests |

## Quick start

```rust
use canvist::prelude::*;

// Create a new document.
let mut doc = Document::new();

// Insert styled text.
let style = Style::new().bold().font_size(24.0).font_family("Inter");
doc.insert_text(Position::zero(), "Hello, canvist!");
doc.apply_style(Selection::all(&doc), &style);
```

## Contributing

[`devenv`](https://devenv.sh/) is used to provide a reproducible development environment. Follow the [getting started instructions](https://devenv.sh/getting-started/).

```bash
# Allow direnv to load the environment.
direnv allow .

# Install dependencies.
install:all

# Build everything.
build:all

# Run tests.
test:all

# Fix formatting and lints.
fix:all
```

See the [contributing guide](./docs/src/contributing.md) for more details.

## License

This project is licensed under the [Unlicense](https://unlicense.org/).
