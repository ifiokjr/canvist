import { mountHero } from "./hero.ts";
import { settleChips } from "./motion.ts";
import { mountPixelDemo } from "./pixel-demo.ts";

const stage = document.querySelector<HTMLElement>("[data-hero-stage]");
if (stage) mountHero(stage);

const pixel = document.querySelector<HTMLElement>("[data-pixel-demo]");
if (pixel) mountPixelDemo(pixel);

settleChips(document.querySelectorAll<HTMLElement>("[data-settle]"));
