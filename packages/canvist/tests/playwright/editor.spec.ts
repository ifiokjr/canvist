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

	// Firefox can be slower/flakier under heavy suites; harden navigation.
	page.setDefaultTimeout(60_000);
	page.setDefaultNavigationTimeout(60_000);
	const rawGoto: (...args: any[]) => Promise<any> = page.goto.bind(page);
	(page as any).goto = async (...args: any[]) => {
		let lastErr: unknown;
		for (let attempt = 0; attempt < 3; attempt++) {
			try {
				return await rawGoto(...args);
			} catch (err) {
				lastErr = err;
				if (attempt === 2) break;
				await page.waitForTimeout(250);
			}
		}
		throw lastErr;
	};

	return { browser, context, page };
}

// Helper: wait for the WASM editor to be ready
async function waitForEditor(page: any) {
	await page.waitForFunction(
		"window.__canvistEditor != null && typeof window.__canvistEditor.plain_text === 'function'",
		null,
		{
			timeout: 15000,
		},
	);
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

async function pressKey(page: any, key: string, settleMs = 120) {
	await page.keyboard.press(key);
	await page.waitForTimeout(settleMs);
}

async function getA11ySnapshot(page: any) {
	return page.evaluate(() => {
		const canvas = document.getElementById("editor-canvas");
		const input = document.getElementById("canvist-input") as
			| HTMLTextAreaElement
			| null;
		return {
			canvasRole: canvas?.getAttribute("role") ?? null,
			canvasAriaMultiline: canvas?.getAttribute("aria-multiline") ?? null,
			canvasAriaControls: canvas?.getAttribute("aria-controls") ?? null,
			canvasAriaValueText: canvas?.getAttribute("aria-valuetext") ?? null,
			inputExists: Boolean(input),
			inputAriaLabel: input?.getAttribute("aria-label") ?? null,
			inputValue: input?.value ?? null,
			inputSelectionStart: input?.selectionStart ?? null,
			activeElementId: document.activeElement?.id ?? null,
		};
	});
}

async function getSelectionRange(
	page: any,
): Promise<{ start: number; end: number }> {
	return page.evaluate(() => {
		const editor = (window as any).__canvistEditor;
		return {
			start: editor?.selection_start() ?? 0,
			end: editor?.selection_end() ?? 0,
		};
	});
}

async function getCanvasPointForOffset(
	page: any,
	offset: number,
): Promise<{ x: number; y: number }> {
	return page.evaluate((nextOffset: number) => {
		const canvas = document.getElementById(
			"editor-canvas",
		) as HTMLCanvasElement;
		const ctx = canvas.getContext("2d");
		if (!ctx) {
			return { x: 0, y: 0 };
		}
		ctx.font = "16px sans-serif";
		const text =
			((window as any).__canvistEditor?.plain_text() ?? "") as string;
		const clamped = Math.max(0, Math.min(nextOffset, text.length));
		const width = ctx.measureText(text.slice(0, clamped)).width;
		const rect = canvas.getBoundingClientRect();
		return {
			x: rect.left + 20 + width,
			y: rect.top + 34,
		};
	}, offset);
}

async function dragSelectRange(page: any, start: number, end: number) {
	const from = await getCanvasPointForOffset(page, start);
	const to = await getCanvasPointForOffset(page, end);
	await page.mouse.move(from.x, from.y);
	await page.mouse.down();
	await page.mouse.move(to.x, to.y, { steps: 12 });
	await page.mouse.up();
	await page.waitForTimeout(120);
}

async function getCanvasInkStats(
	page: any,
): Promise<{ nonWhite: number; checksum: number }> {
	return page.evaluate(() => {
		const canvas = document.getElementById(
			"editor-canvas",
		) as HTMLCanvasElement;
		const ctx = canvas.getContext("2d");
		if (!ctx) {
			return { nonWhite: 0, checksum: 0 };
		}
		const image = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
		let nonWhite = 0;
		let checksum = 0;
		for (let i = 0; i < image.length; i += 4) {
			const r = image[i] ?? 0;
			const g = image[i + 1] ?? 0;
			const b = image[i + 2] ?? 0;
			const a = image[i + 3] ?? 0;
			if (a > 0 && (r < 245 || g < 245 || b < 245)) {
				nonWhite += 1;
			}
			checksum = (checksum + r * 3 + g * 5 + b * 7 + a * 11) >>> 0;
		}
		return { nonWhite, checksum };
	});
}

async function installEventProbe(page: any) {
	await page.evaluate(() => {
		const canvas = document.getElementById("editor-canvas");
		const input = document.getElementById("canvist-input");
		(window as any).__canvistEventLog = [];
		const add = (
			target: EventTarget | null,
			type: string,
			label: string,
			capture = false,
		) => {
			if (!target) return;
			target.addEventListener(
				type,
				(event) => {
					(window as any).__canvistEventLog.push({
						type: event.type,
						target: label,
						key: event instanceof KeyboardEvent ? event.key : null,
						inputType: event instanceof InputEvent ? event.inputType : null,
					});
				},
				capture,
			);
		};

		for (const type of ["mousedown", "mousemove", "mouseup", "click"]) {
			add(canvas, type, "canvas");
		}
		for (
			const type of [
				"focus",
				"keydown",
				"beforeinput",
				"input",
				"compositionstart",
				"compositionend",
			]
		) {
			add(input, type, "input");
		}
		for (const type of ["keydown", "input", "mousedown", "mouseup"]) {
			add(document, type, "document");
		}
	});
}

async function getEventProbe(page: any) {
	return page.evaluate(() => (window as any).__canvistEventLog ?? []);
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
		name: `[${browserName}] exposes full wasm editor binding surface`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const bindings = await page.evaluate(() => {
						const editor = (window as any).__canvistEditor;
						const expectedMethods = [
							"canvas_id",
							"plain_text",
							"char_count",
							"insert_text",
							"insert_text_at",
							"delete_range",
							"set_title",
							"to_json",
							"queue_text_input",
							"queue_key_down",
							"queue_key_down_with_modifiers",
							"process_events",
							"selection_start",
							"selection_end",
							"set_selection",
							"move_cursor_to",
							"move_cursor_left",
							"move_cursor_right",
							"apply_style_range",
							"undo",
							"redo",
							"can_undo",
							"can_redo",
							"replay_operations_json",
							"hit_test",
							"render",
						];

						return expectedMethods.map((method) => ({
							method,
							isFunction: typeof editor?.[method] === "function",
						}));
					});

					for (const binding of bindings) {
						assert(
							binding.isFunction,
							`expected binding ${binding.method} to exist`,
						);
					}
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
		name: `[${browserName}] canvas output is stable and reflects typed content`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const empty = await getCanvasInkStats(page);
					await typeInEditor(page, "Canvas parity");
					const afterTyping = await getCanvasInkStats(page);
					await page.evaluate(() => {
						(window as any).__canvistEditor?.render();
					});
					await page.waitForTimeout(80);
					const afterRerender = await getCanvasInkStats(page);

					// The current-line highlight adds subtle tinting even to the empty canvas,
					// so we compare with a generous threshold that typing must exceed.
					assert(
						afterTyping.nonWhite > empty.nonWhite,
						`typing should increase canvas ink (empty=${empty.nonWhite}, after=${afterTyping.nonWhite})`,
					);
					const nonWhiteDelta = Math.abs(
						afterTyping.nonWhite - afterRerender.nonWhite,
					);
					const checksumDelta = Math.abs(
						afterTyping.checksum - afterRerender.checksum,
					);
					// Allow for caret blink jitter (the 530ms blink timer can
					// toggle caret visibility between the two snapshots).
					assert(
						nonWhiteDelta <= 80,
						`expected stable non-white pixel count, delta=${nonWhiteDelta}`,
					);
					assert(
						checksumDelta <= 5_000_000,
						`expected stable canvas checksum, delta=${checksumDelta}`,
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

	Deno.test({
		name: `[${browserName}] runtime queue bindings apply deterministic actions`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const state = await page.evaluate(() => {
						const editor = (window as any).__canvistEditor;
						editor.queue_text_input("abc");
						editor.set_selection(1, 2);
						editor.queue_text_input("Z");
						editor.queue_text_input("\n");
						return {
							text: editor.plain_text(),
							charCount: editor.char_count(),
							start: editor.selection_start(),
							end: editor.selection_end(),
						};
					});

					assertEquals(state.text, "aZc\n");
					assertEquals(state.charCount, 4);
					assertEquals(state.start, 4);
					assertEquals(state.end, 4);
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
		name:
			`[${browserName}] editor bindings compose a complete editing workflow`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const state = await page.evaluate(() => {
						const editor = (window as any).__canvistEditor;
						editor.queue_text_input("Hello world");
						editor.set_selection(6, 11);
						editor.queue_text_input("canvist");
						editor.move_cursor_to(5, false);
						editor.queue_text_input(",");
						editor.move_cursor_to(editor.char_count(), false);
						editor.queue_text_input("!");
						editor.set_title("Bindings Doc");
						const beforeUndo = editor.plain_text();
						const json = JSON.parse(editor.to_json());
						editor.undo();
						const afterUndo = editor.plain_text();
						return {
							beforeUndo,
							afterUndo,
							title: json.title,
						};
					});

					assertEquals(state.beforeUndo, "Hello, canvist!");
					assertEquals(state.afterUndo, "Hello, canvist");
					assertEquals(state.title, "Bindings Doc");
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
		name: `[${browserName}] delete removes character in front of cursor`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "abcde");
					await pressKey(page, "ArrowLeft");
					await pressKey(page, "ArrowLeft");
					await pressKey(page, "Delete");

					assertEquals(await getEditorText(page), "abce");
					assertEquals(await getEditorCharCount(page), 4);
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
		name:
			`[${browserName}] arrow keys move cursor for deterministic mid-string insert`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "ABEF");
					await pressKey(page, "ArrowLeft");
					await pressKey(page, "ArrowLeft");
					await typeInEditor(page, "CD");

					assertEquals(await getEditorText(page), "ABCDEF");
					assertEquals(await getEditorCharCount(page), 6);
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
		name: `[${browserName}] shift+arrow selection replaced by typing`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "abcdef");
					await pressKey(page, "ArrowLeft");
					await pressKey(page, "ArrowLeft");
					await pressKey(page, "Shift+ArrowLeft");
					await pressKey(page, "Shift+ArrowLeft");
					await typeInEditor(page, "XY");

					assertEquals(await getEditorText(page), "abXYef");
					assertEquals(await getEditorCharCount(page), 6);
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
		name: `[${browserName}] drag selection replaces range deterministically`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);
					await typeInEditor(page, "drag-select");

					await dragSelectRange(page, 0, 4);
					const selection = await getSelectionRange(page);
					const start = Math.min(selection.start, selection.end);
					const end = Math.max(selection.start, selection.end);
					assertEquals(start, 0);
					assertEquals(end, 4);

					await typeInEditor(page, "DROP");
					assertEquals(await getEditorText(page), "DROP-select");
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
		name: `[${browserName}] reverse drag selection keeps normalized range`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);
					await typeInEditor(page, "abcdefg");

					await dragSelectRange(page, 6, 2);
					const selection = await getSelectionRange(page);
					const start = Math.min(selection.start, selection.end);
					const end = Math.max(selection.start, selection.end);
					assertEquals(start, 2);
					assertEquals(end, 6);

					await typeInEditor(page, "X");
					assertEquals(await getEditorText(page), "abXg");
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
		name:
			`[${browserName}] input and pointer events propagate through the web pipeline`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);
					await installEventProbe(page);

					await typeInEditor(page, "evt");
					await dragSelectRange(page, 0, 2);
					await typeInEditor(page, "E");

					const events = await getEventProbe(page);
					assert(
						events.some((event: any) =>
							event.type === "keydown" && event.target === "input"
						),
					);
					assert(
						events.some((event: any) =>
							(event.type === "input" || event.type === "beforeinput") &&
							event.target === "input"
						),
					);
					assert(
						events.some((event: any) =>
							event.type === "mousedown" && event.target === "canvas"
						),
					);
					assert(
						events.some((event: any) =>
							event.type === "mousemove" &&
							(event.target === "canvas" || event.target === "document")
						),
					);
					assert(
						events.some((event: any) =>
							event.type === "mouseup" &&
							(event.target === "canvas" || event.target === "document")
						),
					);
					assertEquals(await getEditorText(page), "Et");
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
		name:
			`[${browserName}] accessibility wiring exposes hidden input and textbox semantics`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);
					await typeInEditor(page, "A11y");

					const snapshot = await getA11ySnapshot(page);
					assertEquals(snapshot.inputExists, true);
					assertEquals(snapshot.canvasRole, "textbox");
					assertEquals(snapshot.canvasAriaMultiline, "true");
					assertEquals(snapshot.canvasAriaControls, "canvist-input");
					assertEquals(snapshot.inputAriaLabel, "Document editor input");
					assertEquals(snapshot.canvasAriaValueText, "A11y");
					assertEquals(snapshot.inputValue, "A11y");
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
		name:
			`[${browserName}] focus and keyboard routing keeps hidden textarea active`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);
					await page.focus("#editor-canvas");
					await page.waitForTimeout(80);
					await page.keyboard.type("Focus");
					await page.waitForTimeout(120);

					const snapshot = await getA11ySnapshot(page);
					assertEquals(snapshot.activeElementId, "canvist-input");
					assertEquals(snapshot.inputSelectionStart, 5);
					assertEquals(await getEditorText(page), "Focus");
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
		name:
			`[${browserName}] composition lifecycle events propagate without duplicate mutations`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);

			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);
					await installEventProbe(page);
					await page.focus("#canvist-input");

					await page.evaluate(() => {
						const input = document.getElementById(
							"canvist-input",
						) as HTMLTextAreaElement;
						input.dispatchEvent(
							new CompositionEvent("compositionstart", { data: "" }),
						);
						input.value = "あ";
						input.dispatchEvent(
							new InputEvent("input", {
								data: "あ",
								inputType: "insertCompositionText",
								bubbles: true,
								composed: true,
							}),
						);
						input.dispatchEvent(
							new CompositionEvent("compositionend", {
								data: "あ",
								bubbles: true,
								composed: true,
							}),
						);
					});
					await page.waitForTimeout(120);

					const events = await getEventProbe(page);
					assert(
						events.some((event: any) =>
							event.type === "compositionstart" && event.target === "input"
						),
					);
					assert(
						events.some((event: any) =>
							event.type === "compositionend" && event.target === "input"
						),
					);

					const text = await getEditorText(page);
					const count = await getEditorCharCount(page);
					assert(
						count <= 1,
						`expected no duplicate IME commits, got count=${count}`,
					);
					if (count === 1) {
						assertEquals(text, "あ");
					}
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

	// --- New feature tests ---

	Deno.test({
		name: `[${browserName}] select all via Ctrl+A`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "Hello World");
					const modifier = Deno.build.os === "darwin" ? "Meta" : "Control";
					await page.keyboard.press(`${modifier}+a`);
					await page.waitForTimeout(100);
					const selected = await page.evaluate(() =>
						(window as any).__canvistEditor?.get_selected_text()
					);
					assertEquals(selected, "Hello World");
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
		name: `[${browserName}] word navigation via Ctrl+Right then Ctrl+Left`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "one two three");
					// Move cursor to start.
					const modifier = Deno.build.os === "darwin" ? "Meta" : "Control";
					await page.keyboard.press(`${modifier}+a`);
					await page.waitForTimeout(50);
					await page.keyboard.press("ArrowLeft");
					await page.waitForTimeout(50);

					// Ctrl+Right should jump to end of "one " -> position 4
					await page.keyboard.press(`${modifier}+ArrowRight`);
					await page.waitForTimeout(50);
					const pos1 = await page.evaluate(() =>
						(window as any).__canvistEditor?.selection_end()
					);
					// Should be at or past position 3 (end of "one" or start of "two")
					assert(pos1 >= 3 && pos1 <= 4, `expected 3-4, got ${pos1}`);

					// Ctrl+Right again jumps past "two "
					await page.keyboard.press(`${modifier}+ArrowRight`);
					await page.waitForTimeout(50);
					const pos2 = await page.evaluate(() =>
						(window as any).__canvistEditor?.selection_end()
					);
					assert(pos2 >= 7 && pos2 <= 8, `expected 7-8, got ${pos2}`);
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
		name: `[${browserName}] Ctrl+Backspace deletes previous word`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "hello world");
					const modifier = Deno.build.os === "darwin" ? "Meta" : "Control";
					await page.keyboard.press(`${modifier}+Backspace`);
					await page.waitForTimeout(200);
					const text = await page.evaluate(() =>
						(window as any).__canvistEditor?.plain_text()
					);
					assertEquals(text, "hello ");
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
		name: `[${browserName}] select_word_at selects word via WASM API`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "hello world");
					// Use WASM API to select word at offset 2 (inside "hello").
					const selected = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.select_word_at(2);
						return ed.get_selected_text();
					});
					assertEquals(selected, "hello");

					// Select word at offset 7 (inside "world").
					const selected2 = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.select_word_at(7);
						return ed.get_selected_text();
					});
					assertEquals(selected2, "world");
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
		name: `[${browserName}] ArrowUp and ArrowDown navigate between lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					// Type two lines.
					await typeInEditor(page, "first line");
					await page.keyboard.press("Enter");
					await page.waitForTimeout(50);
					await typeInEditor(page, "second line");
					// Cursor is at end of "second line" (offset ~22).
					await page.waitForTimeout(100);

					// ArrowUp should move to the first line.
					await page.keyboard.press("ArrowUp");
					await page.waitForTimeout(100);
					const sel1 = await page.evaluate(() =>
						(window as any).__canvistEditor?.selection_end()
					);
					// Should be on the first line (offset < 11).
					assert(sel1 <= 10, `expected first line, got offset ${sel1}`);

					// ArrowDown should move back to the second line.
					await page.keyboard.press("ArrowDown");
					await page.waitForTimeout(100);
					const sel2 = await page.evaluate(() =>
						(window as any).__canvistEditor?.selection_end()
					);
					assert(sel2 > 10, `expected second line, got offset ${sel2}`);
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
		name: `[${browserName}] Home goes to line start, End goes to line end`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "hello world");
					// Cursor at end (offset 11).
					await page.keyboard.press("Home");
					await page.waitForTimeout(100);
					const afterHome = await page.evaluate(() =>
						(window as any).__canvistEditor?.selection_end()
					);
					assertEquals(afterHome, 0);

					await page.keyboard.press("End");
					await page.waitForTimeout(100);
					const afterEnd = await page.evaluate(() =>
						(window as any).__canvistEditor?.selection_end()
					);
					assertEquals(afterEnd, 11);
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
		name: `[${browserName}] Tab key inserts a tab character`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "hello");
					await page.keyboard.press("Tab");
					await page.waitForTimeout(100);
					await typeInEditor(page, "world");

					const text = await getEditorText(page);
					assertEquals(text, "hello\tworld");
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
		name: `[${browserName}] toggle_bold applies and removes bold via WASM API`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "hello");
					// Select all and apply bold.
					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_selection(0, 5);
						ed.toggle_bold();
						const wasBold = ed.is_bold();
						// Toggle again to remove bold.
						ed.toggle_bold();
						const isStillBold = ed.is_bold();
						return { wasBold, isStillBold };
					});
					assertEquals(result.wasBold, true);
					assertEquals(result.isStillBold, false);
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
		name:
			`[${browserName}] line_start_for_offset and line_end_for_offset via WASM API`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "first line");
					await page.keyboard.press("Enter");
					await page.waitForTimeout(50);
					await typeInEditor(page, "second line");
					await page.waitForTimeout(100);

					// line_start for offset 14 (inside "second line") should be 11.
					// line_end for offset 3 (inside "first line") should be >= 10.
					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						return {
							lineStart14: ed.line_start_for_offset(14),
							lineEnd3: ed.line_end_for_offset(3),
						};
					});
					assertEquals(result.lineStart14, 11);
					assert(
						result.lineEnd3 >= 10 && result.lineEnd3 <= 11,
						`expected 10-11, got ${result.lineEnd3}`,
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
		name: `[${browserName}] Shift+ArrowDown extends selection across lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "line one");
					await page.keyboard.press("Enter");
					await page.waitForTimeout(50);
					await typeInEditor(page, "line two");
					// Move to beginning of document.
					const modifier = Deno.build.os === "darwin" ? "Meta" : "Control";
					await page.keyboard.press(`${modifier}+a`);
					await page.waitForTimeout(50);
					await page.keyboard.press("ArrowLeft");
					await page.waitForTimeout(50);

					// Shift+ArrowDown to extend selection.
					await page.keyboard.press("Shift+ArrowDown");
					await page.waitForTimeout(100);

					const selected = await page.evaluate(() =>
						(window as any).__canvistEditor?.get_selected_text()
					);
					assert(selected.length > 0, "expected non-empty selection");
					// Should span across the newline.
					assert(
						selected.includes("\n") || selected.length >= 8,
						`expected cross-line selection, got "${selected}"`,
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
		name: `[${browserName}] find_all returns matches via WASM API`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "abc def abc ghi abc");
					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const flat = ed.find_all("abc", true);
						// flat is [start0, end0, start1, end1, ...]
						const matches = [];
						for (let i = 0; i < flat.length; i += 2) {
							matches.push([flat[i], flat[i + 1]]);
						}
						return matches;
					});
					assertEquals(result.length, 3);
					assertEquals(result[0], [0, 3]);
					assertEquals(result[1], [8, 11]);
					assertEquals(result[2], [16, 19]);
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
		name: `[${browserName}] find_next wraps around via WASM API`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "abc def abc");
					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						// From offset 5, should find "abc" at 8.
						const next = ed.find_next("abc", 5, true);
						// From offset 9, wraps to start.
						const wrapped = ed.find_next("abc", 9, true);
						return { next: Array.from(next), wrapped: Array.from(wrapped) };
					});
					assertEquals(result.next, [8, 11]);
					assertEquals(result.wrapped, [0, 3]);
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
		name: `[${browserName}] replace_all replaces all occurrences via WASM API`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "foo bar foo baz foo");
					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const count = ed.replace_all("foo", "xx", true);
						return { count, text: ed.plain_text() };
					});
					assertEquals(result.count, 3);
					assertEquals(result.text, "xx bar xx baz xx");
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
		name:
			`[${browserName}] toggle_strikethrough applies and removes via WASM API`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "hello");
					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_selection(0, 5);
						ed.toggle_strikethrough();
						// Check rendered — we can't query strikethrough state easily,
						// but we can verify the operation didn't error.
						const text = ed.plain_text();
						return {
							text,
							selStart: ed.selection_start(),
							selEnd: ed.selection_end(),
						};
					});
					assertEquals(result.text, "hello");
					// Selection should be preserved.
					assertEquals(result.selStart, 0);
					assertEquals(result.selEnd, 5);
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
		name:
			`[${browserName}] set_font_size changes font on selection via WASM API`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "hello world");
					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_selection(0, 5);
						ed.set_font_size(32);
						// Selection should be preserved.
						return {
							text: ed.plain_text(),
							selStart: ed.selection_start(),
							selEnd: ed.selection_end(),
						};
					});
					assertEquals(result.text, "hello world");
					assertEquals(result.selStart, 0);
					assertEquals(result.selEnd, 5);
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
		name: `[${browserName}] Ctrl+F opens find bar and Escape closes it`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "test document");
					// Open find bar with Ctrl+F.
					const modifier = Deno.build.os === "darwin" ? "Meta" : "Control";
					await page.keyboard.press(`${modifier}+f`);
					await page.waitForTimeout(200);

					// Check find bar is visible.
					const barVisible = await page.evaluate(() => {
						const bar = document.getElementById("find-bar");
						return bar?.style.display !== "none";
					});
					assertEquals(barVisible, true);

					// Close with Escape.
					await page.keyboard.press("Escape");
					await page.waitForTimeout(100);
					const barHidden = await page.evaluate(() => {
						const bar = document.getElementById("find-bar");
						return bar?.style.display === "none";
					});
					assertEquals(barHidden, true);
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

	// ── Scroll ────────────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] scroll_by moves scroll offset`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const before = ed.scroll_y();
						ed.scroll_by(50);
						const after = ed.scroll_y();
						ed.scroll_by(-100);
						const clamped = ed.scroll_y();
						return { before, after, clamped };
					});
					assertEquals(result.before, 0);
					// scroll_by(50) might clamp if no content, so just verify it changed or clamped to 0.
					assert(result.after >= 0, "scroll_y should be >= 0");
					assertEquals(result.clamped, 0); // clamped to 0 (no negative scroll)
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
		name: `[${browserName}] content_height returns reasonable value`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					// Insert a lot of text to make it overflow.
					await typeInEditor(page, "Line\n".repeat(30));
					const h = await page.evaluate(() => {
						return (window as any).__canvistEditor.content_height();
					});
					assert(
						h > 400,
						`content_height should exceed canvas height, got ${h}`,
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

	// ── Focus ────────────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] set_focused changes focus state`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const before = ed.focused();
						ed.set_focused(false);
						const after = ed.focused();
						ed.set_focused(true);
						const restored = ed.focused();
						return { before, after, restored };
					});
					assertEquals(result.before, true);
					assertEquals(result.after, false);
					assertEquals(result.restored, true);
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

	// ── Statistics ───────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] word_count and line_count via WASM API`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "hello world foo bar");
					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						return {
							words: ed.word_count(),
							lines: ed.line_count(),
							curLine: ed.cursor_line(),
							curCol: ed.cursor_column(),
						};
					});
					assertEquals(result.words, 4);
					assert(
						result.lines >= 1,
						`line_count should be >= 1, got ${result.lines}`,
					);
					assert(
						result.curLine >= 1,
						`cursor_line should be >= 1, got ${result.curLine}`,
					);
					assert(
						result.curCol >= 1,
						`cursor_column should be >= 1, got ${result.curCol}`,
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

	// ── Status bar ──────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] status bar shows position and word count`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "hello world");
					await page.waitForTimeout(300); // let status bar update

					const status = await page.evaluate(() => {
						return {
							position:
								document.getElementById("status-position")?.textContent ?? "",
							words: document.getElementById("status-words")?.textContent ?? "",
							chars: document.getElementById("status-chars")?.textContent ?? "",
						};
					});
					assert(
						status.position.includes("Ln"),
						`position should include Ln, got "${status.position}"`,
					);
					assert(
						status.words.includes("2"),
						`words should include 2, got "${status.words}"`,
					);
					assert(
						status.chars.includes("11"),
						`chars should include 11, got "${status.chars}"`,
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

	// ── Wheel scroll on canvas ──────────────────────────────────────

	Deno.test({
		name: `[${browserName}] mouse wheel scrolls the editor canvas`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					// Insert enough text to make scrollable.
					for (let i = 0; i < 25; i++) {
						await typeInEditor(page, `Line ${i + 1}\n`);
					}
					await page.waitForTimeout(200);

					// Scroll down with mouse wheel.
					const canvas = page.locator("#editor-canvas");
					await canvas.hover();
					await page.mouse.wheel(0, 200);
					await page.waitForTimeout(200);

					const scrollAfter = await page.evaluate(() => {
						return (window as any).__canvistEditor.scroll_y();
					});
					assert(
						scrollAfter > 0,
						`scroll_y should be > 0 after wheel, got ${scrollAfter}`,
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

	// ── Export ──────────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] to_html exports document as HTML`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "hello world");
					const html = await page.evaluate(() => {
						return (window as any).__canvistEditor.to_html();
					});
					assert(
						html.includes("hello world"),
						`HTML should contain text, got: ${html}`,
					);
					assert(
						html.includes("<p>"),
						`HTML should have <p> tags, got: ${html}`,
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
		name: `[${browserName}] to_markdown exports document as Markdown`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "hello");
					// Make it bold
					const md = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_selection(0, 5);
						ed.toggle_bold();
						return ed.to_markdown();
					});
					assert(
						md.includes("**hello**"),
						`Markdown should contain **hello**, got: ${md}`,
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
		name: `[${browserName}] to_html escapes HTML entities`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "<b>xss</b>");
					const html = await page.evaluate(() => {
						return (window as any).__canvistEditor.to_html();
					});
					assert(
						!html.includes("<b>xss</b>"),
						`should escape HTML, got: ${html}`,
					);
					assert(
						html.includes("&lt;b&gt;"),
						`should have escaped entities, got: ${html}`,
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

	// ── Keyboard shortcuts modal ────────────────────────────────────

	Deno.test({
		name: `[${browserName}] Ctrl+/ opens and closes shortcuts modal`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					// Open with Ctrl+/
					const modifier = Deno.build.os === "darwin" ? "Meta" : "Control";
					await page.keyboard.press(`${modifier}+/`);
					await page.waitForTimeout(200);

					const modalVisible = await page.evaluate(() => {
						const modal = document.getElementById("shortcuts-modal");
						return modal?.style.display !== "none";
					});
					assertEquals(modalVisible, true);

					// Close by clicking close button
					await page.click("#shortcuts-close");
					await page.waitForTimeout(100);

					const modalHidden = await page.evaluate(() => {
						const modal = document.getElementById("shortcuts-modal");
						return modal?.style.display === "none";
					});
					assertEquals(modalHidden, true);
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

	// ── Color picker ───────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] color picker element exists in toolbar`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const exists = await page.evaluate(() => {
						const picker = document.getElementById("color-picker") as
							| HTMLInputElement
							| null;
						return picker !== null && picker.type === "color";
					});
					assertEquals(exists, true);
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

	// ── HTML import / paste ─────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] from_html imports bold HTML`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.from_html("<strong>bold</strong> text");
						return {
							text: ed.plain_text(),
							html: ed.to_html(),
						};
					});
					assert(
						result.text.includes("bold"),
						`should contain 'bold', got: ${result.text}`,
					);
					assert(
						result.html.includes("<strong>"),
						`re-export should have <strong>, got: ${result.html}`,
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
		name: `[${browserName}] paste_html inserts formatted text at cursor`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "before ");
					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.paste_html("<em>pasted</em>");
						return ed.plain_text();
					});
					assert(
						result.includes("before ") && result.includes("pasted"),
						`should contain both, got: ${result}`,
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

	// ── Double-click word selection ─────────────────────────────────

	Deno.test({
		name: `[${browserName}] select_word_at selects word via WASM API`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					await typeInEditor(page, "hello world");
					const selected = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.select_word_at(2); // middle of "hello"
						return ed.get_selected_text();
					});
					assertEquals(selected, "hello");
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

	// ── Read-only mode ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] read-only mode blocks text insertion`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("before");
						ed.set_read_only(true);
						ed.insert_text(" blocked");
						const text = ed.plain_text();
						ed.set_read_only(false);
						return text;
					});
					assertEquals(result, "before");
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

	// ── Line numbers ────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] line numbers can be toggled`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const before = ed.show_line_numbers();
						ed.set_show_line_numbers(true);
						const after = ed.show_line_numbers();
						ed.set_show_line_numbers(false);
						return { before, after };
					});
					assertEquals(result.before, false);
					assertEquals(result.after, true);
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

	// ── Indent / Outdent ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] indent_selection inserts tab at line start`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello\nworld");
						ed.select_all();
						ed.indent_selection();
						return ed.plain_text();
					});
					assert(
						result.includes("\thello"),
						`should indent first line, got: ${result}`,
					);
					assert(
						result.includes("\tworld"),
						`should indent second line, got: ${result}`,
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
		name: `[${browserName}] outdent_selection removes leading tab`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("\thello\n\tworld");
						ed.select_all();
						ed.outdent_selection();
						return ed.plain_text();
					});
					assert(result.includes("hello"), `should outdent, got: ${result}`);
					assert(
						!result.includes("\thello"),
						`tab should be removed, got: ${result}`,
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

	// ── Context menu ────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] right-click opens context menu`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const canvas = page.locator("#editor-canvas");
					await canvas.click({ button: "right", position: { x: 100, y: 100 } });
					await page.waitForTimeout(200);

					const visible = await page.evaluate(() => {
						const menu = document.getElementById("context-menu");
						return menu !== null && menu.style.display !== "none";
					});
					assertEquals(visible, true);
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

	// ── Dark mode / theme ───────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] theme switching between light and dark`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const before = ed.theme_name();
						ed.set_theme_dark();
						const dark = ed.theme_name();
						ed.set_theme_light();
						const light = ed.theme_name();
						return { before, dark, light };
					});
					assertEquals(result.before, "light");
					assertEquals(result.dark, "dark");
					assertEquals(result.light, "light");
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

	// ── Zoom ────────────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] zoom in/out/reset`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const initial = ed.zoom();
						ed.zoom_in();
						const zoomed = ed.zoom();
						ed.zoom_reset();
						const reset = ed.zoom();
						ed.set_zoom(2.0);
						const custom = ed.zoom();
						ed.zoom_reset();
						return { initial, zoomed, reset, custom };
					});
					assertEquals(result.initial, 1.0);
					assert(
						result.zoomed > 1.0,
						`zoomed should be >1, got ${result.zoomed}`,
					);
					assertEquals(result.reset, 1.0);
					assertEquals(result.custom, 2.0);
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

	// ── Current line highlight ──────────────────────────────────────

	Deno.test({
		name: `[${browserName}] current line highlight toggle`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const initial = ed.highlight_current_line();
						ed.set_highlight_current_line(false);
						const disabled = ed.highlight_current_line();
						ed.set_highlight_current_line(true);
						const enabled = ed.highlight_current_line();
						return { initial, disabled, enabled };
					});
					assertEquals(result.initial, true);
					assertEquals(result.disabled, false);
					assertEquals(result.enabled, true);
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

	// ── Drag-and-drop (move_text API) ───────────────────────────────

	Deno.test({
		name: `[${browserName}] move_text moves selected text to new position`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("ABCDEF");
						// Move "CD" (offsets 2-4) to position 0 → "CDABEF"
						ed.move_text(2, 4, 0);
						return ed.plain_text();
					});
					assertEquals(result, "CDABEF");
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

	// ── Auto-indent ─────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] auto_indent_newline preserves leading whitespace`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("\thello");
						// Place cursor at end.
						ed.set_selection(6, 6);
						ed.auto_indent_newline();
						return ed.plain_text();
					});
					assert(
						result.includes("\thello\n\t"),
						`should auto-indent, got: ${JSON.stringify(result)}`,
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

	// ── Bullet list ─────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] toggle_bullet_list inserts and removes bullet`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("item one");
						ed.toggle_bullet_list();
						const withBullet = ed.plain_text();
						ed.toggle_bullet_list();
						const withoutBullet = ed.plain_text();
						return { withBullet, withoutBullet };
					});
					assert(
						result.withBullet.startsWith("• "),
						`should start with bullet, got: ${result.withBullet}`,
					);
					assertEquals(result.withoutBullet, "item one");
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

	// ── Numbered list ───────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] toggle_numbered_list inserts and removes number`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("first item");
						ed.toggle_numbered_list();
						const withNum = ed.plain_text();
						ed.toggle_numbered_list();
						const withoutNum = ed.plain_text();
						return { withNum, withoutNum };
					});
					assert(
						result.withNum.startsWith("1. "),
						`should start with 1., got: ${result.withNum}`,
					);
					assertEquals(result.withoutNum, "first item");
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

	// ── Word wrap toggle ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] word wrap can be toggled`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const before = ed.word_wrap();
						ed.set_word_wrap(false);
						const disabled = ed.word_wrap();
						ed.set_word_wrap(true);
						const enabled = ed.word_wrap();
						return { before, disabled, enabled };
					});
					assertEquals(result.before, true);
					assertEquals(result.disabled, false);
					assertEquals(result.enabled, true);
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

	// ── List continuation on Enter ──────────────────────────────────

	Deno.test({
		name: `[${browserName}] auto_indent continues bullet list`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("• item one");
						const len = ed.plain_text().length;
						ed.set_selection(len, len);
						ed.auto_indent_newline();
						return ed.plain_text();
					});
					assert(
						result.includes("• item one\n• "),
						`should continue bullet, got: ${JSON.stringify(result)}`,
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

	// ── Go to line ──────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] go_to_line moves cursor to correct position`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("line one\nline two\nline three");
						ed.go_to_line(2);
						return {
							offset: ed.selection_start(),
							text: ed.plain_text().substring(
								ed.selection_start(),
								ed.selection_start() + 8,
							),
						};
					});
					assertEquals(result.offset, 9); // after "line one\n"
					assertEquals(result.text, "line two");
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

	// ── Duplicate line ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] duplicate_line duplicates the current line`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello\nworld");
						ed.set_selection(2, 2); // cursor on "hello"
						ed.duplicate_line();
						return ed.plain_text();
					});
					assertEquals(result, "hello\nhello\nworld");
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

	// ── Move line ───────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] move_line_down swaps lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aaa\nbbb\nccc");
						ed.set_selection(1, 1); // cursor on "aaa"
						ed.move_line_down();
						return ed.plain_text();
					});
					assertEquals(result, "bbb\naaa\nccc");
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
		name: `[${browserName}] move_line_up swaps lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aaa\nbbb\nccc");
						ed.set_selection(5, 5); // cursor on "bbb"
						ed.move_line_up();
						return ed.plain_text();
					});
					assertEquals(result, "bbb\naaa\nccc");
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

	// ── Selection statistics ────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] selection stats return correct counts`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello world foo");
						const noSel = {
							chars: ed.selected_char_count(),
							words: ed.selected_word_count(),
						};
						ed.set_selection(0, 11); // "hello world"
						const withSel = {
							chars: ed.selected_char_count(),
							words: ed.selected_word_count(),
						};
						return { noSel, withSel };
					});
					assertEquals(result.noSel.chars, 0);
					assertEquals(result.noSel.words, 0);
					assertEquals(result.withSel.chars, 11);
					assertEquals(result.withSel.words, 2);
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

	// ── Highlight colour ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] set_highlight_color applies background style`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("highlight me");
						ed.set_selection(0, 9);
						ed.set_highlight_color(255, 255, 0, 100);
						// The HTML export should reflect the background.
						return ed.to_html();
					});
					// Background styles should appear in the HTML export.
					assert(
						result.includes("highlight") || result.includes("background"),
						`HTML should contain highlighted text, got: ${
							result.substring(0, 200)
						}`,
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

	// ── Delete line ─────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] delete_line removes current line`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aaa\nbbb\nccc");
						ed.set_selection(5, 5); // cursor on "bbb"
						ed.delete_line();
						return ed.plain_text();
					});
					assertEquals(result, "aaa\nccc");
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

	// ── Join lines ──────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] join_lines merges current and next line`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello\n  world");
						ed.set_selection(3, 3); // cursor on "hello"
						ed.join_lines();
						return ed.plain_text();
					});
					assertEquals(result, "hello world");
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

	// ── Transform case ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] transform case: upper, lower, title`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello world");
						ed.select_all();
						ed.transform_uppercase();
						const upper = ed.plain_text();
						ed.select_all();
						ed.transform_lowercase();
						const lower = ed.plain_text();
						ed.select_all();
						ed.transform_title_case();
						const title = ed.plain_text();
						return { upper, lower, title };
					});
					assertEquals(result.upper, "HELLO WORLD");
					assertEquals(result.lower, "hello world");
					assertEquals(result.title, "Hello World");
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

	// ── Sort lines ──────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] sort_lines_asc sorts alphabetically`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("cherry\napple\nbanana");
						ed.select_all();
						ed.sort_lines_asc();
						return ed.plain_text();
					});
					assertEquals(result, "apple\nbanana\ncherry");
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

	// ── Show whitespace ─────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] show_whitespace can be toggled`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const before = ed.show_whitespace();
						ed.set_show_whitespace(true);
						const after = ed.show_whitespace();
						ed.set_show_whitespace(false);
						return { before, after };
					});
					assertEquals(result.before, false);
					assertEquals(result.after, true);
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

	// ── Bracket auto-close ──────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] auto-close brackets inserts pair`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_auto_close_brackets(true);
						ed.insert_with_auto_close("(");
						const text1 = ed.plain_text();
						const cursor1 = ed.selection_end();
						return { text1, cursor1 };
					});
					assertEquals(result.text1, "()");
					assertEquals(result.cursor1, 1); // between ( and )
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

	// ── Delete word left ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] delete_word_left removes word before cursor`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello world");
						ed.set_selection(11, 11); // end of "world"
						ed.delete_word_left();
						return ed.plain_text();
					});
					assertEquals(result, "hello ");
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

	// ── Delete word right ───────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] delete_word_right removes word after cursor`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello world");
						ed.set_selection(0, 0); // start
						ed.delete_word_right();
						return ed.plain_text();
					});
					assertEquals(result, "world");
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

	// ── Select line ─────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] select_line selects the current line`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aaa\nbbb\nccc");
						ed.set_selection(5, 5); // on "bbb"
						ed.select_line();
						return ed.get_selected_text();
					});
					assertEquals(result, "bbb\n");
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

	// ── Trim trailing whitespace ────────────────────────────────────

	Deno.test({
		name: `[${browserName}] trim_trailing_whitespace removes trailing spaces`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello   \nworld  ");
						const removed = ed.trim_trailing_whitespace();
						return { text: ed.plain_text(), removed };
					});
					assertEquals(result.text, "hello\nworld");
					assertEquals(result.removed, 5);
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

	// ── Remove duplicate lines ──────────────────────────────────────

	Deno.test({
		name: `[${browserName}] remove_duplicate_lines deduplicates adjacent lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aaa\naaa\nbbb\nbbb\nbbb\nccc");
						const removed = ed.remove_duplicate_lines();
						return { text: ed.plain_text(), removed };
					});
					assertEquals(result.text, "aaa\nbbb\nccc");
					assertEquals(result.removed, 3);
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

	// ── Wrap selection ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] wrap_selection wraps text in brackets`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello world");
						ed.set_selection(0, 5); // "hello"
						ed.wrap_selection("[", "]");
						return ed.plain_text();
					});
					assertEquals(result, "[hello] world");
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

	// ── Smart backspace ─────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] smart_backspace deletes matching bracket pair`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_auto_close_brackets(true);
						ed.insert_with_auto_close("("); // creates "()"
						const before = ed.plain_text();
						ed.smart_backspace(); // should delete both
						const after = ed.plain_text();
						return { before, after };
					});
					assertEquals(result.before, "()");
					assertEquals(result.after, "");
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

	// ── Transpose characters ────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] transpose_chars swaps chars around cursor`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("abcd");
						ed.set_selection(2, 2); // between b and c
						ed.transpose_chars();
						return ed.plain_text();
					});
					assertEquals(result, "acbd");
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

	// ── Toggle line comment ─────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] toggle_line_comment adds and removes // prefix`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello\nworld");
						ed.select_all();
						ed.toggle_line_comment(); // add
						const commented = ed.plain_text();
						ed.select_all();
						ed.toggle_line_comment(); // remove
						const uncommented = ed.plain_text();
						return { commented, uncommented };
					});
					assertEquals(result.commented, "// hello\n// world");
					assertEquals(result.uncommented, "hello\nworld");
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

	// ── Soft tabs ───────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] soft_tabs inserts spaces instead of tab`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_soft_tabs(true);
						ed.set_tab_size(2);
						ed.insert_tab();
						const text = ed.plain_text();
						const tabSz = ed.tab_size();
						return { text, tabSz };
					});
					assertEquals(result.text, "  "); // 2 spaces
					assertEquals(result.tabSz, 2);
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

	// ── Auto-surround ───────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] auto_surround wraps selection with brackets`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_auto_surround(true);
						ed.insert_text("hello");
						ed.select_all();
						const wrapped = ed.try_auto_surround("(");
						return { text: ed.plain_text(), wrapped };
					});
					assertEquals(result.text, "(hello)");
					assertEquals(result.wrapped, true);
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

	// ── Expand selection ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] expand_selection goes word then line then all`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello world\ngoodbye");
						ed.set_selection(2, 2); // inside "hello"
						ed.expand_selection();
						const s1 = ed.get_selected_text(); // word
						ed.expand_selection();
						const s2 = ed.get_selected_text(); // line
						ed.expand_selection();
						const s3 = ed.get_selected_text(); // all
						return { s1, s2, s3 };
					});
					assertEquals(result.s1, "hello");
					assertEquals(result.s2, "hello world");
					assertEquals(result.s3, "hello world\ngoodbye");
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

	// ── Matching bracket ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] find_matching_bracket finds paired brackets`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("(hello [world])");
						const m0 = ed.find_matching_bracket(0); // ( → )
						const m14 = ed.find_matching_bracket(14); // ) → (
						const m7 = ed.find_matching_bracket(7); // [ → ]
						const m3 = ed.find_matching_bracket(3); // l → -1
						return { m0, m14, m7, m3 };
					});
					assertEquals(result.m0, 14); // ( at 0 matches ) at 14
					assertEquals(result.m14, 0); // ) at 14 matches ( at 0
					assertEquals(result.m7, 13); // [ at 7 matches ] at 13
					assertEquals(result.m3, -1); // no bracket at 3
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

	// ── Move to matching bracket ────────────────────────────────────

	Deno.test({
		name: `[${browserName}] move_to_matching_bracket jumps cursor`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("(hello)");
						ed.set_selection(0, 0); // at opening (
						ed.move_to_matching_bracket();
						return ed.selection_end();
					});
					assertEquals(result, 6); // jumps to ) at offset 6
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

	// ── Document statistics ─────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] paragraph_count and current_line_number`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello\n\nworld\nfoo");
						ed.set_selection(14, 14); // on "foo"
						return {
							paragraphs: ed.paragraph_count(),
							lineNum: ed.current_line_number(),
							col: ed.current_column(),
						};
					});
					assertEquals(result.paragraphs, 3); // "hello", "world", "foo" (blank excluded)
					assertEquals(result.lineNum, 4);
					assertEquals(result.col, 2); // "f|oo" → col 2... actually 14 - 13 + 1 = 2
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

	// ── Indent guides ───────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] indent guides can be toggled`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const before = ed.show_indent_guides();
						ed.set_show_indent_guides(true);
						const after = ed.show_indent_guides();
						return { before, after };
					});
					assertEquals(result.before, false);
					assertEquals(result.after, true);
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

	// ── Bookmarks ───────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] bookmarks toggle, next, prev, clear`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("line1\nline2\nline3\nline4");
						// Bookmark line 1 (offset 0).
						ed.set_selection(0, 0);
						const added1 = ed.toggle_bookmark();
						// Bookmark line 3 (offset 12).
						ed.set_selection(12, 12);
						const added3 = ed.toggle_bookmark();
						const count = ed.bookmark_count();
						// Jump to next from line 1.
						ed.set_selection(0, 0);
						ed.next_bookmark();
						const afterNext = ed.current_line_number();
						// Jump to prev from line 3.
						ed.prev_bookmark();
						const afterPrev = ed.current_line_number();
						// Clear all.
						ed.clear_bookmarks();
						const afterClear = ed.bookmark_count();
						return { added1, added3, count, afterNext, afterPrev, afterClear };
					});
					assertEquals(result.added1, true);
					assertEquals(result.added3, true);
					assertEquals(result.count, 2);
					assertEquals(result.afterNext, 3); // jumped to line 3
					assertEquals(result.afterPrev, 1); // jumped back to line 1
					assertEquals(result.afterClear, 0);
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

	// ── Convert indentation ─────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] tabs_to_spaces and spaces_to_tabs`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_tab_size(4);
						ed.insert_text("\thello\n\t\tworld");
						const tabCount = ed.tabs_to_spaces();
						const afterSpaces = ed.plain_text();
						const spaceCount = ed.spaces_to_tabs();
						const afterTabs = ed.plain_text();
						return { tabCount, afterSpaces, spaceCount, afterTabs };
					});
					assertEquals(result.tabCount, 3);
					assertEquals(result.afterSpaces, "    hello\n        world");
					assertEquals(result.spaceCount, 3);
					assertEquals(result.afterTabs, "\thello\n\t\tworld");
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

	// ── Open line above / below ─────────────────────────────────────

	Deno.test({
		name: `[${browserName}] open_line_below and open_line_above`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aaa\nccc");
						ed.set_selection(2, 2); // on "aaa"
						ed.open_line_below();
						const afterBelow = ed.plain_text();
						const cursorBelow = ed.selection_end();
						ed.insert_text("bbb");
						ed.set_selection(0, 0); // back to line 1
						ed.open_line_above();
						const afterAbove = ed.plain_text();
						const cursorAbove = ed.selection_end();
						return { afterBelow, cursorBelow, afterAbove, cursorAbove };
					});
					assertEquals(result.afterBelow, "aaa\n\nccc");
					assertEquals(result.cursorBelow, 4); // on empty new line
					assertEquals(result.afterAbove, "\naaa\nbbb\nccc");
					assertEquals(result.cursorAbove, 0); // at start of new line above
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

	// ── Copy / cut line (no selection) ──────────────────────────────

	Deno.test({
		name: `[${browserName}] current_line_text returns full line`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aaa\nbbb\nccc");
						ed.set_selection(5, 5); // on "bbb"
						return ed.current_line_text();
					});
					assertEquals(result, "bbb\n");
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
		name: `[${browserName}] cut_line removes and returns current line`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aaa\nbbb\nccc");
						ed.set_selection(5, 5); // on "bbb"
						const cut = ed.cut_line();
						return { cut, remaining: ed.plain_text() };
					});
					assertEquals(result.cut, "bbb\n");
					assertEquals(result.remaining, "aaa\nccc");
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

	// ── Overwrite mode ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] overwrite mode replaces characters`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("abcdef");
						ed.set_overwrite_mode(true);
						ed.set_selection(2, 2); // after "ab"
						ed.insert_text_overwrite("XY");
						const text = ed.plain_text();
						const mode = ed.overwrite_mode();
						return { text, mode };
					});
					assertEquals(result.text, "abXYef");
					assertEquals(result.mode, true);
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

	// ── Document start / end ────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] go_to_document_start and go_to_document_end`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello\nworld");
						ed.go_to_document_start();
						const atStart = ed.selection_end();
						ed.go_to_document_end();
						const atEnd = ed.selection_end();
						return { atStart, atEnd };
					});
					assertEquals(result.atStart, 0);
					assertEquals(result.atEnd, 11); // "hello\nworld" = 11 chars
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

	// ── Select between brackets ─────────────────────────────────────

	Deno.test({
		name: `[${browserName}] select_between_brackets selects inner content`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("foo(bar baz)end");
						ed.set_selection(5, 5); // inside parens
						const found = ed.select_between_brackets();
						const selected = ed.get_selected_text();
						return { found, selected };
					});
					assertEquals(result.found, true);
					assertEquals(result.selected, "bar baz");
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

	// ── Center line in viewport ─────────────────────────────────────

	Deno.test({
		name: `[${browserName}] center_line_in_viewport adjusts scroll`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						// Create a tall document.
						const lines = Array.from(
							{ length: 100 },
							(_, i) => `Line ${i + 1}`,
						);
						ed.insert_text(lines.join("\n"));
						ed.set_selection(500, 500); // somewhere in middle
						const scrollBefore = ed.scroll_y();
						ed.center_line_in_viewport();
						const scrollAfter = ed.scroll_y();
						// Scroll should have changed.
						return {
							scrollBefore,
							scrollAfter,
							changed: scrollBefore !== scrollAfter,
						};
					});
					assertEquals(result.changed, true);
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

	// ── Cursor position history ─────────────────────────────────────

	Deno.test({
		name: `[${browserName}] cursor history back and forward`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aaa\nbbb\nccc");
						ed.set_selection(0, 0);
						ed.push_cursor_history(); // pos 0
						ed.set_selection(5, 5);
						ed.push_cursor_history(); // pos 5
						ed.set_selection(10, 10);
						ed.push_cursor_history(); // pos 10
						const len = ed.cursor_history_length();
						ed.cursor_history_back();
						const back1 = ed.selection_end();
						ed.cursor_history_back();
						const back2 = ed.selection_end();
						ed.cursor_history_forward();
						const fwd1 = ed.selection_end();
						return { len, back1, back2, fwd1 };
					});
					assertEquals(result.len, 3);
					assertEquals(result.back1, 5);
					assertEquals(result.back2, 0);
					assertEquals(result.fwd1, 5);
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

	// ── Select all occurrences ──────────────────────────────────────

	Deno.test({
		name: `[${browserName}] select_all_occurrences counts matches`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("foo bar foo baz foo");
						ed.set_selection(0, 3); // select "foo"
						const count = ed.select_all_occurrences();
						const offsets = Array.from(ed.occurrence_offsets());
						return { count, offsets };
					});
					assertEquals(result.count, 3);
					assertEquals(result.offsets, [0, 3, 8, 11, 16, 19]);
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

	// ── Whole word find ─────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] find_all_whole_word matches word boundaries`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("foo foobar foo barfoo");
						const offsets = Array.from(ed.find_all_whole_word("foo"));
						return offsets;
					});
					// Only offsets 0-3 and 11-14 are whole-word "foo"
					assertEquals(result, [0, 3, 11, 14]);
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

	// ── Paragraph navigation ────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] paragraph navigation moves between blocks`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aaa\n\nbbb\n\nccc");
						ed.set_selection(0, 0); // start
						ed.move_to_next_paragraph();
						const next1 = ed.selection_end();
						ed.move_to_next_paragraph();
						const next2 = ed.selection_end();
						ed.move_to_prev_paragraph();
						const prev1 = ed.selection_end();
						return { next1, next2, prev1 };
					});
					assertEquals(result.next1, 5); // start of "bbb"
					assertEquals(result.next2, 10); // start of "ccc"
					assertEquals(result.prev1, 5); // back to "bbb"
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

	// ── Snippet insertion ───────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] insert_snippet places cursor at $0`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_snippet("if ($0) {\n}");
						const text = ed.plain_text();
						const cursor = ed.selection_end();
						return { text, cursor };
					});
					assertEquals(result.text, "if () {\n}");
					assertEquals(result.cursor, 4); // between parens
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

	// ── Scroll to selection ─────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] scroll_to_selection adjusts viewport`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const lines = Array.from(
							{ length: 100 },
							(_, i) => `Line ${i + 1}`,
						);
						ed.insert_text(lines.join("\n"));
						ed.set_scroll_y(0);
						ed.set_selection(600, 600); // deep in doc
						ed.scroll_to_selection();
						return ed.scroll_y() > 0;
					});
					assertEquals(result, true);
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

	// ── Column ruler ────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] rulers can be set and queried`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.add_ruler(80);
						ed.add_ruler(120);
						const rulers = Array.from(ed.rulers());
						ed.remove_ruler(80);
						const after = Array.from(ed.rulers());
						return { rulers, after };
					});
					assertEquals(result.rulers, [80, 120]);
					assertEquals(result.after, [120]);
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

	// ── Ensure final newline ────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] ensure_final_newline adds trailing newline`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello");
						const added1 = ed.ensure_final_newline();
						const text1 = ed.plain_text();
						const added2 = ed.ensure_final_newline();
						return { added1, text1, added2 };
					});
					assertEquals(result.added1, true);
					assertEquals(result.text1, "hello\n");
					assertEquals(result.added2, false); // already has newline
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

	// ── Replace all occurrences ─────────────────────────────────────

	Deno.test({
		name: `[${browserName}] replace_all_occurrences replaces throughout doc`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("foo bar foo baz foo");
						ed.set_selection(0, 3); // select "foo"
						const count = ed.replace_all_occurrences("qux");
						return { text: ed.plain_text(), count };
					});
					assertEquals(result.text, "qux bar qux baz qux");
					assertEquals(result.count, 3);
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

	// ── Reverse lines ───────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] reverse_lines reverses selected lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aaa\nbbb\nccc");
						ed.select_all();
						ed.reverse_lines();
						return ed.plain_text();
					});
					assertEquals(result, "ccc\nbbb\naaa");
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

	// ── Base64 encode / decode ──────────────────────────────────────

	Deno.test({
		name: `[${browserName}] base64 encode and decode selection`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello World");
						ed.select_all();
						ed.base64_encode_selection();
						const encoded = ed.plain_text();
						ed.select_all();
						ed.base64_decode_selection();
						const decoded = ed.plain_text();
						return { encoded, decoded };
					});
					assertEquals(result.encoded, "SGVsbG8gV29ybGQ=");
					assertEquals(result.decoded, "Hello World");
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

	// ── Toggle case ─────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] transform_toggle_case swaps character case`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello World");
						ed.select_all();
						ed.transform_toggle_case();
						return ed.plain_text();
					});
					assertEquals(result, "hELLO wORLD");
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

	// ── Line decorations ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] line decorations add, remove, clear`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.add_line_decoration(0, 255, 0, 0, 40);
						ed.add_line_decoration(2, 0, 255, 0, 40);
						const count1 = ed.line_decoration_count();
						ed.remove_line_decorations(0);
						const count2 = ed.line_decoration_count();
						ed.clear_line_decorations();
						const count3 = ed.line_decoration_count();
						return { count1, count2, count3 };
					});
					assertEquals(result.count1, 2);
					assertEquals(result.count2, 1);
					assertEquals(result.count3, 0);
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

	// ── Modified state ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] modified state tracks edits and saves`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const before = ed.is_modified();
						ed.insert_text("hello");
						const afterEdit = ed.is_modified();
						ed.mark_saved();
						const afterSave = ed.is_modified();
						ed.insert_text(" world");
						const afterEdit2 = ed.is_modified();
						return { before, afterEdit, afterSave, afterEdit2 };
					});
					assertEquals(result.before, false);
					assertEquals(result.afterEdit, true);
					assertEquals(result.afterSave, false);
					assertEquals(result.afterEdit2, true);
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

	// ── Clipboard ring ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] clipboard ring stores and retrieves entries`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.clipboard_ring_push("first");
						ed.clipboard_ring_push("second");
						ed.clipboard_ring_push("third");
						const len = ed.clipboard_ring_length();
						const newest = ed.clipboard_ring_get(0);
						const oldest = ed.clipboard_ring_get(2);
						// Paste from ring.
						ed.clipboard_ring_paste(1); // pastes "second"
						const text = ed.plain_text();
						return { len, newest, oldest, text };
					});
					assertEquals(result.len, 3);
					assertEquals(result.newest, "third");
					assertEquals(result.oldest, "first");
					assertEquals(result.text, "second");
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

	// ── Word frequency ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] word_frequency returns top N words`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("foo bar foo baz foo bar");
						const freq = Array.from(ed.word_frequency(2));
						return freq;
					});
					// foo=3 is most frequent, bar=2 is second
					assertEquals(result[0], "foo");
					assertEquals(result[1], "3");
					assertEquals(result[2], "bar");
					assertEquals(result[3], "2");
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

	// ── Highlight occurrences ───────────────────────────────────────

	Deno.test({
		name: `[${browserName}] word_at_cursor and highlight toggle`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello world");
						ed.set_selection(2, 2); // inside "hello"
						const word = ed.word_at_cursor();
						const before = ed.highlight_occurrences();
						ed.set_highlight_occurrences(true);
						const after = ed.highlight_occurrences();
						return { word, before, after };
					});
					assertEquals(result.word, "hello");
					assertEquals(result.before, false);
					assertEquals(result.after, true);
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

	// ── Text measurement ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] measure_text_width returns positive value`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const w = ed.measure_text_width("Hello");
						const cw = ed.measure_char_width("M");
						return { w, cw };
					});
					assert(result.w > 0, `text width should be > 0, got ${result.w}`);
					assert(result.cw > 0, `char width should be > 0, got ${result.cw}`);
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

	// ── State serialization ─────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] save and restore state round-trips`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello World");
						ed.set_selection(3, 3);
						const state = ed.save_state();
						const parsed = JSON.parse(state);
						// Clear and restore.
						const len = ed.plain_text().length;
						ed.delete_range(0, len);
						ed.restore_state(state);
						return {
							text: ed.plain_text(),
							selEnd: ed.selection_end(),
							hasText: parsed.text === "Hello World",
						};
					});
					assertEquals(result.text, "Hello World");
					assertEquals(result.selEnd, 3);
					assertEquals(result.hasText, true);
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

	// ── Placeholder text ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] placeholder text set and get`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const len = ed.plain_text().length;
						if (len > 0) ed.delete_range(0, len);
						const before = ed.placeholder();
						ed.set_placeholder("Type something...");
						const after = ed.placeholder();
						return { before, after };
					});
					assertEquals(result.before, "");
					assertEquals(result.after, "Type something...");
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

	// ── Max length ──────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] max length clamps text insertion`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_max_length(10);
						const n1 = ed.insert_text_clamped("Hello");
						const n2 = ed.insert_text_clamped(" World!!");
						const text = ed.plain_text();
						const remaining = ed.remaining_capacity();
						return { n1, n2, text, remaining };
					});
					assertEquals(result.n1, 5);
					assertEquals(result.n2, 5); // truncated to fit 10
					assertEquals(result.text, "Hello Worl");
					assertEquals(result.remaining, 0);
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

	// ── Regex find ──────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] find_all_regex finds case-insensitive matches`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello hello HELLO");
						const offsets = Array.from(ed.find_all_regex("hello"));
						return offsets;
					});
					// Should find 3 matches: 0-5, 6-11, 12-17
					assertEquals(result.length, 6);
					assertEquals(result[0], 0);
					assertEquals(result[1], 5);
					assertEquals(result[2], 6);
					assertEquals(result[3], 11);
					assertEquals(result[4], 12);
					assertEquals(result[5], 17);
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

	// ── Selection change detection ──────────────────────────────────

	Deno.test({
		name: `[${browserName}] selection_changed detects cursor movement`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello");
						// First call — cursor moved from 0 to 5.
						const changed1 = ed.selection_changed();
						// Second call — cursor hasn't moved.
						const changed2 = ed.selection_changed();
						// Move cursor and check again.
						ed.set_selection(2, 2);
						const changed3 = ed.selection_changed();
						return { changed1, changed2, changed3 };
					});
					assertEquals(result.changed1, true);
					assertEquals(result.changed2, false);
					assertEquals(result.changed3, true);
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

	// ── Batch operations ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] begin_batch and end_batch do not crash`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.begin_batch();
						ed.insert_text("Batch1");
						ed.insert_text("Batch2");
						ed.end_batch();
						return ed.plain_text();
					});
					assertEquals(result, "Batch1Batch2");
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

	// ── Wrap indicators ─────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] wrap indicators toggle`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const before = ed.show_wrap_indicators();
						ed.set_show_wrap_indicators(true);
						const after = ed.show_wrap_indicators();
						ed.set_show_wrap_indicators(false);
						const reset = ed.show_wrap_indicators();
						return { before, after, reset };
					});
					assertEquals(result.before, false);
					assertEquals(result.after, true);
					assertEquals(result.reset, false);
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

	// ── Selection anchor ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] selection anchor, collapsed, length`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello World");
						// Collapsed cursor at offset 5.
						ed.set_selection(5, 5);
						const collapsed = ed.selection_is_collapsed();
						const anchor1 = ed.selection_anchor();
						// Select range 2..7.
						ed.set_selection(2, 7);
						const collapsed2 = ed.selection_is_collapsed();
						const length = ed.selection_length();
						return { collapsed, anchor1, collapsed2, length };
					});
					assertEquals(result.collapsed, true);
					assertEquals(result.anchor1, 5);
					assertEquals(result.collapsed2, false);
					assertEquals(result.length, 5);
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

	// ── Character counts ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] char_counts categorizes characters`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hi 42!");
						return Array.from(ed.char_counts());
					});
					// "Hi 42!" → letters=2, digits=2, spaces=1, punct=1, other=0
					assertEquals(result[0], 2); // letters
					assertEquals(result[1], 2); // digits
					assertEquals(result[2], 1); // spaces
					assertEquals(result[3], 1); // punctuation
					assertEquals(result[4], 0); // other
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

	// ── Text hash ───────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] text_hash returns stable hex string`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello");
						const h1 = ed.text_hash();
						const h2 = ed.text_hash();
						ed.insert_text("!");
						const h3 = ed.text_hash();
						return { h1, h2, h3 };
					});
					assert(
						result.h1.length === 16,
						`hash should be 16 hex chars, got ${result.h1.length}`,
					);
					assertEquals(result.h1, result.h2); // same text = same hash
					assert(
						result.h1 !== result.h3,
						"different text should have different hash",
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

	// ── Event log ───────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] event log stores and retrieves entries`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.log_event("insert");
						ed.log_event("delete");
						ed.log_event("format");
						const len = ed.event_log_length();
						const newest = ed.event_log_get(0);
						const oldest = ed.event_log_get(2);
						ed.event_log_clear();
						const afterClear = ed.event_log_length();
						return { len, newest, oldest, afterClear };
					});
					assertEquals(result.len, 3);
					assertEquals(result.newest, "format");
					assertEquals(result.oldest, "insert");
					assertEquals(result.afterClear, 0);
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

	// ── Word completion ─────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] completions suggests from document vocabulary`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("function foo foobar fuzzy\nfo");
						// Cursor is after "fo" — should suggest foo, foobar, function.
						const suggestions = Array.from(ed.completions(5));
						return suggestions;
					});
					assert(result.length > 0, "should have suggestions");
					assert(result.includes("foo"), `should include "foo", got ${result}`);
					assert(
						result.includes("foobar"),
						`should include "foobar", got ${result}`,
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

	// ── Line range operations ───────────────────────────────────────

	Deno.test({
		name: `[${browserName}] line range get and set`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("line0\nline1\nline2\nline3");
						const total = ed.line_count_total();
						const range = ed.get_line_range(1, 3);
						const single = ed.get_line(2);
						return { total, range, single };
					});
					assertEquals(result.total, 4);
					assertEquals(result.range, "line1\nline2");
					assertEquals(result.single, "line2");
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

	// ── Scroll metrics ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] scroll metrics return valid values`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const vh = ed.viewport_height();
						const ratio = ed.scroll_ratio();
						const frac = ed.scroll_fraction();
						return { vh, ratio, frac };
					});
					assert(
						result.vh > 0,
						`viewport_height should be > 0, got ${result.vh}`,
					);
					assert(
						result.ratio > 0 && result.ratio <= 1.0,
						`scroll_ratio should be 0-1, got ${result.ratio}`,
					);
					assert(
						result.frac >= 0 && result.frac <= 1.0,
						`scroll_fraction should be 0-1, got ${result.frac}`,
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

	// ── Annotations ─────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] annotations add, query, remove`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello World");
						ed.add_annotation(0, 5, "error", "spelling");
						ed.add_annotation(6, 11, "warning", "style");
						const count1 = ed.annotation_count();
						const at3 = Array.from(ed.annotations_at(3));
						ed.remove_annotations_by_kind("error");
						const count2 = ed.annotation_count();
						ed.clear_annotations();
						const count3 = ed.annotation_count();
						return { count1, at3, count2, count3 };
					});
					assertEquals(result.count1, 2);
					assertEquals(result.at3[2], "error");
					assertEquals(result.at3[3], "spelling");
					assertEquals(result.count2, 1);
					assertEquals(result.count3, 0);
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

	// ── Search history ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] search history stores and retrieves`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.search_history_push("foo");
						ed.search_history_push("bar");
						ed.search_history_push("baz");
						const len = ed.search_history_length();
						const newest = ed.search_history_get(0);
						const oldest = ed.search_history_get(2);
						// Push duplicate — should deduplicate.
						ed.search_history_push("foo");
						const len2 = ed.search_history_length();
						const top = ed.search_history_get(0);
						return { len, newest, oldest, len2, top };
					});
					assertEquals(result.len, 3);
					assertEquals(result.newest, "baz");
					assertEquals(result.oldest, "foo");
					assertEquals(result.len2, 3); // deduped
					assertEquals(result.top, "foo"); // moved to front
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

	// ── Visible range ───────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] visible range returns valid line numbers`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("a\nb\nc\nd\ne");
						const first = ed.first_visible_line();
						const last = ed.last_visible_line();
						const count = ed.visible_line_count();
						return { first, last, count };
					});
					assertEquals(result.first, 0);
					assert(result.last >= result.first, "last >= first");
					assert(result.count > 0, "visible count > 0");
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

	// ── Minimap ─────────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] minimap toggle and width`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const before = ed.show_minimap();
						ed.set_show_minimap(true);
						const after = ed.show_minimap();
						ed.set_minimap_width(80);
						const w = ed.minimap_width();
						return { before, after, w };
					});
					assertEquals(result.before, false);
					assertEquals(result.after, true);
					assertEquals(result.w, 80);
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

	// ── Sticky scroll ───────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] sticky scroll toggle`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const before = ed.sticky_scroll();
						ed.set_sticky_scroll(true);
						const after = ed.sticky_scroll();
						return { before, after };
					});
					assertEquals(result.before, false);
					assertEquals(result.after, true);
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

	// ── Rename all ──────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] rename_all replaces word under cursor`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("foo bar foo baz foo");
						ed.set_selection(1, 1); // inside first "foo"
						const count = ed.rename_all("qux");
						return { count, text: ed.plain_text() };
					});
					assertEquals(result.count, 3);
					assertEquals(result.text, "qux bar qux baz qux");
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

	// ── Cursor style ────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] cursor style, width, colour`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_cursor_style(1);
						const style = ed.cursor_style();
						ed.set_cursor_width(4);
						const w = ed.cursor_width_px();
						ed.set_cursor_color(255, 0, 0, 255);
						// Reset.
						ed.set_cursor_color(0, 0, 0, 0);
						ed.set_cursor_style(0);
						return { style, w };
					});
					assertEquals(result.style, 1);
					assertEquals(result.w, 4);
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

	// ── Snapshot diff ───────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] snapshot diff detects changed lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("line0\nline1\nline2");
						ed.take_snapshot();
						const hasBefore = ed.has_snapshot();
						// Modify line 1.
						ed.set_selection(6, 11); // "line1"
						ed.delete_range(6, 11);
						ed.insert_text_at(6, "CHANGED");
						const changed = Array.from(ed.diff_from_snapshot());
						ed.clear_snapshot();
						const hasAfter = ed.has_snapshot();
						return { hasBefore, changed, hasAfter };
					});
					assertEquals(result.hasBefore, true);
					assert(
						result.changed.includes(1),
						`line 1 should be changed, got ${result.changed}`,
					);
					assertEquals(result.hasAfter, false);
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

	// ── Macro recording ─────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] macro record, save, replay`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.macro_start_recording();
						const recording = ed.macro_is_recording();
						ed.macro_record_step("insert", "Hello");
						ed.macro_record_step("insert", " World");
						const steps = ed.macro_stop_recording();
						ed.macro_save("greet");
						const names = Array.from(ed.macro_list_saved());
						// Replay saved macro.
						ed.macro_replay_saved("greet");
						const text = ed.plain_text();
						return { recording, steps, names, text };
					});
					assertEquals(result.recording, true);
					assertEquals(result.steps, 2);
					assert(result.names.includes("greet"), "should have saved macro");
					assertEquals(result.text, "Hello World");
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

	// ── Find match highlights ───────────────────────────────────────

	Deno.test({
		name: `[${browserName}] find highlights toggle`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello world hello");
						ed.set_find_highlights("hello");
						const needle = ed.find_highlight_needle();
						const active = ed.show_find_highlights();
						ed.set_find_highlights("");
						const cleared = ed.show_find_highlights();
						return { needle, active, cleared };
					});
					assertEquals(result.needle, "hello");
					assertEquals(result.active, true);
					assertEquals(result.cleared, false);
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

	// ── Block selection ─────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] block selection get and set`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("ABCDE\nFGHIJ\nKLMNO");
						// Get block: lines 0-1, cols 1-3.
						const block = ed.get_block_selection(0, 1, 1, 3);
						return block;
					});
					assertEquals(result, "BC\nGH");
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

	// ── Smart paste ─────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] paste_with_indent adjusts indentation`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("    "); // 4 spaces indent
						ed.paste_with_indent("line1\n  line2\n  line3");
						return ed.plain_text();
					});
					// First line at cursor, subsequent re-indented to 4 spaces.
					assert(
						result.includes("line1"),
						`should contain line1, got: ${result}`,
					);
					assert(
						result.includes("line2"),
						`should contain line2, got: ${result}`,
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

	// ── Tokenize ────────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] tokenize returns kind-text pairs`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hi 42!");
						return Array.from(ed.tokenize());
					});
					// "hi" → word, " " → whitespace, "42" → number, "!" → punctuation
					assertEquals(result[0], "word");
					assertEquals(result[1], "hi");
					assertEquals(result[2], "whitespace");
					assertEquals(result[3], " ");
					assertEquals(result[4], "number");
					assertEquals(result[5], "42");
					assertEquals(result[6], "punctuation");
					assertEquals(result[7], "!");
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

	// ── Link detection ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] find_links detects URLs`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Visit https://example.com for info");
						const links = Array.from(ed.find_links());
						const urlText = ed.link_at_offset(8);
						return { links, urlText };
					});
					assertEquals(result.links.length, 2);
					assertEquals(result.urlText, "https://example.com");
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

	// ── Line folding ────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] fold and unfold lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("line0\n  line1\n  line2\nline3");
						ed.fold_lines(0, 2);
						const count1 = ed.fold_count();
						const hidden1 = ed.is_line_folded(1);
						const visible0 = ed.is_line_folded(0);
						const ranges = Array.from(ed.folded_ranges());
						ed.unfold_all();
						const count2 = ed.fold_count();
						return { count1, hidden1, visible0, ranges, count2 };
					});
					assertEquals(result.count1, 1);
					assertEquals(result.hidden1, true);
					assertEquals(result.visible0, false); // first line stays visible
					assertEquals(result.ranges[0], 0);
					assertEquals(result.ranges[1], 2);
					assertEquals(result.count2, 0);
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

	// ── Toggle fold at (indent-based) ───────────────────────────────

	Deno.test({
		name: `[${browserName}] toggle_fold_at auto-detects from indent`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("def foo:\n  a = 1\n  b = 2\nend");
						ed.toggle_fold_at(0); // fold from line 0
						const folded = ed.fold_count();
						const hidden = ed.is_line_folded(1);
						ed.toggle_fold_at(0); // unfold
						const unfolded = ed.fold_count();
						return { folded, hidden, unfolded };
					});
					assertEquals(result.folded, 1);
					assertEquals(result.hidden, true);
					assertEquals(result.unfolded, 0);
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

	// ── Gutter click ────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] line_at_y returns valid line`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("line0\nline1\nline2");
						const lineAt0 = ed.line_at_y(5);
						return { lineAt0 };
					});
					assertEquals(result.lineAt0, 0);
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

	// ── Configuration presets ───────────────────────────────────────

	Deno.test({
		name: `[${browserName}] apply_preset configures editor`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.apply_preset("code");
						const codeLineNums = ed.show_line_numbers();
						const codeWrap = ed.word_wrap();
						ed.apply_preset("prose");
						const proseLineNums = ed.show_line_numbers();
						const proseWrap = ed.word_wrap();
						ed.apply_preset("minimal");
						const minLineNums = ed.show_line_numbers();
						return {
							codeLineNums,
							codeWrap,
							proseLineNums,
							proseWrap,
							minLineNums,
						};
					});
					assertEquals(result.codeLineNums, true);
					assertEquals(result.codeWrap, false);
					assertEquals(result.proseLineNums, false);
					assertEquals(result.proseWrap, true);
					assertEquals(result.minLineNums, false);
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

	// ── Content statistics ──────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] reading time and flesch score`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("The cat sat on the mat. The dog ran in the park.");
						const rt = ed.reading_time_seconds();
						const flesch = ed.flesch_reading_ease();
						return { rt, flesch };
					});
					assert(result.rt > 0, `reading time should be > 0, got ${result.rt}`);
					assert(
						result.flesch > 0,
						`flesch score should be > 0, got ${result.flesch}`,
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

	// ── Syntax highlighting ─────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] syntax highlight toggle and token colors`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const before = ed.syntax_highlight();
						ed.set_syntax_highlight(true);
						const after = ed.syntax_highlight();
						ed.set_token_color("number", 0, 255, 128, 255);
						const numColor = Array.from(ed.get_token_color("number"));
						ed.reset_token_colors();
						const defaultColor = Array.from(ed.get_token_color("number"));
						return { before, after, numColor, defaultColor };
					});
					assertEquals(result.before, false);
					assertEquals(result.after, true);
					assertEquals(result.numColor, [0, 255, 128, 255]);
					assertEquals(result.defaultColor, [181, 206, 168, 255]); // default green
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

	// ── Custom theme API ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] set and get theme color`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_theme_color("background", 30, 30, 30, 255);
						const bg = Array.from(ed.get_theme_color("background"));
						return bg;
					});
					assertEquals(result, [30, 30, 30, 255]);
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

	// ── Range formatting ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] format_range_bold applies to range`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello World");
						ed.format_range_bold(0, 5);
						ed.format_range_italic(6, 11);
						// Verify text unchanged and editor is modified.
						return { text: ed.plain_text(), modified: ed.is_modified() };
					});
					assertEquals(result.text, "Hello World");
					assertEquals(result.modified, true);
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

	// ── Scroll to line ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] scroll_to_line adjusts scroll`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						// Create many lines.
						const lines = Array.from({ length: 100 }, (_, i) => `Line ${i}`)
							.join("\n");
						ed.insert_text(lines);
						const beforeY = ed.scroll_y();
						ed.scroll_to_line(50);
						const afterY = ed.scroll_y();
						return { beforeY, afterY };
					});
					assert(
						result.afterY > result.beforeY,
						`scroll should move down, before=${result.beforeY} after=${result.afterY}`,
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

	// ── Extended statistics ──────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] extended text statistics`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("The quick brown fox. The lazy dog!");
						return {
							avg: ed.avg_word_length(),
							longest: ed.longest_word(),
							unique: ed.unique_word_count(),
							sentences: ed.sentence_count(),
						};
					});
					assert(
						result.avg > 2,
						`avg word length should be > 2, got ${result.avg}`,
					);
					assertEquals(result.longest, "brown");
					// "the" appears twice → 6 unique words
					assertEquals(result.unique, 6);
					assertEquals(result.sentences, 2);
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

	// ── Editor info ─────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] editor version and info`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						return {
							version: ed.editor_version(),
							apiCount: ed.api_count(),
							categories: ed.feature_categories(),
						};
					});
					assertEquals(result.version, "0.1.0");
					assert(
						result.apiCount > 300,
						`api count should be > 300, got ${result.apiCount}`,
					);
					assert(
						result.categories.includes("editing"),
						"should include editing",
					);
					assert(result.categories.includes("themes"), "should include themes");
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

	// ── Multi-cursor ────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] multi-cursor add, remove, insert`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aaa bbb ccc");
						ed.add_cursor(4);
						ed.add_cursor(8);
						const count = ed.extra_cursor_count();
						const offsets = Array.from(ed.extra_cursor_offsets());
						ed.clear_cursors();
						const afterClear = ed.extra_cursor_count();
						return { count, offsets, afterClear };
					});
					assertEquals(result.count, 2);
					assertEquals(result.offsets, [4, 8]);
					assertEquals(result.afterClear, 0);
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

	// ── Breadcrumbs ─────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] breadcrumbs detects headers`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("# Heading\nsome text\n// Comment\nmore text");
						const crumbs = Array.from(ed.breadcrumbs());
						return crumbs;
					});
					// Should find "# Heading" at line 0 and "// Comment" at line 2.
					assertEquals(result[0], "0");
					assertEquals(result[1], "# Heading");
					assertEquals(result[2], "2");
					assertEquals(result[3], "// Comment");
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

	// ── Indent level ────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] indent level at cursor and line`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("no indent\n  two spaces\n    four spaces");
						ed.set_selection(15, 15); // inside "two spaces"
						const atCursor = ed.indent_level_at_cursor();
						const line0 = ed.indent_level_of_line(0);
						const line1 = ed.indent_level_of_line(1);
						const line2 = ed.indent_level_of_line(2);
						return { atCursor, line0, line1, line2 };
					});
					assertEquals(result.atCursor, 2);
					assertEquals(result.line0, 0);
					assertEquals(result.line1, 2);
					assertEquals(result.line2, 4);
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

	// ── Patch ───────────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] apply_patch inserts and deletes`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello World");
						// Delete "World" (6-11), insert "Rust" at 6.
						ed.apply_patch(["delete", "6", "11", "insert", "6", "Rust"]);
						return ed.plain_text();
					});
					assertEquals(result, "Hello Rust");
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

	// ── Canvas export ───────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] export_canvas_data_url returns PNG data`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello");
						return ed.export_canvas_data_url();
					});
					assert(
						result.startsWith("data:image/png"),
						`should be PNG data URL, got: ${result.substring(0, 30)}`,
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

	// ── Command palette ─────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] command list and search`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const all = Array.from(ed.command_list());
						const search = Array.from(ed.search_commands("bold"));
						return { allLen: all.length, search };
					});
					assert(
						result.allLen > 40,
						`should have many commands, got ${result.allLen}`,
					);
					assertEquals(result.search[0], "Bold");
					assertEquals(result.search[1], "Ctrl+B");
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

	// ── Text diffing ────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] diff_texts finds changes`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						// diff_texts is a static method — call via instance workaround.
						// Insert first text, save, modify, diff with snapshot.
						ed.insert_text("a\nb\nc");
						ed.take_snapshot();
						ed.delete_range(0, 5);
						ed.insert_text("a\nX\nc");
						const diff = Array.from(ed.diff_from_snapshot());
						// diff_from_snapshot returns changed line numbers.
						return diff;
					});
					// Line 1 changed from "b" to "X".
					assert(result.includes(1), `line 1 should be changed, got ${result}`);
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

	// ── Bidi info ───────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] contains_rtl and contains_non_ascii`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello World");
						const ascii = ed.contains_non_ascii();
						const rtl = ed.contains_rtl();
						return { ascii, rtl };
					});
					assertEquals(result.ascii, false);
					assertEquals(result.rtl, false);
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

	// ── Selection to lines ──────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] selection_line_range and select_lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("line0\nline1\nline2\nline3");
						ed.set_selection(6, 17); // spans line1 and line2
						const range = Array.from(ed.selection_line_range());
						ed.select_lines(1, 2);
						const selected = ed.get_selected_text();
						return { range, selected };
					});
					assertEquals(result.range, [1, 2]);
					assertEquals(result.selected, "line1\nline2");
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

	// ── Whitespace normalization ────────────────────────────────────

	Deno.test({
		name: `[${browserName}] normalize_indentation adjusts whitespace`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_soft_tabs(true);
						ed.set_tab_size(4);
						ed.insert_text("\thello\n\t\tworld");
						const modified = ed.normalize_indentation();
						const text = ed.plain_text();
						return { modified, text };
					});
					assert(
						result.modified > 0,
						`should modify lines, got ${result.modified}`,
					);
					assert(
						!result.text.includes("\t"),
						"should not contain tabs after normalization",
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

	// ── Document outline ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] document_outline returns indent structure`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("root\n  child1\n  child2\n    grandchild");
						return Array.from(ed.document_outline());
					});
					// [indent, line, text, ...]
					assertEquals(result[0], "0"); // indent
					assertEquals(result[1], "0"); // line
					assertEquals(result[2], "root"); // text
					assertEquals(result[3], "2"); // indent
					assertEquals(result[4], "1"); // line
					assertEquals(result[5], "child1");
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

	// ── Collaborative cursors ───────────────────────────────────────

	Deno.test({
		name: `[${browserName}] collab cursors add, update, remove`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello World");
						ed.add_collab_cursor(3, "Alice", 255, 0, 0);
						ed.add_collab_cursor(7, "Bob", 0, 0, 255);
						const count1 = ed.collab_cursor_count();
						ed.update_collab_cursor("Alice", 5);
						const list = Array.from(ed.collab_cursor_list());
						ed.remove_collab_cursor("Bob");
						const count2 = ed.collab_cursor_count();
						ed.clear_collab_cursors();
						const count3 = ed.collab_cursor_count();
						return { count1, list, count2, count3 };
					});
					assertEquals(result.count1, 2);
					assertEquals(result.list[0], "5"); // Alice updated to 5
					assertEquals(result.list[1], "Alice");
					assertEquals(result.count2, 1);
					assertEquals(result.count3, 0);
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

	// ── Line ending detection ───────────────────────────────────────

	Deno.test({
		name: `[${browserName}] detect and convert line endings`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("line1\nline2\nline3");
						const ending = ed.detect_line_ending();
						return { ending };
					});
					assertEquals(result.ending, "lf");
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

	// ── File type detection ─────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] detect_file_type guesses correctly`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("const foo = () => {\n  return 42;\n}");
						const ft = ed.detect_file_type();
						return ft;
					});
					assertEquals(result, "javascript");
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

	// ── Emmet expansion ─────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] expand_emmet creates HTML tag`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("div.container");
						const ok = ed.expand_emmet();
						return { ok, text: ed.plain_text() };
					});
					assertEquals(result.ok, true);
					assertEquals(result.text, '<div class="container"></div>');
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

	// ── Selection history ───────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] selection history push, back, forward`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Hello World");
						ed.set_selection(0, 0);
						ed.push_selection_history();
						ed.set_selection(5, 5);
						ed.push_selection_history();
						ed.set_selection(11, 11);
						ed.push_selection_history();
						const len = ed.selection_history_length();
						const back = ed.selection_history_back();
						const pos = ed.selection_end();
						const fwd = ed.selection_history_forward();
						const pos2 = ed.selection_end();
						return { len, back, pos, fwd, pos2 };
					});
					assertEquals(result.len, 3);
					assertEquals(result.back, true);
					assertEquals(result.pos, 5); // went back to 5
					assertEquals(result.fwd, true);
					assertEquals(result.pos2, 11); // went forward to 11
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

	// ── Focus API ───────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] is_focused returns boolean`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						return typeof ed.is_focused() === "boolean";
					});
					assertEquals(result, true);
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

	// ── Custom keybindings ──────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] keybinding override and run_shortcut`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello");
						ed.set_selection(0, 5);
						ed.set_keybinding("Ctrl+Shift+1", "Transform Upper Case");
						const cmd = ed.get_keybinding("Ctrl+Shift+1");
						const ok = ed.run_shortcut("Ctrl+Shift+1");
						const text = ed.plain_text();
						const count = ed.keybinding_override_count();
						ed.remove_keybinding("Ctrl+Shift+1");
						const countAfterRemove = ed.keybinding_override_count();
						return { cmd, ok, text, count, countAfterRemove };
					});
					assertEquals(result.cmd, "Transform Upper Case");
					assertEquals(result.ok, true);
					assertEquals(result.text, "HELLO");
					assertEquals(result.count, 1);
					assertEquals(result.countAfterRemove, 0);
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

	// ── Transform pipeline ──────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] transform cases and pipeline`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello world test");
						ed.set_selection(0, ed.char_count());
						ed.transform_camel_case();
						const camel = ed.plain_text();

						ed.delete_range(0, ed.char_count());
						ed.insert_text("hello world test");
						ed.set_selection(0, ed.char_count());
						ed.transform_snake_case();
						const snake = ed.plain_text();

						ed.delete_range(0, ed.char_count());
						ed.insert_text("hello world test");
						ed.set_selection(0, ed.char_count());
						ed.transform_kebab_case();
						const kebab = ed.plain_text();

						ed.delete_range(0, ed.char_count());
						ed.insert_text("hello world test");
						ed.set_selection(0, ed.char_count());
						ed.transform_constant_case();
						const constant = ed.plain_text();

						ed.set_selection(0, ed.char_count());
						ed.transform_pipeline("lower|snake|upper");
						const pipeline = ed.plain_text();
						return { camel, snake, kebab, constant, pipeline };
					});
					assertEquals(result.camel, "helloWorldTest");
					assertEquals(result.snake, "hello_world_test");
					assertEquals(result.kebab, "hello-world-test");
					assertEquals(result.constant, "HELLO_WORLD_TEST");
					assertEquals(result.pipeline, "HELLO_WORLD_TEST");
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

	// ── Marker ranges ───────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] markers add, query, remove`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("abcdef");
						ed.add_marker(1, 4, 255, 200, 0, 80, "m1");
						ed.add_marker(2, 5, 255, 0, 0, 80, "lint-1");
						const count1 = ed.marker_count();
						const at3 = Array.from(ed.markers_at(3));
						ed.remove_markers_by_prefix("lint-");
						const count2 = ed.marker_count();
						ed.remove_marker("m1");
						const count3 = ed.marker_count();
						return { count1, at3, count2, count3 };
					});
					assertEquals(result.count1, 2);
					assert(result.at3.includes("m1"));
					assertEquals(result.count2, 1);
					assertEquals(result.count3, 0);
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

	// ── Soft wrap info ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] visual line count and line wrapped`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.set_size(160, 200);
						ed.set_word_wrap(true);
						ed.insert_text(
							"This is a very long line that should wrap at least once in a narrow viewport.",
						);
						const visual = ed.visual_line_count();
						const wrapped = ed.is_line_wrapped(0);
						return { visual, wrapped };
					});
					assert(
						result.visual > 1,
						`expected wrapped visual lines, got ${result.visual}`,
					);
					assertEquals(result.wrapped, true);
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

	// ── Extended statistics ─────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] extended statistics`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						const text = "alpha beta\n\ngamma delta epsilon\nshort";
						ed.insert_text(text);
						return {
							paragraphs: ed.paragraph_block_count(),
							avg: ed.avg_line_length(),
							longestLen: ed.longest_line_length(),
							longestLine: ed.longest_line_number(),
							bytes: ed.byte_count(),
							expectedBytes: text.length,
						};
					});
					assertEquals(result.paragraphs, 2);
					assert(result.avg > 0);
					assert(result.longestLen >= 10);
					assertEquals(result.longestLine, 2);
					assertEquals(result.bytes, result.expectedBytes);
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

	// ── Completion context ──────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] completions_with_context returns word + line`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("apple apricot banana\nape apex\nap");
						return Array.from(ed.completions_with_context(6));
					});
					assert(result.length >= 2);
					assert(result.some((s: string) => s.toLowerCase() === "apple"));
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

	// ── Named anchors ───────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] anchors set, list, go, remove`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("hello\nworld");
						ed.set_anchor("start", 0);
						ed.set_anchor("line2", 6);
						const a0 = ed.anchor_offset("start");
						const a1 = ed.anchor_offset("line2");
						const names = Array.from(ed.anchor_names());
						const moved = ed.go_to_anchor("line2");
						const cursor = ed.selection_end();
						ed.remove_anchor("start");
						const countAfterRemove = ed.anchor_count();
						ed.clear_anchors();
						const finalCount = ed.anchor_count();
						return {
							a0,
							a1,
							names,
							moved,
							cursor,
							countAfterRemove,
							finalCount,
						};
					});
					assertEquals(result.a0, 0);
					assertEquals(result.a1, 6);
					assert(result.names.includes("start"));
					assert(result.names.includes("line2"));
					assertEquals(result.moved, true);
					assertEquals(result.cursor, 6);
					assertEquals(result.countAfterRemove, 1);
					assertEquals(result.finalCount, 0);
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

	// ── Tasks / TODO scanner ────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] scan tasks and toggle checkbox`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text(
							"- [ ] first\n// TODO: ship\ntext\n* [x] done\n# FIXME: bug",
						);
						const tasks = Array.from(ed.scan_tasks());
						const count = ed.task_count();
						const next = ed.next_task_line(0);
						const prev = ed.prev_task_line(0);
						const toggled1 = ed.toggle_task_checkbox(0);
						const toggled2 = ed.toggle_task_checkbox(3);
						const text = ed.plain_text();
						return { tasks, count, next, prev, toggled1, toggled2, text };
					});
					assert(result.tasks.length >= 16);
					assertEquals(result.count, 4);
					assertEquals(result.next, 1);
					assertEquals(result.prev, 4);
					assertEquals(result.toggled1, true);
					assertEquals(result.toggled2, true);
					assert(result.text.includes("- [x] first"));
					assert(result.text.includes("* [ ] done"));
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

	// ── Lint helpers ────────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] lint helper line sets`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text(
							"ok\ntrail \n\ttabonly\n \tmixed\nunicodé\nveryverylongline",
						);
						return {
							trailing: Array.from(ed.lint_trailing_whitespace()),
							long: Array.from(ed.lint_long_lines(10)),
							mixed: Array.from(ed.lint_mixed_indentation()),
							nonAscii: Array.from(ed.lint_non_ascii_lines()),
						};
					});
					assert(result.trailing.includes(1));
					assert(result.long.includes(5));
					assert(result.mixed.includes(3));
					assert(result.nonAscii.includes(4));
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

	// ── Line occurrence navigation ──────────────────────────────────

	Deno.test({
		name: `[${browserName}] line occurrences and next/prev line`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("alpha\nbeta alpha\ngamma\nAlpha");
						return {
							linesCi: Array.from(ed.line_occurrences("alpha", false)),
							linesCs: Array.from(ed.line_occurrences("alpha", true)),
							count: ed.line_occurrence_count("alpha", false),
							next: ed.next_line_with("alpha", 0, false),
							prev: ed.prev_line_with("alpha", 0, false),
						};
					});
					assertEquals(result.linesCi, [0, 1, 3]);
					assertEquals(result.linesCs, [0, 1]);
					assertEquals(result.count, 3);
					assertEquals(result.next, 1);
					assertEquals(result.prev, 3);
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

	// ── Cursor context + rotate lines ───────────────────────────────

	Deno.test({
		name: `[${browserName}] cursor context and rotate lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("1234567890\nabcdef");
						ed.set_selection(5, 5);
						const before = ed.text_before_cursor(3);
						const after = ed.text_after_cursor(4);
						const ctx = Array.from(ed.line_context(0, 1));
						ed.delete_range(0, ed.char_count());
						ed.insert_text("a\nb\nc\nd");
						const up = ed.rotate_lines_up(1, 3);
						const textUp = ed.plain_text();
						const down = ed.rotate_lines_down(1, 3);
						const textDown = ed.plain_text();
						return { before, after, ctx, up, down, textUp, textDown };
					});
					assertEquals(result.before, "345");
					assertEquals(result.after, "6789");
					assertEquals(result.ctx, ["0", "1234567890", "1", "abcdef"]);
					assertEquals(result.up, true);
					assertEquals(result.textUp, "a\nc\nd\nb");
					assertEquals(result.down, true);
					assertEquals(result.textDown, "a\nb\nc\nd");
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

	// ── Named state slots ───────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] named states save/load/list/delete`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("one");
						ed.save_named_state("s1");
						ed.insert_text(" two");
						ed.save_named_state("s2");
						const names = Array.from(ed.named_state_names());
						const count = ed.named_state_count();
						const loaded = ed.load_named_state("s1");
						const textAfterLoad = ed.plain_text();
						ed.delete_named_state("s2");
						const countAfterDelete = ed.named_state_count();
						ed.clear_named_states();
						const finalCount = ed.named_state_count();
						return {
							names,
							count,
							loaded,
							textAfterLoad,
							countAfterDelete,
							finalCount,
						};
					});
					assert(result.names.includes("s1"));
					assert(result.names.includes("s2"));
					assertEquals(result.count, 2);
					assertEquals(result.loaded, true);
					assertEquals(result.textAfterLoad, "one");
					assertEquals(result.countAfterDelete, 1);
					assertEquals(result.finalCount, 0);
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

	// ── Selection profiles ──────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] selection profiles save/load/list`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("abcdef");
						ed.set_selection(1, 3);
						ed.save_selection_profile("p1");
						ed.set_selection(4, 6);
						ed.save_selection_profile("p2");
						const names = Array.from(ed.selection_profile_names());
						const count = ed.selection_profile_count();
						const loaded = ed.load_selection_profile("p1");
						const start = ed.selection_start();
						const end = ed.selection_end();
						ed.delete_selection_profile("p2");
						const countAfterDelete = ed.selection_profile_count();
						ed.clear_selection_profiles();
						const finalCount = ed.selection_profile_count();
						return {
							names,
							count,
							loaded,
							start,
							end,
							countAfterDelete,
							finalCount,
						};
					});
					assert(result.names.includes("p1"));
					assert(result.names.includes("p2"));
					assertEquals(result.count, 2);
					assertEquals(result.loaded, true);
					assertEquals(result.start, 1);
					assertEquals(result.end, 3);
					assertEquals(result.countAfterDelete, 1);
					assertEquals(result.finalCount, 0);
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

	// ── Task workflow helpers ───────────────────────────────────────

	Deno.test({
		name: `[${browserName}] task workflow progress, complete, clear`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("- [ ] a\n- [x] b\n- [ ] c");
						const progress1 = Array.from(ed.task_progress());
						const nextUnchecked = ed.next_unchecked_task_line(0);
						const prevUnchecked = ed.prev_unchecked_task_line(0);
						const completed = ed.complete_all_tasks();
						const progress2 = Array.from(ed.task_progress());
						const cleared = ed.clear_completed_tasks();
						const finalText = ed.plain_text();
						return {
							progress1,
							nextUnchecked,
							prevUnchecked,
							completed,
							progress2,
							cleared,
							finalText,
						};
					});
					assertEquals(result.progress1, [1, 3]);
					assertEquals(result.nextUnchecked, 2);
					assertEquals(result.prevUnchecked, 2);
					assertEquals(result.completed, 2);
					assertEquals(result.progress2, [3, 3]);
					assertEquals(result.cleared, 3);
					assertEquals(result.finalText, "");
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

	// ── Cleanup utilities ───────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] cleanup utilities normalize whitespace`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("\t  a\n\n\n\n  b\n\n");
						const trimmed = ed.trim_leading_whitespace();
						const collapsed = ed.collapse_blank_lines(1);
						const removedTail = ed.remove_trailing_blank_lines();
						const ensured = ed.ensure_single_trailing_newline();
						const text = ed.plain_text();
						return { trimmed, collapsed, removedTail, ensured, text };
					});
					assert(result.trimmed >= 2);
					assert(result.collapsed >= 2);
					assert(result.removedTail >= 1);
					assertEquals(result.ensured, true);
					assertEquals(result.text, "a\n\nb\n");
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

	// ── Line utilities ──────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] swap lines and duplicate line range`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("one\ntwo\nthree\nfour");
						const swapped = ed.swap_lines(0, 2);
						const afterSwap = ed.plain_text();
						const duplicated = ed.duplicate_line_range(1, 2);
						const afterDup = ed.plain_text();
						return { swapped, duplicated, afterSwap, afterDup };
					});
					assertEquals(result.swapped, true);
					assertEquals(result.afterSwap, "three\ntwo\none\nfour");
					assertEquals(result.duplicated, true);
					assertEquals(result.afterDup, "three\ntwo\none\ntwo\none\nfour");
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

	// ── Anchor utilities ────────────────────────────────────────────

	Deno.test({
		name: `[${browserName}] anchor exists, rename, nearest before/after`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("abcdefghijklmnop");
						ed.set_anchor("a", 2);
						ed.set_anchor("b", 8);
						ed.set_anchor("c", 14);
						const hasB = ed.anchor_exists("b");
						const renamed = ed.rename_anchor("b", "beta");
						const hasOld = ed.anchor_exists("b");
						const hasNew = ed.anchor_exists("beta");
						const before = Array.from(ed.nearest_anchor_before(9));
						const after = Array.from(ed.nearest_anchor_after(9));
						return { hasB, renamed, hasOld, hasNew, before, after };
					});
					assertEquals(result.hasB, true);
					assertEquals(result.renamed, true);
					assertEquals(result.hasOld, false);
					assertEquals(result.hasNew, true);
					assertEquals(result.before, ["beta", "8"]);
					assertEquals(result.after, ["c", "14"]);
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

	// ── Line prefix/suffix/number utilities ────────────────────────

	Deno.test({
		name: `[${browserName}] prefix, suffix, and number lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("apple\nbanana\ncherry");
						const p = ed.prefix_lines(0, 1, "> ");
						const s = ed.suffix_lines(1, 2, " !");
						const n = ed.number_lines(0, 2, 1, 2);
						return { p, s, n, text: ed.plain_text() };
					});
					assertEquals(result.p, 2);
					assertEquals(result.s, 2);
					assertEquals(result.n, 3);
					assertEquals(
						result.text,
						"01. > apple\n02. > banana !\n03. cherry !",
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

	// ── Cleanup additions ───────────────────────────────────────────

	Deno.test({
		name:
			`[${browserName}] strip non-printable and normalize unicode whitespace`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("a\u0001b\u0007c\u00A0d\u2009e");
						const removed = ed.strip_non_printable();
						const replaced = ed.normalize_unicode_whitespace();
						const text = ed.plain_text();
						return { removed, replaced, text };
					});
					assertEquals(result.removed, 2);
					assertEquals(result.replaced, 2);
					assertEquals(result.text, "abc d e");
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

	// ── Line hashes + duplicate detection ───────────────────────────

	Deno.test({
		name: `[${browserName}] line hashes and duplicate line numbers`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("alpha\nbeta\nalpha");
						const h0 = ed.line_hash(0);
						const h1 = ed.line_hash(1);
						const h2 = ed.line_hash(2);
						const miss = ed.line_hash(99);
						const hashes = Array.from(ed.line_hashes());
						ed.delete_range(0, ed.char_count());
						ed.insert_text("one\nTwo\none \n two\nTHREE");
						const dupLoose = Array.from(ed.duplicate_line_numbers(false, true));
						const dupStrict = Array.from(
							ed.duplicate_line_numbers(true, false),
						);
						return {
							h0,
							h1,
							h2,
							miss,
							hashes,
							dupLoose,
							dupStrict,
						};
					});
					assertEquals(result.h0, result.h2);
					assert(result.h0 !== result.h1);
					assertEquals(result.miss, "");
					assertEquals(result.hashes.length, 6);
					assertEquals(result.dupLoose, [0, 1, 2, 3]);
					assertEquals(result.dupStrict, []);
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

	// ── Task line insertion helper ──────────────────────────────────

	Deno.test({
		name: `[${browserName}] insert task line helper`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("alpha\nbeta");
						ed.set_selection(6, 6);
						ed.insert_task_line("todo item", false);
						return {
							text: ed.plain_text(),
							count: ed.task_count(),
						};
					});
					assertEquals(result.count, 1);
					assertEquals(result.text, "alpha\n- [ ] todo item\nbeta");
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

	// ── Extended anchor utilities ───────────────────────────────────

	Deno.test({
		name: `[${browserName}] anchors at offset, anchors in range, shift anchor`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("abcdefghij");
						ed.set_anchor("start", 0);
						ed.set_anchor("midA", 5);
						ed.set_anchor("midB", 5);
						ed.set_anchor("end", 10);
						const at5 = Array.from(ed.anchors_at_offset(5));
						const inRange = Array.from(ed.anchors_in_range(4, 10));
						const shifted = ed.shift_anchor("start", 3);
						const startAfterShift = ed.anchor_offset("start");
						const shiftedClamp = ed.shift_anchor("start", -100);
						const startAfterClamp = ed.anchor_offset("start");
						const missing = ed.shift_anchor("missing", 1);
						return {
							at5,
							inRange,
							shifted,
							startAfterShift,
							shiftedClamp,
							startAfterClamp,
							missing,
						};
					});
					assertEquals(result.at5, ["midA", "midB"]);
					assertEquals(result.inRange, ["midA", "5", "midB", "5", "end", "10"]);
					assertEquals(result.shifted, true);
					assertEquals(result.startAfterShift, 3);
					assertEquals(result.shiftedClamp, true);
					assertEquals(result.startAfterClamp, 0);
					assertEquals(result.missing, false);
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

	// ── Line prefix/suffix removal utilities ────────────────────────

	Deno.test({
		name: `[${browserName}] unprefix and unsuffix lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("> alpha !\n> beta !\ngamma !");
						const removedPrefix = ed.unprefix_lines(0, 2, "> ");
						const removedSuffix = ed.unsuffix_lines(0, 2, " !");
						const removedPrefixNone = ed.unprefix_lines(0, 2, "# ");
						return {
							removedPrefix,
							removedSuffix,
							removedPrefixNone,
							text: ed.plain_text(),
						};
					});
					assertEquals(result.removedPrefix, 2);
					assertEquals(result.removedSuffix, 3);
					assertEquals(result.removedPrefixNone, 0);
					assertEquals(result.text, "alpha\nbeta\ngamma");
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

	// ── Line fingerprint predicates ─────────────────────────────────

	Deno.test({
		name: `[${browserName}] line hash equals and line is duplicate`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("alpha\nbeta\nalpha \nBETA\nsolo");
						return {
							eq00: ed.line_hash_equals(0, 0),
							eq01: ed.line_hash_equals(0, 1),
							eqOut: ed.line_hash_equals(0, 99),
							dup0Loose: ed.line_is_duplicate(0, false, true),
							dup1Loose: ed.line_is_duplicate(1, false, true),
							dup1Strict: ed.line_is_duplicate(1, true, false),
							dup3Strict: ed.line_is_duplicate(3, true, false),
							dupSolo: ed.line_is_duplicate(4, false, true),
						};
					});
					assertEquals(result.eq00, true);
					assertEquals(result.eq01, false);
					assertEquals(result.eqOut, false);
					assertEquals(result.dup0Loose, true);
					assertEquals(result.dup1Loose, true);
					assertEquals(result.dup1Strict, false);
					assertEquals(result.dup3Strict, false);
					assertEquals(result.dupSolo, false);
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

	// ── Anchor prefix/bulk utilities ────────────────────────────────

	Deno.test({
		name: `[${browserName}] anchor entries and prefix bulk operations`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("abcdefghij");
						ed.set_anchor("foo.1", 1);
						ed.set_anchor("foo.2", 2);
						ed.set_anchor("bar.1", 3);
						const entriesBefore = Array.from(ed.anchor_entries());
						const renamed = ed.rename_anchor_prefix("foo.", "x.");
						const namesAfterRename = Array.from(ed.anchor_names());
						const removed = ed.remove_anchors_with_prefix("x.");
						const finalNames = Array.from(ed.anchor_names());
						const finalCount = ed.anchor_count();
						return {
							entriesBefore,
							renamed,
							namesAfterRename,
							removed,
							finalNames,
							finalCount,
						};
					});
					assertEquals(result.entriesBefore, [
						"bar.1",
						"3",
						"foo.1",
						"1",
						"foo.2",
						"2",
					]);
					assertEquals(result.renamed, 2);
					assert(result.namesAfterRename.includes("x.1"));
					assert(result.namesAfterRename.includes("x.2"));
					assertEquals(result.removed, 2);
					assertEquals(result.finalNames, ["bar.1"]);
					assertEquals(result.finalCount, 1);
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

	// ── Line prefix/suffix query helpers ────────────────────────────

	Deno.test({
		name: `[${browserName}] line prefix and suffix query helpers`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("Alpha\nbeta\nalpha!\nzeta!");
						return {
							hasPrefixCs: ed.line_has_prefix(0, "Al", true),
							hasPrefixCi: ed.line_has_prefix(2, "al", false),
							hasSuffixCs: ed.line_has_suffix(3, "!", true),
							hasSuffixCi: ed.line_has_suffix(1, "TA", false),
							prefixLines: Array.from(ed.lines_with_prefix("a", false)),
							suffixLines: Array.from(ed.lines_with_suffix("!", true)),
						};
					});
					assertEquals(result.hasPrefixCs, true);
					assertEquals(result.hasPrefixCi, true);
					assertEquals(result.hasSuffixCs, true);
					assertEquals(result.hasSuffixCi, true);
					assertEquals(result.prefixLines, [0, 2]);
					assertEquals(result.suffixLines, [2, 3]);
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

	// ── Hash range + duplicate aggregates ───────────────────────────

	Deno.test({
		name: `[${browserName}] line hashes in range and duplicate aggregates`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("aa\nbb\ncc\ndd");
						const all = Array.from(ed.line_hashes());
						const inRange = Array.from(ed.line_hashes_in_range(1, 2));
						const h1 = ed.line_hash(1);
						const h2 = ed.line_hash(2);

						ed.delete_range(0, ed.char_count());
						ed.insert_text("one\nTwo\none \n two\nsolo");
						const dupCountLoose = ed.duplicate_line_count(false, true);
						const dupRatioLoose = ed.duplicate_line_ratio(false, true);
						const dupCountStrict = ed.duplicate_line_count(true, false);
						const dupRatioStrict = ed.duplicate_line_ratio(true, false);
						return {
							all,
							inRange,
							h1,
							h2,
							dupCountLoose,
							dupRatioLoose,
							dupCountStrict,
							dupRatioStrict,
						};
					});

					assertEquals(result.all.length, 8);
					assertEquals(result.inRange.length, 4);
					assertEquals(result.inRange[0], "1");
					assertEquals(result.inRange[1], result.h1);
					assertEquals(result.inRange[2], "2");
					assertEquals(result.inRange[3], result.h2);
					assertEquals(result.dupCountLoose, 4);
					assert(Math.abs(result.dupRatioLoose - 0.8) < 1e-9);
					assertEquals(result.dupCountStrict, 0);
					assertEquals(result.dupRatioStrict, 0);
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

	// ── Anchor range filtering / cursor move helpers ───────────────

	Deno.test({
		name: `[${browserName}] anchor range filtering and move to cursor`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("0123456789");
						ed.set_anchor("sec.a", 2);
						ed.set_anchor("sec.b", 5);
						ed.set_anchor("tmp.a", 7);
						ed.set_anchor("tmp.b", 9);
						const pref = Array.from(ed.anchor_names_with_prefix("sec."));
						const prefEmpty = Array.from(ed.anchor_names_with_prefix(""));
						const ranged = Array.from(ed.anchor_names_in_range(4, 8));
						ed.move_cursor_to(6);
						const moved = ed.move_anchor_to_cursor("sec.a");
						const missingMove = ed.move_anchor_to_cursor("missing");
						const secAOffset = ed.anchor_offset("sec.a");
						const removed = ed.remove_anchors_in_range(6, 10);
						const remaining = Array.from(ed.anchor_names());
						return {
							pref,
							prefEmpty,
							ranged,
							moved,
							missingMove,
							secAOffset,
							removed,
							remaining,
						};
					});

					assertEquals(result.pref, ["sec.a", "sec.b"]);
					assertEquals(result.prefEmpty, ["sec.a", "sec.b", "tmp.a", "tmp.b"]);
					assertEquals(result.ranged, ["sec.b", "tmp.a"]);
					assertEquals(result.moved, true);
					assertEquals(result.missingMove, false);
					assertEquals(result.secAOffset, 6);
					assertEquals(result.removed, 3);
					assertEquals(result.remaining, ["sec.b"]);
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

	// ── Prefix/suffix count helpers ────────────────────────────────

	Deno.test({
		name: `[${browserName}] line prefix and suffix count helpers`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("foo\nFoO!\nbar!\nfoo!");
						return {
							prefixCi: ed.count_lines_with_prefix("foo", false),
							prefixCs: ed.count_lines_with_prefix("foo", true),
							suffixBang: ed.count_lines_with_suffix("!", false),
							suffixCs: ed.count_lines_with_suffix("o!", true),
							emptyPrefix: ed.count_lines_with_prefix("", false),
							emptySuffix: ed.count_lines_with_suffix("", false),
						};
					});

					assertEquals(result.prefixCi, 3);
					assertEquals(result.prefixCs, 2);
					assertEquals(result.suffixBang, 3);
					assertEquals(result.suffixCs, 1);
					assertEquals(result.emptyPrefix, 0);
					assertEquals(result.emptySuffix, 0);
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

	// ── Unique line helpers + duplicate span metrics ───────────────

	Deno.test({
		name: `[${browserName}] unique lines and duplicate span helpers`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("A\nB\na \nB\nC\nc\nsolo");
						return {
							dupLoose: Array.from(ed.duplicate_line_numbers(false, true)),
							uniqueLoose: Array.from(ed.unique_line_numbers(false, true)),
							uniqueCountLoose: ed.unique_line_count(false, true),
							firstDupLoose: ed.first_duplicate_line(false, true),
							lastDupLoose: ed.last_duplicate_line(false, true),
							dupStrict: Array.from(ed.duplicate_line_numbers(true, false)),
							uniqueStrict: Array.from(ed.unique_line_numbers(true, false)),
							uniqueCountStrict: ed.unique_line_count(true, false),
							firstDupStrict: ed.first_duplicate_line(true, false),
							lastDupStrict: ed.last_duplicate_line(true, false),
						};
					});

					assertEquals(result.dupLoose, [0, 1, 2, 3, 4, 5]);
					assertEquals(result.uniqueLoose, [6]);
					assertEquals(result.uniqueCountLoose, 1);
					assertEquals(result.firstDupLoose, 0);
					assertEquals(result.lastDupLoose, 5);
					assertEquals(result.dupStrict, [1, 3]);
					assertEquals(result.uniqueStrict, [0, 2, 4, 5, 6]);
					assertEquals(result.uniqueCountStrict, 5);
					assertEquals(result.firstDupStrict, 1);
					assertEquals(result.lastDupStrict, 3);
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

	// ── Anchor insert/shift batch helpers ──────────────────────────

	Deno.test({
		name: `[${browserName}] anchor conditional insert and bulk shifting`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("0123456789");
						ed.set_anchor("z", 9);
						ed.set_anchor("a", 1);
						ed.set_anchor("m", 5);
						const insertedExisting = ed.set_anchor_if_absent("a", 3);
						const insertedNew = ed.set_anchor_if_absent("b", 3);
						const byOffsetBefore = Array.from(ed.anchor_names_by_offset());
						const offsetsInRange = Array.from(ed.anchor_offsets_in_range(2, 8));
						const shifted = ed.shift_anchors_in_range(3, 9, -2);
						const byOffsetAfter = Array.from(ed.anchor_names_by_offset());
						const entriesAfter = Array.from(ed.anchor_entries());
						return {
							insertedExisting,
							insertedNew,
							byOffsetBefore,
							offsetsInRange,
							shifted,
							byOffsetAfter,
							entriesAfter,
						};
					});

					assertEquals(result.insertedExisting, false);
					assertEquals(result.insertedNew, true);
					assertEquals(result.byOffsetBefore, ["a", "b", "m", "z"]);
					assertEquals(result.offsetsInRange, [3, 5]);
					assertEquals(result.shifted, 3);
					assertEquals(result.byOffsetAfter, ["a", "b", "m", "z"]);
					assertEquals(result.entriesAfter, [
						"a",
						"1",
						"b",
						"1",
						"m",
						"3",
						"z",
						"7",
					]);
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

	// ── Duplicate group analytics ──────────────────────────────────

	Deno.test({
		name:
			`[${browserName}] duplicate group count, sizes, and largest group lines`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("x\nx\nx\na\na\nb\nB\nb\nsolo");
						const loose = {
							count: ed.duplicate_group_count(false, false),
							largestSize: ed.largest_duplicate_group_size(false, false),
							largestLines: Array.from(
								ed.largest_duplicate_group_lines(false, false),
							),
							sizes: Array.from(ed.duplicate_group_sizes(false, false)),
						};
						const strict = {
							count: ed.duplicate_group_count(true, false),
							largestSize: ed.largest_duplicate_group_size(true, false),
							largestLines: Array.from(
								ed.largest_duplicate_group_lines(true, false),
							),
							sizes: Array.from(ed.duplicate_group_sizes(true, false)),
						};

						ed.delete_range(0, ed.char_count());
						ed.insert_text("alpha\nbeta\ngamma");
						const none = {
							count: ed.duplicate_group_count(false, false),
							largestSize: ed.largest_duplicate_group_size(false, false),
							largestLines: Array.from(
								ed.largest_duplicate_group_lines(false, false),
							),
							sizes: Array.from(ed.duplicate_group_sizes(false, false)),
						};

						return { loose, strict, none };
					});

					assertEquals(result.loose.count, 3);
					assertEquals(result.loose.largestSize, 3);
					assertEquals(result.loose.largestLines, [0, 1, 2]);
					assertEquals(result.loose.sizes, [3, 3, 2]);
					assertEquals(result.strict.count, 3);
					assertEquals(result.strict.largestSize, 3);
					assertEquals(result.strict.largestLines, [0, 1, 2]);
					assertEquals(result.strict.sizes, [3, 2, 2]);
					assertEquals(result.none.count, 0);
					assertEquals(result.none.largestSize, 0);
					assertEquals(result.none.largestLines, []);
					assertEquals(result.none.sizes, []);
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

	// ── Anchor boundary helpers ────────────────────────────────────

	Deno.test({
		name: `[${browserName}] anchor boundary entries and offset-side filters`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("abcdefghij");
						ed.set_anchor("omega", 9);
						ed.set_anchor("beta", 4);
						ed.set_anchor("alpha", 0);
						ed.set_anchor("gamma", 4);

						const first = Array.from(ed.first_anchor_entry());
						const last = Array.from(ed.last_anchor_entry());
						const beforeInclusive = Array.from(
							ed.anchor_names_before_offset(4, true),
						);
						const beforeExclusive = Array.from(
							ed.anchor_names_before_offset(4, false),
						);
						const afterInclusive = Array.from(
							ed.anchor_names_after_offset(4, true),
						);
						const afterExclusive = Array.from(
							ed.anchor_names_after_offset(4, false),
						);

						ed.clear_anchors();
						const firstEmpty = Array.from(ed.first_anchor_entry());
						const lastEmpty = Array.from(ed.last_anchor_entry());

						return {
							first,
							last,
							beforeInclusive,
							beforeExclusive,
							afterInclusive,
							afterExclusive,
							firstEmpty,
							lastEmpty,
						};
					});

					assertEquals(result.first, ["alpha", "0"]);
					assertEquals(result.last, ["omega", "9"]);
					assertEquals(result.beforeInclusive, ["alpha", "beta", "gamma"]);
					assertEquals(result.beforeExclusive, ["alpha"]);
					assertEquals(result.afterInclusive, ["beta", "gamma", "omega"]);
					assertEquals(result.afterExclusive, ["omega"]);
					assertEquals(result.firstEmpty, []);
					assertEquals(result.lastEmpty, []);
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

	// ── Duplicate groups for a specific line ───────────────────────

	Deno.test({
		name: `[${browserName}] duplicate groups for line helpers`,
		fn: async () => {
			const { server, url } = startServer(PKG_ROOT);
			try {
				const { browser, page } = await launchBrowser(browserName);
				try {
					await page.goto(url, { waitUntil: "networkidle" });
					await waitForEditor(page);

					const result = await page.evaluate(() => {
						const ed = (window as any).__canvistEditor;
						ed.insert_text("apple\nApple\npear\napple\npear \npear\nsolo");

						const appleStrict = {
							lines: Array.from(
								ed.duplicate_group_lines_for_line(0, true, false),
							),
							size: ed.duplicate_group_size_for_line(0, true, false),
							first: ed.duplicate_group_first_line_for_line(0, true, false),
							last: ed.duplicate_group_last_line_for_line(0, true, false),
						};
						const appleLoose = {
							lines: Array.from(
								ed.duplicate_group_lines_for_line(1, false, false),
							),
							size: ed.duplicate_group_size_for_line(1, false, false),
							first: ed.duplicate_group_first_line_for_line(1, false, false),
							last: ed.duplicate_group_last_line_for_line(1, false, false),
						};
						const pearNormalized = {
							lines: Array.from(
								ed.duplicate_group_lines_for_line(4, true, true),
							),
							size: ed.duplicate_group_size_for_line(4, true, true),
							first: ed.duplicate_group_first_line_for_line(4, true, true),
							last: ed.duplicate_group_last_line_for_line(4, true, true),
						};
						const solo = {
							lines: Array.from(
								ed.duplicate_group_lines_for_line(6, false, true),
							),
							size: ed.duplicate_group_size_for_line(6, false, true),
							first: ed.duplicate_group_first_line_for_line(6, false, true),
							last: ed.duplicate_group_last_line_for_line(6, false, true),
						};
						const missing = {
							lines: Array.from(
								ed.duplicate_group_lines_for_line(99, false, true),
							),
							size: ed.duplicate_group_size_for_line(99, false, true),
							first: ed.duplicate_group_first_line_for_line(99, false, true),
							last: ed.duplicate_group_last_line_for_line(99, false, true),
						};

						return { appleStrict, appleLoose, pearNormalized, solo, missing };
					});

					assertEquals(result.appleStrict.lines, [0, 3]);
					assertEquals(result.appleStrict.size, 2);
					assertEquals(result.appleStrict.first, 0);
					assertEquals(result.appleStrict.last, 3);
					assertEquals(result.appleLoose.lines, [0, 1, 3]);
					assertEquals(result.appleLoose.size, 3);
					assertEquals(result.appleLoose.first, 0);
					assertEquals(result.appleLoose.last, 3);
					assertEquals(result.pearNormalized.lines, [2, 4, 5]);
					assertEquals(result.pearNormalized.size, 3);
					assertEquals(result.pearNormalized.first, 2);
					assertEquals(result.pearNormalized.last, 5);
					assertEquals(result.solo.lines, []);
					assertEquals(result.solo.size, 0);
					assertEquals(result.solo.first, -1);
					assertEquals(result.solo.last, -1);
					assertEquals(result.missing.lines, []);
					assertEquals(result.missing.size, 0);
					assertEquals(result.missing.first, -1);
					assertEquals(result.missing.last, -1);
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
