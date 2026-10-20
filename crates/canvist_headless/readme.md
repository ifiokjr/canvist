# canvist_headless

The [canvist](https://github.com/ifiokjr/canvist) document model and editing engine with no UI, for servers, scripts, and tests. It is the core the TypeScript and Dart server packages wrap.

- **Formats:** load and export ProseMirror/TipTap JSON, HTML, Markdown, and plain text.
- **Typed commands:** register extensions once on an `Engine` and get typed handles, so an unregistered extension's commands do not compile. Bindings and scripts use named commands and queries, whose argument types come from the extension metadata.
- **Explicit selections:** no layout is needed. Select by model points, or by `Position`s (block path plus UTF-16 offset) that mean the same thing in Rust, TypeScript, and Dart.
- **Collaboration** (`collab` feature): join from Yjs state, merge updates in any order, answer y-sync messages as a y-websocket server would, and check the merged state against the schema.
- **Server rendering** (`ssr` feature): HTML and SVG laid out exactly as the canvas editor paints them, using `parley` and a bundled font.
- **Canvas scenes** (`scene` feature): load canvas scenes of shapes, frames, images, vector paths, and text frames from JSON or Yjs state, run canvas commands and inspector edits, and export SVG and PNG with the renderer the canvas draws with. Loading a 10,000-node scene takes about 25 ms, exporting it as SVG about 23 ms, and a PNG thumbnail about 10 ms.
- **Thread-safe:** an `Engine` is `Send + Sync`, and a `Doc` is `Send`.

```rust
use canvist_headless::Engine;
use canvist_headless::Format;
use canvist_headless::Position;

let engine = Engine::starter_kit();
let kit = engine.handles();
let mut doc = engine.parse(Format::Markdown, "Hello world")?;

doc.select_range(&Position::new([0], 6), &Position::new([0], 11))?;
doc.run(&kit.bold.toggle())?;

assert_eq!(doc.export(Format::Html), "<p>Hello <strong>world</strong></p>");
# Ok::<(), canvist_headless::HeadlessError>(())
```

The examples convert Markdown to JSON, run a scripted transform, and merge Yjs updates on a server:

```sh
echo '# Hello' | cargo run -p canvist_headless --example markdown_to_json
cargo run -p canvist_headless --example transform < notes.md
cargo run -p canvist_headless --features collab --example merge_updates
```

The language-neutral fixtures in [`conformance/`](../../conformance) are the contract every binding passes. Loading or exporting a 10,000-paragraph document takes 3–20 ms, a command costs about 0.2 µs, and applying a collaborator's keystroke about 2 µs. Run `cargo bench -p canvist_headless --all-features` to reproduce.
