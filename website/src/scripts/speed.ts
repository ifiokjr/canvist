import { loadScrollKit } from "./motion.ts";

/** Grows each benchmark bar from zero as the table scrolls into view. */
export function mountSpeed(root: HTMLElement): void {
	const bars = [...root.querySelectorAll<HTMLElement>("[data-bench-bar]")];
	if (bars.length === 0) return;

	void loadScrollKit().then((kit) => {
		if (!kit) return;
		kit.gsap.fromTo(
			bars,
			{ scaleX: 0 },
			{
				scaleX: 1,
				ease: "power3.out",
				stagger: 0.06,
				duration: 0.9,
				scrollTrigger: {
					trigger: root.querySelector(".bench") ?? root,
					start: "top 85%",
					once: true,
				},
			},
		);
	});
}
