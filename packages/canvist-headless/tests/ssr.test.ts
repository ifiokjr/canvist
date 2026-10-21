/**
 * Server-side rendering from TypeScript: HTML, CSS, pages, and SVG.
 *
 * @module
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

import { createEngine } from "../src/mod.ts";
import { createRenderer, isCanvistError } from "../src/ssr.ts";

/** The Inter file `examples/web` serves and renders with. */
const INTER = new URL(
	"../../../examples/web/public/fonts/inter-variable.ttf",
	import.meta.url,
);

const NOTES = {
	format: "markdown",
	input: "# Release notes\n\n- Faster **typing**",
} as const;

test("renders HTML and a stylesheet scoped to .cv-doc", async () => {
	const renderer = await createRenderer();
	const { html, css } = renderer.html(NOTES, { width: 640 });

	assert.match(html, /^<div class="cv-doc"/);
	assert.match(html, /<h1[^>]*>Release notes<\/h1>/);
	assert.match(html, /<strong[^>]*>typing<\/strong>/);
	assert.match(css, /\.cv-doc/);
});

test("ProseMirror JSON renders like the text it came from", async () => {
	const renderer = await createRenderer();
	const engine = await createEngine();
	using doc = engine.parse(NOTES.format, NOTES.input);

	assert.deepEqual(
		renderer.html(doc.toJson(), { width: 480 }),
		renderer.html(NOTES, { width: 480 }),
	);
});

test("the theme reaches the stylesheet", async () => {
	const light = await createRenderer();
	const dark = await createRenderer({
		theme: { scheme: "dark", bodyFamily: "Inter, sans-serif", size: 18 },
	});
	const lightCss = light.html(NOTES, { width: 640 }).css;
	const darkCss = dark.html(NOTES, { width: 640 }).css;

	assert.notEqual(darkCss, lightCss);
	assert.match(darkCss, /Inter, sans-serif/);
	assert.match(darkCss, /18px/);
});

test("page wraps the fragment and stylesheet in a document", async () => {
	const renderer = await createRenderer();
	const page = renderer.page(NOTES, { width: 640 });
	const { html, css } = renderer.html(NOTES, { width: 640 });

	assert.match(page, /^<!doctype html>/);
	assert.ok(page.includes(`<style>${css}</style>`));
	assert.ok(page.includes(`<body style="margin:0">${html}</body>`));
});

test("SVG text is measured with registered fonts", async () => {
	const renderer = await createRenderer({
		theme: { bodyFamily: "Inter, sans-serif" },
	});
	const bundled = renderer.svg(NOTES, { width: 640 });
	const families = renderer.registerFont(await readFile(INTER));
	const inter = renderer.svg(NOTES, { width: 640 });

	assert.deepEqual(families, ["Inter"]);
	assert.match(bundled, /^<svg/);
	assert.match(inter, /^<svg/);
	assert.notEqual(inter, bundled, "Inter measures differently");
});

test("fonts can be registered when the renderer is created", async () => {
	const font = await readFile(INTER);
	const created = await createRenderer({
		theme: { bodyFamily: "Inter, sans-serif" },
		fonts: [font],
	});
	const registered = await createRenderer({
		theme: { bodyFamily: "Inter, sans-serif" },
	});
	registered.registerFont(font.buffer.slice(
		font.byteOffset,
		font.byteOffset + font.byteLength,
	));

	assert.equal(
		created.svg(NOTES, { width: 320 }),
		registered.svg(NOTES, { width: 320 }),
	);
});

test("bad themes and widths are CanvistErrors", async () => {
	const renderer = await createRenderer();
	const invalidArgs = (error: unknown) =>
		isCanvistError(error) && error.code === "invalidArgs";

	await assert.rejects(
		createRenderer({ theme: { size: -1 } }),
		invalidArgs,
	);
	assert.throws(() => renderer.html(NOTES, { width: 0 }), invalidArgs);
	assert.throws(() => renderer.svg(NOTES, { width: Number.NaN }), invalidArgs);
	assert.throws(
		() => renderer.html({ format: "json", input: "{" }, { width: 640 }),
		isCanvistError,
	);
});
