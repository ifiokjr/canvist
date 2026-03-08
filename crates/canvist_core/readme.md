# canvist_core

> Core document model, deterministic runtime pipeline primitives, editor operations, and CRDT collaboration for the canvist canvas editor.

This crate provides the foundational data structures and algorithms for building a canvas-based text editor:

- **Document model** — Hierarchical node tree with paragraphs, text runs, and inline elements
- **Selections** — Cursor positioning, range selections, and multi-cursor support
- **Events** — Canonical `EditorEvent` + `EventSource` abstraction for platform adapters
- **Action pipeline contract** — Intent/event → validated action envelope → operations/transactions
- **Operations** — Insert, delete, and format operations with transaction + replay support
- **Operation log** — `LogEntry` and `OperationLog` for deterministic replay/precondition checks
- **Styling** — Rich text styling (font, size, weight, color, decoration)
- **Layout** — Text layout computation with Unicode-aware line breaking
- **Collaboration** — Real-time collaboration via Yjs CRDTs
