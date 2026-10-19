import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import { PAGES, WIDTHS } from "./pages.ts";

test.describe("responsive", () => {
	test.use({ reducedMotion: "reduce" });

	for (const width of WIDTHS) {
		for (const entry of PAGES) {
			test(`${entry.path} fits ${width}px with no horizontal scroll`, async ({ page }) => {
				await page.setViewportSize({ width, height: 900 });
				await page.goto(entry.path);
				const overflow = await page.evaluate(() =>
					document.documentElement.scrollWidth - innerWidth
				);
				expect(overflow).toBeLessThanOrEqual(0);

				// Nothing important is wider than the viewport, even when clipped.
				const offenders = await page.evaluate(() =>
					[...document.querySelectorAll<HTMLElement>(
						"h1, h2, h3, p, a.btn, button, input, pre, table",
					)]
						.filter((element) => {
							const box = element.getBoundingClientRect();
							if (box.width === 0) return false;
							// Code and tables scroll inside their own wrappers.
							if (element.closest("pre, .docs-table")) return false;
							return box.left < -1 || box.right > innerWidth + 1;
						})
						.map((element) =>
							`${element.tagName.toLowerCase()}: ${
								element.textContent?.trim().slice(0, 40)
							}`
						)
				);
				expect(offenders).toEqual([]);
				await expect(page.getByRole("navigation", { name: "Main" }))
					.toBeVisible();
			});
		}
	}

	test("tap targets are at least 24px at 320px", async ({ page }) => {
		await page.setViewportSize({ width: 320, height: 800 });
		await page.goto("/");
		const small = await page.evaluate(() =>
			[...document.querySelectorAll<HTMLElement>(
				"header a, header button, .btn, .hosts button, .switch",
			)]
				.filter((element) => {
					const box = element.getBoundingClientRect();
					return box.width > 0 && (box.width < 24 || box.height < 24);
				})
				.map((element) =>
					element.textContent?.trim() || element.getAttribute("aria-label")
				)
		);
		expect(small).toEqual([]);
	});
});

test.describe("accessibility", () => {
	test.use({ reducedMotion: "reduce" });

	for (const scheme of ["light", "dark"] as const) {
		for (const entry of PAGES) {
			test(`${entry.path} has no serious axe violations (${scheme})`, async ({ page }) => {
				await page.emulateMedia({
					colorScheme: scheme,
					reducedMotion: "reduce",
				});
				await page.goto(entry.path);
				const results = await new AxeBuilder({ page }).withTags([
					"wcag2a",
					"wcag2aa",
					"wcag21aa",
					"wcag22aa",
				]).analyze();
				const serious = results.violations
					.filter((violation) =>
						violation.impact === "serious" || violation.impact === "critical"
					)
					.map((violation) =>
						`${violation.id}: ${
							violation.nodes.map((node) => node.target.join(" ")).join(", ")
						}`
					);
				expect(serious).toEqual([]);
			});
		}
	}
});
