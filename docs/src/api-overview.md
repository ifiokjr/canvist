# API Overview

This document defines the public programming model used by both human-driven applications and autonomous agents.

The model is intentionally layered:

1. **Intent/Event** — what the user/agent wants to achieve
2. **Action envelope** — a concrete, policy-checked command + metadata
3. **Operation** — deterministic, atomic document mutation
4. **State transition** — verified application of operations/log entries
5. **Render delta** — visual updates derived from the new state

Keeping these layers explicit makes replay, undo/redo, automation, and cross-platform behavior predictable.

## Capability-to-test matrix (maintained)

Use this matrix to keep documentation claims and automated coverage aligned.  
Status legend: ✅ covered by automated tests, ⚠️ partially covered, ❌ no automated coverage yet.

| Capability (documented/claimed) | Primary source | Automated tests | Status | Notes / gap |
| --- | --- | --- | --- | --- |
| Browser editor boots and canvas renders | `packages/canvist/README.md`, architecture layering | `packages/canvist/tests/playwright/editor.spec.ts` (`[browser] editor loads and renders`) | ✅ | Runs across Chromium/Firefox/WebKit via `CI_BROWSERS`/default matrix. |
| Text insertion from keyboard input | API layer model (`EditorEvent::TextInsert`) + README behavior | `packages/canvist/tests/playwright/editor.spec.ts` (`typing inserts text`, `multiple sequential inserts`) | ✅ | Asserts plain text + char count. |
| Backspace behavior (`delete_backward`) | Action intent examples in architecture/API docs | `packages/canvist/tests/playwright/editor.spec.ts` (`backspace deletes characters`) | ✅ | Validates character removal at cursor end. |
| Forward delete behavior (`delete` key) | Playwright suite contract comment, keyboard parity expectations | `packages/canvist/tests/playwright/editor.spec.ts` (`delete removes character in front of cursor`) | ✅ | Validates mid-string delete after cursor movement. |
| Arrow-key cursor navigation | Keyboard parity expectations in tests/docs | `packages/canvist/tests/playwright/editor.spec.ts` (`arrow keys move cursor for deterministic mid-string insert`) | ✅ | Behavior verified through deterministic insertion point. |
| Enter/newline insertion | Basic editor behavior in README/examples | `packages/canvist/tests/playwright/editor.spec.ts` (`enter creates newline`) | ✅ | Asserts both lines present in resulting plain text. |
| Range selection and replace (shift+arrow + typing) | Selection model in architecture/API docs | `packages/canvist/tests/playwright/editor.spec.ts` (`shift+arrow selection replaced by typing`) | ✅ | Confirms keyboard-created selection replacement path. |
| `initWasm` idempotent initialization | Public API expectations (`src/mod.ts`/README) | `packages/canvist/tests/api_contract_test.ts` (`initWasm is idempotent across repeated calls`) | ✅ | Also checks exported `CanvistEditor` constructor exists. |
| `initWasm` failure surfacing | Robust API contract expectations | `packages/canvist/tests/api_contract_test.ts` (`initWasm surfaces module-load failures`), fixture `packages/canvist/tests/fixtures/wasm_fail.ts` | ✅ | Ensures import/initialization failures reject instead of hanging/silent fail. |
| `createEditor` browser API availability | README API contract | _No stable automated assertion in Deno unit tests_ | ⚠️ | Existing attempts require DOM/runtime browser harness. Keep validation in Playwright/browser context. |
| `setTitle()` API contract | README/API promises | _No active test currently in repo_ | ❌ | Add Deno unit or browser API contract test once stable import/runtime boundary agreed. |
| `toJSON()` API contract/schema | README/API promises | _No active test currently in repo_ | ❌ | Add schema/shape assertion test for deterministic serialization contract. |
| `destroy()` cleanup contract | README/API promises | _No active test currently in repo_ | ❌ | Add lifecycle cleanup test (event listeners/resources/input detachment). |
| Clipboard operations (copy/cut/paste) | Canonical event pipeline includes clipboard | _None_ | ❌ | Add Playwright clipboard interaction tests (permissions + keyboard shortcuts/context menu). |
| IME/composition input | Canonical event pipeline includes composition | _None_ | ❌ | Add browser integration tests for compositionstart/update/end paths. |
| Accessibility shadow DOM + focus semantics | `canvist_wasm` architecture section | _None_ | ❌ | Add assertions for a11y DOM synchronization, roles/labels, focus handoff. |
| Runtime action-envelope/policy/precondition pipeline | Architecture + API layers 2–4 | _None in `packages/canvist/tests`_ | ❌ | Requires core/runtime-level deterministic transition tests. |
| Operation log replay guarantees | API Layer 4 contract | _None in `packages/canvist/tests`_ | ❌ | Add replay/divergence tests around `OperationLog` preconditions. |
| Render-delta/invalidation contract | API Layer 5 contract | _None_ | ❌ | Need backend-visible invalidation assertions beyond “canvas visible”. |
| Collaboration/CRDT sync path | Architecture (`canvist_core` CRDT sync) | _None_ | ❌ | Add multi-actor convergence tests once web sync harness exists. |

### Maintenance checklist

When adding/changing functionality:

1. Update this matrix row (or add a new row) in the same PR.
2. Link concrete test file(s) and test name(s) that verify the capability.
3. If a capability remains uncovered, mark as ❌/⚠️ and include a follow-up task reference.
4. Prefer browser-level tests for DOM/input behavior and unit tests for pure API contracts.

## Layer 0: Document state

The `Document` is the canonical editor state. It holds a tree of nodes representing rich text content.

```rust
use canvist_core::Document;
use canvist_core::Position;

let mut doc = Document::new();
doc.set_title("My Document");
doc.insert_text(Position::zero(), "Hello, world!");

assert_eq!(doc.plain_text(), "Hello, world!");
```

## Layer 1: Intent/Event API (human or agent)

An **intent/event** is high-level and goal-oriented. Examples:

- `EditorEvent::TextInsert` from keyboard/IME
- “Toggle bold for current selection” from command palette
- “Apply remote patch entry #42” from sync pipeline

At this layer, payloads may still be ambiguous (e.g. “at cursor” needs resolved position) and must not mutate state directly.

## Layer 2: Action envelope API (validated commands)

An **action envelope** is an intent transformed into a fully-specified command after validation and policy checks.

Action metadata contract:

- `action_id` (`ActionId`) — stable command identity
- `actor` (`ActorId`) — user/agent/service identity
- `intent` (`ActionIntent`) — semantic category
- `args` (`ActionArgs`) — fully-resolved concrete parameters
- timestamps/logical metadata — ordering + observability

Typical validation/normalization includes:

- resolving symbolic targets (`cursor`, `selection`, `document-start`)
- bounds checks for positions/ranges
- feature/policy checks (allowed style changes, read-only regions)
- idempotency tagging (request IDs, actor/session IDs)

Actions should compile to one or more core operations.

### Usage example: event → action envelope → transaction/log/apply

```rust
use canvist_core::{Document, EditorEvent, EventSource};
use canvist_core::operation::{Operation, Transaction, LogEntry, OperationLog};

struct VecEventSource {
    events: std::vec::IntoIter<EditorEvent>,
}

impl EventSource for VecEventSource {
    fn poll_event(&mut self) -> Option<EditorEvent> {
        self.events.next()
    }
}

let mut doc = Document::new();
let mut source = VecEventSource {
    events: vec![EditorEvent::TextInsert { text: "Hello".into() }].into_iter(),
};

// 1) Pull normalized event.
let event = source.poll_event().expect("event");

// 2) Build validated action envelope (app/runtime layer).
let action_id = "act-1";
let actor = "agent:demo";

// 3) Compile action args into canonical operations/transaction.
let tx = match event {
    EditorEvent::TextInsert { text } => Transaction::new()
        .push(Operation::insert(canvist_core::Position::zero(), text)),
    _ => Transaction::new(),
};

// 4) Apply deterministically.
tx.apply(&mut doc);

// 5) Persist replay log envelope.
let entry = LogEntry::new("op-1", 1, 1_700_000_000_000, actor, Operation::insert(canvist_core::Position::zero(), "Hello"));
let log = OperationLog::new().push(entry);

assert_eq!(action_id, "act-1");
assert_eq!(doc.plain_text(), "Hello");
assert_eq!(log.entries().len(), 1);
```

This concise envelope keeps UI-driven and agent-driven edits on the same deterministic path.

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

## Layer 4: Deterministic state transition (operation log)

For replay, undo/redo, and agent workflows, wrap operations in immutable `LogEntry` envelopes and append them to an `OperationLog`.

Each `LogEntry` includes:

- `op_id`: stable operation identifier
- `logical_clock`: deterministic ordering key
- `timestamp_ms`: wall-clock metadata
- `actor`: author/session identity
- `preconditions`: expected pre-state (e.g. `state_hash`, `char_count`)
- `recovery`: optional inverse operation or checkpoint reference

Replay guarantees:

- preconditions are checked before each operation
- divergence fails fast instead of silently corrupting state
- same ordered log + same initial state ⇒ same result

## Layer 5: Render delta contract

Rendering is derived from document state and layout, not ad-hoc UI side effects.

Expected flow:

1. Apply operation/transaction/log entry to `Document`
2. Recompute affected layout regions
3. Emit render invalidation/delta to renderer backend
4. Backend draws updated viewport

For agents, success criteria can include both state assertions (hash/text/selection) and render assertions (changed ranges/repaint regions).
