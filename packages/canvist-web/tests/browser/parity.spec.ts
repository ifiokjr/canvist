/**
 * Pixel parity: every fixture rendered by the canvas editor, as
 * server-rendered HTML, and as SVG, compared by geometry (exact, to the
 * browser's layout unit) and by pixels (pixelmatch and SSIM, within each
 * platform's thresholds). Each case writes its images and numbers to
 * `report/parity/`, which `scripts/parity-report.ts` turns into a
 * side-by-side HTML report.
 *
 * @module
 */
import { expect, test } from "@playwright/test";
import { mkdir, writeFile } from "node:fs/promises";
import { platform } from "node:os";
import pixelmatch from "pixelmatch";
import { PNG } from "pngjs";
import { ssim } from "ssim.js";
import { FIXTURES } from "../parity/fixtures.ts";
import type { GeometryReport } from "../parity/geometry.ts";
import { thresholds } from "../parity/thresholds.ts";

/** A rendering variant: device pixel ratio (browser zoom shows up as a fractional one) and colour scheme. */
interface Variant {
	readonly scale: number;
	readonly scheme: "light" | "dark";
}

/** The device pixel ratios browser zoom produces at 125%, 150%, and 175% on a 1x screen. */
const ZOOM_125 = 1.25;
const ZOOM_150 = 1.5;
const ZOOM_175 = 1.75;

/** The page size, in CSS px: wide and tall enough for the three renderings side by side. */
const VIEWPORT_WIDTH = 1600;
const VIEWPORT_HEIGHT = 1400;

const EVERYWHERE: readonly Variant[] = [
	{ scale: 1, scheme: "light" },
	{ scale: 2, scheme: "light" },
];
/** Fixtures that also run dark, at 3x, and at the fractional ratios browser zoom produces. */
const EXTRA: Readonly<Record<string, readonly Variant[]>> = {
	"system-sans": [
		{ scale: 2, scheme: "dark" },
		{ scale: ZOOM_125, scheme: "light" },
		{ scale: ZOOM_150, scheme: "light" },
	],
	inter: [
		{ scale: 2, scheme: "dark" },
		{ scale: 3, scheme: "light" },
		{ scale: ZOOM_125, scheme: "light" },
		{ scale: ZOOM_175, scheme: "light" },
	],
	cjk: [{ scale: 2, scheme: "dark" }, { scale: ZOOM_150, scheme: "light" }],
	rtl: [{ scale: 2, scheme: "dark" }, { scale: 3, scheme: "light" }],
	"emoji-system": [{ scale: 3, scheme: "light" }],
};

const REPORT = new URL("../../report/parity/", import.meta.url);

/** The page's result once everything is painted. */
interface PageResult {
	readonly ready: true;
	readonly width: number;
	readonly height: number;
	readonly report: GeometryReport;
}

/**
 * Pixel differences between two same-sized screenshots, and a heatmap of
 * them.
 *
 * - `mismatch` counts pixels pixelmatch finds different that are not
 *   antialiasing: a glyph, line, or box in a different place or shape.
 * - `antialiasing` is the mean colour difference over pixels where either
 *   image has ink, so differences in how edges are shaded (coverage, gamma,
 *   font smoothing) show up without failing the structural check.
 */
function comparePixels(
	a: PNG,
	b: PNG,
): { mismatch: number; antialiasing: number; ssim: number; heatmap: PNG } {
	const { width, height } = a;
	const diff = new PNG({ width, height });
	const changed = pixelmatch(a.data, b.data, diff.data, width, height, {
		threshold: 0.1,
		includeAA: false,
		alpha: 0.2,
	});
	const heatmap = new PNG({ width, height });
	let edgeDelta = 0;
	let edges = 0;

	for (let index = 0; index < a.data.length; index += 4) {
		const delta = channelDelta(a, b, index);

		if (delta > 0) {
			edgeDelta += delta;
			edges += 1;
		}

		paintHeat(heatmap, b, index, delta);
	}

	const similarity = ssim(
		{ data: new Uint8ClampedArray(a.data), width, height },
		{ data: new Uint8ClampedArray(b.data), width, height },
	).mssim;

	return {
		mismatch: changed / (width * height),
		antialiasing: edges === 0 ? 0 : edgeDelta / edges / 255,
		ssim: similarity,
		heatmap,
	};
}

/** The largest difference of any colour channel between the pixels starting at byte `index`. */
function channelDelta(a: PNG, b: PNG, index: number): number {
	let delta = 0;

	for (let channel = 0; channel < 3; channel += 1) {
		delta = Math.max(
			delta,
			Math.abs((a.data[index + channel] ?? 0) - (b.data[index + channel] ?? 0)),
		);
	}

	return delta;
}

/** Unchanged heatmap pixels are the reference's three channels averaged, then dimmed to a quarter. */
const HEATMAP_DIMMING = 12;

/**
 * Paint the heatmap pixel at byte `index`: the reference image, dimmed to
 * gray, where nothing differs, and red through yellow by how much it does.
 */
function paintHeat(
	heatmap: PNG,
	reference: PNG,
	index: number,
	delta: number,
): void {
	const { data } = reference;
	const gray = Math.round(
		((data[index] ?? 0) + (data[index + 1] ?? 0) + (data[index + 2] ?? 0)) /
			HEATMAP_DIMMING,
	);
	heatmap.data[index] = delta === 0 ? gray : 255;
	heatmap.data[index + 1] = delta === 0 ? gray : Math.min(255, delta * 3);
	heatmap.data[index + 2] = delta === 0 ? gray : 0;
	heatmap.data[index + 3] = 255;
}

for (const fixture of FIXTURES) {
	for (const variant of [...EVERYWHERE, ...(EXTRA[fixture.id] ?? [])]) {
		const name = `${fixture.id} @${variant.scale}x ${variant.scheme}`;

		test(name, async ({ browser, browserName }) => {
			const context = await browser.newContext({
				deviceScaleFactor: variant.scale,
				viewport: { width: VIEWPORT_WIDTH, height: VIEWPORT_HEIGHT },
			});
			const page = await context.newPage();
			const errors: string[] = [];
			page.on("pageerror", (error) => errors.push(error.message));
			const breaks = process.env.CANVIST_BREAKS === undefined
				? ""
				: `&breaks=${process.env.CANVIST_BREAKS}`;
			await page.goto(
				`/parity.html?fixture=${fixture.id}&scheme=${variant.scheme}${breaks}`,
			);
			await page.waitForFunction(() =>
				(globalThis as { parity?: { ready: boolean } }).parity?.ready === true
			);
			const result = await page.evaluate(() =>
				(globalThis as unknown as { parity: PageResult }).parity
			);
			expect(errors).toEqual([]);

			const shots = {
				canvas: PNG.sync.read(await page.locator("#canvas-frame").screenshot()),
				dom: PNG.sync.read(await page.locator("#dom-frame").screenshot()),
				svg: PNG.sync.read(await page.locator("#svg-frame").screenshot()),
			};
			const canvas = comparePixels(shots.canvas, shots.dom);
			const svg = comparePixels(shots.svg, shots.dom);
			await context.close();

			const host = `${browserName}-${platform()}`;
			const slug = `${fixture.id}-${variant.scale}x-${variant.scheme}`;
			const folder = new URL(`${host}/`, REPORT);
			await mkdir(folder, { recursive: true });
			await Promise.all([
				writeFile(
					new URL(`${slug}-canvas.png`, folder),
					PNG.sync.write(shots.canvas),
				),
				writeFile(
					new URL(`${slug}-dom.png`, folder),
					PNG.sync.write(shots.dom),
				),
				writeFile(
					new URL(`${slug}-svg.png`, folder),
					PNG.sync.write(shots.svg),
				),
				writeFile(
					new URL(`${slug}-diff.png`, folder),
					PNG.sync.write(canvas.heatmap),
				),
				writeFile(
					new URL(`${slug}-svg-diff.png`, folder),
					PNG.sync.write(svg.heatmap),
				),
			]);

			const limits = thresholds(browserName, platform(), fixture.id);
			const max = result.report.max;
			const vertical = Math.max(max.height, max.block, max.baseline, max.line);
			const horizontal = Math.max(max.run, max.grapheme);
			const summary = {
				host,
				slug,
				fixture: fixture.id,
				title: fixture.title,
				...variant,
				vertical,
				horizontal,
				pixels: canvas.mismatch,
				antialiasing: canvas.antialiasing,
				ssim: canvas.ssim,
				svgPixels: svg.mismatch,
				svgSsim: svg.ssim,
				limits,
				max: result.report.max,
				counts: result.report.counts,
				worst: result.report.worst.slice(0, 12),
			};
			await writeFile(
				new URL(`${slug}.json`, folder),
				JSON.stringify(summary, null, "\t"),
			);

			expect.soft(vertical, "block, line, and baseline positions")
				.toBeLessThanOrEqual(limits.vertical);
			expect.soft(horizontal, "run and caret positions").toBeLessThanOrEqual(
				limits.horizontal,
			);
			expect.soft(canvas.mismatch, "share of differing pixels")
				.toBeLessThanOrEqual(limits.pixels);
			expect.soft(canvas.ssim, "structural similarity").toBeGreaterThanOrEqual(
				limits.ssim,
			);
		});
	}
}
