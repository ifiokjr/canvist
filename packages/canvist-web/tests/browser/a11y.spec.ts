/**
 * Accessibility: an axe audit of the editor in each state, and smoke tests
 * of what a screen reader announces, through a virtual screen reader that
 * reads the accessibility tree as NVDA and VoiceOver do.
 *
 * @module
 */
import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import type { Editor, StarterKit } from "../../src/mod.ts";

declare global {
	var editor: Editor<StarterKit>;
	var speak: (steps: number) => Promise<string[]>;
}

/** The WCAG levels the audit holds the editor to. */
const WCAG = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];

const DOCUMENT =
	"# Notes\n\nA paragraph with **bold** text.\n\n- First\n- Second";

async function open(page: Page, params: Record<string, string>): Promise<void> {
	await page.goto(`/editing.html?${new URLSearchParams(params)}`);
	await page.waitForFunction(() => globalThis.editor !== undefined);
}

async function audit(page: Page): Promise<void> {
	const results = await new AxeBuilder({ page }).withTags(WCAG).analyze();
	expect(
		results.violations.map((violation) => ({
			id: violation.id,
			nodes: violation.nodes.map((node) => node.html),
		})),
	).toEqual([]);
}

test.describe("axe", () => {
	for (const input of ["auto", "textarea"] as const) {
		test(`an editable editor, ${input} input`, async ({ page }) => {
			await open(page, { input, markdown: DOCUMENT });
			await page.evaluate(() => editor.focus());
			await audit(page);
		});
	}

	test("a read-only editor", async ({ page }) => {
		await open(page, { markdown: DOCUMENT, editable: "false" });
		await audit(page);
	});

	test("an empty editor with a placeholder", async ({ page }) => {
		await open(page, { markdown: "", placeholder: "Write something" });
		await audit(page);
	});

	test("an editor with its bubble menu showing", async ({ page }) => {
		await open(page, { markdown: DOCUMENT, menus: "" });
		await page.evaluate(() => {
			editor.select({ path: [1], offset: 0 }, { path: [1], offset: 5 });
			editor.focus();
		});
		await expect(page.getByRole("toolbar", { name: "Formatting" }))
			.toBeVisible();
		await audit(page);
	});
});

test.describe("screen reader", () => {
	async function listen(
		page: Page,
		params: Record<string, string>,
		steps: number,
	): Promise<string[]> {
		await page.goto(`/screen-reader.html?${new URLSearchParams(params)}`);
		await page.waitForFunction(() => globalThis.speak !== undefined);
		// The mirror refreshes when the browser is idle.
		await expect(page.getByRole("document", { name: "Notes" })).toContainText(
			params["markdown"]?.split("\n")[0]?.replace(/^#+ /, "") ?? "",
		);

		return page.evaluate((steps) => globalThis.speak(steps), steps);
	}

	test("announces the editor, then reads the document with its structure", async ({ page }) => {
		const spoken = await listen(page, { markdown: DOCUMENT }, 16);

		expect(spoken).toContain("textbox, Notes");
		const reading = spoken.slice(spoken.indexOf("textbox, Notes") + 1);
		expect(reading).toEqual(expect.arrayContaining([
			"heading, Notes, level 1",
			"A paragraph with",
			"bold",
			"listitem, level 1, position 1, set size 2",
		]));
		expect(reading.indexOf("heading, Notes, level 1")).toBeLessThan(
			reading.indexOf("A paragraph with"),
		);
	});

	test("announces read-only editors and placeholders", async ({ page }) => {
		const readOnly = await listen(
			page,
			{ markdown: DOCUMENT, editable: "false" },
			4,
		);
		expect(readOnly.find((phrase) => phrase.startsWith("textbox")))
			.toMatch(/read ?only/);

		const empty = await listen(page, { placeholder: "Write something" }, 4);
		expect(empty).toContain("textbox, Notes, placeholder Write something");
	});

	test("reads edits once the mirror catches up", async ({ page }) => {
		await listen(page, { markdown: DOCUMENT }, 0);
		await page.evaluate(() => {
			editor.select({ path: [1], offset: 0 });
			editor.insertText("Edited: ");
		});
		await expect(page.getByRole("document", { name: "Notes" })).toContainText(
			"Edited: A paragraph",
		);

		const spoken = await page.evaluate(() => globalThis.speak(10));
		expect(spoken).toContain("Edited: A paragraph with");
	});
});
