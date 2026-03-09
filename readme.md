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

| Crate                                       | Description                                                |
| ------------------------------------------- | ---------------------------------------------------------- |
| [`canvist`](./crates/canvist)               | Umbrella re-export crate                                   |
| [`canvist_core`](./crates/canvist_core)     | Document model, runtime/action envelope, deterministic operation log, CRDT collaboration |
| [`canvist_render`](./crates/canvist_render) | Platform-agnostic rendering traits and text layout         |
| [`canvist_wasm`](./crates/canvist_wasm)     | WebAssembly + Canvas2D rendering backend                   |
| [`canvist_test`](./crates/canvist_test)     | Playwright browser integration tests                       |

## Runtime/API model

canvist uses a deterministic, layered runtime contract:

1. **Intent** — user or extension intent (e.g. "insert text").
2. **Action envelope** — normalized `EditorEvent` + `EventSource` context.
3. **Operation transaction** — validated `Operation`s grouped into a `Transaction`.
4. **Log + render** — append to `OperationLog`, then re-render from document state.

This keeps input handling, collaboration, undo/redo, and rendering on the same pipeline.

See [Architecture](./docs/src/architecture.md) and [API Overview](./docs/src/api-overview.md) for the full model.

## Quick start (runtime-first)

```rust
use canvist::core::{Document, EditorEvent, EditorRuntime, Position, Selection};

let mut runtime = EditorRuntime::new(
    Document::new(),
    Selection::collapsed(Position::zero()),
    "user:demo",
);

runtime
    .handle_event(EditorEvent::TextInsert {
        text: "Hello, canvist!".to_string(),
    })
    .expect("event should be accepted");

assert_eq!(runtime.document().plain_text(), "Hello, canvist!");

// Deterministic replay on a fresh document.
let log = runtime.export_log();
let mut replayed = Document::new();
log.replay(&mut replayed).expect("replay should succeed");
assert_eq!(replayed.plain_text(), "Hello, canvist!");
```

If you only need low-level document manipulation, you can still work directly with `Document`, `Selection`, and `Style` APIs.

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
