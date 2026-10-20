/**
 * The canvist canvas in the browser.
 *
 * `mountCanvas` puts the canvas on a `<canvas>` element: vello on WebGPU
 * where the browser has it, `vello_cpu` and Canvas2D everywhere else. It
 * starts from a scene you pass as scene JSON (one empty page by default),
 * feeds DOM events to the canvas, paints on animation frames, and hands
 * every collaboration update to a {@link Transport}.
 *
 * @example
 * ```ts
 * import { BroadcastTransport, mountCanvas } from "@canvist/canvas";
 *
 * const canvas = document.querySelector("canvas")!;
 * const handle = await mountCanvas(canvas, {
 * 	scene: { type: "canvist-scene", version: 1, nodes: [{ type: "page" }] },
 * 	transport: new BroadcastTransport("my-room"),
 * });
 * handle.setTool("rectangle");
 * const saved = handle.toJson();
 * ```
 *
 * @module
 */
import type { SceneJson } from "@canvist/headless/scene";
import init, { CanvasApp } from "../wasm/canvist_canvas_wasm.js";

export { CanvasApp };
export type { SceneJson } from "@canvist/headless/scene";
export { BroadcastTransport } from "./broadcast.ts";
export type { SyncMessage, Transport } from "./transport.ts";
import type { SyncMessage, Transport } from "./transport.ts";

/** The Shift bit in a modifier mask. */
const SHIFT = 1;
/** The Alt bit. */
const ALT = 2;
/** The primary bit: Cmd on Apple platforms, Ctrl elsewhere. */
const PRIMARY = 4;
/** The client id starting scenes are built under, which peers must not use. */
const SEED_CLIENT = 1;
/** One empty page: the scene a canvas starts from by default. */
const EMPTY_SCENE: SceneJson = {
	type: "canvist-scene",
	version: 1,
	nodes: [{ type: "page" }],
};
/** Frame timings kept for {@link CanvasHandle.frameTimes}. */
const FRAME_HISTORY = 600;

/** Single-key tool shortcuts, as in most design tools. */
const TOOL_KEYS: Readonly<Record<string, string>> = {
	v: "select",
	h: "hand",
	r: "rectangle",
	o: "ellipse",
	y: "polygon",
	p: "pen",
	f: "frame",
	t: "text",
};

/** How to mount the canvas. */
export interface MountOptions {
	/**
	 * The scene to start from, as scene JSON (nested or flat), such as one
	 * saved from {@link CanvasHandle.toJson}. Defaults to one empty page.
	 *
	 * Peers that start from the same JSON start from the same shared state,
	 * so give every peer in a room the same scene and they merge without
	 * duplicating anything.
	 */
	scene?: SceneJson;
	/** Start from canvist's demo scene instead of `scene`. */
	demo?: boolean;
	/** Where collaboration updates go and come from. Omit to edit alone. */
	transport?: Transport;
	/** Use WebGPU when the browser has it. Defaults to `true`. */
	preferGpu?: boolean;
	/** The wasm module's URL, when it is not next to the JS glue. */
	wasmUrl?: URL | string;
	/** Add this many extra rectangles, for measuring frame times. */
	stress?: number;
	/** Called after the canvas changes, for host UI such as a toolbar. */
	onChange?: (handle: CanvasHandle) => void;
}

/** One painted frame's timing, in milliseconds. */
export interface FrameTime {
	/** When the frame started, from `performance.now()`. */
	at: number;
	/** Time spent painting and presenting it. */
	work: number;
}

/** A mounted canvas. */
export interface CanvasHandle {
	/** The wasm app, for anything this handle does not wrap. */
	readonly app: CanvasApp;
	/** This peer's client id. */
	readonly client: number;
	/** `"webgpu"` or `"cpu"`. */
	readonly backend: string;
	/** The registered tools' names. */
	readonly tools: readonly string[];
	/** Switch tools by name. Returns whether one exists. */
	setTool(name: string): boolean;
	/** The active tool's name. */
	tool(): string;
	/** Recent frame timings, oldest first. */
	frameTimes(): readonly FrameTime[];
	/** Forget recorded frame timings. */
	resetFrameTimes(): void;
	/** The scene as scene JSON, to store or to start another canvas from. */
	toJson(): SceneJson;
	/** The whole shared scene as one v1 Yjs update, to store or to send a joining peer. */
	encodeState(): Uint8Array;
	/** Merge a v1 Yjs update, such as stored state; returns whether the scene changed. */
	applyUpdate(update: Uint8Array): boolean;
	/** Paint on the next animation frame. */
	requestFrame(): void;
	/** Stop listening and painting. */
	destroy(): void;
}

/** A random client id other than the seed id. */
function randomClient(): number {
	const [value = 0] = crypto.getRandomValues(new Uint32Array(1));

	return value <= SEED_CLIENT ? value + SEED_CLIENT + 1 : value;
}

function modifierMask(event: MouseEvent | KeyboardEvent): number {
	const apple = /Mac|iPhone|iPad/.test(navigator.platform);
	const primary = apple ? event.metaKey : event.ctrlKey;

	return (event.shiftKey ? SHIFT : 0) | (event.altKey ? ALT : 0) |
		(primary ? PRIMARY : 0);
}

let initialized: Promise<unknown> | undefined;

/** Mount the canvas on `canvas`. */
export async function mountCanvas(
	canvas: HTMLCanvasElement,
	options: MountOptions = {},
): Promise<CanvasHandle> {
	initialized ??= init(
		options.wasmUrl ? { module_or_path: options.wasmUrl } : undefined,
	);
	await initialized;

	const client = randomClient();
	const app = await CanvasApp.create(
		canvas,
		client,
		options.preferGpu ?? true,
		options.demo === true
			? undefined
			: JSON.stringify(options.scene ?? EMPTY_SCENE),
	);

	if (options.stress) app.add_stress(options.stress);

	return new Mounted(canvas, app, client, options);
}

/** The mounted canvas: DOM events in, frames out. */
class Mounted implements CanvasHandle {
	readonly backend: string;
	readonly tools: readonly string[];
	#frame = 0;
	#lastFrame = 0;
	#times: FrameTime[] = [];
	readonly #controller = new AbortController();
	readonly #resize: ResizeObserver;
	readonly #text = hiddenTextBox();

	readonly canvas: HTMLCanvasElement;
	readonly app: CanvasApp;
	readonly client: number;
	readonly options: MountOptions;

	constructor(
		canvas: HTMLCanvasElement,
		app: CanvasApp,
		client: number,
		options: MountOptions,
	) {
		this.canvas = canvas;
		this.app = app;
		this.client = client;
		this.options = options;
		this.backend = app.backend();
		this.tools = app.tools();
		canvas.tabIndex = 0;
		canvas.style.touchAction = "none";
		canvas.after(this.#text);

		this.#resize = new ResizeObserver(() => this.#fit());
		this.#resize.observe(canvas);
		this.#fit();
		this.#listen();
		options.transport?.connect(client, (message) => this.#receive(message));
		options.transport?.send({
			type: "hello",
			from: client,
			stateVector: app.state_vector(),
		});
	}

	setTool(name: string): boolean {
		const found = this.app.set_tool(name);
		this.#changed(found);

		return found;
	}

	tool(): string {
		return this.app.tool();
	}

	frameTimes(): readonly FrameTime[] {
		return this.#times;
	}

	resetFrameTimes(): void {
		this.#times = [];
	}

	toJson(): SceneJson {
		return JSON.parse(this.app.scene_json());
	}

	encodeState(): Uint8Array {
		return this.app.encode_state();
	}

	applyUpdate(update: Uint8Array): boolean {
		const changed = this.app.apply_update(update);

		if (changed) {
			this.requestFrame();
			this.options.onChange?.(this);
		}

		return changed;
	}

	requestFrame(): void {
		if (this.#frame === 0) {
			this.#frame = requestAnimationFrame((at) => this.#paint(at));
		}
	}

	destroy(): void {
		this.#controller.abort();
		this.#resize.disconnect();
		cancelAnimationFrame(this.#frame);
		this.options.transport?.close();
		this.#text.remove();
		this.app.free();
	}

	#fit(): void {
		const rect = this.canvas.getBoundingClientRect();
		this.app.resize(rect.width, rect.height, devicePixelRatio);
		this.requestFrame();
	}

	#paint(at: number): void {
		this.#frame = 0;
		const elapsed = this.#lastFrame ? (at - this.#lastFrame) / 1000 : 0;
		this.#lastFrame = at;
		const coasting = this.app.frame(elapsed);

		const start = performance.now();
		this.app.render();
		this.#times.push({ at: start, work: performance.now() - start });

		if (this.#times.length > FRAME_HISTORY) this.#times.shift();

		this.canvas.style.cursor = this.app.cursor();

		if (coasting) this.requestFrame();
		else this.#lastFrame = 0;
	}

	/** Repaint and broadcast after an event the canvas handled. */
	#changed(redraw: boolean): void {
		this.#focus();

		for (const update of this.app.take_updates()) {
			this.options.transport?.send({
				type: "update",
				from: this.client,
				update,
			});
		}

		if (redraw) {
			this.requestFrame();
			this.options.onChange?.(this);
		}
	}

	#receive(message: SyncMessage): void {
		if (message.from === this.client) return;

		switch (message.type) {
			case "hello":
				this.#sendMissing(message.stateVector);
				// The newcomer may have changes of its own: ask for them.
				this.options.transport?.send({
					type: "welcome",
					from: this.client,
					stateVector: this.app.state_vector(),
				});
				break;
			case "welcome":
				this.#sendMissing(message.stateVector);
				break;
			case "update":
				this.#integrate(message.update);
				break;
		}
	}

	/** Send the peer with `stateVector` everything it lacks. */
	#sendMissing(stateVector: Uint8Array): void {
		this.options.transport?.send({
			type: "update",
			from: this.client,
			update: this.app.encode_diff(stateVector),
		});
	}

	/** Apply a collaborator's update, repainting if anything changed. */
	#integrate(update: Uint8Array): void {
		this.applyUpdate(update);
	}

	#listen(): void {
		const signal = this.#controller.signal;
		const canvas = this.canvas;
		const position = (event: MouseEvent) => {
			const rect = canvas.getBoundingClientRect();
			return [event.clientX - rect.left, event.clientY - rect.top] as const;
		};
		const pointer = (phase: string) => (event: PointerEvent) => {
			if (phase === "down") {
				canvas.focus();
				canvas.setPointerCapture(event.pointerId);
			}

			const [x, y] = position(event);
			this.#changed(this.app.pointer(
				phase,
				event.pointerId,
				event.pointerType,
				x,
				y,
				event.buttons,
				modifierMask(event),
				event.timeStamp,
			));
		};

		canvas.addEventListener("pointerdown", pointer("down"), { signal });
		canvas.addEventListener("pointermove", pointer("move"), { signal });
		canvas.addEventListener("pointerup", pointer("up"), { signal });
		canvas.addEventListener("pointercancel", pointer("cancel"), { signal });
		canvas.addEventListener("wheel", (event) => {
			event.preventDefault();
			const [x, y] = position(event);
			this.#changed(this.app.wheel(
				event.deltaX,
				event.deltaY,
				x,
				y,
				modifierMask(event),
				event.ctrlKey,
				event.timeStamp,
			));
		}, { signal, passive: false });

		// Keys reach the canvas while it has focus, and the hidden text box
		// while a text frame is open, so IMEs, dictation, and autocorrect
		// deliver text the way they do to any text field.
		const targets: readonly HTMLElement[] = [canvas, this.#text];

		for (const target of targets) {
			target.addEventListener("keydown", (event) => this.#key(event, true), {
				signal,
			});
			target.addEventListener("keyup", (event) => this.#key(event, false), {
				signal,
			});
			target.addEventListener("paste", (event) => this.#paste(event), {
				signal,
			});
		}

		this.#text.addEventListener("beforeinput", (event) => {
			if (event.inputType === "insertText" && event.data) {
				event.preventDefault();
				this.#changed(this.app.type_text(event.data, event.timeStamp));
			}
		}, { signal });
		this.#text.addEventListener("compositionend", (event) => {
			this.#text.value = "";
			if (event.data) {
				this.#changed(this.app.type_text(event.data, event.timeStamp));
			}
		}, { signal });
	}

	/** Pasted plain text goes into the open text frame. */
	#paste(event: ClipboardEvent): void {
		const text = event.clipboardData?.getData("text/plain");
		if (!text) return;
		event.preventDefault();
		this.#changed(this.app.type_text(text, event.timeStamp));
	}

	#key(event: KeyboardEvent, down: boolean): void {
		const mask = modifierMask(event);
		const editing = this.app.editing();

		if (event.isComposing) return;

		const shortcut = !editing && down && mask === 0
			? TOOL_KEYS[event.key.toLowerCase()]
			: undefined;

		if (shortcut) {
			event.preventDefault();
			this.setTool(shortcut);
			return;
		}

		// Characters arrive through the text box's input events instead.
		if (editing && isCharacter(event, mask)) return;

		const used = this.app.key(event.key, down, mask, event.timeStamp);

		if (used) event.preventDefault();
		this.#changed(used);
	}

	/** Keep focus on the text box while a text frame is open. */
	#focus(): void {
		const editing = this.app.editing();
		const active = document.activeElement;

		if (editing && active !== this.#text) {
			this.#text.focus({ preventScroll: true });
		} else if (!editing && active === this.#text) {
			this.canvas.focus({ preventScroll: true });
		}
	}
}

/** Whether `event` types a character rather than running a command. */
function isCharacter(event: KeyboardEvent, mask: number): boolean {
	return (mask & PRIMARY) === 0 && [...event.key].length === 1;
}

/** A text box the user never sees, which receives typed text. */
function hiddenTextBox(): HTMLTextAreaElement {
	const text = document.createElement("textarea");
	text.setAttribute("aria-label", "Text frame");
	text.autocapitalize = "off";
	text.spellcheck = false;
	Object.assign(text.style, {
		position: "fixed",
		top: "0",
		left: "0",
		width: "1px",
		height: "1px",
		opacity: "0",
		pointerEvents: "none",
	});

	return text;
}
