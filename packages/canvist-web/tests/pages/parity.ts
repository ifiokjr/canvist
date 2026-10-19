/**
 * One parity fixture rendered three ways, side by side: the canvas editor,
 * the server-rendered HTML the editor would hydrate from, and the SVG
 * renderer's output. `window.parity` reports when everything is painted,
 * and the geometry comparison.
 */
import { fontFaceCss, fonts, type FontSource } from "../../src/fonts.ts";
import { createEditor, type ThemeOptions } from "../../src/mod.ts";
import { fixture } from "../parity/fixtures.ts";
import {
	type CanvasGeometry,
	compare,
	type GeometryReport,
} from "../parity/geometry.ts";

const GAP = 16;
/** How many frame pairs to wait for the DOM to stop moving. */
const MAX_SETTLE_FRAMES = 20;
const params = new URLSearchParams(location.search);
const chosen = fixture(params.get("fixture") ?? "system-sans");
const width = chosen.width ?? 480;
const theme: ThemeOptions = {
	...chosen.theme,
	...(params.get("scheme") === "dark" ? { scheme: "dark" as const } : {}),
};
const faces: FontSource[] = (chosen.fonts ?? []).map((font) => ({
	family: font.family,
	source: `/${font.file}`,
	...(font.weight === undefined ? {} : { weight: font.weight }),
	...(font.style === undefined ? {} : { style: font.style }),
	...(font.unicodeRange === undefined
		? {}
		: { unicodeRange: font.unicodeRange }),
	display: "block",
	fallback: false,
}));

const host = document.querySelector<HTMLElement>("#canvas")!;
host.style.width = `${width}px`;
const breaks = params.get("breaks");
const editor = await createEditor(host, {
	content: chosen.doc,
	theme,
	...(breaks === "chromium" || breaks === "icu" ? { breaks } : {}),
	plugins: [fonts({ faces })],
});
await editor.ready();

// What server rendering sends down: the HTML and CSS, with the same faces
// declared as @font-face.
const { html, css } = editor.toHtml();
const style = document.createElement("style");
style.textContent = `${fontFaceCss(faces)}\n${css}`;
document.head.append(style);

const dom = document.querySelector<HTMLElement>("#dom-frame")!;
dom.style.left = `${8 + width + GAP}px`;
dom.innerHTML = html;

const svg = document.querySelector<HTMLElement>("#svg-frame")!;
svg.style.left = `${8 + 2 * (width + GAP)}px`;
svg.innerHTML = editor.toSvg();

// Load whatever faces the HTML uses that the canvas did not already load.
await document.fonts.load(
	`16px ${theme.bodyFamily ?? "sans-serif"}`,
	dom.textContent ?? "",
);
await document.fonts.ready;
await editor.ready();
editor.render();

const geometry: CanvasGeometry = editor.geometry();
const root = dom.querySelector(".cv-doc")!;
const height = Math.ceil(
	Math.max(geometry.height, root.getBoundingClientRect().height),
);

for (const frame of document.querySelectorAll<HTMLElement>(".frame")) {
	frame.style.width = `${width}px`;
	frame.style.height = `${height}px`;
}

const report = await settledComparison(geometry, root);
Object.assign(globalThis, {
	parity: { ready: true, width, height, report, stats: editor.stats() },
	editor,
});

/**
 * Compare once the DOM has settled. Under load, a face can finish loading
 * after `document.fonts.ready` resolves, and the DOM reflows when it does,
 * so compare until two frames in a row measure the same DOM.
 */
async function settledComparison(
	geometry: CanvasGeometry,
	root: Element,
): Promise<GeometryReport> {
	let previous = "";

	for (let attempt = 0; attempt < MAX_SETTLE_FRAMES; attempt += 1) {
		await document.fonts.ready;
		await new Promise((resolve) =>
			requestAnimationFrame(() => requestAnimationFrame(resolve))
		);
		const report = compare(geometry, root);
		const measured = JSON.stringify(report);

		if (measured === previous) {
			return report;
		}

		previous = measured;
	}

	throw new Error(`the DOM did not settle in ${MAX_SETTLE_FRAMES} frames`);
}
