<p align="center">
  <a href="https://github.com/ifiokjr/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img alt="canvist" src="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup.svg" width="220">
    </picture>
  </a>
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
