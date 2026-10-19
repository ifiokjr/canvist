import type { Platform } from "../data/platforms.ts";
import { prefersReducedMotion, trackVisibility } from "./motion.ts";

const AUTO_ADVANCE_MS = 2800;

/**
 * "Everywhere": the platform pills are an ARIA tab list over a stack of
 * device frames. Selecting a host crossfades its frame in (opacity and
 * transform only, in CSS). With motion allowed, hosts advance on a timer
 * while the stage is on screen, until the visitor picks one.
 */
export function mountEverywhere(
	root: HTMLElement,
	platforms: Platform[],
): void {
	const tabs = [...root.querySelectorAll<HTMLButtonElement>("[data-host]")];
	const frames = [...root.querySelectorAll<HTMLElement>("[data-device]")];
	if (tabs.length !== platforms.length || frames.length !== platforms.length) {
		throw new Error("Everywhere section markup is incomplete");
	}

	let current = 0;
	let userChose = false;
	let timer: number | undefined;

	function select(index: number, focus = false): void {
		const tab = tabs[index];
		if (!tab) return;
		current = index;
		for (const [i, other] of tabs.entries()) {
			other.setAttribute("aria-selected", String(i === index));
			other.tabIndex = i === index ? 0 : -1;
		}
		for (const [i, frame] of frames.entries()) frame.hidden = i !== index;
		if (focus) tab.focus();
	}

	function choose(index: number, focus = false): void {
		userChose = true;
		clearInterval(timer);
		select(index, focus);
	}

	for (const [index, tab] of tabs.entries()) {
		tab.addEventListener("click", () => choose(index));
		tab.addEventListener("keydown", (event) => {
			const last = tabs.length - 1;
			const next = {
				ArrowRight: current === last ? 0 : current + 1,
				ArrowDown: current === last ? 0 : current + 1,
				ArrowLeft: current === 0 ? last : current - 1,
				ArrowUp: current === 0 ? last : current - 1,
				Home: 0,
				End: last,
			}[event.key];
			if (next === undefined) return;
			event.preventDefault();
			choose(next, true);
		});
	}

	if (prefersReducedMotion()) return;

	const stage = root.querySelector(".stage") ?? root;
	trackVisibility(stage, (visible) => {
		clearInterval(timer);
		if (!visible || userChose) return;
		timer = window.setInterval(
			() => select((current + 1) % platforms.length),
			AUTO_ADVANCE_MS,
		);
	});
}
