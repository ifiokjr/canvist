# Quick start: Design canvas

In this guide you put canvist's design canvas on a web page with `@canvist/canvas`: shapes, frames, images, vector paths, and text frames edited with the real canvist text editor, with collaboration between tabs. It takes about five minutes.

**Status: available.** `@canvist/canvas` renders with vello on WebGPU and falls back to a CPU rasteriser where WebGPU is missing. Its tests run in Chromium, on both renderers, on every change to the canvas. It ships to npm with the SDK once its package name is set up there; until then, build it from source.

## 1. Install

After it's on npm:

```sh
npm install @canvist/canvas
```

Until then, build it once and install the tarball. You need Node 22 or newer, Rust, and [wasm-pack](https://rustwasm.github.io/wasm-pack/):

```sh
git clone https://github.com/canvist/canvist
cd canvist/packages/canvist-headless && npm ci && npm run build:wasm && npm run build
cd ../canvist-canvas && npm ci && npm run build:wasm && npm pack
```

## 2. Mount a canvas

Give it a `<canvas>` with a size. It starts from the scene you pass as scene JSON, or one empty page:

```html
<canvas id="canvas" style="width: 100%; height: 600px"></canvas>
```

```ts no_run
import { mountCanvas, type SceneJson } from "@canvist/canvas";

const scene: SceneJson = {
	type: "canvist-scene",
	version: 1,
	nodes: [{
		type: "page",
		children: [{
			type: "rectangle",
			props: {
				name: "Card",
				size: [240, 160],
				transform: [1, 0, 0, 1, 80, 80],
				fills: [{ type: "solid", color: "#3b2cf5" }],
			},
		}],
	}],
};

const element = document.querySelector<HTMLCanvasElement>("#canvas");

if (element !== null) {
	const canvas = await mountCanvas(element, { scene });
	console.log(canvas.backend); // "webgpu" or "cpu"

	canvas.setTool("ellipse");
	// Save the scene, and start another canvas from it later.
	localStorage.setItem("scene", JSON.stringify(canvas.toJson()));
}
```

The canvas handles the pointer, the wheel and trackpad (pan and pinch zoom), and the keyboard. These are the tools and their shortcuts:

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

Double-click a text frame to edit it. Typing, input methods, and paste go through a hidden text field, as in any text box.

## 3. Collaborate

Pass a `Transport`. `BroadcastTransport` syncs the tabs of one browser, and a WebSocket transport is a few lines on the same three-message interface (`hello`, `welcome`, and `update`, in `src/transport.ts`):

```ts no_run
import {
	BroadcastTransport,
	mountCanvas,
	type SceneJson,
} from "@canvist/canvas";

declare const scene: SceneJson;
const element = document.querySelector<HTMLCanvasElement>("#canvas");

if (element !== null) {
	await mountCanvas(element, {
		scene,
		transport: new BroadcastTransport("design-review"),
		onChange: (canvas) => console.log(canvas.tool()),
	});
}
```

Give every peer in a room **the same starting scene**. Every peer builds it the same way, so their shared states start identical, and merging them duplicates nothing. To store a live scene, keep `canvas.encodeState()` (a Yjs update) and merge it into a new canvas with `canvas.applyUpdate(state)`. Undo reverts only your own changes.

## 4. Render scenes on a server

`@canvist/headless/scene` loads the same scene JSON with no browser and exports SVG and PNG, for thumbnails and social cards. [Server: TypeScript](./server-typescript.md) covers the engine.

## Run the demo

```sh
cd packages/canvist-canvas
deno task build && deno task demo
```

Open <http://127.0.0.1:8787/>, then open it in a second tab. `?cpu` forces the CPU renderer, `?stress=2000` adds 2,000 rectangles, and `?scene=` starts from scene JSON instead of the demo scene.
