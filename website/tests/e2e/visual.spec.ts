import { expect, type Locator, type Page, test } from "@playwright/test";
import process from "node:process";
import { SECTIONS } from "./pages.ts";

/**
 * Visual snapshots of each landing section, plus the other page types, in
 * both themes. Rendered with reduced motion so every section is in its final,
 * settled state. Baselines are macOS-only (see playwright.config.ts).
 */
test.skip(
	process.platform !== "darwin",
	"Visual baselines are rendered on macOS; CI runs them on a macOS runner.",
);
test.use({ reducedMotion: "reduce" });

/**
 * Fonts use `font-display: optional`, so a cold load may paint the fallback.
 * Load once to warm the cache, then reload so the web fonts are used.
 */
async function open(page: Page, path: string): Promise<void> {
	await page.goto(path);
	await page.evaluate(() =>
		Promise.all([
			document.fonts.load("700 1em Bricolage"),
			document.fonts.load("1em 'Geist Mono'"),
		])
	);
	await page.reload();
	await page.evaluate(() => document.fonts.ready);
	expect(await page.evaluate(() => document.fonts.check("700 1em Bricolage")))
		.toBe(true);
}

/**
 * An element's box in document coordinates, rounded. Sections sit at
 * fractional offsets that shift when content above them changes, and an
 * element screenshot's size would round differently with them.
 */
interface Box {
	x: number;
	y: number;
	width: number;
	height: number;
}

async function documentBox(page: Page, element: Locator): Promise<Box> {
	const box = await element.boundingBox();
	if (box === null) throw new Error("Section is not rendered");
	const scrolled = await page.evaluate(() => window.scrollY);
	return {
		x: Math.round(box.x),
		y: Math.round(box.y + scrolled),
		width: Math.round(box.width),
		height: Math.round(box.height),
	};
}

/** The live demos paint changing numbers; mask them so snapshots stay stable. */
function masks(page: Page) {
	return [
		page.locator("[data-pixel-readout]"),
		page.locator("#pixel-canvas"),
		page.locator("[data-video-slot] video"),
	];
}

for (const scheme of ["light", "dark"] as const) {
	test.describe(`${scheme} theme`, () => {
		test.beforeEach(async ({ page }) => {
			await page.emulateMedia({ colorScheme: scheme, reducedMotion: "reduce" });
		});

		for (const width of [1280, 375] as const) {
			test(`landing sections at ${width}px`, async ({ page }) => {
				await page.setViewportSize({ width, height: 900 });
				await open(page, "/");
				for (const id of SECTIONS) {
					// The speed chip's table and headline come from the repo's budget
					// table, which grows with every benchmark; snapshot its fixed header.
					const section = id === "speed"
						? page.locator("#speed .speed__copy")
						: page.locator(`#${id}`);
					await section.scrollIntoViewIfNeeded();
					await expect(page).toHaveScreenshot(
						`${id}-${width}-${scheme}.png`,
						{
							mask: masks(page),
							fullPage: true,
							clip: await documentBox(page, section),
						},
					);
				}
			});
		}

		for (const path of ["/pricing", "/services/sync", "/docs/v2/roadmap"]) {
			test(`${path} at 1280px`, async ({ page }) => {
				await page.setViewportSize({ width: 1280, height: 900 });
				await open(page, path);
				const name = path.slice(1).replaceAll("/", "-");
				await expect(page).toHaveScreenshot(`${name}-${scheme}.png`);
			});
		}
	});
}
