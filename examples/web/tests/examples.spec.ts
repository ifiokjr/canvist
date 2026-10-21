/**
 * Every example loads, edits, and keeps its toolbar in step; the
 * server-rendered pages hydrate without moving anything.
 *
 * @module
 */
import type { Editor, StarterKit } from "@canvist/web";
import { expect, type Page, test } from "@playwright/test";

/** How far, in CSS px, content may move when the editor hydrates: one engine layout unit. */
const LAYOUT_UNIT_TOLERANCE = 1 / 32;

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

for (const example of ["ssr", "ssr-react"] as const) {
	test(`${example}: the editor replaces server-rendered HTML without moving it`, async ({ page }) => {
		const errors: string[] = [];
		page.on("console", (message) => {
			if (message.type() === "error") {
				errors.push(message.text());
			}
		});
		await page.addInitScript(() => {
			globalThis.shifts = 0;

			if (PerformanceObserver.supportedEntryTypes.includes("layout-shift")) {
				new PerformanceObserver((list) => {
					for (const entry of list.getEntries()) {
						globalThis.shifts +=
							(entry as PerformanceEntry & { value: number }).value;
					}
				}).observe({ type: "layout-shift", buffered: true });
			}

			// Before the editor's module has loaded: the server's HTML is showing.
			document.addEventListener("DOMContentLoaded", () => {
				globalThis.served = {
					html: document.querySelector(".editor > .cv-doc") !== null,
					after: document.querySelector("#after")!.getBoundingClientRect().top,
				};
			});
		});
		await page.goto(`/${example}.html`);
		await waitForEditor(page);

		const served = await page.evaluate(() => globalThis.served);
		expect(served?.html).toBe(true);
		await expect(page.locator(".editor > .cv-doc")).toHaveCount(0);
		await expect(page.locator(".editor > canvas")).toBeVisible();
		// Within one layout unit: WebKit rounds boxes to 1/64 px and Firefox to
		// 1/60 px, while the canvas's height sums unrounded line boxes. Nothing
		// moves by a device pixel; the parity tests hold lines to the same 1/32.
		const after = await page.evaluate(() =>
			document.querySelector("#after")!.getBoundingClientRect().top
		);
		expect(Math.abs(after - (served?.after ?? Number.NaN)))
			.toBeLessThanOrEqual(LAYOUT_UNIT_TOLERANCE);
		expect(await page.evaluate(() => globalThis.shifts)).toBe(0);

		// The document came from the server's HTML, and React hydrated it
		// without a mismatch.
		expect(await page.evaluate(() => globalThis.editor?.export("markdown")))
			.toContain("# Rendered on the server");
		expect(errors).toEqual([]);
	});
}
