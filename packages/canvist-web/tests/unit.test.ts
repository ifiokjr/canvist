/**
 * Unit tests for the parts of `@canvist/web` that need no browser.
 *
 * @module
 */
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { fontFaceCss, stack } from "../src/fonts.ts";
import { splitFamilies } from "../src/measure.ts";

describe("splitFamilies", () => {
	it("splits on commas outside quotes", () => {
		assert.deepEqual(
			splitFamilies(`"JetBrains Mono", 'Noto, Sans', Menlo , monospace`),
			[`"JetBrains Mono"`, `'Noto, Sans'`, "Menlo", "monospace"],
		);
	});

	it("drops empty entries", () => {
		assert.deepEqual(splitFamilies(" , serif,"), ["serif"]);
	});
});

describe("stack", () => {
	it("puts the metric-compatible fallback straight after the family", () => {
		assert.equal(
			stack("Inter", "system-ui", "sans-serif"),
			`"Inter", "Inter Fallback", system-ui, sans-serif`,
		);
	});
});

describe("fontFaceCss", () => {
	it("writes the same faces as @font-face rules for server rendering", () => {
		const css = fontFaceCss([
			{ family: "Inter", source: "/fonts/inter.woff2", weight: "100 900" },
			{
				family: "Noto Sans JP",
				source: ["/fonts/jp-kana.woff2", "/fonts/jp-kana.ttf"],
				unicodeRange: "U+3000-30FF",
				display: "optional",
			},
		]);

		assert.equal(
			css,
			[
				`@font-face{font-family:"Inter";src:url("/fonts/inter.woff2");font-weight:100 900;font-display:swap}`,
				`@font-face{font-family:"Noto Sans JP";src:url("/fonts/jp-kana.woff2"),url("/fonts/jp-kana.ttf");` +
				`unicode-range:U+3000-30FF;font-display:optional}`,
			].join("\n"),
		);
	});

	it("skips faces given as bytes, which have no URL", () => {
		assert.equal(
			fontFaceCss([{ family: "Bytes", source: new ArrayBuffer(8) }]),
			"",
		);
	});
});
