/**
 * Renders the "Type into the real engine" document to HTML at build time
 * with `@canvist/headless/ssr`, so the landing page shows it before any
 * JavaScript runs and `@canvist/web` hydrates it without moving.
 */
import { createRenderer, type WasmSource } from "@canvist/headless/ssr";
import { fluid, pageColor, themedCss } from "./cv-doc-css.ts";
import { PIXEL_CONTENT, pixelTheme, type Scheme } from "./pixel-document.ts";

/**
 * Widths to render at, in CSS px. Server-rendered HTML wraps where the
 * browser wraps it, so only the stylesheet's `.cv-doc` width depends on
 * the width; rendering at two widths checks that.
 */
const WIDTHS = [320, 640] as const;

export interface PixelDocument {
	/** The `.cv-doc` fragment to put inside the editor's element. */
	readonly html: string;
	/** The stylesheet for both colour schemes. */
	readonly css: string;
	/** The page colour behind the document in each scheme, which the canvas fills too. */
	readonly page: Readonly<Record<Scheme, string>>;
}

let rendering: Promise<PixelDocument> | undefined;

/**
 * The document as HTML and its stylesheet, rendered once per build. The
 * build bundles `@canvist/headless` away from its rendering module, so the
 * caller passes it; only the first call's `wasm` is used.
 */
export function renderPixelDocument(wasm: WasmSource): Promise<PixelDocument> {
	rendering ??= render(wasm);
	return rendering;
}

async function render(wasm: WasmSource): Promise<PixelDocument> {
	const [light, dark] = await Promise.all([
		renderScheme("light", wasm),
		renderScheme("dark", wasm),
	]);
	if (light.html !== dark.html) {
		throw new Error("The light and dark themes rendered different HTML");
	}
	return {
		html: light.html,
		css: themedCss(light.css, dark.css),
		page: { light: pageColor(light.css), dark: pageColor(dark.css) },
	};
}

async function renderScheme(
	scheme: Scheme,
	wasm: WasmSource,
): Promise<{ html: string; css: string }> {
	const renderer = await createRenderer({ theme: pixelTheme(scheme), wasm });
	const [narrow, wide] = WIDTHS.map((width) =>
		renderer.html(PIXEL_CONTENT, { width })
	);
	if (!narrow || !wide || narrow.html !== wide.html) {
		throw new Error(
			"The document's HTML depends on its width, so it can't fill a fluid editor",
		);
	}
	renderer[Symbol.dispose]();
	return { html: wide.html, css: fluid(wide.css, WIDTHS[1]) };
}
