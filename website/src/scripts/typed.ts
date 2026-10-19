import { loadScrollKit } from "./motion.ts";

/**
 * "Headless and typed": a switch registers or removes the Highlight
 * extension. Removing it strikes the extension out and lights up the type
 * error on the call that needed it. With motion allowed, scrolling past the
 * code removes it once on its own, so a visitor who never clicks still sees
 * the error appear.
 */
export function mountTyped(root: HTMLElement): void {
	const toggle = root.querySelector<HTMLButtonElement>("[data-typed-switch]");
	const problems = root.querySelector<HTMLElement>("[data-typed-problems]");
	if (!toggle || !problems) {
		throw new Error("Typed section markup is incomplete");
	}

	let touched = false;

	function set(registered: boolean): void {
		if (!toggle || !problems) return;
		root.dataset["highlight"] = registered ? "on" : "off";
		toggle.setAttribute("aria-checked", String(registered));
		problems.textContent = registered ? "0 problems" : "1 problem";
	}

	toggle.addEventListener("click", () => {
		touched = true;
		set(toggle.getAttribute("aria-checked") !== "true");
	});

	void loadScrollKit().then((kit) => {
		if (!kit) return;
		kit.ScrollTrigger.create({
			trigger: root,
			start: "top 35%",
			once: true,
			onEnter: () => {
				if (!touched) set(false);
			},
		});
	});
}
