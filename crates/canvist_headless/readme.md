<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/quick-start/server-rust.md"><strong>Quick start</strong></a> ·
  <a href="https://docs.rs/canvist_headless"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_headless"><img src="https://img.shields.io/crates/v/canvist_headless?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_headless"><img src="https://img.shields.io/docsrs/canvist_headless" alt="docs.rs status" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/ci.yml"><img src="https://github.com/canvist/canvist/actions/workflows/ci.yml/badge.svg?branch=main" alt="ci status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_headless

The [canvist](https://github.com/canvist/canvist) document model and editing engine with no UI, for servers, scripts, and tests. It is the core the TypeScript and Dart server packages wrap.

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
