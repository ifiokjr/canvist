/**
 * Input sequences particular to one platform, replayed on the textarea
 * path every engine without EditContext uses: Safari's composition events
 * and its Enter that commits a composition, Android keyboards that send
 * every key as `Unidentified` and say what they mean in `beforeinput`, and
 * touch on a phone-sized screen.
 *
 * @module
 */
import { devices, expect, type Page, test } from "@playwright/test";
import type { Editor, StarterKit } from "../../src/mod.ts";

declare global {
	var editor: Editor<StarterKit>;
}

/** An input event the platform sends, as the page should replay it. */
type Event =
	| { readonly type: "keydown"; readonly key: string; readonly keyCode: number }
	| {
		readonly type: "compositionstart" | "compositionupdate" | "compositionend";
		readonly data: string;
	}
	| {
		readonly type: "beforeinput";
		readonly inputType: string;
		readonly data: string | null;
	};

/** The key code browsers give every key an input method handles. */
const IME_KEY_CODE = 229;

async function open(page: Page, markdown = "Hello world"): Promise<void> {
	const params = new URLSearchParams({ input: "textarea", markdown });
	await page.goto(`/editing.html?${params}`);
	await page.waitForFunction(() => globalThis.editor !== undefined);
	// The caret at the end of the first paragraph.
	await page.evaluate(() => {
		editor.select({ path: [0], offset: 0 });
		editor.select({ path: [0], offset: editor.blockText()?.length ?? 0 });
		editor.focus();
	});
}

/** Dispatch `events` to the focused textarea, as the platform would. */
function replay(page: Page, events: readonly Event[]): Promise<void> {
	return page.evaluate((events) => {
		const textarea = document.querySelector("textarea.cv-input")!;

		for (const event of events) {
			switch (event.type) {
				case "keydown":
					textarea.dispatchEvent(
						new KeyboardEvent("keydown", {
							key: event.key,
							keyCode: event.keyCode,
							bubbles: true,
							cancelable: true,
						}),
					);
					break;
				case "beforeinput":
					textarea.dispatchEvent(
						new InputEvent("beforeinput", {
							inputType: event.inputType,
							data: event.data,
							bubbles: true,
							cancelable: true,
						}),
					);
					break;
				default:
					textarea.dispatchEvent(
						new CompositionEvent(event.type, {
							data: event.data,
							bubbles: true,
						}),
					);
			}
		}
	}, events);
}

function paragraphs(page: Page): Promise<string[]> {
	return page.evaluate(() => editor.export("text").split("\n"));
}

const compose = (texts: readonly string[]): Event[] => [
	{ type: "keydown", key: "Process", keyCode: IME_KEY_CODE },
	{ type: "compositionstart", data: "" },
	...texts.flatMap((data): Event[] => [
		{ type: "compositionupdate", data },
		{ type: "beforeinput", inputType: "insertCompositionText", data },
	]),
];

test.describe("Safari", () => {
	test("inserts composed text once, whichever side of compositionend the final input lands", async ({ page }) => {
		await open(page);
		const before: Event[] = [
			...compose(["に", "日本"]),
			{ type: "beforeinput", inputType: "deleteCompositionText", data: null },
			{ type: "beforeinput", inputType: "insertFromComposition", data: "日本" },
			{ type: "compositionend", data: "日本" },
		];
		await replay(page, before);
		expect(await paragraphs(page)).toEqual(["Hello world日本"]);

		const after: Event[] = [
			...compose(["ご"]),
			{ type: "compositionend", data: "語" },
			{ type: "beforeinput", inputType: "insertFromComposition", data: "語" },
		];
		await replay(page, after);
		expect(await paragraphs(page)).toEqual(["Hello world日本語"]);
	});

	test("the Enter that commits a composition does not split the paragraph", async ({ page }) => {
		await open(page);
		await replay(page, [
			...compose(["かな"]),
			{ type: "compositionend", data: "仮名" },
			// Safari sends the committing Enter afterwards, as key code 229.
			{ type: "keydown", key: "Enter", keyCode: IME_KEY_CODE },
		]);

		expect(await paragraphs(page)).toEqual(["Hello world仮名"]);
	});
});

test.describe("Android", () => {
	const unidentified: Event = {
		type: "keydown",
		key: "Unidentified",
		keyCode: IME_KEY_CODE,
	};

	test("types, deletes, and splits through beforeinput", async ({ page }) => {
		await open(page);
		await replay(page, [
			unidentified,
			{ type: "beforeinput", inputType: "insertText", data: "!" },
			unidentified,
			{ type: "beforeinput", inputType: "deleteContentBackward", data: null },
			unidentified,
			{ type: "beforeinput", inputType: "deleteContentBackward", data: null },
			unidentified,
			{ type: "beforeinput", inputType: "insertParagraph", data: null },
			unidentified,
			{ type: "beforeinput", inputType: "insertText", data: "x" },
		]);

		expect(await paragraphs(page)).toEqual(["Hello worl", "x"]);
	});

	test("composes each word, as Gboard does", async ({ page }) => {
		await open(page, "");
		await replay(page, [
			...compose(["h", "he", "hel", "help"]),
			// Correcting the word inside the composition.
			{ type: "compositionupdate", data: "hello" },
			{ type: "compositionend", data: "hello" },
			unidentified,
			{ type: "beforeinput", inputType: "insertText", data: " " },
			...compose(["w", "wo"]),
			{ type: "compositionend", data: "world" },
		]);

		expect(await paragraphs(page)).toEqual(["hello world"]);
	});

	test("undo and redo arrive as history input", async ({ page }) => {
		await open(page);
		await replay(page, [
			{ type: "beforeinput", inputType: "insertText", data: "!" },
			{ type: "beforeinput", inputType: "historyUndo", data: null },
		]);
		expect(await paragraphs(page)).toEqual(["Hello world"]);

		await replay(page, [
			{ type: "beforeinput", inputType: "historyRedo", data: null },
		]);
		expect(await paragraphs(page)).toEqual(["Hello world!"]);
	});
});

test.describe("on a phone", () => {
	test.use({
		viewport: devices["Pixel 7"].viewport,
		hasTouch: true,
		isMobile: true,
	});
	test.skip(
		({ browserName }) => browserName === "firefox",
		"Firefox has no mobile emulation",
	);

	test("a tap places the caret and focuses the text field", async ({ page }) => {
		await open(page);
		await page.evaluate(() =>
			document.activeElement instanceof HTMLElement &&
			document.activeElement.blur()
		);
		const point = await page.evaluate(() => {
			editor.select({ path: [0], offset: 5 });
			const caret = editor.toClientRect(editor.caretRect()!);
			editor.select({ path: [0], offset: 0 });
			return { x: caret.x + caret.width / 2, y: caret.y + caret.height / 2 };
		});
		await page.touchscreen.tap(point.x, point.y);

		expect(await page.evaluate(() => editor.selection?.head.offset)).toBe(5);
		expect(
			await page.evaluate(() => document.activeElement?.className),
		).toBe("cv-input");
		expect(await page.evaluate(() => editor.focused)).toBe(true);
	});
});
