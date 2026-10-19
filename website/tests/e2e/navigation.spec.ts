import { expect, test } from "@playwright/test";
import { PAGES, SECTIONS } from "./pages.ts";

test.describe("navigation", () => {
	for (const page of PAGES) {
		test(`${page.path} loads without errors`, async ({ page: browser }) => {
			const errors: string[] = [];
			browser.on("pageerror", (error) => errors.push(error.message));
			browser.on("console", (message) => {
				if (message.type() === "error") errors.push(message.text());
			});
			const response = await browser.goto(page.path);
			expect(response?.status()).toBe(200);
			await expect(browser).toHaveTitle(page.title);
			await expect(browser.locator("h1")).toHaveCount(1);
			await expect(browser.getByRole("navigation", { name: "Main" }))
				.toBeVisible();
			expect(errors).toEqual([]);
		});
	}

	test("the header links reach every top-level page", async ({ page }) => {
		await page.goto("/");
		const main = page.getByRole("navigation", { name: "Main" });

		await main.getByRole("link", { name: "Pricing" }).click();
		await expect(page).toHaveURL(/\/pricing$/);
		await expect(main.getByRole("link", { name: "Pricing" })).toHaveAttribute(
			"aria-current",
			"page",
		);

		await main.getByRole("link", { name: "Services" }).click();
		await expect(page).toHaveURL(/\/services$/);

		await page.getByRole("link", { name: /Sync/ }).first().click();
		await expect(page).toHaveURL(/\/services\/sync$/);
		await expect(page.getByRole("heading", { level: 1 })).toHaveText("Sync");

		await main.getByRole("link", { name: "Docs" }).click();
		await expect(page).toHaveURL(/\/docs$/);

		await page.getByRole("link", { name: "canvist home" }).first().click();
		await expect(page).toHaveURL(/\/$/);
	});

	test("the landing page has every deck section in order", async ({ page }) => {
		await page.goto("/");
		const ids = await page.locator(".deck > section").evaluateAll((sections) =>
			sections.map((section) => section.id)
		);
		expect(ids).toEqual([...SECTIONS]);
	});

	test("the platform tabs switch the device frame", async ({ page }) => {
		await page.goto("/");
		const tabs = page.getByRole("tablist", { name: "Platforms" });
		await tabs.getByRole("tab", { name: "Flutter" }).click();
		await expect(tabs.getByRole("tab", { name: "Flutter" })).toHaveAttribute(
			"aria-selected",
			"true",
		);
		await expect(page.locator("#host-flutter")).toBeVisible();
		await expect(page.locator("#host-web")).toBeHidden();
		await page.keyboard.press("ArrowRight");
		await expect(tabs.getByRole("tab", { name: "React Native" })).toBeFocused();
		await expect(page.locator("#host-react-native")).toBeVisible();
	});

	test("internal links all resolve", async ({ page, request }) => {
		const seen = new Set<string>();
		for (const entry of ["/", "/pricing", "/services/sync", "/docs"]) {
			await page.goto(entry);
			const hrefs = await page.locator("a[href^='/']").evaluateAll((links) =>
				links.map((link) => new URL((link as HTMLAnchorElement).href).pathname)
			);
			for (const href of hrefs) seen.add(href);
		}
		for (const href of seen) {
			const response = await request.get(href);
			expect(response.status(), href).toBe(200);
		}
	});

	test("unknown paths get the 404 page", async ({ page }) => {
		const response = await page.goto("/no-such-swatch");
		expect(response?.status()).toBe(404);
		await expect(page.getByRole("heading", { level: 1 })).toContainText(
			"missing from the deck",
		);
	});

	test("the theme toggle switches and remembers the theme", async ({ page }) => {
		await page.emulateMedia({ colorScheme: "light" });
		await page.goto("/pricing");
		const toggle = page.getByRole("button", { name: "Use dark theme" });
		await toggle.click();
		await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
		await page.reload();
		await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
		await expect(page.getByRole("button", { name: "Use light theme" }))
			.toBeVisible();
	});

	test("docs search finds pages", async ({ page }) => {
		await page.goto("/docs");
		await page.keyboard.press("ControlOrMeta+k");
		const dialog = page.getByRole("dialog");
		await expect(dialog).toBeVisible();
		await dialog.getByRole("combobox").fill("budget");
		await expect(dialog.getByRole("option").first()).toBeVisible();
	});
});

test("canonical URLs have no .html and the home page is /", async ({ page }) => {
	for (
		const [path, expected] of [["/", "/"], ["/pricing", "/pricing"], [
			"/docs",
			"/docs",
		]] as const
	) {
		await page.goto(path);
		const canonical = await page.locator('link[rel="canonical"]').getAttribute(
			"href",
		);
		expect(new URL(canonical ?? "").pathname).toBe(expected);
	}
});
