/**
 * Read-only editors, placeholders, and menus that follow the selection,
 * in every engine and through both input paths.
 *
 * @module
 */
import { expect, type Page, test } from "@playwright/test";
import type { Editor, StarterKit } from "../../src/mod.ts";

declare global {
	var editor: Editor<StarterKit>;
	var modifier: "Meta" | "Control";
}

interface Open {
	readonly input?: "auto" | "textarea";
	readonly markdown?: string;
	readonly editable?: boolean;
	readonly placeholder?: string;
	readonly menus?: boolean;
}

async function open(page: Page, options: Open = {}): Promise<void> {
	const params = new URLSearchParams({
		input: options.input ?? "auto",
		markdown: options.markdown ?? "Hello world\n\nSecond paragraph",
	});

	if (options.editable === false) {
		params.set("editable", "false");
	}

	if (options.placeholder !== undefined) {
		params.set("placeholder", options.placeholder);
	}

	if (options.menus === true) {
		params.set("menus", "");
	}

	await page.goto(`/editing.html?${params}`);
	await page.waitForFunction(() => globalThis.editor !== undefined);
}

function markdown(page: Page): Promise<string> {
	return page.evaluate(() => editor.export("markdown").trim());
}

/** The viewport point of UTF-16 `offset` in top-level block `block`, mid-line. */
function pointAt(
	page: Page,
	block: number,
	offset: number,
): Promise<{ x: number; y: number }> {
	return page.evaluate(([block, offset]) => {
		editor.select({ path: [block], offset });
		const caret = editor.toClientRect(editor.caretRect()!);
		editor.select({ path: [0], offset: 0 });

		return { x: caret.x + caret.width / 2, y: caret.y + caret.height / 2 };
	}, [block, offset] as const);
}

async function pressMod(page: Page, key: string): Promise<void> {
	const modifier = await page.evaluate(() => globalThis.modifier);
	await page.keyboard.press(`${modifier}+${key}`);
}

function dispatchClipboard(
	page: Page,
	type: "copy" | "cut" | "paste",
	text = "",
): Promise<string> {
	return page.evaluate(([type, text]) => {
		const event = new ClipboardEvent(type, {
			clipboardData: new DataTransfer(),
			bubbles: true,
			cancelable: true,
		});

		if (type === "paste") {
			event.clipboardData!.setData("text/plain", text);
		}

		document.activeElement?.dispatchEvent(event);
		// Firefox copies the init's DataTransfer, so read the event's.
		return event.clipboardData!.getData("text/plain");
	}, [type, text] as const);
}

for (const input of ["auto", "textarea"] as const) {
	test.describe(`read-only, ${input} input`, () => {
		test.skip(
			({ browserName }) => input === "textarea" && browserName !== "chromium",
			"only Chromium has EditContext, so elsewhere auto is the textarea",
		);

		test("selects and copies but changes nothing", async ({ page }) => {
			await open(page, { input, editable: false });
			const original = await markdown(page);
			const textbox = page.getByRole("textbox", { name: "Test editor" });
			await expect(textbox).toHaveAttribute("aria-readonly", "true");

			const start = await pointAt(page, 0, 0);
			await page.mouse.click(start.x, start.y);
			await page.keyboard.type("typed");
			await page.keyboard.press("Enter");
			await page.keyboard.press("Backspace");
			await pressMod(page, "b");
			expect(await markdown(page)).toBe(original);

			// Navigation keys still select, and copying works.
			await page.keyboard.press("Shift+End");
			expect(await page.evaluate(() => editor.selection?.head.offset)).toBe(
				11,
			);
			expect(await dispatchClipboard(page, "copy")).toBe("Hello world");
			expect(await dispatchClipboard(page, "cut")).toBe("Hello world");
			await dispatchClipboard(page, "paste", "pasted");
			expect(await markdown(page)).toBe(original);

			const dropped = await page.evaluate(() => {
				const data = new DataTransfer();
				data.setData("text/plain", "XYZ");
				const drop = new DragEvent("dragover", {
					dataTransfer: data,
					bubbles: true,
					cancelable: true,
				});
				editor.canvas.dispatchEvent(drop);
				return drop.defaultPrevented;
			});
			expect(dropped, "a read-only editor refuses drops").toBe(false);
		});

		test("commands still change it, and setEditable turns typing back on", async ({ page }) => {
			await open(page, { input, editable: false });
			await page.evaluate(() => {
				editor.select({ path: [0], offset: 0 }, { path: [0], offset: 5 });
				editor.commands.toggleBold();
			});
			expect(await markdown(page)).toContain("**Hello**");

			const changes = await page.evaluate(() => {
				let count = 0;
				editor.subscribe(() => count++);
				editor.setEditable(true);
				return count;
			});
			expect(changes, "subscribers hear about editability").toBe(1);
			await expect(page.getByRole("textbox", { name: "Test editor" }))
				.toHaveAttribute("aria-readonly", "false");

			const end = await pointAt(page, 1, 16);
			await page.mouse.click(end.x, end.y);
			await page.keyboard.type("!");
			expect(await markdown(page)).toContain("Second paragraph!");
		});
	});

	test.describe(`placeholder, ${input} input`, () => {
		test.skip(
			({ browserName }) => input === "textarea" && browserName !== "chromium",
			"only Chromium has EditContext, so elsewhere auto is the textarea",
		);

		test("shows while the document is empty", async ({ page }) => {
			await open(page, { input, markdown: "", placeholder: "Write…" });
			const placeholder = page.locator(".cv-placeholder");
			await expect(placeholder).toBeVisible();
			await expect(placeholder).toHaveText("Write…");
			await expect(page.getByRole("textbox", { name: "Test editor" }))
				.toHaveAttribute("aria-placeholder", "Write…");
			expect(await page.evaluate(() => editor.isEmpty)).toBe(true);

			// It starts where typed text will: at the caret, on the first line.
			const [box, caret] = await page.evaluate(() => {
				const box = document.querySelector(".cv-placeholder")!
					.getBoundingClientRect();
				const caret = editor.toClientRect(editor.caretRect()!);
				return [box.toJSON(), caret.toJSON()] as const;
			});
			expect(Math.abs(box.x - (caret.x + caret.width / 2))).toBeLessThan(0.5);
			expect(Math.abs(box.y - caret.y)).toBeLessThan(0.5);
			expect(Math.abs(box.height - caret.height)).toBeLessThan(0.5);

			await page.evaluate(() => editor.focus());
			await page.keyboard.type("Hi");
			await expect(placeholder).toBeHidden();
			expect(await page.evaluate(() => editor.isEmpty)).toBe(false);

			await page.keyboard.press("Backspace");
			await page.keyboard.press("Backspace");
			await expect(placeholder).toBeVisible();

			await page.evaluate(() => editor.setPlaceholder(""));
			await expect(placeholder).toBeHidden();
			await expect(page.getByRole("textbox", { name: "Test editor" })).not
				.toHaveAttribute("aria-placeholder");
		});
	});
}

test.describe("menus", () => {
	test("the bubble menu sits over selected text and acts on it", async ({ page }) => {
		await open(page, { menus: true });
		const bubble = page.getByRole("toolbar", { name: "Formatting" });
		await expect(bubble).toBeHidden();

		const word = await pointAt(page, 0, 8);
		await page.mouse.dblclick(word.x, word.y);
		await expect(bubble).toBeVisible();

		const [menu, selection] = await page.evaluate(() =>
			[
				document.querySelector("#bubble")!.getBoundingClientRect().toJSON(),
				editor.toClientRect(editor.selectionRect()!).toJSON(),
			] as const
		);
		expect(menu.bottom).toBeLessThanOrEqual(selection.top);
		expect(
			Math.abs(menu.x + menu.width / 2 - (selection.x + selection.width / 2)),
		)
			.toBeLessThan(1);
		await expect(bubble).toHaveAttribute("data-placement", "top");

		await bubble.getByRole("button", { name: "Bold" }).click();
		expect(await markdown(page)).toContain("Hello **world**");
		expect(await page.evaluate(() => editor.focused), "the editor kept focus")
			.toBe(true);

		await page.keyboard.press("ArrowRight");
		await expect(bubble).toBeHidden();
	});

	test("the bubble menu flips below a selection at the top of the viewport", async ({ page }) => {
		await open(page, { menus: true });
		await page.evaluate(() => {
			document.querySelector<HTMLElement>("#host")!.style.marginTop = "4px";
		});
		const word = await pointAt(page, 0, 8);
		await page.mouse.dblclick(word.x, word.y);
		const bubble = page.getByRole("toolbar", { name: "Formatting" });

		await expect(bubble).toHaveAttribute("data-placement", "bottom");
	});

	test("the floating menu sits beside an empty line", async ({ page }) => {
		await open(page, { menus: true, markdown: "Hello\n\n" });
		const floating = page.getByRole("toolbar", { name: "Insert" });
		const end = await pointAt(page, 0, 5);
		await page.mouse.click(end.x, end.y);
		await expect(floating).toBeHidden();

		await page.keyboard.press("Enter");
		await expect(floating).toBeVisible();
		const [menu, caret] = await page.evaluate(() =>
			[
				document.querySelector("#floating")!.getBoundingClientRect().toJSON(),
				editor.toClientRect(editor.caretRect()!).toJSON(),
			] as const
		);
		expect(menu.right).toBeLessThanOrEqual(caret.left);

		await floating.getByRole("button", { name: "Heading" }).click();
		expect(await page.evaluate(() => editor.query.headingLevel())).toBe(2);

		await page.keyboard.type("Title");
		await expect(floating).toBeHidden();
	});

	test("menus stay hidden in a read-only editor", async ({ page }) => {
		await open(page, { menus: true, editable: false });
		const word = await pointAt(page, 0, 8);
		await page.mouse.dblclick(word.x, word.y);

		await expect(page.getByRole("toolbar", { name: "Formatting" }))
			.toBeHidden();
	});
});
