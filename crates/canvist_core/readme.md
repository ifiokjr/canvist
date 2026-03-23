# canvist_core

> Core document model, runtime, operations, and CRDT collaboration for the canvist canvas editor.

## Features

- **Document model** — paragraph tree with styled text runs
- **Runtime** — event processing, undo/redo with coalescing, cursor management
- **Operations** — insert, delete, format with transaction batching and replay
- **Styling** — bold, italic, underline, strikethrough, font size, color, alignment
- **Layout** — Unicode-aware line breaking with dynamic line height
- **Collaboration** — Yrs CRDT integration for real-time multi-peer sync
- **Import/Export** — HTML, Markdown, and JSON serialization

## Quick start

```rust
use canvist_core::{Document, EditorRuntime, EditorEvent, Position, Selection, Style};

// Create a runtime.
let mut runtime = EditorRuntime::new(
    Document::new(),
    Selection::collapsed(Position::zero()),
    "user:demo",
);

// Insert and format text.
runtime.handle_event(EditorEvent::TextInsert {
    text: "Hello, world!".to_string(),
}).unwrap();

runtime.apply_operation(canvist_core::operation::Operation::format(
    Selection::range(Position::new(0), Position::new(5)),
    Style::new().bold(),
));

assert_eq!(runtime.document().plain_text(), "Hello, world!");

// Export.
let html = runtime.document().to_html();
let md = runtime.document().to_markdown();

// Undo.
runtime.undo();
```

## Collaboration

```rust
use canvist_core::collaboration::CollaborationSession;

let peer_a = CollaborationSession::new();
let peer_b = CollaborationSession::new();

peer_a.insert(0, "Hello");
let update = peer_a.encode_state();
peer_b.apply_update(&update);

assert_eq!(peer_b.text(), "Hello");
```

## License

[Unlicense](https://unlicense.org/)
