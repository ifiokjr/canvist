<p align="center">
  <a href="https://github.com/ifiokjr/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/canvist_scene"><strong>API</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_scene"><img src="https://img.shields.io/crates/v/canvist_scene?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_scene"><img src="https://img.shields.io/docsrs/canvist_scene" alt="docs.rs status" /></a>
  <a href="https://github.com/ifiokjr/canvist/actions/workflows/canvas-web.yml"><img src="https://github.com/ifiokjr/canvist/actions/workflows/canvas-web.yml/badge.svg?branch=main" alt="canvas-web status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_scene

The scene model behind [canvist](https://github.com/ifiokjr/canvist)'s canvas: the design-tool sibling of `canvist_model`.

- **Nodes:** pages, frames, groups, rectangles, ellipses, polygons and stars, vector paths, boolean operations, images, components, instances, and text frames that each hold a full canvist text document.
- **Schema:** node types are registered by name. Several types can share a built-in behaviour, and custom types draw their own geometry, so plugins add a `sticky` frame or a generative shape without forking the model.
- **Props:** typed core properties (transform, size, fills, strokes, effects, blend modes, constraints, auto-layout) plus custom values and per-property instance overrides.
- **Steps:** five invertible edits (`InsertNodes`, `RemoveNodes`, `Move`, `SetProp`, `Text`) that map one-to-one onto Yjs operations, so undo and collaboration share one pipeline.
- **Fractional keys:** siblings are ordered by keys that always fit between any two, so reordering touches one node.
- **JSON:** a canonical flat format, plus a nested form with optional ids and keys that people and agents can write by hand.

Setting one property takes about 0.15 µs and reordering a layer about 0.6 µs, whether the scene has 100 nodes or 100,000. Run `cargo bench -p canvist_scene` to reproduce.

The design is in [the canvas architecture](https://github.com/ifiokjr/canvist/blob/main/docs/internal/canvas.md).
