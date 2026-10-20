/**
 * The canvas demo: every kind of node, a text frame on the real canvist
 * editor, and multiplayer between tabs of this browser.
 *
 * Query parameters: `?cpu` draws on the CPU even where WebGPU exists, and
 * `?stress=2000` adds that many rectangles for measuring frame times.
 */
import "./globals.ts";
import {
	BroadcastTransport,
	type CanvasHandle,
	mountCanvas,
} from "../src/mod.ts";

/** Toolbar entries: tool name, label, shortcut, and icon path. */
const TOOLBAR: readonly (readonly [string, string, string, string])[] = [
	["select", "Select", "V", "M5 3l13 8-6 1.5L9 19z"],
	[
		"hand",
		"Hand",
		"H",
		"M8 12V5a1.5 1.5 0 013 0v6m0-1V4a1.5 1.5 0 013 0v7m0-5a1.5 1.5 0 013 0v8a6 6 0 01-12 0v-3a1.5 1.5 0 013 0",
	],
	["rectangle", "Rectangle", "R", "M4 6h16v12H4z"],
	["ellipse", "Ellipse", "O", "M12 5a8 7 0 100 14 8 7 0 100-14z"],
	[
		"polygon",
		"Star",
		"Y",
		"M12 3l2.6 5.6 6 .7-4.5 4.1 1.3 6L12 16.4 6.6 19.4l1.3-6L3.4 9.3l6-.7z",
	],
	["pen", "Pen", "P", "M4 20c4-1 6-3 8-7l4-8 3 3-8 4c-4 2-6 4-7 8z"],
	["frame", "Frame", "F", "M7 3v18M17 3v18M3 7h18M3 17h18"],
	["text", "Text", "T", "M5 5h14M12 5v14M9 19h6"],
];

const params = new URLSearchParams(location.search);
const canvas = document.querySelector<HTMLCanvasElement>("#canvas");
const tools = document.querySelector<HTMLElement>("#tools");
const backend = document.querySelector<HTMLElement>("#backend");
const zoom = document.querySelector<HTMLElement>("#zoom");
const fps = document.querySelector<HTMLElement>("#fps");

if (!canvas || !tools || !backend || !zoom || !fps) {
	throw new Error("the demo page is missing an element");
}

const buttons = new Map<string, HTMLButtonElement>();

function showTool(handle: CanvasHandle): void {
	const active = handle.tool();

	for (const [name, button] of buttons) {
		button.setAttribute("aria-pressed", String(name === active));
	}

	zoom!.textContent = `${Math.round(handle.app.zoom() * 100)}%`;
}

const handle = await mountCanvas(canvas, {
	transport: new BroadcastTransport(params.get("room") ?? "demo"),
	preferGpu: !params.has("cpu"),
	wasmUrl: new URL("../wasm/canvist_canvas_wasm_bg.wasm", import.meta.url),
	stress: Number(params.get("stress") ?? 0),
	onChange: showTool,
});

for (const [name, label, key, path] of TOOLBAR) {
	if (!handle.tools.includes(name)) continue;
	const button = document.createElement("button");
	button.type = "button";
	button.title = `${label} (${key})`;
	button.setAttribute("aria-label", label);
	button.innerHTML =
		`<svg viewBox="0 0 24 24" aria-hidden="true"><path d="${path}"/></svg><kbd>${key}</kbd>`;
	button.addEventListener("click", () => {
		handle.setTool(name);
		canvas.focus();
	});

	buttons.set(name, button);
	tools.append(button);
}

backend.textContent = handle.backend === "webgpu" ? "WebGPU" : "CPU";
showTool(handle);

// A frame rate readout from the frames the canvas actually painted.
setInterval(() => {
	const now = performance.now();
	const recent = handle.frameTimes().filter((frame) => now - frame.at < 1000);
	const work = recent.reduce((sum, frame) => sum + frame.work, 0);
	fps.textContent = recent.length
		? `${recent.length} fps · ${(work / recent.length).toFixed(1)} ms`
		: "idle";
}, 500);

// Playwright drives and measures the demo through this.
globalThis.__canvas = handle;
canvas.focus();
