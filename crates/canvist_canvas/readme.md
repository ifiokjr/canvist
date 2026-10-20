<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/canvist_canvas"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_canvas"><img src="https://img.shields.io/crates/v/canvist_canvas?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_canvas"><img src="https://img.shields.io/docsrs/canvist_canvas" alt="docs.rs status" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/canvas-web.yml"><img src="https://github.com/canvist/canvist/actions/workflows/canvas-web.yml/badge.svg?branch=main" alt="canvas-web status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_canvas

The headless interaction core of [canvist](https://github.com/canvist/canvist)'s design canvas: everything between input and pixels, with no UI. Build your own Figma on top of it.

- **Camera:** pan, zoom about a point, pinch, momentum that doesn't depend on frame rate, and zoom-to-fit.
- **Scene index:** world transforms, auto-layout, boolean outlines, and an R-tree, updated only where a commit touched.
- **Hit testing:** exact tests on fills, strokes, and curves, clipped by frames, plus marquee selection.
- **Transform handles:** resize with aspect lock or from the centre, flip, rotate with snapping, and skew.
- **Snapping:** smart guides from nearby edges and centres, grid snapping, and ruler ticks.
- **Tools as typed extensions:** select, hand, rectangle, ellipse, polygon, frame, pen, and text, with inspectors as data any host or agent can read.
- **Overlay as data:** selection outlines, handles, guides, and pen previews. Draw them yourself or hand them to [`canvist_paint`](../canvist_paint).

At 100,000 nodes, a hit test takes about 0.3 µs and a drag step about 1.3 µs. Run `cargo bench -p canvist_canvas` to reproduce.
