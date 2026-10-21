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
  <a href="https://docs.rs/canvist"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist"><img src="https://img.shields.io/crates/v/canvist?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist"><img src="https://img.shields.io/docsrs/canvist" alt="docs.rs status" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/ci.yml"><img src="https://github.com/canvist/canvist/actions/workflows/ci.yml/badge.svg?branch=main" alt="ci status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist

The [canvist](https://github.com/canvist/canvist) rich-text editor engine in one crate. Add `canvist` and reach the document model, the editing engine, the extensions, and the headless engine through its modules; layout, collaboration, and the design canvas are a feature away.

```sh
cargo add canvist
```

The default features, `headless` and `starter-kit`, are what a server, a script, or a test needs: load and export ProseMirror/TipTap JSON, HTML, Markdown, and plain text, and run typed commands against explicit selections.

```rust
use canvist::headless::Engine;
use canvist::headless::Format;
use canvist::headless::Position;

let engine = Engine::starter_kit();
let kit = engine.handles();
let mut doc = engine.parse(Format::Markdown, "Hello world")?;

doc.select_range(&Position::new([0], 6), &Position::new([0], 11))?;
doc.run(&kit.bold.toggle())?;

assert_eq!(doc.export(Format::Html), "<p>Hello <strong>world</strong></p>");
# Ok::<(), canvist::headless::HeadlessError>(())
```

Each module is the crate it names, so `canvist::headless::Engine` is `canvist_headless::Engine` and every crate's documentation applies unchanged.

| Module                | Crate                                         | Feature                                        |
| --------------------- | --------------------------------------------- | ---------------------------------------------- |
| `canvist::model`      | [`canvist_model`](../canvist_model)           | always                                         |
| `canvist::core`       | [`canvist_core`](../canvist_core)             | always                                         |
| `canvist::extensions` | [`canvist_extensions`](../canvist_extensions) | always; `starter-kit`, `extras`, `full`        |
| `canvist::headless`   | [`canvist_headless`](../canvist_headless)     | `headless`; `journal`, `ssr`, `headless-scene` |
| `canvist::collab`     | [`canvist_collab`](../canvist_collab)         | `collab`                                       |
| `canvist::layout`     | [`canvist_layout`](../canvist_layout)         | `layout`; `parley` for real font measurement   |
| `canvist::scene`      | [`canvist_scene`](../canvist_scene)           | `canvas`                                       |
| `canvist::canvas`     | [`canvist_canvas`](../canvist_canvas)         | `canvas`                                       |
| `canvist::paint`      | [`canvist_paint`](../canvist_paint)           | `paint`; `png` (CPU), `gpu` (vello on wgpu)    |

- **`collab`:** Yjs collaboration. With `headless`, a document is a Yjs peer and a y-sync endpoint; with `canvas`, so is a scene.
- **`ssr`:** server-side HTML and SVG, laid out exactly as the canvas editor paints them.
- **`headless-scene`:** load, edit, and export canvas scenes as SVG and PNG on a server.

The desktop editor, [`canvist_desktop`](../canvist_desktop), is a separate crate: add it next to `canvist`.
