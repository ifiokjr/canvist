/**
 * Shared motion setup for the landing page.
 *
 * GSAP and ScrollTrigger load once, on demand, and never when the visitor
 * asks for reduced motion: every section renders its final, readable state
 * without them, and motion only ever adds to that.
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

let kit: Promise<ScrollKit> | undefined;

/** Resolves to GSAP with ScrollTrigger registered, or `null` under reduced motion. */
export async function loadScrollKit(): Promise<ScrollKit | null> {
	if (prefersReducedMotion()) return null;
	kit ??= Promise.all([import("gsap"), import("gsap/ScrollTrigger")]).then(
		([core, plugin]) => {
			core.gsap.registerPlugin(plugin.ScrollTrigger);
			return { gsap: core.gsap, ScrollTrigger: plugin.ScrollTrigger };
		},
	);
	return kit;
}

/** Runs `onChange` when the visitor flips reduced motion, so pages can reload into the right mode. */
export function onReducedMotionChange(
	onChange: (reduced: boolean) => void,
): void {
	reducedQuery.addEventListener("change", (event) => onChange(event.matches));
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
