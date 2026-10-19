import { loadScrollKit } from "./motion.ts";

/** Paints a stroke through each refused business practice, one after another, on scroll. */
export function mountFree(root: HTMLElement): void {
	const strokes = [
		...root.querySelectorAll<SVGPathElement>("[data-free-stroke]"),
	];
	if (strokes.length === 0) return;

	void loadScrollKit().then((kit) => {
		if (!kit) return;
		kit.gsap.fromTo(
			strokes,
			{ strokeDashoffset: 1 },
			{
				strokeDashoffset: 0,
				ease: "power2.inOut",
				stagger: 0.18,
				duration: 0.55,
				scrollTrigger: { trigger: root, start: "top 80%", once: true },
			},
		);
	});
}
