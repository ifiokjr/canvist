import { mountHero } from "./hero.ts";
import { loadScrollKit } from "./motion.ts";
import { mountPixelDemo } from "./pixel-demo.ts";

const stage = document.querySelector<HTMLElement>("[data-hero-stage]");
if (stage) mountHero(stage);

const pixel = document.querySelector<HTMLElement>("[data-pixel-demo]");
if (pixel) mountPixelDemo(pixel);

/**
 * The deck fans open as you scroll: each chip swings down from its rivet and
 * settles flat, and the hero's fan of upcoming chips folds shut as the hero
 * leaves. Scrubbed, so the motion is the scroll.
 */
void loadScrollKit().then((kit) => {
	if (!kit) return;
	const { gsap } = kit;

	for (
		const chip of document.querySelectorAll<HTMLElement>("[data-fan-chip]")
	) {
		gsap.fromTo(
			chip,
			{ rotate: 2.2, y: 48 },
			{
				rotate: 0,
				y: 0,
				ease: "none",
				scrollTrigger: {
					trigger: chip,
					start: "top bottom",
					end: "top 62%",
					scrub: 0.5,
				},
			},
		);
	}

	const fan = document.querySelector<HTMLElement>("[data-fan]");
	const hero = document.getElementById("top");
	if (fan && hero) {
		gsap.to(fan.querySelectorAll("li"), {
			rotate: 0,
			ease: "none",
			stagger: 0.02,
			scrollTrigger: {
				trigger: hero,
				start: "top top",
				end: "bottom 45%",
				scrub: 0.4,
			},
		});
	}
});
