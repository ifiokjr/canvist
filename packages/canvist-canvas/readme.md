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

# @canvist/canvas

canvist's canvas in the browser. It renders with vello on WebGPU, and falls back to a CPU rasteriser where WebGPU is missing. Text frames are edited with the real canvist text editor. A pluggable transport handles collaboration.

```ts
import { BroadcastTransport, mountCanvas } from "@canvist/canvas";

const canvas = document.querySelector("canvas")!;
const handle = await mountCanvas(canvas, {
	transport: new BroadcastTransport("my-room"),
});

handle.setTool("rectangle");
console.log(handle.backend); // "webgpu" or "cpu"
```

A `Transport` carries three messages: `hello`, `welcome`, and `update`. See `src/transport.ts`. `BroadcastTransport` syncs the tabs of one browser. A WebSocket transport is a few lines on top of the same interface.

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

## Tests

```bash
deno task test
```

This needs Chromium from `npx playwright@1.58 install chromium`. The suite has three tests:

- Two record frame times during a scripted pan and drag over a 2,000-node scene, one on WebGPU and one on the CPU fallback. They print `CANVAS_PERF` lines and fail when the 95th-percentile frame goes over budget: 8 ms on WebGPU, 33 ms on the CPU. Set `CANVIST_PERF_SLACK` to scale the budgets on slow machines.
- One drives two tabs editing one scene: drawing a rectangle and typing in a text frame.
