<p align="center">
  <a href="https://github.com/ifiokjr/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img alt="canvist" src="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup.svg" width="220">
    </picture>
  </a>
</p>

# canvist_paint

Rendering for [canvist](https://github.com/ifiokjr/canvist)'s canvas. A scene walker turns one page into `Painter` calls, culled to the viewport and with level of detail. Every backend implements `Painter`:

- **`gpu`:** vello on wgpu. This covers WebGPU in browsers, and Metal, Vulkan, and DX12 natively.
- **`cpu`:** `vello_cpu`, for PNG export, server thumbnails, and the browser fallback.
- **SVG**, with text kept as text.
- **`Recorder`**, which records the calls themselves, for tests.

Text frames draw their `canvist_layout` display list. Glyphs are shaped with `parley` and the bundled Atkinson Hyperlegible font (SIL Open Font License, in `fonts/`), so text draws where layout put it.

With 10,000 nodes on screen, encoding a frame for the GPU takes about 1 ms. Rasterising it on the CPU at 1080p takes about 4 ms.
