<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/canvist_paint"><strong>API</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/canvist_paint"><img src="https://img.shields.io/crates/v/canvist_paint?color=3B2CF5" alt="crates.io version" /></a>
  <a href="https://docs.rs/canvist_paint"><img src="https://img.shields.io/docsrs/canvist_paint" alt="docs.rs status" /></a>
  <a href="https://github.com/canvist/canvist/actions/workflows/canvas-web.yml"><img src="https://github.com/canvist/canvist/actions/workflows/canvas-web.yml/badge.svg?branch=main" alt="canvas-web status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_paint

Rendering for [canvist](https://github.com/canvist/canvist)'s canvas. A scene walker turns one page into `Painter` calls, culled to the viewport and with level of detail. Every backend implements `Painter`:

- **`gpu`:** vello on wgpu. This covers WebGPU in browsers, and Metal, Vulkan, and DX12 natively.
- **`cpu`:** `vello_cpu`, for PNG export, server thumbnails, and the browser fallback.
- **SVG**, with text kept as text.
- **`Recorder`**, which records the calls themselves, for tests.

Text frames draw their `canvist_layout` display list. Glyphs are shaped with `parley` and the bundled Atkinson Hyperlegible font (SIL Open Font License, in `fonts/`), so text draws where layout put it.

With 10,000 nodes on screen, encoding a frame for the GPU takes about 1 ms. Rasterising it on the CPU at 1080p takes about 4 ms.
