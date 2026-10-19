import { expect, test } from "@playwright/test";

/** Splits "1.10.2" into numbers so versions compare numerically. */
function versionParts(version: string): number[] {
	return version.split(".").map((part) => Number.parseInt(part, 10));
}

test("the changelog shows releases newest first, or an empty state before the first", async ({ page }) => {
	await page.goto("/changelog");
	await expect(page.getByRole("heading", { level: 1, name: "What's new" }))
		.toBeVisible();

	const versions = await page.locator(".release__version").allTextContents();
	if (versions.length === 0) {
		await expect(page.getByText("Nothing to report yet.")).toBeVisible();
		return;
	}
	const sorted = [...versions].sort((a, b) => {
		const [left, right] = [versionParts(a), versionParts(b)];
		for (let index = 0; index < Math.max(left.length, right.length); index++) {
			const difference = (right[index] ?? 0) - (left[index] ?? 0);
			if (difference !== 0) return difference;
		}
		return 0;
	});
	expect(versions).toEqual(sorted);
	// Every release links to itself, so a release can be shared.
	const first = page.locator(".release").first();
	const anchor = await first.getAttribute("id");
	await expect(first.locator(".release__version a")).toHaveAttribute(
		"href",
		`#${anchor}`,
	);
});

test("the footer links to the changelog", async ({ page }) => {
	await page.goto("/");
	await page.getByRole("contentinfo").getByRole("link", { name: "Changelog" })
		.click();
	await expect(page).toHaveURL(/\/changelog$/);
});
