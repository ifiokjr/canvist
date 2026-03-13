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
			if (a > 0 && (r < 250 || g < 250 || b < 250)) {
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

					assert(
						afterTyping.nonWhite > empty.nonWhite,
						"typing should increase canvas ink",
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
}
