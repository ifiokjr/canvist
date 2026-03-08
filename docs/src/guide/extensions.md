# Extensions and Plugins (Deterministic Model)

This guide proposes a TipTap-like extension surface for canvist that remains fully deterministic and replayable from an operation log.

## Current public API surface

Today, the public API is intentionally small:

- `canvist` umbrella crate re-exports:
  - `canvist::core` (`canvist_core`)
  - `canvist::render` (`canvist_render`)
  - `canvist::wasm` (feature-gated)
  - `canvist::prelude::{Document, NodeId, Position, Selection, Style, Canvas, Renderer, Viewport}`
- `canvist_core` exports document + editing primitives:
  - `Document`, `Node`, `NodeId`, `NodeKind`
  - `Position`, `Selection`
  - `Style` (+ `Color`, `FontWeight`)
  - `operation::{Operation, Transaction}`
  - `event::{EditorEvent, EventSource, EditorKey, PointerEvent, ...}`
  - `collaboration::CollaborationSession`

This gives a strong foundation, but there is no first-class extension registry, command abstraction, or plugin hooks yet.

## Design goals for extension model

A plugin system should be:

1. **Deterministic**: same initial state + same ordered intent stream => same result.
2. **Replayable**: every accepted change is representable as canonical operations.
3. **Side-effect isolated**: extension logic can inspect context, but persisted changes must flow through transactions.
4. **Composable**: many extensions can coexist with predictable ordering.

## Proposed core concepts

### 1) Command layer

Commands are high-level actions that synthesize one or more operations.

```rust
pub struct CommandContext<'a> {
	pub doc: &'a Document,
	pub selection: Selection,
	pub event: Option<&'a EditorEvent>,
}

pub trait Command: Send + Sync {
	fn name(&self) -> &'static str;
	fn execute(&self, ctx: &CommandContext<'_>) -> Option<Transaction>;
}
```

Rules:

- Commands are pure decision functions over context.
- Output is `Transaction` only (no direct document mutation).
- Runtime applies returned transaction through one canonical pipeline.

### 2) Schema-like extension specs

Extensions declare document capabilities and behavior slices.

```rust
pub trait Extension: Send + Sync {
	fn name(&self) -> &'static str;

	// Optional command registrations
	fn commands(&self) -> Vec<Box<dyn Command>> {
		vec![]
	}

	// Optional text/input rules
	fn input_rules(&self) -> Vec<Box<dyn InputRule>> {
		vec![]
	}

	// Optional transaction hooks
	fn transaction_hooks(&self) -> Vec<Box<dyn TransactionHook>> {
		vec![]
	}
}
```

This is analogous to TipTap’s extension packs while preserving Rust trait-based typing.

### 3) Input rules

Input rules map recent text/event context into canonical transactions.

```rust
pub trait InputRule: Send + Sync {
	fn name(&self) -> &'static str;
	fn apply(
		&self,
		doc: &Document,
		selection: Selection,
		event: &EditorEvent,
	) -> Option<Transaction>;
}
```

Examples:

- `"-- "` at paragraph start => em dash or horizontal rule insertion transaction.
- Markdown-like `"# "` => heading style transaction.

### 4) Transaction hooks (pre/post)

Hooks can validate, annotate, or derive metadata from transactions.

```rust
pub struct TransactionMeta {
	pub origin: &'static str, // e.g. "keyboard", "command.toggle_bold"
	pub timestamp_ms: u64,
}

pub trait TransactionHook: Send + Sync {
	fn before_apply(
		&self,
		tx: &Transaction,
		meta: &TransactionMeta,
		doc: &Document,
	) -> HookDecision {
		HookDecision::Accept
	}

	fn after_apply(&self, _tx: &Transaction, _meta: &TransactionMeta, _doc: &Document) {}
}

pub enum HookDecision {
	Accept,
	Reject { reason: String },
	Replace(Transaction),
}
```

Determinism constraints:

- `before_apply` must not depend on wall-clock/network randomness unless captured in `meta`.
- `Replace` must produce canonical operations only.

## Deterministic replay contract

To guarantee replay:

- Persist **ordered transaction log** as `(meta, transaction)`.
- Rehydrate by applying each transaction in order on a fresh document.
- Extensions may assist generation-time decisions, but persisted artifact stays `Transaction`.

In other words: extensions influence _how we create ops_, never the shape of replay runtime.

## Suggested runtime pipeline

1. Normalize platform input into `EditorEvent`.
2. Run extension input rules in deterministic priority order.
3. Optionally run a bound command.
4. Produce candidate `Transaction`.
5. Run `before_apply` hooks (ordered).
6. Apply final transaction to `Document`.
7. Append `(meta, transaction)` to log.
8. Emit to collaboration bridge / CRDT adapter.
9. Run `after_apply` hooks.

## Ordering and priority

Each extension should declare:

- `priority: i32` (higher first)
- stable registration `name`

Tie-break on name for deterministic ordering.

## Interop with collaboration

Current `CollaborationSession` is plain-text oriented. For extension-safe replay, collaboration should transport canonical transactions (or a lossless projection) instead of extension callbacks.

Until tree-CRDT mapping exists, keep collaboration boundary explicit:

- local extension logic => `Transaction`
- sync layer => diff/update encoding of that transaction
- remote apply => same transaction semantics

## Migration plan (incremental)

1. Add command/input-rule/hook traits in `canvist_core`.
2. Introduce an `EditorRuntime` coordinator to host extension registry.
3. Keep existing `Document` mutators as low-level primitives used by runtime.
4. Add transaction metadata + persistent log API.
5. Bridge transaction log with collaboration codec.

This sequence keeps API growth additive while preserving today’s simple model.
