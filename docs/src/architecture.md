# Architecture

canvist is organised into layered crates, each with a clear responsibility:

```text
┌────────────────────────────────────────────────────────────────────────────┐
│                              App / Agent Layer                            │
│  User UI, macros, autonomous planners, external automations               │
└────────────────────────────────────────────────────────────────────────────┘
                                    │ intents / events
                                    ▼
┌────────────────────────────────────────────────────────────────────────────┐
│                         Runtime / Action Layer                            │
│  EditorRuntime, action envelopes, policy checks, command resolution       │
└────────────────────────────────────────────────────────────────────────────┘
                                    │ transactions / log entries
                                    ▼
┌─────────────────────────────────────────────┐
│                  canvist                     │  ← Umbrella re-export crate
├──────────────┬──────────────┬───────────────┤
│ canvist_core │canvist_render│ canvist_wasm  │
│              │              │               │
│ • Document   │ • Canvas     │ • Canvas2D    │
│ • Selection  │ • Renderer   │ • DOM events  │
│ • Operations │ • Viewport   │ • A11y DOM    │
│ • Event API  │ • FontCache  │ • JS bridge   │
│ • Op Log     │              │               │
│ • Style      │              │               │
│ • Layout     │              │               │
│ • CRDT sync  │              │               │
└──────────────┴──────────────┴───────────────┘
                                    │ render deltas
                                    ▼
                           Platform drawing surfaces
```

## End-to-end contract (intent → action → operation → render)

Every edit path (human or agent) should follow this lifecycle:

1. **Intent/Event capture**
   - Source: DOM/native events, command UI, API call, automation agent
   - Output: normalized `EditorEvent` or high-level intent
2. **Action resolution**
   - Validate permissions/policies and resolve concrete targets
   - Output: deterministic validated action envelope (`Action` + `ActionMeta`)
3. **Operation generation**
   - Translate one action into one or more ordered core `Operation`s
   - Optionally package in `Transaction`
4. **Deterministic state transition**
   - Wrap operations in `LogEntry` and append to `OperationLog`
   - Enforce preconditions (`state_hash`, char_count, etc.)
   - Apply to `Document` in stable logical-clock order
5. **Render delta + paint**
   - Compute layout/viewport invalidation from changed state
   - Render backend repaints only affected regions

This contract is the shared API boundary for product code, tests, and agents.

## Concrete runtime pipeline (`EditorRuntime`)

`EditorRuntime` is the orchestration point for this contract. It coordinates event intake, action validation, operation application, logging, and invalidation.

Runtime I/O shape:

- **Input**: `EditorEvent` stream via `EventSource::poll_event`
- **Envelope**: `Action` with `ActionMeta` (`action_id`, `actor`, `intent`, timestamps)
- **Output**: `RuntimeOutput` containing applied actions + invalidation metadata

Concise flow:

1. Poll `EditorEvent` values from `EventSource`.
2. Map each event/command to a fully-resolved `Action` envelope.
3. Validate policy + preconditions.
4. Compile into `Transaction` / `Operation` values.
5. Apply to `Document` and append `LogEntry` to `OperationLog`.
6. Emit `Invalidation` for render backends.

By keeping envelopes explicit, wasm/native adapters stay thin while deterministic replay remains intact.

## Action envelope (developer-facing contract)

An action envelope should be treated as the durable boundary between intent and mutation:

- `ActionId` — stable identity for dedupe/tracing
- `ActorId` — user/agent/service identity
- `ActionIntent` — semantic category (`insert_text`, `delete_backward`, `toggle_bold`, ...)
- `ActionArgs` — concrete resolved payload (no symbolic placeholders)
- temporal/order metadata — logical ordering + observability fields

Only envelopes that pass validation can produce operations.

## canvist_core

The core crate is platform-agnostic. It contains:

- **Document model** — a tree of nodes (root → paragraphs → text runs)
- **Selections** — cursor positions and range selections
- **Operations** — atomic edit operations with transaction batching
- **Operation log** — immutable replay envelope for deterministic application
- **Event model** — canonical `EditorEvent` and `EventSource`
- **Style** — composable text styling with builder pattern
- **Layout** — line breaking and paragraph layout computation
- **Collaboration** — Yjs CRDT integration for real-time sync

Core invariants:

- same starting state + same ordered log = same resulting state
- failed preconditions reject a transition instead of applying partial edits
- operations are the only mutation path into `Document`

## canvist_render

Defines abstract rendering traits that platform backends implement:

- `Canvas` — fill rects, draw text, draw lines, clip, transform
- `Renderer` — manages a viewport and implements Canvas
- `FontCache` — font loading and glyph caching via fontdue

Render contract:

- consume already-validated state/layout results
- do not mutate core document state
- provide deterministic draw behavior for a given layout snapshot

## canvist_wasm

The WebAssembly backend that runs in the browser:

- Implements `Renderer` using `CanvasRenderingContext2d`
- Maps hidden-input + DOM events into canonical `EditorEvent` values via `WebEventSource`
- Generates an accessibility shadow DOM
- Exposes a `CanvistEditor` class to JavaScript via wasm-bindgen

## Canonical editor event pipeline

Before any edit operations are produced, every platform normalizes raw input into a shared core event model (`canvist_core::event`):

- `EditorEvent` — canonical user intent (text insert/delete, key, pointer, composition, selection, clipboard, focus)
- `EventSource` — trait for polling normalized events from a platform adapter

This enables web, mobile, and desktop backends to converge on one input contract:

1. Platform APIs emit native events (DOM events, hidden input deltas, UIKit, Android, desktop window events)
2. Backend-specific adapters map those into `EditorEvent`
3. Runtime resolves events into action envelopes
4. Actions compile into operations/log entries
5. Renderer paints deltas from new state

Current adapters include:

- `WebEventSource` (`canvist_wasm::dom`) for DOM + hidden-input integration
- `NativeEventSource` (`canvist_wasm::dom`) as a reference shape for mobile event mapping into the same canonical stream
