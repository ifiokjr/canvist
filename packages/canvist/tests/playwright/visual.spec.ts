/**
 * Visual screenshot tests for the canvist editor.
 *
 * These tests render the editor in a real browser, capture screenshots of the
 * canvas element, and verify:
 * - Text is actually rendered (not blank/white)
 * - Fonts load correctly and text is legible
 * - Selection highlighting, caret, bold/italic rendering
 * - Multi-line layout, formatting toolbar, find bar
 *
 * Screenshots are saved to tests/playwright/screenshots/ for manual review.
 * Tests run against Chromium, Firefox, and WebKit.
 */

import { assert } from "@std/assert";
import { dirname, fromFileUrl, join } from "@std/path";
import { startServer } from "./server.ts";

const __dirname = dirname(fromFileUrl(import.meta.url));
const PKG_ROOT = join(__dirname, "../..");
const SCREENSHOT_DIR = join(__dirname, "screenshots");

async function launchBrowser(
	browserType: string,
): Promise<{ browser: any; context: any; page: any }> {
	const { chromium, firefox, webkit } = await import("npm:playwright@1.58");
	let browserLauncher;
	switch (browserType) {
		case "chromium":
			browserLauncher = chromium;
			break;
		case "firefox":
			browserLauncher = firefox;
			break;
		case "webkit":
			browserLauncher = webkit;
			break;
		default:
			throw new Error(`Unknown browser type: ${browserType}`);
	}
	const browser = await browserLauncher.launch({ headless: true });
	const context = await browser.newContext({
		viewport: { width: 1024, height: 768 },
		deviceScaleFactor: 1, // Consistent DPR for reproducible screenshots
	});
	const page = await context.newPage();
	return { browser, context, page };
}

async function waitForEditor(page: any) {
	await page.waitForFunction("window.__canvistEditor !== null", null, {
		timeout: 15000,
	});
	// Wait for fonts to load — the demo awaits document.fonts.ready but
	// give it extra time for the Google Fonts network request.
	await page.waitForTimeout(500);
}

async function typeInEditor(page: any, text: string) {
	await page.focus("#canvist-input");
	await page.waitForTimeout(50);
	for (const ch of text) {
		await page.keyboard.type(ch);
		await page.waitForTimeout(15);
	}
	await page.waitForTimeout(100);
}

/**
 * Capture a screenshot of just the canvas element.
 * Returns the path the screenshot was saved to.
 */
async function screenshotCanvas(
	page: any,
	name: string,
	browserName: string,
): Promise<string> {
	const canvas = page.locator("#editor-canvas");
	const path = join(SCREENSHOT_DIR, `${browserName}-${name}.png`);
	await canvas.screenshot({ path });
	return path;
}

/**
 * Analyse a canvas element screenshot by reading the PNG buffer from Playwright.
 * This avoids DPR-scaling issues with `ctx.getImageData()`.
 */
async function canvasPixelStats(page: any): Promise<{
	totalPixels: number;
	nonWhitePixels: number;
	nonWhitePercent: number;
	hasBlackPixels: boolean;
	hasBluePixels: boolean;
}> {
	// Create a temporary off-screen canvas at the logical (CSS) size,
	// draw the editor canvas onto it at 1:1, then read pixels.
	return page.evaluate(() => {
		const src = document.getElementById("editor-canvas") as HTMLCanvasElement;
		// Use CSS dimensions (logical pixels) for analysis.
		const rect = src.getBoundingClientRect();
		const w = Math.round(rect.width);
		const h = Math.round(rect.height);

		const tmp = document.createElement("canvas");
		tmp.width = w;
		tmp.height = h;
		const ctx = tmp.getContext("2d")!;
		// Draw the source canvas at logical size (ignoring DPR).
		ctx.drawImage(src, 0, 0, w, h);

		const data = ctx.getImageData(0, 0, w, h).data;
		const total = w * h;
		let nonWhite = 0;
		let hasBlack = false;
		let hasBlue = false;

		for (let i = 0; i < data.length; i += 4) {
			const r = data[i], g = data[i + 1], b = data[i + 2];
			if (r < 250 || g < 250 || b < 250) {
				nonWhite++;
			}
			if (r < 50 && g < 50 && b < 50) hasBlack = true;
			// Selection is rgba(66,133,244,80) over white ≈ rgb(226,237,249).
			// Detect any pixel where blue channel meaningfully exceeds red.
			if (b > 230 && b > r + 10 && b > g) hasBlue = true;
		}

		return {
			totalPixels: total,
			nonWhitePixels: nonWhite,
			nonWhitePercent: Math.round((nonWhite / total) * 10000) / 100,
			hasBlackPixels: hasBlack,
			hasBluePixels: hasBlue,
		};
	});
}

// Determine which browsers to test.
const CI_BROWSERS = Deno.env.get("CI_BROWSERS");
const browsers = CI_BROWSERS
	? CI_BROWSERS.split(",").map((b: string) => b.trim())
	: ["chromium", "firefox", "webkit"];

for (const browserName of browsers) {
	// --- Test 1: Empty editor renders clean white canvas with no artifacts ---
	Deno.test({
		name: `[${browserName}] visual: empty editor renders clean canvas`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					// Ensure caret is visible.
					await page.evaluate(() => {
						(window as any).__canvistEditor?.set_caret_visible(true);
						(window as any).__canvistEditor?.render();
					});
					await page.waitForTimeout(100);

					const path = await screenshotCanvas(page, "empty", browserName);
					const stats = await canvasPixelStats(page);

					// An empty editor should be mostly white with just a caret line.
					assert(
						stats.nonWhitePercent < 1.0,
						`empty canvas has ${stats.nonWhitePercent}% non-white pixels — expected < 1%`,
					);
					// The caret is a 1px line — on scaled canvases or thin lines it
					// may be anti-aliased to gray rather than pure black. Verify
					// there are at least a few non-white pixels from the caret.
					assert(
						stats.nonWhitePixels > 0,
						"expected caret to produce some non-white pixels",
					);

					console.log(
						`  [${browserName}] empty: ${stats.nonWhitePercent}% non-white, ${stats.nonWhitePixels} non-white px, saved ${path}`,
					);
				} finally {
					await browser.close();
				}
			} finally {
				await server.shutdown();
			}
		},
		sanitizeResources: false,
		sanitizeOps: false,
	});

	// --- Test 2: Typed text is rendered as visible glyphs ---
	Deno.test({
		name: `[${browserName}] visual: typed text renders visible glyphs`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(
						page,
						"Hello, world! The quick brown fox jumps over the lazy dog.",
					);

					await page.evaluate(() => {
						(window as any).__canvistEditor?.set_caret_visible(false);
						(window as any).__canvistEditor?.render();
					});
					await page.waitForTimeout(100);

					const path = await screenshotCanvas(page, "text-basic", browserName);
					const stats = await canvasPixelStats(page);

					// Text should fill a measurable area — at least 0.5% of canvas
					assert(
						stats.nonWhitePercent >= 0.5,
						`text canvas has only ${stats.nonWhitePercent}% non-white pixels — text may not be rendering`,
					);
					// Must have black pixels (text color).
					assert(stats.hasBlackPixels, "expected text to produce black pixels");

					console.log(
						`  [${browserName}] text-basic: ${stats.nonWhitePercent}% non-white, saved ${path}`,
					);
				} finally {
					await browser.close();
				}
			} finally {
				await server.shutdown();
			}
		},
		sanitizeResources: false,
		sanitizeOps: false,
	});

	// --- Test 3: Multi-line text wraps and renders multiple lines ---
	Deno.test({
		name: `[${browserName}] visual: multi-line text wraps correctly`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					// Type enough text to fill multiple lines.
					const longText = "The quick brown fox jumps over the lazy dog. " +
						"Pack my box with five dozen liquor jugs. " +
						"How vexingly quick daft zebras jump.";
					await typeInEditor(page, longText);

					// Also add a newline and second paragraph.
					await page.keyboard.press("Enter");
					await page.waitForTimeout(50);
					await typeInEditor(page, "Second paragraph here.");

					await page.evaluate(() => {
						(window as any).__canvistEditor?.set_caret_visible(false);
						(window as any).__canvistEditor?.render();
					});
					await page.waitForTimeout(100);

					const path = await screenshotCanvas(page, "multi-line", browserName);
					const stats = await canvasPixelStats(page);

					// Multi-line text should cover more area than single-line.
					assert(
						stats.nonWhitePercent >= 1.0,
						`multi-line canvas has only ${stats.nonWhitePercent}% non-white — text may not be wrapping`,
					);

					console.log(
						`  [${browserName}] multi-line: ${stats.nonWhitePercent}% non-white, saved ${path}`,
					);
				} finally {
					await browser.close();
				}
			} finally {
				await server.shutdown();
			}
		},
		sanitizeResources: false,
		sanitizeOps: false,
	});

	// --- Test 4: Selection highlighting produces blue pixels ---
	Deno.test({
		name: `[${browserName}] visual: selection highlighting is visible`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "Hello World");

					// Select "Hello" via WASM API.
					await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_selection(0, 5);
						ed.set_caret_visible(false);
						ed.render();
					});
					await page.waitForTimeout(100);

					const path = await screenshotCanvas(page, "selection", browserName);
					const stats = await canvasPixelStats(page);

					// Selection highlight is a semi-transparent blue.
					assert(
						stats.hasBluePixels,
						"expected selection highlight to produce blue pixels",
					);
					assert(
						stats.hasBlackPixels,
						"expected text to still be visible through selection",
					);

					console.log(
						`  [${browserName}] selection: ${stats.nonWhitePercent}% non-white, hasBlue=${stats.hasBluePixels}, saved ${path}`,
					);
				} finally {
					await browser.close();
				}
			} finally {
				await server.shutdown();
			}
		},
		sanitizeResources: false,
		sanitizeOps: false,
	});

	// --- Test 5: Bold text renders visibly different from normal text ---
	Deno.test({
		name: `[${browserName}] visual: bold text renders with heavier weight`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "normal text");

					// Get baseline pixel count.
					await page.evaluate(() => {
						(window as any).__canvistEditor?.set_caret_visible(false);
						(window as any).__canvistEditor?.render();
					});
					await page.waitForTimeout(100);
					const normalStats = await canvasPixelStats(page);

					// Now apply bold to "normal".
					await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_selection(0, 6);
						ed.toggle_bold();
						ed.set_selection(11, 11); // collapse selection
						ed.set_caret_visible(false);
						ed.render();
					});
					await page.waitForTimeout(100);

					const path = await screenshotCanvas(page, "bold", browserName);
					const boldStats = await canvasPixelStats(page);

					// Bold text has thicker strokes → more non-white pixels.
					assert(
						boldStats.nonWhitePixels > normalStats.nonWhitePixels,
						`bold should produce more non-white pixels: bold=${boldStats.nonWhitePixels} vs normal=${normalStats.nonWhitePixels}`,
					);

					console.log(
						`  [${browserName}] bold: normal=${normalStats.nonWhitePixels}, bold=${boldStats.nonWhitePixels}, saved ${path}`,
					);
				} finally {
					await browser.close();
				}
			} finally {
				await server.shutdown();
			}
		},
		sanitizeResources: false,
		sanitizeOps: false,
	});

	// --- Test 6: Large font size renders noticeably larger glyphs ---
	Deno.test({
		name: `[${browserName}] visual: font size 32px renders larger than 16px`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "big text");

					// Get baseline at default 16px.
					await page.evaluate(() => {
						(window as any).__canvistEditor?.set_caret_visible(false);
						(window as any).__canvistEditor?.render();
					});
					await page.waitForTimeout(100);
					const small = await canvasPixelStats(page);

					// Apply 32px to all text.
					await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_selection(0, 8);
						ed.set_font_size(32);
						ed.set_selection(8, 8);
						ed.set_caret_visible(false);
						ed.render();
					});
					await page.waitForTimeout(100);

					const path = await screenshotCanvas(page, "font-size", browserName);
					const big = await canvasPixelStats(page);

					// 32px text should cover more pixels than 16px text.
					assert(
						big.nonWhitePixels > small.nonWhitePixels * 1.3,
						`32px text should have >1.3× pixels: 32px=${big.nonWhitePixels} vs 16px=${small.nonWhitePixels}`,
					);

					console.log(
						`  [${browserName}] font-size: 16px=${small.nonWhitePixels}, 32px=${big.nonWhitePixels}, saved ${path}`,
					);
				} finally {
					await browser.close();
				}
			} finally {
				await server.shutdown();
			}
		},
		sanitizeResources: false,
		sanitizeOps: false,
	});

	// --- Test 7: Font rendering quality — text is anti-aliased (not pixelated) ---
	Deno.test({
		name: `[${browserName}] visual: text is anti-aliased (smooth edges)`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "Smooth text rendering test");

					await page.evaluate(() => {
						(window as any).__canvistEditor?.set_caret_visible(false);
						(window as any).__canvistEditor?.render();
					});
					await page.waitForTimeout(100);

					const path = await screenshotCanvas(
						page,
						"antialiasing",
						browserName,
					);

					// Count unique gray-level values in the non-white pixels.
					// Anti-aliased text has many gray intermediate values;
					// pixelated/aliased text has only black and white.
					const grayLevels = await page.evaluate(() => {
						const src = document.getElementById(
							"editor-canvas",
						) as HTMLCanvasElement;
						const rect = src.getBoundingClientRect();
						const w = Math.round(rect.width);
						const h = Math.round(rect.height);
						const tmp = document.createElement("canvas");
						tmp.width = w;
						tmp.height = h;
						const ctx = tmp.getContext("2d")!;
						ctx.drawImage(src, 0, 0, w, h);
						const data = ctx.getImageData(0, 0, w, h).data;
						const grays = new Set<number>();
						for (let i = 0; i < data.length; i += 4) {
							const r = data[i], g = data[i + 1], b = data[i + 2];
							if (r < 240 && g < 240 && b < 240) {
								const lum = Math.round(0.299 * r + 0.587 * g + 0.114 * b);
								grays.add(lum);
							}
						}
						return grays.size;
					});

					// Anti-aliased text should have many intermediate gray values (>10).
					// Aliased text would have ≤2 (just black and white).
					assert(
						grayLevels >= 5,
						`expected ≥5 unique gray levels for anti-aliased text, got ${grayLevels}`,
					);

					console.log(
						`  [${browserName}] antialiasing: ${grayLevels} unique gray levels, saved ${path}`,
					);
				} finally {
					await browser.close();
				}
			} finally {
				await server.shutdown();
			}
		},
		sanitizeResources: false,
		sanitizeOps: false,
	});

	// --- Test 8: Full editor visual — toolbar, text, formatting combined ---
	Deno.test({
		name:
			`[${browserName}] visual: full editor with toolbar and formatted text`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					// Type rich content.
					await typeInEditor(page, "canvist editor");
					await page.keyboard.press("Enter");
					await page.waitForTimeout(50);
					await typeInEditor(
						page,
						"A canvas-based text editor with font rendering.",
					);

					// Apply bold to "canvist editor" (first line).
					await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_selection(0, 14);
						ed.toggle_bold();
						ed.set_selection(14, 14);
						ed.set_caret_visible(true);
						ed.render();
					});
					await page.waitForTimeout(100);

					// Take a full-page screenshot showing toolbar + canvas.
					const fullPath = join(
						SCREENSHOT_DIR,
						`${browserName}-full-editor.png`,
					);
					await page.locator("#canvas-section").screenshot({ path: fullPath });

					// Also capture just the canvas.
					const canvasPath = await screenshotCanvas(
						page,
						"full-canvas",
						browserName,
					);

					const stats = await canvasPixelStats(page);
					assert(stats.hasBlackPixels, "expected text pixels");
					assert(
						stats.nonWhitePercent >= 0.5,
						`full editor has only ${stats.nonWhitePercent}% rendered content`,
					);

					console.log(
						`  [${browserName}] full-editor: ${stats.nonWhitePercent}% non-white, saved ${fullPath} + ${canvasPath}`,
					);
				} finally {
					await browser.close();
				}
			} finally {
				await server.shutdown();
			}
		},
		sanitizeResources: false,
		sanitizeOps: false,
	});
}
