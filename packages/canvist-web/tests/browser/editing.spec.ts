/**
 * Editing in the browser: typing, key bindings, IME composition, pointer
 * selection, the clipboard, drag and drop, and the accessibility mirror,
 * through EditContext (Chromium) and the textarea fallback (everywhere,
 * and in Chromium with `input: "textarea"`). The performance tests measure
 * keystroke-to-paint and type a 1,000-key burst, writing their numbers to
 * `report/perf/`.
 *
 * @module
 */
import { expect, type Page, test } from "@playwright/test";
import { mkdir, writeFile } from "node:fs/promises";
import { platform } from "node:os";
import type { Editor, InputLatency, StarterKit } from "../../src/mod.ts";

/** The p95 keystroke-to-paint budget, in milliseconds. */
const KEYSTROKE_P95_BUDGET_MS = 8;
/** How many characters the latency test types. */
const LATENCY_KEYS = 200;
/** The pause between keys in the latency test, in milliseconds: longer than a frame. */
const KEY_DELAY_MS = 20;
/** How many keys the burst test types without pausing. */
const BURST_KEYS = 1000;
/** Event Timing reports only events at least this long, in milliseconds. */
const EVENT_TIMING_THRESHOLD_MS = 16;

type Inputs = "auto" | "textarea";

declare global {
	// The page's editor, which `tests/pages/editing.ts` puts on `globalThis`.
	var editor: Editor<StarterKit>;
	var slowEvents: PerformanceEventTiming[];
	// The key `Mod` means in the page's key bindings.
	var modifier: "Meta" | "Control";
}

async function open(
	page: Page,
	input: Inputs,
	markdown = "Hello world\n\nSecond paragraph",
): Promise<void> {
	const params = new URLSearchParams({ input, markdown });
	await page.goto(`/editing.html?${params}`);
	await page.waitForFunction(() => globalThis.editor !== undefined);
}

/** Press `key` with the modifier `Mod` means in the page. */
async function pressMod(page: Page, key: string): Promise<void> {
	const modifier = await page.evaluate(() => globalThis.modifier);
	await page.keyboard.press(`${modifier}+${key}`);
}

/** The text of every paragraph-like block, in order. */
function paragraphs(page: Page): Promise<string[]> {
	return page.evaluate(() =>
		editor.geometry().blocks.flatMap((block) =>
			block.stops === undefined ? [] : [block.text ?? ""]
		)
	);
}

/** The client coordinates of UTF-16 `offset` in the `index`th text block, mid-line. */
async function pointAt(
	page: Page,
	index: number,
	offset: number,
): Promise<{ x: number; y: number }> {
	return await page.evaluate(([index, offset]) => {
		const blocks = editor.geometry().blocks.filter((block) =>
			block.stops !== undefined
		);
		const stop = blocks[index]?.stops?.find((entry) => entry[0] === offset);

		if (stop === undefined) {
			throw new Error(`no caret stop at ${index}:${offset}`);
		}

		const canvas = editor.canvas.getBoundingClientRect();

		return { x: canvas.left + stop[1], y: canvas.top + stop[3] + stop[4] / 2 };
	}, [index, offset] as const);
}

async function clickAt(
	page: Page,
	index: number,
	offset: number,
): Promise<void> {
	const { x, y } = await pointAt(page, index, offset);
	await page.mouse.click(x, y);
}

function selection(page: Page): Promise<[number, number] | null> {
	return page.evaluate(() => {
		const range = editor.selection;

		return range === null ? null : [range.anchor.offset, range.head.offset];
	});
}

for (const input of ["auto", "textarea"] as const) {
	test.describe(`${input} input`, () => {
		test.skip(
			({ browserName }) => input === "textarea" && browserName !== "chromium",
			"only Chromium has EditContext, so elsewhere auto is the textarea",
		);

		test("uses the input path it was asked for", async ({ page, browserName }) => {
			await open(page, input);
			const expected = input === "auto" && browserName === "chromium"
				? "editcontext"
				: "textarea";

			expect(await page.evaluate(() => editor.inputMode)).toBe(expected);
			await expect(page.locator("textarea.cv-input")).toHaveCount(
				expected === "textarea" ? 1 : 0,
			);
			await expect(page.getByRole("textbox", { name: "Test editor" }))
				.toHaveCount(1);
		});

		test("types text and runs key bindings", async ({ page }) => {
			await open(page, input);
			await clickAt(page, 0, 11);
			await page.keyboard.type("!");
			await page.keyboard.press("Enter");
			await page.keyboard.type("New");
			await page.keyboard.press("Backspace");

			expect(await paragraphs(page)).toEqual([
				"Hello world!",
				"Ne",
				"Second paragraph",
			]);

			const typed = await paragraphs(page);
			await pressMod(page, "z");
			expect(await paragraphs(page)).not.toEqual(typed);
			await pressMod(page, "Shift+z");
			expect(await paragraphs(page)).toEqual(typed);
		});

		test("runs input rules as typing does", async ({ page }) => {
			await open(page, input, "");
			await page.evaluate(() => editor.focus());
			await page.keyboard.type("## Title");

			expect(await page.evaluate(() => editor.export("markdown").trim())).toBe(
				"## Title",
			);
		});

		test("selects with the pointer", async ({ page }) => {
			await open(page, input);
			const word = await pointAt(page, 0, 8);
			await page.mouse.dblclick(word.x, word.y);
			expect(await selection(page)).toEqual([6, 11]);

			await page.mouse.click(word.x, word.y, { clickCount: 3 });
			expect(await selection(page)).toEqual([0, 11]);

			await clickAt(page, 0, 0);
			const end = await pointAt(page, 0, 5);
			await page.keyboard.down("Shift");
			await page.mouse.click(end.x, end.y);
			await page.keyboard.up("Shift");
			expect(await selection(page)).toEqual([0, 5]);

			const start = await pointAt(page, 0, 11);
			await page.mouse.move(start.x, start.y);
			await page.mouse.down();
			await page.mouse.move(end.x, end.y, { steps: 4 });
			await page.mouse.up();
			expect(await selection(page)).toEqual([11, 5]);

			const bold = await pointAt(page, 0, 8);
			await page.mouse.dblclick(bold.x, bold.y);
			await pressMod(page, "b");
			expect(await page.evaluate(() => editor.export("markdown"))).toContain(
				"Hello **world**",
			);
		});

		test("copies, cuts, and pastes", async ({ page }) => {
			await open(page, input);
			const word = await pointAt(page, 0, 8);
			await page.mouse.dblclick(word.x, word.y);

			const copied = await page.evaluate(() => {
				// Firefox copies the init's DataTransfer, so read the event's.
				const event = new ClipboardEvent("copy", {
					clipboardData: new DataTransfer(),
					bubbles: true,
					cancelable: true,
				});
				document.activeElement?.dispatchEvent(event);
				const data = event.clipboardData!;

				return [data.getData("text/plain"), data.getData("text/html")];
			});
			expect(copied[0]).toBe("world");
			expect(copied[1]).toContain("world");

			await clickAt(page, 1, 16);
			await page.evaluate(() => {
				const event = new ClipboardEvent("paste", {
					clipboardData: new DataTransfer(),
					bubbles: true,
					cancelable: true,
				});
				event.clipboardData!.setData("text/html", "<strong>bold</strong>");
				event.clipboardData!.setData("text/plain", "bold");
				document.activeElement?.dispatchEvent(event);
			});
			expect(await page.evaluate(() => editor.export("markdown"))).toContain(
				"Second paragraph**bold**",
			);

			const hello = await pointAt(page, 0, 2);
			await page.mouse.dblclick(hello.x, hello.y);
			const cut = await page.evaluate(() => {
				// Firefox copies the init's DataTransfer, so read the event's.
				const event = new ClipboardEvent("cut", {
					clipboardData: new DataTransfer(),
					bubbles: true,
					cancelable: true,
				});
				document.activeElement?.dispatchEvent(event);
				const data = event.clipboardData!;

				return data.getData("text/plain");
			});
			expect(cut).toBe("Hello");
			expect((await paragraphs(page))[0]).toBe(" world");
		});

		test("drops text where it lands", async ({ page }) => {
			await open(page, input);
			const target = await pointAt(page, 0, 5);
			await page.evaluate(({ x, y }) => {
				const data = new DataTransfer();
				data.setData("text/plain", "XYZ");
				const init = {
					clientX: x,
					clientY: y,
					dataTransfer: data,
					bubbles: true,
					cancelable: true,
				};
				editor.canvas.dispatchEvent(new DragEvent("dragover", init));
				editor.canvas.dispatchEvent(new DragEvent("drop", init));
			}, target);

			expect((await paragraphs(page))[0]).toBe("HelloXYZ world");
		});

		test("mirrors the document for assistive technology", async ({ page }) => {
			await open(page, input);
			const mirror = page.getByRole("document", { name: "Test editor" });
			await expect(mirror).toContainText("Hello world");

			await clickAt(page, 0, 5);
			await page.keyboard.type(",");
			await expect(mirror).toContainText("Hello, world");
		});
	});
}

test.describe("composition", () => {
	test("composes through the platform's input method", async ({ page, browserName }) => {
		test.skip(browserName !== "chromium", "IME needs the DevTools protocol");

		for (const input of ["auto", "textarea"] as const) {
			await open(page, input);
			await clickAt(page, 0, 11);
			const session = await page.context().newCDPSession(page);

			for (const text of ["に", "にほ", "にほん"]) {
				await session.send("Input.imeSetComposition", {
					text,
					selectionStart: text.length,
					selectionEnd: text.length,
				});
			}

			expect((await paragraphs(page))[0]).toBe("Hello worldにほん");
			await session.send("Input.insertText", { text: "日本" });
			expect((await paragraphs(page))[0]).toBe("Hello world日本");

			await page.keyboard.type("!");
			expect((await paragraphs(page))[0]).toBe("Hello world日本!");
			await session.detach();
		}
	});

	test("composes dead keys in the textarea", async ({ page }) => {
		await open(page, "textarea");
		test.skip(
			await page.evaluate(() => editor.inputMode) !== "textarea",
			"synthetic composition events reach only the textarea",
		);
		await clickAt(page, 0, 11);

		await page.evaluate(() => {
			const textarea = document.querySelector("textarea.cv-input")!;
			const compose = (type: string, data: string) =>
				textarea.dispatchEvent(
					new CompositionEvent(type, { data, bubbles: true }),
				);
			compose("compositionstart", "");
			compose("compositionupdate", "´");
		});
		expect((await paragraphs(page))[0]).toBe("Hello world´");

		await page.evaluate(() => {
			const textarea = document.querySelector("textarea.cv-input")!;
			textarea.dispatchEvent(
				new CompositionEvent("compositionend", { data: "é", bubbles: true }),
			);
		});
		expect((await paragraphs(page))[0]).toBe("Hello worldé");
	});
});

test("moves a dragged selection", async ({ page, browserName }) => {
	test.skip(browserName !== "chromium", "native drag and drop from the mouse");
	await open(page, "auto");
	await page.evaluate(() =>
		editor.select({ path: [0], offset: 0 }, { path: [0], offset: 5 })
	);
	const hello = await pointAt(page, 0, 2);

	const end = await pointAt(page, 1, 16);
	await page.mouse.move(hello.x, hello.y);
	await page.mouse.down();
	await page.mouse.move(end.x, end.y, { steps: 8 });
	await page.mouse.up();

	expect(await paragraphs(page)).toEqual([" world", "Second paragraphHello"]);
});

test.describe("performance", () => {
	/** Write this browser's numbers to `report/perf/` for the CI summary. */
	async function record(
		name: string,
		browser: string,
		numbers: Record<string, unknown>,
	): Promise<void> {
		const directory = new URL("../../report/perf/", import.meta.url);
		await mkdir(directory, { recursive: true });
		await writeFile(
			new URL(`${browser}-${platform()}-${name}.json`, directory),
			JSON.stringify(numbers, null, "\t"),
		);
		test.info().annotations.push({
			type: name,
			description: JSON.stringify(numbers),
		});
	}

	/** Start collecting Event Timing entries for slow input events. */
	async function observeSlowEvents(page: Page): Promise<void> {
		await page.evaluate((threshold) => {
			globalThis.slowEvents = [];

			if (!PerformanceObserver.supportedEntryTypes.includes("event")) {
				return;
			}

			new PerformanceObserver((list) => {
				globalThis.slowEvents.push(
					...(list.getEntries() as PerformanceEventTiming[]),
				);
			}).observe(
				{
					type: "event",
					durationThreshold: threshold,
				} as PerformanceObserverInit,
			);
		}, EVENT_TIMING_THRESHOLD_MS);
	}

	async function slowEvents(page: Page): Promise<{
		readonly supported: boolean;
		readonly count: number;
		readonly longest: number;
	}> {
		return await page.evaluate(() => ({
			supported: PerformanceObserver.supportedEntryTypes.includes("event"),
			count: globalThis.slowEvents.length,
			longest: Math.max(
				0,
				...globalThis.slowEvents.map((entry) => entry.duration),
			),
		}));
	}

	test("keystroke to paint", async ({ page, browserName }) => {
		await open(page, "auto");
		await clickAt(page, 1, 16);
		await observeSlowEvents(page);
		const text = "The quick brown fox jumps over the lazy dog. ".repeat(5)
			.slice(
				0,
				LATENCY_KEYS,
			);
		// Spaced out, so each key's Event Timing entry covers only its own paint.
		await page.keyboard.type(text, { delay: KEY_DELAY_MS });

		const latency: InputLatency = await page.evaluate(() =>
			editor.inputLatency()
		);
		const events = await slowEvents(page);
		await record("keystroke", browserName, { latency, slowEvents: events });

		expect((await paragraphs(page))[1]).toBe(`Second paragraph${text}`);
		expect(latency.samples).toBeGreaterThanOrEqual(LATENCY_KEYS);
		expect(latency.p95).toBeLessThan(KEYSTROKE_P95_BUDGET_MS);
	});

	test("a burst of 1,000 keys", async ({ page, browserName }) => {
		await open(page, "auto");
		await clickAt(page, 0, 11);
		await observeSlowEvents(page);
		const text = "lorem ipsum dolor sit amet ".repeat(40).slice(0, BURST_KEYS);
		const started = Date.now();
		await page.keyboard.type(text);
		const elapsed = Date.now() - started;

		const latency: InputLatency = await page.evaluate(() =>
			editor.inputLatency()
		);
		const events = await slowEvents(page);
		const frame = await page.evaluate(() => editor.render());
		await record("burst", browserName, {
			keys: BURST_KEYS,
			elapsedMs: elapsed,
			latency,
			slowEvents: events,
			lastFrame: frame,
		});

		expect((await paragraphs(page))[0]).toBe(`Hello world${text}`);
		expect(latency.p95).toBeLessThan(KEYSTROKE_P95_BUDGET_MS);
	});
});
