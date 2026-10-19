/**
 * Shared motion helpers for the landing page.
 *
 * There is no scroll-linked JavaScript: nothing runs per scroll event, and
 * nothing is pinned. Motion is CSS transitions on `transform` and `opacity`,
 * started once by an IntersectionObserver, plus loops (the caret demos) that
 * run only while their element is on screen. Under reduced motion none of it
 * runs, and every section's resting state is its final state.
 */

const reducedQuery = matchMedia("(prefers-reduced-motion: reduce)");

export function prefersReducedMotion(): boolean {
	return reducedQuery.matches;
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

/**
 * Resolves once the page has not scrolled for `quietMs`, so heavy start-up
 * work (like initialising the wasm editor) never lands on a scrolling frame.
 */
export function whenScrollSettles(quietMs = 300): Promise<void> {
	return new Promise((resolve) => {
		const controller = new AbortController();
		let timer = window.setTimeout(done, quietMs);
		function done(): void {
			controller.abort();
			resolve();
		}
		addEventListener(
			"scroll",
			() => {
				clearTimeout(timer);
				timer = window.setTimeout(done, quietMs);
			},
			{ passive: true, signal: controller.signal },
		);
	});
}

/**
 * Chips below the fold settle onto the deck as they scroll in: they wait
 * slightly low and transparent, then a single CSS transition brings them
 * home. Chips already on screen are left alone, and nothing is hidden unless
 * this script runs, so a failed script never hides content.
 */
export function settleChips(chips: Iterable<HTMLElement>): void {
	if (prefersReducedMotion()) return;
	const waiting = [...chips].filter((chip) =>
		chip.getBoundingClientRect().top > innerHeight
	);
	if (waiting.length === 0) return;
	for (const chip of waiting) chip.classList.add("is-waiting");

	const observer = new IntersectionObserver(
		(entries) => {
			for (const entry of entries) {
				if (!entry.isIntersecting) continue;
				const chip = entry.target as HTMLElement;
				observer.unobserve(chip);
				chip.classList.add("is-settling");
				// Let the waiting state paint before moving, so the transition runs.
				requestAnimationFrame(() => chip.classList.remove("is-waiting"));
				// Opacity finishes first; the settle ends when the transform does.
				const settled = (event: TransitionEvent) => {
					if (event.target !== chip || event.propertyName !== "transform") {
						return;
					}
					chip.classList.remove("is-settling");
					chip.removeEventListener("transitionend", settled);
				};
				chip.addEventListener("transitionend", settled);
			}
		},
		// Start a little before the chip's top reaches the viewport.
		{ rootMargin: "0px 0px 10% 0px" },
	);
	for (const chip of waiting) observer.observe(chip);
}

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
