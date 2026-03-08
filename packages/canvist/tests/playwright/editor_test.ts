/**
 * Playwright-based browser tests for the canvist editor.
 *
 * These tests verify that the canvas editor has feature parity with
 * contenteditable for basic text operations: typing, backspace, delete,
 * arrow key navigation, and Enter for newlines.
 *
 * Tests run against Chromium, Firefox, and WebKit (Safari).
 */

import { assert, assertEquals } from "@std/assert";
import { dirname, fromFileUrl, join } from "@std/path";
import { startServer } from "./server.ts";

const __dirname = dirname(fromFileUrl(import.meta.url));
const PKG_ROOT = join(__dirname, "../..");

interface BrowserInfo {
	name: string;
	launch: () => Promise<{ browser: any; context: any; page: any }>;
	skip?: boolean;
}

// Detect if playwright browsers are available
async function launchBrowser(
	browserType: string,
): Promise<{ browser: any; context: any; page: any }> {
	// Use Deno.Command to run a node script that launches Playwright.
	// We use a subprocess approach since playwright-rs is in the Rust crate
	// and Deno doesn't have native playwright bindings. Instead we use the
	// npm playwright package via Deno's npm: specifier.
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
	const context = await browser.newContext();
	const page = await context.newPage();
	return { browser, context, page };
}

// Helper: wait for the WASM editor to be ready
async function waitForEditor(page: any) {
	await page.waitForFunction("window.__canvistEditor !== null", null, {
		timeout: 15000,
	});
}

// Helper: type into the hidden textarea the editor uses
async function typeInEditor(page: any, text: string) {
	// Focus the hidden textarea that captures input
	await page.focus("#canvist-input");
	await page.waitForTimeout(50);
	// Type character by character so beforeinput fires for each
	for (const ch of text) {
		await page.keyboard.type(ch);
		await page.waitForTimeout(20);
	}
	await page.waitForTimeout(100);
}

// Helper: get the editor's text content via the WASM API
async function getEditorText(page: any): Promise<string> {
	return page.evaluate("window.__canvistEditor?.plain_text() ?? ''");
}

// Helper: get the editor's char count
async function getEditorCharCount(page: any): Promise<number> {
	return page.evaluate("window.__canvistEditor?.char_count() ?? 0");
}

// Determine which browsers to test based on CI environment.
// CI_BROWSERS env var can be set to a space-separated list (e.g. "chromium firefox").
// Locally, test all three.
function getBrowsers(): string[] {
	const envBrowsers = Deno.env.get("CI_BROWSERS");
	if (envBrowsers) return envBrowsers.trim().split(/\s+/);
	return ["chromium", "firefox", "webkit"];
}

const BROWSERS = getBrowsers();

for (const browserName of BROWSERS) {
	Deno.test({
		name: `[${browserName}] editor loads and renders`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					// Canvas should be visible
					const canvas = await page.$("#editor-canvas");
					assert(canvas, "Canvas element should exist");

					const isVisible = await canvas.isVisible();
					assert(isVisible, "Canvas should be visible");
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

	Deno.test({
		name: `[${browserName}] typing inserts text`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "Hello, canvist!");

					const text = await getEditorText(page);
					assertEquals(text, "Hello, canvist!");

					const count = await getEditorCharCount(page);
					assertEquals(count, 15);
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

	Deno.test({
		name: `[${browserName}] backspace deletes characters`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "Hello!");
					await page.keyboard.press("Backspace");
					await page.waitForTimeout(300);

					const text = await getEditorText(page);
					assertEquals(text, "Hello");
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

	Deno.test({
		name: `[${browserName}] enter creates newline`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "Line 1");
					await page.keyboard.press("Enter");
					await page.waitForTimeout(100);
					await typeInEditor(page, "Line 2");

					const text = await getEditorText(page);
					assert(
						text.includes("Line 1") && text.includes("Line 2"),
						`Expected two lines, got: ${text}`,
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

	Deno.test({
		name: `[${browserName}] multiple sequential inserts`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "abc");
					await typeInEditor(page, "def");
					await typeInEditor(page, "ghi");

					const text = await getEditorText(page);
					assertEquals(text, "abcdefghi");
					assertEquals(await getEditorCharCount(page), 9);
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
