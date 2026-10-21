<p align="center">
  <a href="https://github.com/canvist/canvist">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup-dark.svg">
      <img width="220" src="https://raw.githubusercontent.com/canvist/canvist/main/assets/brand/vector/lockup.svg" alt="canvist" />
    </picture>
  </a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/readme.md"><strong>Docs</strong></a> ·
  <a href="https://github.com/canvist/canvist/blob/main/docs/src/reference/contributing.md"><strong>Contributing</strong></a> ·
  <a href="https://github.com/canvist/canvist"><strong>canvist</strong></a>
</p>

<p align="center">
  <a href="https://github.com/canvist/canvist/actions/workflows/canvas-web.yml"><img src="https://github.com/canvist/canvist/actions/workflows/canvas-web.yml/badge.svg?branch=main" alt="canvas-web status" /></a>
  <a href="https://unlicense.org"><img src="https://img.shields.io/badge/license-Unlicense-3B2CF5" alt="Unlicense" /></a>
</p>

# @canvist/canvas

canvist's design canvas in the browser: shapes, frames, images, vector paths, and text frames edited with the real canvist text editor. It renders with vello on WebGPU and falls back to a CPU rasteriser where WebGPU is missing. A pluggable transport handles collaboration.

```sh
npm install @canvist/canvas
```

```ts
import { BroadcastTransport, mountCanvas } from "@canvist/canvas";

const canvas = await mountCanvas(document.querySelector("canvas")!, {
	scene: { type: "canvist-scene", version: 1, nodes: [{ type: "page" }] },
	transport: new BroadcastTransport("my-room"),
});

canvas.setTool("rectangle");
console.log(canvas.backend); // "webgpu" or "cpu"
const saved = canvas.toJson(); // scene JSON, to store or start from later
```

- **`scene`** is scene JSON, nested or flat: the same format `@canvist/headless/scene` reads and agents write. It defaults to one empty page, and `demo: true` starts from canvist's demo scene instead.
- **Collaboration.** A `Transport` carries three messages: `hello`, `welcome`, and `update` (see `src/transport.ts`). `BroadcastTransport` syncs the tabs of one browser, and a WebSocket transport is a few lines on the same interface. Give every peer the same starting scene: each builds it the same way, so merging peers duplicates nothing.
- **State.** `encodeState()` is the shared scene as one Yjs update, and `applyUpdate(update)` merges one, such as stored state.

[The design canvas quick start](../../docs/src/quick-start/canvas.md) walks through it.

## The demo

```bash
deno task build && deno task demo
```

Open <http://127.0.0.1:8787/>, then open it in a second tab. The scene has rectangles, ellipses, a star, a vector path, an image, a frame that clips its children, and a text frame. Double-click the text to edit it.

| Key | Tool      |
| --- | --------- |
| V   | Select    |
| H   | Hand      |
| R   | Rectangle |
| O   | Ellipse   |
| Y   | Star      |
| P   | Pen       |
| F   | Frame     |
| T   | Text      |

The wheel pans, and a trackpad pinch (or Ctrl with the wheel) zooms. Cmd/Ctrl+Z undoes only your own changes. Query parameters:

- `?cpu` forces the CPU renderer.
- `?stress=2000` adds 2,000 rectangles.
- `?room=name` picks a room.
- `?scene=` starts from that scene JSON instead of the demo scene.

## Tests

```bash
deno task test
```

This needs Chromium from `npx playwright@1.58 install chromium`. The suite has four tests:

- Two record frame times during a scripted pan and drag over a 2,000-node scene, one on WebGPU and one on the CPU fallback. They print `CANVAS_PERF` lines and fail when the 95th-percentile frame goes over budget: 8 ms on WebGPU, 33 ms on the CPU. Set `CANVIST_PERF_SLACK` to scale the budgets on slow machines.
- One drives two tabs editing one scene: drawing a rectangle and typing in a text frame.
- One starts two tabs from the same scene JSON and checks they share it without duplicates, and that it exports.

For npm, `npm run build:wasm && npm run build` builds the module and `dist/`.
