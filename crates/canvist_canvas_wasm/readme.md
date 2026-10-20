<p align="center">
  <a href="https://github.com/ifiokjr/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/ifiokjr/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/ifiokjr/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/ifiokjr/canvist/actions/workflows/canvas-web.yml"><img src="https://github.com/ifiokjr/canvist/actions/workflows/canvas-web.yml/badge.svg?branch=main" alt="canvas-web status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# canvist_canvas_wasm

[canvist](https://github.com/ifiokjr/canvist)'s canvas in the browser.

- **`App`** wires a `canvist_canvas::Canvas` to a `canvist_collab::SceneBinding`, and opens text frames in a `TextSession` on the real canvist text editor. It has no browser dependencies, so it is tested natively (`cargo test -p canvist_canvas_wasm`).
- **`CanvasApp`** (wasm only) puts an `App` on a `<canvas>`. It renders with vello on WebGPU where the browser supports it, and otherwise with `vello_cpu` and `putImageData`.

The demo scene is [`src/demo.json`](src/demo.json), written in the nested scene JSON format. Every peer builds it under one reserved client id, so every tab starts from the same shared state.

Build it with `deno task build` in [`packages/canvist-canvas`](https://github.com/ifiokjr/canvist/blob/main/packages/canvist-canvas/readme.md), which wraps it in a typed `mountCanvas` and runs the demo.
