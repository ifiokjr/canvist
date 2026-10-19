import { expect, test } from "@playwright/test";

test.describe("reduced motion", () => {
	test.use({ reducedMotion: "reduce" });

	test("the landing page renders its final state without loading motion code", async ({ page }) => {
		const scripts: string[] = [];
		page.on("request", (request) => {
			if (
				request.resourceType() === "script" ||
				request.url().endsWith(".wasm") || request.url().endsWith(".riv")
			) {
				scripts.push(request.url());
			}
		});
		await page.goto("/");

		// The lockup is shown as is: no Rive, no painted stroke, no waiting caret.
		const stage = page.locator("[data-hero-stage]");
		await expect(stage).toHaveAttribute("data-state", "static");
		await expect(page.locator("html")).not.toHaveClass(/motion-ok/);
		const wordmark = page.locator("[data-hero-lockup] path").first();
		expect(
			await wordmark.evaluate((path) => getComputedStyle(path).animationName),
		).toBe("none");

		await page.evaluate(() => scrollTo(0, document.body.scrollHeight));
		await page.waitForLoadState("networkidle");

		// GSAP, Rive and the auto-loading v1 engine all stay unloaded.
		expect(
			scripts.filter((url) =>
				/gsap|ScrollTrigger|rive|canvist_wasm/i.test(url)
			),
		).toEqual([]);

		// Chips sit flat; nothing is mid-animation.
		const transforms = await page.locator("[data-fan-chip]").evaluateAll((
			chips,
		) => chips.map((chip) => getComputedStyle(chip).transform));
		for (const transform of transforms) {
			expect(["none", "matrix(1, 0, 0, 1, 0, 0)"]).toContain(transform);
		}

		// The collaboration demo shows the merged result straight away.
		await expect(page.locator("[data-together-log]")).toContainText(
			"All four edits merged",
		);
		// The typed demo starts valid and only changes when asked.
		await expect(page.locator("[data-typed]")).toHaveAttribute(
			"data-highlight",
			"on",
		);
		await page.getByRole("switch", { name: /Register Highlight/ }).click();
		await expect(page.locator("[data-typed]")).toHaveAttribute(
			"data-highlight",
			"off",
		);
		await expect(page.locator("[data-typed-problems]")).toHaveText("1 problem");
	});

	test("the v1 engine still loads on request", async ({ page }) => {
		await page.goto("/#every-pixel");
		await page.getByRole("button", { name: "Load the engine and type" })
			.click();
		await expect(page.locator("[data-pixel-demo]")).toHaveAttribute(
			"data-state",
			"ready",
			{ timeout: 20_000 },
		);
		await page.keyboard.type("hi");
		await expect(page.locator("[data-pixel-count]")).toHaveText("2");
	});
});

test.describe("full motion", () => {
	test.use({ reducedMotion: "no-preference" });

	test("the hero paints the wordmark and the deck fans open", async ({ page }) => {
		await page.setViewportSize({ width: 1440, height: 900 });
		await page.goto("/");
		await expect(page.locator("html")).toHaveClass(/motion-ok/);
		// Rive takes over, or the CSS stroke does if Rive misses its deadline.
		await expect(page.locator("[data-hero-stage]")).toHaveAttribute(
			"data-state",
			/rive|css/,
			{ timeout: 10_000 },
		);

		const chip = page.locator("#typed");
		await chip.scrollIntoViewIfNeeded();
		await expect.poll(() =>
			chip.evaluate((element) => getComputedStyle(element).transform)
		).toMatch(/matrix/);
	});

	test("the v1 engine loads near the viewport and types by itself", async ({ page }) => {
		await page.goto("/");
		await page.locator("#every-pixel").scrollIntoViewIfNeeded();
		await expect(page.locator("[data-pixel-demo]")).toHaveAttribute(
			"data-state",
			"ready",
			{ timeout: 20_000 },
		);
		await expect.poll(async () =>
			Number(await page.locator("[data-pixel-count]").textContent())
		).toBeGreaterThan(3);
	});
});
