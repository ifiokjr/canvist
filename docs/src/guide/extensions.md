# Extensions and Plugins (Deterministic Runtime Model)

canvist exposes a deterministic extension model inspired by editors like TipTap:

- extensions can react to events
- extensions can synthesize transactions
- only canonical operations/transactions mutate the `Document`

This keeps replay and collaboration reliable.

## Current extension API surface

`canvist_core` currently exposes first-class extension/runtime types:

- `Extension`, `Command`, `InputRule`, `TransactionHook`
- `ExtensionRuntime`, `RuntimeTransaction`, `TransactionMeta`
- `EditorRuntime` (event/action/operation orchestration)
- `Action`, `ActionMeta`, `ActionIntent`, `ActionArgs`

Core mutation/replay contracts remain:

- `Operation`, `Transaction`
- `LogEntry`, `OperationLog`

## Deterministic runtime pipeline

Use this pipeline for extension-safe behavior:

1. Platform input is normalized into `EditorEvent`.
2. Runtime validates and envelopes event as an `Action`.
3. Runtime resolves built-in action mappings and extension rules/commands in deterministic order.
4. Runtime applies final `Transaction` to `Document`.
5. Runtime appends `LogEntry` values for deterministic replay.
6. Runtime emits invalidation output for rendering.

## Minimal extension example

```rust
use canvist_core::{
    Document, EditorEvent, EditorRuntime, Extension, ExtensionRuntime,
    Position, Selection, Command,
};
use canvist_core::operation::{Operation, Transaction};

struct InsertHello;
impl Command for InsertHello {
    fn id(&self) -> &'static str { "insert_hello" }

    fn execute(
        &self,
        _doc: &Document,
        _selection: Selection,
        event: &EditorEvent,
    ) -> Option<Transaction> {
        match event {
            EditorEvent::Focus => Some(
                Transaction::new().push(Operation::insert(Position::zero(), "Hello"))
            ),
            _ => None,
        }
    }
}

struct HelloExtension;
impl Extension for HelloExtension {
    fn id(&self) -> &'static str { "hello" }

    fn commands(&self) -> Vec<Box<dyn Command>> {
        vec![Box::new(InsertHello)]
    }
}

let mut runtime = EditorRuntime::new(
    Document::new(),
    Selection::collapsed(Position::zero()),
    "user:demo",
)
.with_extensions(ExtensionRuntime::new(vec![Box::new(HelloExtension)]));

runtime.handle_event(EditorEvent::Focus).expect("focus handled");
assert_eq!(runtime.document().plain_text(), "Hello");
assert_eq!(runtime.export_log().entries().len(), 1);
```

## Best practices

- Keep command/rule logic pure and deterministic.
- Avoid non-deterministic side effects in hooks.
- Treat runtime log output as the source of replay truth.
- Use stable extension IDs and priorities for predictable ordering.

## Roadmap

Future work will likely add higher-level packaging/distribution ergonomics (preset bundles, registry helpers, multi-language extension SDKs), while preserving the same deterministic core contract.
