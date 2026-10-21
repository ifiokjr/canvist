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

		// Rive and the auto-loading editor stay unloaded, and the editor's
		// document is there as server-rendered HTML.
		expect(requests.filter((url) => /rive|canvist_web|\.wasm$/i.test(url)))
			.toEqual([]);
		await expect(page.locator("#pixel-editor > .cv-doc")).toContainText(
			"painted by canvist",
		);
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

	test("the editor still loads on request", async ({ page }) => {
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

	test("pressing the server-rendered document loads the editor and focuses it", async ({ page }) => {
		await page.goto("/#every-pixel");
		await expect(page.locator("[data-pixel-demo]")).toHaveAttribute(
			"data-state",
			"idle",
		);
		await page.locator("#pixel-editor").click({ position: { x: 8, y: 8 } });
		await expect(page.locator("[data-pixel-demo]")).toHaveAttribute(
			"data-state",
			"ready",
			{ timeout: 20_000 },
		);
		await page.keyboard.type("!");
		await expect(page.locator("[data-pixel-source]")).toHaveText(
			"Your keystroke to paint",
		);
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

	test("the editor takes over the server-rendered document without moving it", async ({ page }) => {
		await page.addInitScript(() => {
			const shifts: number[] = [];
			Object.assign(window, { shifts });
			new PerformanceObserver((list) => {
				for (const entry of list.getEntries()) {
					shifts.push((entry as PerformanceEntry & { value: number }).value);
				}
			}).observe({ type: "layout-shift", buffered: true });
		});
		await page.setViewportSize({ width: 1280, height: 900 });
		await page.goto("/");
		const editor = page.locator("#pixel-editor");
		// The editor's height and the next section's position, ignoring the
		// transforms that settle chips in.
		const layout = () =>
			page.evaluate(() => ({
				editor: document.querySelector<HTMLElement>("#pixel-editor")
					?.offsetHeight,
				below: document.querySelector<HTMLElement>("#everywhere")?.offsetTop,
			}));
		const before = await layout();
		await page.locator("#every-pixel").scrollIntoViewIfNeeded();
		await expect(page.locator("[data-pixel-demo]")).toHaveAttribute(
			"data-state",
			"ready",
			{ timeout: 20_000 },
		);
		await expect(editor.locator(":scope > .cv-doc")).toHaveCount(0);
		await expect(editor.locator(":scope > canvas")).toBeVisible();
		expect(await layout()).toEqual(before);
		// Starting the editor must not steal focus from the page.
		expect(await page.evaluate(() => document.activeElement === document.body))
			.toBe(true);
		expect(
			await page.evaluate(() =>
				(window as Window & { shifts?: number[] }).shifts ?? []
			),
		).toEqual([]);
	});

	test("the editor repaints in the theme the visitor picks", async ({ page }) => {
		const errors: string[] = [];
		page.on("pageerror", (error) => errors.push(error.message));
		await page.emulateMedia({ colorScheme: "light" });
		await page.goto("/");
		await page.locator("#every-pixel").scrollIntoViewIfNeeded();
		await expect(page.locator("[data-pixel-demo]")).toHaveAttribute(
			"data-state",
			"ready",
			{ timeout: 20_000 },
		);
		const corner = () =>
			page.locator("#pixel-editor canvas").evaluate((canvas) =>
				[
					...(canvas as HTMLCanvasElement).getContext("2d")!.getImageData(
						1,
						1,
						1,
						1,
					).data,
				].slice(0, 3)
			);
		expect(await corner()).toEqual([255, 255, 255]);
		await page.locator("[data-theme-toggle]").first().click();
		await expect.poll(corner).toEqual([26, 24, 23]);
		await expect(page.locator("#pixel-editor")).toHaveCSS(
			"background-color",
			"rgba(0, 0, 0, 0)",
		);
		await expect(page.locator(".sheet__paper")).toHaveCSS(
			"background-color",
			"rgb(26, 24, 23)",
		);
		// Toggling twice in a row swaps editors one at a time.
		const toggle = page.locator("[data-theme-toggle]").first();
		await toggle.click();
		await toggle.click();
		await expect.poll(corner).toEqual([26, 24, 23]);
		expect(errors).toEqual([]);
	});

	test("the editor loads near the viewport and types by itself", async ({ page }) => {
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
