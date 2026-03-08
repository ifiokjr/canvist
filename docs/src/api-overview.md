# API Overview

This document defines the public programming model used by both human-driven
applications and autonomous agents.

The model is intentionally layered:

1. **Intent** — what the user/agent wants to achieve
2. **Action (validated command)** — a concrete, policy-checked command
3. **Operation** — deterministic, atomic document mutation
4. **State transition** — verified application of operations/log entries
5. **Render delta** — visual updates derived from the new state

Keeping these layers explicit makes replay, undo/redo, automation, and
cross-platform behavior predictable.

## Layer 0: Document state

The `Document` is the canonical editor state. It holds a tree of nodes
representing rich text content.

```rust
use canvist_core::Document;
use canvist_core::Position;

let mut doc = Document::new();
doc.set_title("My Document");
doc.insert_text(Position::zero(), "Hello, world!");

assert_eq!(doc.plain_text(), "Hello, world!");
```

`Document` is the source of truth for:

- content (`plain_text`, node tree)
- metadata (title, styles)
- deterministic checks (`state_hash`, char count preconditions)

## Layer 1: Intent API (human or agent)

An **intent** is high-level and goal-oriented. Examples:

- “Insert `Hello` at cursor”
- “Toggle bold for current selection”
- “Delete previous word”
- “Apply remote patch entry #42”

Intents can be produced by:

- UI events (keyboard, IME, pointer, command palette)
- application integrations
- autonomous agents/planners

At this layer, payloads may still be ambiguous (e.g. “at cursor” needs a
resolved position) and must not mutate state directly.

## Layer 2: Action API (validated commands)

An **action** is an intent transformed into a fully-specified command after
validation and policy checks.

Typical validation/normalization includes:

- resolving symbolic targets (`cursor`, `selection`, `document-start`)
- bounds checks for positions/ranges
- feature/policy checks (allowed style changes, read-only regions)
- idempotency tagging (request IDs, actor/session IDs)

Recommended command envelope fields:

- `action_id` (stable command ID)
- `actor` (user/agent/service identity)
- `intent_type` (semantic category)
- `resolved_args` (fully concrete params)
- `timestamp_ms`

Actions should compile to one or more core operations.

## Layer 3: Operation API (deterministic mutations)

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

Contract:

- operations are minimal and explicit (`Insert`, `Delete`, `Format`)
- operation semantics are platform-independent
- transaction ordering is deterministic

## Layer 4: Deterministic state transition (operation log)

For replay, undo/redo, and agent workflows, wrap operations in immutable
`LogEntry` envelopes and append them to an `OperationLog`.

Each `LogEntry` includes:

- `op_id`: stable operation identifier
- `logical_clock`: deterministic ordering key
- `timestamp_ms`: wall-clock metadata
- `actor`: author/session identity
- `preconditions`: expected pre-state (e.g. `state_hash`, `char_count`)
- `recovery`: optional inverse operation or checkpoint reference

```rust
use canvist_core::{Document, Position};
use canvist_core::operation::{
    LogEntry, Operation, OperationLog, Preconditions, RecoveryRef,
};

let mut doc = Document::new();

let pre = Preconditions {
    expected_document_hash: Some(doc.state_hash()),
    expected_char_count: Some(doc.char_count()),
};

let entry = LogEntry::new(
    "op-1",
    1,
    1_700_000_000_000,
    "agent:planner",
    Operation::insert(Position::zero(), "Hello"),
)
.with_preconditions(pre)
.with_recovery(RecoveryRef::Inverse(Operation::delete(
    canvist_core::Selection::range(Position::zero(), Position::new(5)),
)));

let log = OperationLog::new().push(entry);
log.replay(&mut doc).expect("preconditions hold");
```

Replay guarantees:

- preconditions are checked before each operation
- divergence fails fast instead of silently corrupting state
- same ordered log + same initial state ⇒ same result

## Layer 5: Render delta contract

Rendering is derived from document state and layout, not from ad-hoc UI side
effects.

Expected flow:

1. Apply operation/transaction/log entry to `Document`
2. Recompute affected layout regions
3. Emit render invalidation/delta to renderer backend
4. Backend draws updated viewport

For agent tooling, this means success criteria can include both:

- state assertions (hash/text/selection)
- render assertions (changed ranges/viewport repaint regions)

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

When integrating collaboration with operation logs, treat remote updates as
inputs that must still pass through action/operation/state-transition contracts
before local rendering.
