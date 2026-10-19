/**
 * Shared motion setup for the landing page.
 *
 * GSAP and ScrollTrigger load once, on demand, and never when the visitor
 * asks for reduced motion: every section renders its final, readable state
 * without them, and motion only ever adds to that.
 *
 * Every effect they drive responds to scrolling, so they load on the
 * visitor's first sign of moving through the page (or at once when the page
 * opens already scrolled). That keeps ScrollTrigger's layout measurement
 * out of the initial load, where it cost a 430 ms task on a throttled phone.
 */
import type { gsap as GsapNamespace } from "gsap";
import type { ScrollTrigger as ScrollTriggerClass } from "gsap/ScrollTrigger";

export interface ScrollKit {
	gsap: typeof GsapNamespace;
	ScrollTrigger: typeof ScrollTriggerClass;
}

const reducedQuery = matchMedia("(prefers-reduced-motion: reduce)");

export function prefersReducedMotion(): boolean {
	return reducedQuery.matches;
}

const INTENT_EVENTS = [
	"scroll",
	"wheel",
	"touchstart",
	"keydown",
	"pointerdown",
] as const;

/** Resolves on the first scroll, wheel, touch, key or pointer press. */
function firstIntent(): Promise<void> {
	if (scrollY > 0) return Promise.resolve();
	return new Promise((resolve) => {
		const controller = new AbortController();
		const done = () => {
			controller.abort();
			resolve();
		};
		for (const type of INTENT_EVENTS) {
			addEventListener(type, done, {
				passive: true,
				signal: controller.signal,
			});
		}
	});
}

/**
 * Callers create their scroll triggers when the kit resolves, and pinned
 * sections insert scroll space above whatever follows them. If the visitor
 * is looking at a `#section` they jumped to, bring it back once that layout
 * change has happened.
 */
function keepAnchorInView(): void {
	const id = decodeURIComponent(location.hash.slice(1));
	const anchor = id === "" ? null : document.getElementById(id);
	if (anchor === null) return;
	const box = anchor.getBoundingClientRect();
	if (box.bottom <= 0 || box.top >= innerHeight) return;
	requestAnimationFrame(() => anchor.scrollIntoView());
}

let kit: Promise<ScrollKit> | undefined;

/** Resolves to GSAP with ScrollTrigger registered, or `null` under reduced motion. */
export async function loadScrollKit(): Promise<ScrollKit | null> {
	if (prefersReducedMotion()) return null;
	kit ??= firstIntent()
		.then(() => Promise.all([import("gsap"), import("gsap/ScrollTrigger")]))
		.then(([core, plugin]) => {
			core.gsap.registerPlugin(plugin.ScrollTrigger);
			keepAnchorInView();
			return { gsap: core.gsap, ScrollTrigger: plugin.ScrollTrigger };
		});
	return kit;
}

/** Calls `callback` once the element first comes within `margin` of the viewport. */
export function whenNear(
	element: Element,
	callback: () => void,
	margin = "25%",
): void {
	const observer = new IntersectionObserver(
		(entries) => {
			if (entries.some((entry) => entry.isIntersecting)) {
				observer.disconnect();
				callback();
			}
		},
		{ rootMargin: margin },
	);
	observer.observe(element);
}

/** Tracks whether an element is on screen, for loops that should pause off screen. */
export function trackVisibility(
	element: Element,
	onChange: (visible: boolean) => void,
): void {
	const observer = new IntersectionObserver((entries) => {
		for (const entry of entries) onChange(entry.isIntersecting);
	});
	observer.observe(element);
}

export const isDesktopLayout = (): boolean =>
	matchMedia("(min-width: 64.0625rem)").matches;

/**
 * Resolves a CSS colour expression (tokens use `light-dark()`, which
 * `getPropertyValue` returns unresolved) to the colour the browser computes,
 * for painting on a canvas.
 */
export function resolveColor(scope: Element, expression: string): string {
	const probe = document.createElement("span");
	probe.style.color = expression;
	probe.style.display = "none";
	scope.append(probe);
	const color = getComputedStyle(probe).color;
	probe.remove();
	return color;
}
