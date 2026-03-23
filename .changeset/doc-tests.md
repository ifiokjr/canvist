---
"canvist_core": patch
---

Add doc tests for core public API

Added runnable doc examples to key public methods:

- `Document::insert_text` — basic insertion and paragraph splitting
- `Document::delete` — range deletion
- `Document::apply_style` — partial-run formatting with run splitting
- `Document::from_markdown` — Markdown import with styled runs
- `EditorRuntime::new` — runtime creation, text insert, and undo
- `Operation::insert` — create and apply an insert operation
- `Operation::delete` — create and apply a delete operation
- `Operation::format` — create and apply a format operation
- `CollaborationSession` — multi-peer sync example

Doc test count: 9 → 18 (+9 new).
