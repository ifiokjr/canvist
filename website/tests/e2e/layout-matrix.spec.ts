import { expect, type Page, test } from "@playwright/test";
import { PAGES, VIEWPORT_MATRIX } from "./pages.ts";

/**
 * Nothing may be cut off, at any common screen size: every page is scrolled
 * end to end with motion on (so reveals have run), then every rendered
 * element is checked against its clipping ancestors.
 */

interface Clipped {
	element: string;
	by: string;
	overflow: string;
}

/**
 * Scrolls like a reader, top to bottom, so entrance motion and lazy content
 * run, and checks for clipping at every stop: pinned content is only cut off
 * while it is pinned.
 */
async function scrollThrough(page: Page): Promise<Clipped[]> {
	const found = new Map<string, Clipped>();
	let previous = -1;
	for (let step = 0; step < 200; step++) {
		const y = await page.evaluate(() => {
			scrollBy(0, Math.round(innerHeight * 0.6));
			return scrollY;
		});
		if (y === previous) break;
		previous = y;
		await page.waitForTimeout(80);
		for (const problem of await page.evaluate(findClipped)) {
			found.set(`${problem.element} ${problem.by}`, problem);
		}
	}
	// Let the last settle transitions finish, then check the resting page.
	await page.waitForTimeout(1200);
	for (const problem of await page.evaluate(findClipped)) {
		found.set(`${problem.element} ${problem.by}`, problem);
	}
	return [...found.values()];
}

function findClipped(): Clipped[] {
	const describe = (element: Element): string => {
		const id = element.id ? `#${element.id}` : "";
		const classes = [...element.classList].slice(0, 2).map((name) => `.${name}`)
			.join("");
		const text =
			element.textContent?.trim().replace(/\s+/g, " ").slice(0, 30) ?? "";
		return `${element.tagName.toLowerCase()}${id}${classes}${
			text ? ` "${text}"` : ""
		}`;
	};
	const clips = (value: string) => value === "hidden" || value === "clip";
	const scrolls = (value: string) => value === "auto" || value === "scroll";
	const problems: Clipped[] = [];

	for (const element of document.body.querySelectorAll("*")) {
		if (
			element.closest(
				"[hidden], [data-allow-clip], script, style, template, dialog:not([open])",
			)
		) continue;
		const style = getComputedStyle(element);
		if (
			style.display === "none" || style.visibility === "hidden" ||
			Number(style.opacity) === 0
		) continue;
		const box = element.getBoundingClientRect();
		if (box.width < 1 || box.height < 1) continue;

		// Fixed content (including GSAP-style pinned panels) is clipped by the viewport itself.
		const pinned = (() => {
			for (
				let node: Element | null = element;
				node;
				node = node.parentElement
			) {
				const position = getComputedStyle(node).position;
				if (position === "fixed") return true;
				if (scrolls(getComputedStyle(node).overflowY)) return false;
			}
			return false;
		})();
		if (pinned && (box.bottom > innerHeight + 1 || box.top < -1)) {
			problems.push({
				element: describe(element),
				by: "the viewport (fixed position)",
				overflow: "fixed",
			});
			continue;
		}

		for (
			let ancestor = element.parentElement;
			ancestor;
			ancestor = ancestor.parentElement
		) {
			const outer = getComputedStyle(ancestor);
			// Content inside a scroll container is reachable by scrolling it.
			if (scrolls(outer.overflowX) || scrolls(outer.overflowY)) break;
			const clipX = clips(outer.overflowX);
			const clipY = clips(outer.overflowY);
			if (!clipX && !clipY) continue;
			const limit = ancestor === document.body
				? {
					left: 0,
					right: document.documentElement.clientWidth,
					top: -Infinity,
					bottom: Infinity,
				}
				: ancestor.getBoundingClientRect();
			const outX = clipX &&
				(box.left < limit.left - 1 || box.right > limit.right + 1);
			const outY = clipY &&
				(box.top < limit.top - 1 || box.bottom > limit.bottom + 1);
			if (outX || outY) {
				problems.push({
					element: describe(element),
					by: describe(ancestor),
					overflow: `${outer.overflowX}/${outer.overflowY}`,
				});
				break;
			}
		}
	}
	return problems;
}

for (const viewport of VIEWPORT_MATRIX) {
	test.describe(`${viewport.name} (${viewport.width}×${viewport.height})`, () => {
		test.use({
			viewport: { width: viewport.width, height: viewport.height },
			reducedMotion: "no-preference",
		});

		for (const entry of PAGES) {
			test(`${entry.path}: nothing clipped, no horizontal overflow`, async ({ page }) => {
				await page.goto(entry.path);
				const clipped = await scrollThrough(page);
				const overflow = await page.evaluate(() =>
					document.documentElement.scrollWidth - innerWidth
				);
				expect(overflow, "horizontal overflow in px").toBeLessThanOrEqual(0);
				expect(clipped).toEqual([]);
			});
		}
	});
}
