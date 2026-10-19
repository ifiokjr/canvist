import type { Platform } from "../data/platforms.ts";
import {
	isDesktopLayout,
	loadScrollKit,
	prefersReducedMotion,
	trackVisibility,
} from "./motion.ts";

const AUTO_ADVANCE_MS = 2400;
/** Viewport heights of scroll per host while the stage is pinned. */
const SCROLL_PER_HOST = 0.55;

/**
 * "Everywhere": one device frame morphs between the seven hosts. The host
 * list is an ARIA tab list that always works by click and arrow keys. With
 * motion allowed, scrolling steps through the hosts on desktop (the grid is
 * pinned), and on small screens the hosts advance on a timer while visible.
 */
export function mountEverywhere(
	root: HTMLElement,
	platforms: Platform[],
): void {
	const device = root.querySelector<HTMLElement>("[data-device]");
	const tabs = [...root.querySelectorAll<HTMLButtonElement>("[data-host]")];
	const name = root.querySelector<HTMLElement>("[data-host-name]");
	const detail = root.querySelector<HTMLElement>("[data-host-detail]");
	if (!device || tabs.length !== platforms.length || !name || !detail) {
		throw new Error("Everywhere section markup is incomplete");
	}

	let current = 0;
	let userChose = false;

	function select(index: number, focus = false): void {
		const platform = platforms[index];
		const tab = tabs[index];
		if (!platform || !tab || !device || !name || !detail) return;
		current = index;
		device.dataset["platform"] = platform.id;
		device.setAttribute("aria-labelledby", tab.id);
		for (const [i, other] of tabs.entries()) {
			other.setAttribute("aria-selected", String(i === index));
			other.tabIndex = i === index ? 0 : -1;
		}
		name.textContent = `${platform.name} (${platform.statusText})`;
		detail.textContent = platform.how;
		if (focus) tab.focus();
	}

	for (const [index, tab] of tabs.entries()) {
		tab.addEventListener("click", () => {
			userChose = true;
			select(index);
		});
		tab.addEventListener("keydown", (event) => {
			const last = tabs.length - 1;
			const next = {
				ArrowDown: current === last ? 0 : current + 1,
				ArrowRight: current === last ? 0 : current + 1,
				ArrowUp: current === 0 ? last : current - 1,
				ArrowLeft: current === 0 ? last : current - 1,
				Home: 0,
				End: last,
			}[event.key];
			if (next === undefined) return;
			event.preventDefault();
			userChose = true;
			select(next, true);
		});
	}
	select(0);

	if (prefersReducedMotion()) return;

	if (!isDesktopLayout()) {
		let timer: number | undefined;
		trackVisibility(device, (visible) => {
			clearInterval(timer);
			if (!visible) return;
			timer = window.setInterval(() => {
				if (userChose) return clearInterval(timer);
				select((current + 1) % platforms.length);
			}, AUTO_ADVANCE_MS);
		});
		return;
	}

	void loadScrollKit().then((kit) => {
		if (!kit) return;
		const chip = root.closest("section") ?? root;
		kit.ScrollTrigger.create({
			trigger: chip,
			start: "top top",
			end: () => `+=${innerHeight * SCROLL_PER_HOST * platforms.length}`,
			pin: chip,
			anticipatePin: 1,
			onUpdate: (self) => {
				const index = Math.min(
					platforms.length - 1,
					Math.floor(self.progress * platforms.length),
				);
				if (index !== current) select(index);
			},
		});
	});
}
