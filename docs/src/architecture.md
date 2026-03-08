# Architecture

canvist is organised into layered crates, each with a clear responsibility:

```text
┌─────────────────────────────────────────────┐
│                  canvist                     │  ← Umbrella re-export crate
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

## canvist_core

The core crate is platform-agnostic. It contains:

- **Document model** — a tree of nodes (root → paragraphs → text runs)
- **Selections** — cursor positions and range selections
- **Operations** — atomic edit operations with transaction batching
- **Style** — composable text styling with builder pattern
- **Layout** — line breaking and paragraph layout computation
- **Collaboration** — Yjs CRDT integration for real-time sync

## canvist_render

Defines abstract rendering traits that platform backends implement:

- `Canvas` — fill rects, draw text, draw lines, clip, transform
- `Renderer` — manages a viewport and implements Canvas
- `FontCache` — font loading and glyph caching via fontdue

## canvist_wasm

The WebAssembly backend that runs in the browser:

- Implements `Renderer` using `CanvasRenderingContext2d`
- Handles DOM events (keyboard, mouse, IME)
- Generates an accessibility shadow DOM
- Exposes a `CanvistEditor` class to JavaScript via wasm-bindgen
