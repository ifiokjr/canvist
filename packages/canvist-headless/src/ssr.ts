/**
 * Server-side rendering for canvist documents: semantic HTML and a
 * stylesheet laid out exactly as `@canvist/web` paints the document, so a
 * page can show the text before any JavaScript runs and the live editor
 * can take its place without moving a pixel. SVG images of the whole
 * document, for previews and thumbnails, come from the same layout.
 *
 * Rendering lives in its own WebAssembly module, so the document entry
 * point stays small. It measures text with its own copy of each font,
 * never the system's: the bundled Atkinson Hyperlegible stands in for
 * every family until you register yours.
 *
 * ```ts
 * import { createRenderer } from "@canvist/headless/ssr";
 *
 * const renderer = await createRenderer({ theme: { scheme: "dark" } });
 * const { html, css } = renderer.html(
 * 	{ format: "markdown", input: "# Release notes" },
 * 	{ width: 640 },
 * );
 * ```
 *
 * @module
 */
import init, { WasmRenderer } from "../wasm-ssr/canvist_headless_ssr.js";
import { starterKit } from "./extensions.generated.ts";
import { bundled, type WasmSource } from "./load.ts";
import type { Content, Extensions, StarterKit, ThemeOptions } from "./types.ts";

export { type CanvistError, isCanvistError } from "./engine.ts";
export type { WasmSource } from "./load.ts";
export type { Content, StarterKit, ThemeOptions } from "./types.ts";

const BUNDLED = new URL(
	"../wasm-ssr/canvist_headless_ssr_bg.wasm",
	import.meta.url,
);

let loading: Promise<void> | undefined;

/**
 * Load the rendering WebAssembly module once. Later calls return the first
 * call's promise. Without a source, Node, Deno, and Bun read the bundled
 * file; runtimes without a file system pass the module they imported.
 */
export function loadRenderer(source?: WasmSource): Promise<void> {
	loading ??= (async () => {
		await init({ module_or_path: source ?? await bundled(BUNDLED) });
	})();
	return loading;
}

/** Options for {@link createRenderer}. */
export interface RendererOptions<E extends Extensions> {
	/** The extensions documents use, TipTap's StarterKit by default. Pass the editor's. */
	readonly extensions?: E;
	/** The theme. Pass the browser editor's `theme`, or the HTML won't line up with it. */
	readonly theme?: ThemeOptions;
	/**
	 * TrueType or OpenType font files to measure SVG text with: the files
	 * the page's `@font-face` rules load. HTML is laid out by the browser
	 * and needs none.
	 */
	readonly fonts?: readonly (ArrayBuffer | ArrayBufferView)[];
	/** Where the rendering module comes from; see {@link loadRenderer}. */
	readonly wasm?: WasmSource;
}

/** How wide to render. */
export interface RenderOptions {
	/**
	 * The content width in CSS pixels. It must equal the width of the
	 * element the browser editor hydrates in.
	 */
	readonly width: number;
}

/** Server-rendered HTML and the stylesheet it needs. */
export interface RenderedHtml {
	/** A `<div class="cv-doc">` fragment. Put it inside the element `createEditor` hydrates. */
	readonly html: string;
	/** The stylesheet, scoped under `.cv-doc`. */
	readonly css: string;
}

/** Load the rendering module and create a renderer. */
export async function createRenderer<
	const E extends Extensions = StarterKit,
>(options: RendererOptions<E> = {}): Promise<Renderer<E>> {
	await loadRenderer(options.wasm);
	const renderer = new Renderer<E>(
		(options.extensions ?? starterKit) as E,
		options.theme ?? {},
	);

	for (const font of options.fonts ?? []) {
		renderer.registerFont(font);
	}

	return renderer;
}

/**
 * Renders documents to HTML, CSS, and SVG. It caches text measurements,
 * so keep one per server and reuse it. Create one with
 * {@link createRenderer}.
 */
export class Renderer<E extends Extensions = StarterKit> {
	readonly #wasm: WasmRenderer;

	/** @internal Use {@link createRenderer}, which loads the module first. */
	constructor(extensions: E, theme: ThemeOptions) {
		this.#wasm = new WasmRenderer(
			extensions.map((extension) => extension.name),
			JSON.stringify(theme),
		);
	}

	/**
	 * Register a TrueType or OpenType font for SVG text, and return the
	 * family names it added, such as `["Inter"]`.
	 */
	registerFont(data: ArrayBuffer | ArrayBufferView): string[] {
		return this.#wasm.registerFont(bytesOf(data));
	}

	/** `content` as semantic HTML and its stylesheet. */
	html(content: Content<E>, options: RenderOptions): RenderedHtml {
		const [html = "", css = ""] = this.#wasm.html(
			...encoded(content),
			options.width,
		);

		return { html, css };
	}

	/** `content` as a complete HTML page with no body margin, ready to serve. */
	page(content: Content<E>, options: RenderOptions): string {
		const { html, css } = this.html(content, options);

		return `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><style>${css}</style></head><body style="margin:0">${html}</body></html>`;
	}

	/** The whole of `content` as an SVG image. */
	svg(content: Content<E>, options: RenderOptions): string {
		return this.#wasm.svg(...encoded(content), options.width);
	}

	/** Release the renderer's memory now rather than when it is collected. */
	[Symbol.dispose](): void {
		this.#wasm.free();
	}
}

/** `content` as the `[format, input]` pair the module reads. */
function encoded<E extends Extensions>(
	content: Content<E>,
): [format: string, input: string] {
	return "format" in content
		? [content.format, content.input]
		: ["json", JSON.stringify(content)];
}

function bytesOf(data: ArrayBuffer | ArrayBufferView): Uint8Array {
	return data instanceof ArrayBuffer
		? new Uint8Array(data)
		: new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
}
