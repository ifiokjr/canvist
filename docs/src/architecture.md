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
- Maps hidden-input + DOM events into canonical `EditorEvent` values via `WebEventSource`
- Generates an accessibility shadow DOM
- Exposes a `CanvistEditor` class to JavaScript via wasm-bindgen

## Canonical editor event pipeline

Before any edit operations are produced, every platform normalizes raw input
into a shared core event model (`canvist_core::event`):

- `EditorEvent` — canonical user intent (text insert/delete, key, pointer,
  composition, selection, clipboard, focus)
- `EventSource` — trait for polling normalized events from a platform adapter

This enables web, mobile, and desktop backends to converge on one input
contract:

1. Platform APIs emit native events (DOM events, hidden input deltas, UIKit,
   Android, desktop window events)
2. Backend-specific adapters map those into `EditorEvent`
3. Core editor logic consumes `EventSource` and translates events into
   operations/transactions

Current adapters include:

- `WebEventSource` (`canvist_wasm::dom`) for DOM + hidden-input integration
- `NativeEventSource` (`canvist_wasm::dom`) as a reference shape for mobile
  event mapping into the same canonical stream
