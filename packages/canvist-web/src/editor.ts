/**
 * The canvas editor: a DOM element that hosts the WebAssembly editor's
 * canvas, keeps it sized to the visible part of the document, and turns
 * scrolling, resizing, zoom, and font loading into repaints on the next
 * animation frame.
 *
 * @module
 */
import type {
	CommandName,
	CommandParams,
	Commands,
	DocJson,
	Extensions,
	Format,
	Position,
	Queries,
	QueryName,
	QueryParams,
	QueryResult,
	Range,
	Registered,
	StarterKit,
} from "@canvist/headless";
import { core, starterKit } from "@canvist/headless/extensions";
import { WebEditor } from "../wasm/canvist_web.js";
import { FontWatcher } from "./font-watcher.ts";
import { load, type WasmSource } from "./load.ts";
import { CanvasMeasurer } from "./measure.ts";
import type { PluginApis, PluginContext, Plugins } from "./plugin.ts";

/** How the caret blinks: on and off for this long each. */
const CARET_BLINK_MS = 530;

/** How close, in CSS px, a measured border width must be to a snapped one to count as snapped. */
const BORDER_SNAP_TOLERANCE = 1 / 128;

/** Which line-breaking rules layout follows: Blink's tailoring, or ICU's defaults (Firefox and WebKit). */
export type LineBreaks = "chromium" | "icu";

/** The StarterKit theme's options. Server rendering must use the same ones. */
export interface ThemeOptions {
	/** Light or dark colours. Default `"light"`. */
	readonly scheme?: "light" | "dark";
	/** The CSS `font-family` list for body text and headings. */
	readonly bodyFamily?: string;
	/** The CSS `font-family` list for code. */
	readonly codeFamily?: string;
	/** The body font size in pixels. Default 16. */
	readonly size?: number;
	/** The body line height as a multiple of the font size. Default 1.5. */
	readonly lineHeight?: number;
	/** CSS `letter-spacing` of body text, in pixels. Default 0. */
	readonly letterSpacing?: number;
}

/** A document to start with: text in a format, or ProseMirror JSON. */
export type Content<E extends Extensions> =
	| { readonly format: Format; readonly input: string }
	| DocJson<Registered<E>>;

/** Options for {@link createEditor}. */
export interface EditorOptions<E extends Extensions, P extends Plugins> {
	/** The extensions, TipTap's StarterKit by default. */
	readonly extensions?: E;
	/** Plugins, such as `fonts()` from `@canvist/web/fonts`. */
	readonly plugins?: P;
	/** The initial document. */
	readonly content?: Content<E>;
	readonly theme?: ThemeOptions;
	/** Line-breaking tailoring; detected from the browser by default. */
	readonly breaks?: LineBreaks;
	/** The client block ids are allocated under. Default 1. */
	readonly client?: number;
	/** Pixels beyond the viewport to lay out ahead of scrolling. Default one screen. */
	readonly overscan?: number;
	/** Where to load the WebAssembly module from. */
	readonly wasm?: WasmSource;
}

/** Measurement statistics, from {@link Editor.stats}. */
export interface EditorStats {
	readonly hits: number;
	readonly misses: number;
	/** The share of segment lookups answered from the cache, 0 to 1. */
	readonly hitRate: number;
	/** Approximate bytes held by the measurement cache. */
	readonly bytes: number;
	readonly measuredSegments: number;
	readonly measureCalls: number;
	readonly fonts: readonly {
		readonly font: string | null;
		readonly segments: number;
		readonly bytes: number;
	}[];
}

/** The laid-out document, from {@link Editor.geometry}. */
export interface Geometry {
	readonly height: number;
	readonly blocks: readonly {
		readonly id: string;
		readonly rect: readonly [number, number, number, number] | null;
		readonly text?: string;
		/** `[utf16, downstream x, upstream x, line top, line height]` per grapheme boundary. */
		readonly stops?:
			readonly (readonly [number, number, number, number, number])[];
	}[];
	readonly runs: readonly {
		readonly block: string;
		/** The run's UTF-16 offset in its block. */
		readonly start: number;
		readonly text: string;
		readonly x: number;
		readonly width: number;
		readonly baseline: number;
		/** The content area's height above the baseline. */
		readonly ascent: number;
		readonly font: string;
		readonly rtl: boolean;
	}[];
}

/** What the last frame did, for tests and profiling. */
export interface FrameReport {
	readonly height: number;
	readonly laidOut: number;
	readonly measured: number;
	readonly drawn: number;
	readonly full: boolean;
	readonly painted: boolean;
}

/**
 * Create an editor that renders into `element`.
 *
 * The element becomes the editor: give it a width (the document wraps to
 * its content box) and, for a scrolling editor, a height and
 * `overflow: auto`. Without a height it grows with the document and the
 * page scrolls. Don't put padding on it; pad a wrapper instead, as with
 * server-rendered `.cv-doc`.
 *
 * ```ts
 * const editor = await createEditor(element, {
 * 	content: { format: "markdown", input: "# Hello" },
 * });
 * editor.commands.toggleBold();
 * ```
 */
export async function createEditor<
	const E extends Extensions = StarterKit,
	const P extends Plugins = [],
>(
	element: HTMLElement,
	options: EditorOptions<E, P> = {},
): Promise<Editor<E, P>> {
	await load(options.wasm);

	return new Editor(element, options);
}

/** A canvas editor. Create one with {@link createEditor}. */
export class Editor<E extends Extensions, P extends Plugins = []> {
	/** Run a registered command; returns whether it applied. */
	readonly commands: Commands<Registered<E>>;
	/** Whether a registered command would apply. Nothing changes. */
	readonly can: Commands<Registered<E>>;
	/** Answer a registered query. */
	readonly query: Queries<Registered<E>>;
	/** Each registered plugin's API, by name. */
	readonly plugins: PluginApis<P>;

	readonly #element: HTMLElement;
	readonly #canvas: HTMLCanvasElement;
	readonly #spacer: HTMLDivElement;
	readonly #wasm: WebEditor;
	readonly #measurer: CanvasMeasurer;
	readonly #fonts: FontWatcher;
	readonly #extensions: Registered<E>;
	readonly #abort = new AbortController();
	readonly #waits = new Set<Promise<unknown>>();
	readonly #resize: ResizeObserver;
	#frame = 0;
	#blink = 0;
	#height = 0;
	#width = 0;
	#lastFrame: FrameReport | undefined;
	#focused = false;

	/** @internal Use {@link createEditor}, which loads the WebAssembly module first. */
	constructor(element: HTMLElement, options: EditorOptions<E, P>) {
		const extensions = (options.extensions ?? starterKit) as E;
		this.#extensions = [...core, ...extensions];
		this.#element = element;
		this.#canvas = document.createElement("canvas");
		this.#spacer = document.createElement("div");
		this.#measurer = new CanvasMeasurer(element.ownerDocument);
		this.#prepareDom();
		this.#width = contentWidth(element);

		this.#wasm = new WebEditor(
			this.#canvas,
			this.#measurer,
			extensions.map((extension) => extension.name),
			wasmOptions(options),
		);
		this.#snapToDevice();
		this.#fonts = new FontWatcher(
			element.ownerDocument.fonts,
			(families) => this.#fontsLoaded(families),
		);
		this.#measurer.listen((used) => this.#fonts.request(used));

		if (options.content !== undefined) {
			this.#load(options.content);
		}

		const commands = registeredNames(this.#extensions, "commands");
		const queries = registeredNames(this.#extensions, "queries");
		this.commands = calls(
			commands,
			(name, args) => this.#runEncoded(name, args),
		);
		this.can = calls(
			commands,
			(name, args) => this.#wasm.can(name, encode(args)),
		);
		this.query = calls(
			queries,
			(name, args) => JSON.parse(this.#wasm.query(name, encode(args))),
		);
		this.plugins = this.#setupPlugins(options.plugins ?? []);

		this.#resize = new ResizeObserver((entries) => this.#observe(entries));
		this.#resize.observe(element, { box: "content-box" });
		this.#listenForViewportChanges();
		this.#watchScale();

		this.#syncSize();
		this.#render();
	}

	/** The registered extensions, including the ones every editor has. */
	get extensions(): Registered<E> {
		return this.#extensions;
	}

	/** The canvas the editor paints. */
	get canvas(): HTMLCanvasElement {
		return this.#canvas;
	}

	/** What the last frame did. */
	get lastFrame(): FrameReport | undefined {
		return this.#lastFrame;
	}

	/**
	 * Resolve once fonts the document uses have loaded (or failed), plugins
	 * have finished their setup work, and the result is painted.
	 */
	async ready(): Promise<void> {
		for (;;) {
			await this.#fonts.settled();
			await Promise.allSettled([...this.#waits]);

			if (!this.#fonts.loading && this.#waits.size === 0) {
				break;
			}
		}

		// Paint in an animation frame, before the browser measures layout for
		// ResizeObserver. Fonts can finish loading while observers are being
		// notified, and resizing the canvas then is a ResizeObserver loop.
		await nextFrame();
		this.#render();
	}

	/** Run the command named `name`, for callers that pick commands at runtime. */
	run<N extends CommandName<Registered<E>>>(
		name: N,
		...args: CommandParams<Registered<E>, N>
	): boolean {
		return this.#runEncoded(name, args[0]);
	}

	/** Answer the query named `name`, for callers that pick queries at runtime. */
	ask<N extends QueryName<Registered<E>>>(
		name: N,
		...args: QueryParams<Registered<E>, N>
	): QueryResult<Registered<E>, N> {
		return JSON.parse(this.#wasm.query(name, encode(args[0])));
	}

	/** Replace the document. */
	setContent(content: Content<E>): void {
		this.#load(content);
		this.#invalidate();
	}

	/** The document as `format`. */
	export(format: Format): string {
		return this.#wasm.export(format);
	}

	/** The document as ProseMirror JSON. */
	toJson(): DocJson<Registered<E>> {
		return JSON.parse(this.#wasm.export("json"));
	}

	/** Server-rendered HTML for the document at the editor's width, and its stylesheet. */
	toHtml(): { readonly html: string; readonly css: string } {
		const [html = "", css = ""] = this.#wasm.html();

		return { html, css };
	}

	/** The whole document as SVG, painted from the same display list as the canvas. */
	toSvg(): string {
		return this.#wasm.svg();
	}

	/**
	 * Where layout put every block, line, caret stop, and text run, in
	 * document coordinates: what the parity tests compare with the DOM.
	 */
	geometry(): Geometry {
		return JSON.parse(this.#wasm.geometry());
	}

	/** The text selection, or `null` for a block selection. */
	get selection(): Range | null {
		return JSON.parse(this.#wasm.selection());
	}

	/** Select from `anchor` to `head`, or place a caret at `anchor`. Offsets count UTF-16 code units. */
	select(anchor: Position, head: Position = anchor): void {
		this.#wasm.select(
			Uint32Array.from(anchor.path),
			anchor.offset,
			Uint32Array.from(head.path),
			head.offset,
		);
		this.#invalidate();
	}

	/** Replace the selection with `text`, as typing does (input rules apply). */
	insertText(text: string): boolean {
		const changed = this.#wasm.insertText(text);
		this.#invalidate();

		return changed;
	}

	/** Undo the last change. */
	undo(): boolean {
		const changed = this.#wasm.undo();
		this.#invalidate();

		return changed;
	}

	/** Redo the last undone change. */
	redo(): boolean {
		const changed = this.#wasm.redo();
		this.#invalidate();

		return changed;
	}

	/** Show the caret and blink it, as when the editor has focus. */
	setFocused(focused: boolean): void {
		this.#focused = focused;
		this.#wasm.setFocused(focused);
		this.#restartBlink();
		this.#invalidate();
	}

	/** The caret's rectangle in document coordinates, or `null` when the selection is a range. */
	caretRect(): DOMRect | null {
		const [x, y, width, height] = this.#wasm.caretRect();

		return x === undefined || y === undefined || width === undefined ||
				height === undefined
			? null
			: new DOMRect(x, y, width, height);
	}

	/** Measurement cache statistics. */
	stats(): EditorStats {
		const stats: Omit<EditorStats, "hitRate"> = JSON.parse(this.#wasm.stats());
		const lookups = stats.hits + stats.misses;

		return { ...stats, hitRate: lookups === 0 ? 1 : stats.hits / lookups };
	}

	/** The CSS `font` of every font the layout has used. */
	fonts(): string[] {
		return this.#wasm.fonts();
	}

	/** Lay out and paint now instead of on the next animation frame. */
	render(): FrameReport | undefined {
		this.#render();

		return this.#lastFrame;
	}

	/** Remove the editor's elements and listeners and release its memory. */
	destroy(): void {
		this.#abort.abort();
		cancelAnimationFrame(this.#frame);
		clearTimeout(this.#blink);
		this.#resize.disconnect();
		this.#fonts.dispose();
		this.#canvas.remove();
		this.#spacer.remove();
		this.#wasm.free();
	}

	[Symbol.dispose](): void {
		this.destroy();
	}

	#load(content: Content<E>): void {
		if ("format" in content) {
			this.#wasm.load(content.format, content.input);
		} else {
			this.#wasm.load("json", JSON.stringify(content));
		}
	}

	#prepareDom(): void {
		const element = this.#element;

		if (getComputedStyle(element).position === "static") {
			element.style.position = "relative";
		}

		element.classList.add("cv-editor");
		const canvas = this.#canvas;
		canvas.className = "cv-canvas";
		Object.assign(canvas.style, {
			display: "block",
			position: "sticky",
			top: "0",
			width: "100%",
			height: "0px",
		});
		this.#spacer.className = "cv-spacer";
		this.#spacer.setAttribute("aria-hidden", "true");
		element.append(canvas, this.#spacer);
	}

	#setupPlugins(plugins: Plugins): PluginApis<P> {
		const context: PluginContext = {
			element: this.#element,
			measurer: this.#measurer,
			signal: this.#abort.signal,
			fontsLoaded: (families) => this.#fontsLoaded(families),
			hideFamilies: (families) => {
				this.#wasm.hideFamilies([...families]);
				this.#invalidate();
			},
			invalidate: () => this.#invalidate(),
			waitFor: (work) => {
				const tracked = work.finally(() => this.#waits.delete(tracked));
				this.#waits.add(tracked);
			},
		};
		const apis: Record<string, unknown> = {};

		for (const plugin of plugins) {
			apis[plugin.name] = plugin.setup(context);
		}

		return apis as PluginApis<P>;
	}

	/**
	 * A family's faces load one subset, weight, or style at a time, so the
	 * same family can arrive more than once; each arrival can change
	 * advances, so each forgets that family's measurements again.
	 */
	#fontsLoaded(families: readonly string[]): void {
		this.#measurer.forget();

		if (families.length > 0 && this.#wasm.fontsLoaded([...families]) > 0) {
			this.#invalidate();
		}
	}

	/** Run the command named `name` with `args` and repaint on the next frame. */
	#runEncoded(name: string, args: unknown): boolean {
		const applied = this.#wasm.run(name, encode(args));
		this.#invalidate();

		return applied;
	}

	/** Repaint on the next frame whenever the document scrolls or the window resizes. */
	#listenForViewportChanges(): void {
		const signal = this.#abort.signal;

		this.#scroller().addEventListener("scroll", () => this.#invalidate(), {
			passive: true,
			signal,
		});
		window.addEventListener("resize", () => this.#invalidate(), { signal });
	}

	#invalidate(): void {
		if (this.#frame !== 0) {
			return;
		}

		this.#frame = requestAnimationFrame(() => this.#render());
	}

	#render(): void {
		cancelAnimationFrame(this.#frame);
		this.#frame = 0;

		// The canvas is as tall as the visible part of the document, which
		// depends on the document's height, which the frame measures. A
		// second pass settles a height change in the same frame.
		for (let pass = 0; pass < 2; pass += 1) {
			const changed = this.#syncSize();
			this.#wasm.setTop(this.#viewportTop());
			const [
				height = 0,
				shift = 0,
				laidOut = 0,
				measured = 0,
				drawn = 0,
				full = 0,
				painted = 0,
			] = this.#wasm.frame();
			this.#lastFrame = {
				height,
				laidOut,
				measured,
				drawn,
				full: full === 1,
				painted: painted === 1,
			};
			const heightChanged = height !== this.#height;
			this.#height = height;

			if (shift !== 0) {
				this.#scroller().scrollBy(0, shift);
			}

			if (!heightChanged && !changed) {
				break;
			}
		}
	}

	/** Size the canvas to the visible part of the document. Returns whether it changed. */
	#syncSize(): boolean {
		const visible = this.#visibleHeight();
		const height = Math.max(0, Math.min(this.#height, visible));
		const changed = Number.parseFloat(this.#canvas.style.height) !== height;
		const scale = window.devicePixelRatio;
		this.#spacer.style.height = `${Math.max(0, this.#height - height)}px`;

		if (changed) {
			this.#canvas.style.height = `${height}px`;
		}

		// Resizing to the same size is free: the editor compares first.
		const box = this.#canvas.getBoundingClientRect();
		this.#wasm.resize(
			this.#width,
			height,
			scale,
			devicePixels(box.left, box.width, scale),
			devicePixels(box.top, box.height, scale),
		);

		return changed;
	}

	/**
	 * Record new sizes and paint on the next frame. Resizing the canvas here
	 * would change an observed box inside the observer's own callback,
	 * which browsers report as a ResizeObserver loop.
	 */
	#observe(entries: readonly ResizeObserverEntry[]): void {
		for (const entry of entries) {
			if (entry.target === this.#element) {
				this.#elementResized(entry);
			}
		}

		this.#invalidate();
	}

	/** Wrap to the element's new content width. */
	#elementResized(entry: ResizeObserverEntry): void {
		this.#width = entry.contentBoxSize[0]?.inlineSize ??
			entry.contentRect.width;
	}

	/** Snap to the pixel grid and border widths this browser uses at the current ratio. */
	#snapToDevice(): void {
		const document = this.#element.ownerDocument;
		this.#wasm.setPixelGrid(pixelGrid(document, navigator.userAgent));
		this.#wasm.setBorderScale(borderScale(document) ?? 0);
	}

	/**
	 * Measure again when the device pixel ratio changes (browser zoom, or a
	 * window moving to another screen): browsers round line metrics to
	 * device pixels, so line positions move with it.
	 */
	#watchScale(): void {
		const query = matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
		query.addEventListener("change", () => {
			this.#measurer.forget();
			this.#wasm.forgetMeasurements();
			this.#snapToDevice();
			this.#invalidate();
			this.#watchScale();
		}, { once: true, signal: this.#abort.signal });
	}

	/** The element whose scrolling moves the document: the editor itself, or the page. */
	#scroller(): HTMLElement | Window {
		const overflow = getComputedStyle(this.#element).overflowY;

		return overflow === "auto" || overflow === "scroll"
			? this.#element
			: window;
	}

	#visibleHeight(): number {
		const scroller = this.#scroller();

		return scroller === window
			? window.innerHeight
			: this.#element.clientHeight;
	}

	/** The document coordinate at the canvas's top edge. */
	#viewportTop(): number {
		const canvas = this.#canvas.getBoundingClientRect();
		const element = this.#element.getBoundingClientRect();
		const scrolled = this.#scroller() === window ? 0 : this.#element.scrollTop;

		return Math.max(
			0,
			canvas.top - element.top - this.#element.clientTop + scrolled,
		);
	}

	#restartBlink(): void {
		clearTimeout(this.#blink);

		if (!this.#focused) {
			return;
		}

		let on = true;
		const tick = () => {
			on = !on;
			this.#wasm.setCaretOn(on);
			this.#invalidate();
			this.#blink = window.setTimeout(tick, CARET_BLINK_MS);
		};
		this.#blink = window.setTimeout(tick, CARET_BLINK_MS);
	}
}

/**
 * The grid this browser snaps DOM text baselines and backgrounds to.
 *
 * Firefox and WebKit snap to device pixels. Chromium does too when it lays
 * out in device units, as it does on real high-density screens; when it
 * lays out in CSS units (as under device emulation), it snaps to whole CSS
 * pixels and scales. Layout in device units can represent a box 1/64 of a
 * device pixel wide, which CSS-unit layout rounds to zero.
 */
export function pixelGrid(
	document: Document,
	userAgent: string,
): "device" | "css" {
	const scale = window.devicePixelRatio;

	if (scale === 1 || detectBreaks(userAgent) !== "chromium") {
		return "device";
	}

	const probe = document.createElement("div");
	probe.style.cssText = `position:absolute;left:0;top:0;height:1px;width:${
		1 / (64 * scale)
	}px;visibility:hidden`;
	document.body.append(probe);
	const width = probe.getBoundingClientRect().width;
	probe.remove();

	return width > 0 ? "device" : "css";
}

/**
 * How many units per CSS pixel this browser snaps border widths down to: the
 * device pixel ratio, 1 when it lays out in CSS pixels, or `null` when it
 * keeps exact widths. A 1.7 px border tells them apart.
 */
export function borderScale(document: Document): number | null {
	const width = 1.7;
	const outer = document.createElement("div");
	outer.style.cssText =
		`position:absolute;left:0;top:0;visibility:hidden;border-left:${width}px solid;width:10px;height:1px`;
	const inner = document.createElement("div");
	inner.style.cssText = "width:1px;height:1px";
	outer.append(inner);
	document.body.append(outer);
	const used = inner.getBoundingClientRect().left -
		outer.getBoundingClientRect().left;
	outer.remove();

	for (const scale of [window.devicePixelRatio, 1]) {
		if (
			Math.abs(used - Math.max(1, Math.floor(width * scale)) / scale) <
				BORDER_SNAP_TOLERANCE
		) {
			return scale;
		}
	}

	return null;
}

/**
 * How many device pixels a box spanning `size` CSS px from `start` covers.
 * Browsers snap a box's edges to device pixels, not its size, so at
 * fractional ratios this can differ from `round(size * scale)` by one.
 * Computed here rather than observed with `device-pixel-content-box`,
 * which Firefox and Safari don't support, so the size is exact in every
 * engine and only the element needs observing.
 */
function devicePixels(start: number, size: number, scale: number): number {
	return Math.max(
		0,
		Math.round((start + size) * scale) - Math.round(start * scale),
	);
}

/** Resolve in the next animation frame. */
function nextFrame(): Promise<void> {
	return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

/** The names of everything `extensions` register as `kind`: their commands or their queries. */
function registeredNames(
	extensions: Extensions,
	kind: "commands" | "queries",
): string[] {
	return extensions.flatMap((extension) =>
		extension[kind].map((spec) => spec.name)
	);
}

/** The JSON options `WebEditor` reads, with each default filled in. */
function wasmOptions<E extends Extensions, P extends Plugins>(
	options: EditorOptions<E, P>,
): string {
	return JSON.stringify({
		client: options.client ?? 1,
		breaks: options.breaks ?? detectBreaks(navigator.userAgent),
		overscan: options.overscan ?? window.innerHeight,
		theme: options.theme ?? {},
	});
}

/** The width of `element`'s content box, with its fraction. */
function contentWidth(element: HTMLElement): number {
	const style = getComputedStyle(element);
	const insets = [
		"borderLeftWidth",
		"borderRightWidth",
		"paddingLeft",
		"paddingRight",
	] as const;
	return insets.reduce(
		(width, inset) => width - Number.parseFloat(style[inset]),
		element.getBoundingClientRect().width,
	);
}

/**
 * The line-breaking tailoring of the browser running this code. CSS leaves
 * the details to each engine and there is no feature test for them, so this
 * reads the user agent.
 */
export function detectBreaks(userAgent: string): LineBreaks {
	return /(Chrome|Chromium|Edg)\//.test(userAgent) ? "chromium" : "icu";
}

type Call = (args?: unknown) => unknown;

/** An object with one method per name, each calling `invoke`; see `@canvist/headless`. */
function calls<Methods>(
	names: readonly string[],
	invoke: (name: string, args: unknown) => unknown,
): Methods {
	const methods: Record<string, Call> = Object.fromEntries(
		names.map((name) => [name, (args?: unknown) => invoke(name, args)]),
	);

	return methods as Methods;
}

function encode(args: unknown): string {
	return args === undefined ? "null" : JSON.stringify(args);
}
