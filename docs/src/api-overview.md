# API Overview

## Document

The `Document` is the central data structure. It holds a tree of nodes representing rich text content.

```rust
use canvist_core::Document;
use canvist_core::Position;

let mut doc = Document::new();
doc.set_title("My Document");
doc.insert_text(Position::zero(), "Hello, world!");

assert_eq!(doc.plain_text(), "Hello, world!");
```

## Style

Styles are composable via a fluent builder pattern:

```rust
use canvist_core::Style;

let heading = Style::new()
    .bold()
    .font_size(24.0)
    .font_family("Inter")
    .color(26, 26, 46, 255);
```

Styles can be merged — fields from the overlay take priority:

```rust
use canvist_core::Style;

let base = Style::new().font_size(16.0).bold();
let overlay = Style::new().italic().font_size(24.0);
let merged = base.merge(&overlay);

// merged is: bold + italic + font_size(24.0)
```

## Selection

Selections model both cursors (collapsed) and highlighted ranges:

```rust
use canvist_core::{Position, Selection};

// A blinking caret at position 5.
let cursor = Selection::collapsed(Position::new(5));

// A range from 2 to 10.
let range = Selection::range(Position::new(2), Position::new(10));
assert_eq!(range.len(), 8);
```

## Operations

Operations are atomic edits that can be batched into transactions:

```rust
use canvist_core::{Document, Position, Selection, Style};
use canvist_core::operation::{Operation, Transaction};

let mut doc = Document::new();

let tx = Transaction::new()
    .push(Operation::insert(Position::zero(), "Hello, world!"))
    .push(Operation::format(
        Selection::range(Position::zero(), Position::new(5)),
        Style::new().bold(),
    ));

tx.apply(&mut doc);
```

## Collaboration

Real-time sync is built on Yjs CRDTs:

```rust
use canvist_core::collaboration::CollaborationSession;

let peer_a = CollaborationSession::new();
let peer_b = CollaborationSession::new();

peer_a.insert(0, "Hello");

// Sync A → B.
let update = peer_a.encode_state();
peer_b.apply_update(&update);

assert_eq!(peer_b.text(), "Hello");
```
