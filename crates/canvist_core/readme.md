# canvist_core

> Core document model, editor operations, and CRDT collaboration for the canvist canvas editor.

This crate provides the foundational data structures and algorithms for building a canvas-based text editor:

- **Document model** — Hierarchical node tree with paragraphs, text runs, and inline elements
- **Selections** — Cursor positioning, range selections, and multi-cursor support
- **Operations** — Insert, delete, format, split, and merge operations with undo/redo
- **Styling** — Rich text styling (font, size, weight, color, decoration)
- **Layout** — Text layout computation with Unicode-aware line breaking
- **Collaboration** — Real-time collaboration via Yjs CRDTs
