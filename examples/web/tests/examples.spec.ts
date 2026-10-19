/**
 * Every example loads, edits, and keeps its toolbar in step; the
 * server-rendered page hydrates without moving anything.
 *
 * @module
 */
import type { Editor, StarterKit } from "@canvist/web";
import { expect, type Page, test } from "@playwright/test";

declare global {
	var editor: Editor<StarterKit> | null | undefined;
	var shifts: number;
	var served: { html: boolean; after: number } | undefined;
}

async function waitForEditor(page: Page): Promise<void> {
	await page.waitForFunction(() =>
		globalThis.editor !== null && globalThis.editor !== undefined &&
		!globalThis.editor.hydrating
	);
}

for (const example of ["vanilla", "react", "vue"] as const) {
	test(`${example}: the toolbar follows the selection`, async ({ page }) => {
		await page.goto(`/${example}.html`);
		await waitForEditor(page);
		const button = page.getByRole("button", { name: "Bold" });
		await expect(button).toHaveAttribute("aria-pressed", "false");

		// Select "Every" in the first paragraph and make it bold.
		await page.evaluate(() =>
			globalThis.editor?.select({ path: [1], offset: 0 }, {
				path: [1],
				offset: 5,
			})
		);
		await button.click();
		await expect(button).toHaveAttribute("aria-pressed", "true");
		expect(await page.evaluate(() => globalThis.editor?.export("markdown")))
			.toContain("**Every**");

		// The editor kept focus: typing after the word goes into it.
		await page.keyboard.press("ArrowRight");
		await page.keyboard.type("!");
		expect(await page.evaluate(() => globalThis.editor?.export("markdown")))
			.toMatch(/\*\*Every!?\*\*!? pixel/);
	});
}

test("ssr: the editor replaces server-rendered HTML without moving it", async ({ page }) => {
	await page.addInitScript(() => {
		globalThis.shifts = 0;

		if (PerformanceObserver.supportedEntryTypes.includes("layout-shift")) {
			new PerformanceObserver((list) => {
				for (const entry of list.getEntries()) {
					globalThis.shifts += (entry as PerformanceEntry & { value: number })
						.value;
				}
			}).observe({ type: "layout-shift", buffered: true });
		}

		// Before the editor's module has loaded: the server's HTML is showing.
		document.addEventListener("DOMContentLoaded", () => {
			globalThis.served = {
				html: document.querySelector("#editor > .cv-doc") !== null,
				after: document.querySelector("#after")!.getBoundingClientRect().top,
			};
		});
	});
	await page.goto("/ssr.html");
	await waitForEditor(page);

	const served = await page.evaluate(() => globalThis.served);
	expect(served?.html).toBe(true);
	await expect(page.locator("#editor > .cv-doc")).toHaveCount(0);
	await expect(page.locator("#editor > canvas")).toBeVisible();
	expect(
		await page.evaluate(() =>
			document.querySelector("#after")!.getBoundingClientRect().top
		),
	).toBe(served?.after);
	expect(await page.evaluate(() => globalThis.shifts)).toBe(0);

	// The document came from the server's HTML.
	expect(await page.evaluate(() => globalThis.editor?.export("markdown")))
		.toContain("# Rendered on the server");
});
