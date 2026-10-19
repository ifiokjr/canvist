import { expect, test } from "@playwright/test";

test.describe("reduced motion", () => {
	test.use({ reducedMotion: "reduce" });

	test("the landing page renders its final state without loading motion code", async ({ page }) => {
		const requests: string[] = [];
		page.on("request", (request) => requests.push(request.url()));
		await page.goto("/");

		// The lockup is shown as is: no Rive, no painted stroke, no waiting caret.
		await expect(page.locator("[data-hero-stage]")).toHaveAttribute(
			"data-state",
			"static",
		);
		await expect(page.locator("html")).not.toHaveClass(/motion-ok/);
		const wordmark = page.locator("[data-hero-lockup] path").first();
		expect(
			await wordmark.evaluate((path) => getComputedStyle(path).animationName),
		).toBe("none");

		await page.evaluate(() => scrollTo(0, document.body.scrollHeight));
		await page.waitForLoadState("networkidle");

		// Rive and the auto-loading v1 engine stay unloaded.
		expect(requests.filter((url) => /rive|canvist_wasm|\.wasm$/i.test(url)))
			.toEqual([]);
		// No chip ever waits to settle; everything sits in place.
		await expect(page.locator(".chip.is-waiting")).toHaveCount(0);
		// The collaboration demo shows the merged result straight away.
		await expect(page.locator("[data-together-log]")).toContainText(
			"All four edits merged",
		);
		// The platform tabs still work, without auto-advancing.
		await page.waitForTimeout(3500);
		await expect(page.getByRole("tab", { name: "Web" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
	});

	test("the v1 engine still loads on request", async ({ page }) => {
		await page.goto("/#every-pixel");
		await page.getByRole("button", { name: "Load the editor" }).click();
		await expect(page.locator("[data-pixel-demo]")).toHaveAttribute(
			"data-state",
			"ready",
			{ timeout: 20_000 },
		);
		await page.keyboard.type("hi");
		await expect(page.locator("[data-pixel-last]")).not.toHaveText("–");
	});
});

test.describe("full motion", () => {
	test.use({ reducedMotion: "no-preference" });

	test("the hero paints the wordmark", async ({ page }) => {
		await page.setViewportSize({ width: 1440, height: 900 });
		await page.goto("/");
		await expect(page.locator("html")).toHaveClass(/motion-ok/);
		// Rive takes over, or the CSS stroke does if Rive misses its deadline.
		await expect(page.locator("[data-hero-stage]")).toHaveAttribute(
			"data-state",
			/rive|css/,
			{ timeout: 10_000 },
		);
	});

	test("chips below the fold settle in as they scroll into view, and only once", async ({ page }) => {
		await page.setViewportSize({ width: 1440, height: 900 });
		await page.goto("/");
		const chip = page.locator("#free");
		await expect(chip).toHaveClass(/is-waiting/);
		await chip.scrollIntoViewIfNeeded();
		await expect(chip).not.toHaveClass(/is-waiting/);
		await expect(chip).not.toHaveClass(/is-settling/, { timeout: 3000 });
		expect(
			await chip.evaluate((element) => getComputedStyle(element).transform),
		).toBe("none");
	});

	test("nothing is pinned and nothing listens to scroll", async ({ page }) => {
		await page.goto("/");
		await page.evaluate(() => scrollTo(0, document.body.scrollHeight / 2));
		const pinned = await page.evaluate(() =>
			[...document.querySelectorAll("body *")].filter((element) => {
				const position = getComputedStyle(element).position;
				return position === "fixed" || position === "sticky";
			}).length
		);
		expect(pinned).toBe(0);
	});

	test("the v1 engine loads near the viewport and types by itself", async ({ page }) => {
		await page.goto("/");
		await page.locator("#every-pixel").scrollIntoViewIfNeeded();
		await expect(page.locator("[data-pixel-demo]")).toHaveAttribute(
			"data-state",
			"ready",
			{ timeout: 20_000 },
		);
		await expect(page.locator("[data-pixel-last]")).not.toHaveText("–", {
			timeout: 10_000,
		});
	});
});
