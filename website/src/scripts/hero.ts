import { prefersReducedMotion, trackVisibility } from "./motion.ts";

/**
 * The hero's Wet Caret. The static lockup is in the HTML; under
 * `motion-ok` it starts unpainted with the caret waiting at the left.
 *
 * - When Rive loads within `RIVE_DEADLINE_MS`, its canvas takes over and the
 *   caret paints the wordmark, lets the drip fall, then blinks.
 * - Otherwise the CSS stroke in Hero.astro paints it (it also starts on its
 *   own after a delay, so the page never depends on this script).
 * - Under reduced motion nothing animates: the lockup is simply there.
 */

/** Names inside website/public/rive/wet-caret.riv (see website/rive/). */
const RIVE_SRC = "/rive/wet-caret.riv";
const STATE_MACHINE = "Lockup";
const ARTBOARDS = { light: "Light", dark: "Dark" } as const;
const RIVE_DEADLINE_MS = 2200;

type StageState = "waiting" | "rive" | "css" | "static";

function theme(): keyof typeof ARTBOARDS {
	const pinned = document.documentElement.dataset["theme"];
	if (pinned === "light" || pinned === "dark") return pinned;
	return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function idle(callback: () => void): void {
	if ("requestIdleCallback" in window) {
		requestIdleCallback(callback, { timeout: 400 });
	} else setTimeout(callback, 1);
}

export function mountHero(stage: HTMLElement): void {
	const canvas = stage.querySelector<HTMLCanvasElement>("[data-hero-rive]");
	if (!canvas) throw new Error("Hero stage has no Rive canvas");
	const surface = canvas;

	const setState = (state: StageState) => {
		stage.dataset["state"] = state;
	};

	if (prefersReducedMotion()) {
		setState("static");
		return;
	}

	let settled = false;
	const startedAt = performance.now();
	const deadline = setTimeout(() => {
		if (settled) return;
		settled = true;
		setState("css");
	}, RIVE_DEADLINE_MS);

	const start = async () => {
		const [
			{ Rive, Layout, Fit, Alignment, RuntimeLoader },
			{ default: wasmUrl },
		] = await Promise.all([
			import("@rive-app/canvas-lite"),
			import("@rive-app/canvas-lite/rive.wasm?url"),
		]);
		RuntimeLoader.setWasmUrl(wasmUrl);
		if (settled) return;

		const rive = new Rive({
			src: RIVE_SRC,
			canvas: surface,
			artboard: ARTBOARDS[theme()],
			stateMachine: STATE_MACHINE,
			layout: new Layout({ fit: Fit.Contain, alignment: Alignment.CenterLeft }),
			autoplay: true,
			shouldDisableRiveListeners: true,
			isTouchScrollEnabled: true,
			onLoad: () => {
				if (settled) {
					rive.cleanup();
					return;
				}
				settled = true;
				clearTimeout(deadline);
				rive.resizeDrawingSurfaceToCanvas();
				setState("rive");
				stage.dataset["riveMs"] = String(
					Math.round(performance.now() - startedAt),
				);
			},
			onLoadError: () => {
				if (settled) return;
				settled = true;
				clearTimeout(deadline);
				setState("css");
			},
		});

		trackVisibility(stage, (visible) => {
			if (stage.dataset["state"] !== "rive") return;
			if (visible) rive.play();
			else rive.pause();
		});
		new ResizeObserver(() => rive.resizeDrawingSurfaceToCanvas()).observe(
			surface,
		);
		// The painted artboard is per theme. After a switch, the static master
		// (already themed by CSS) is the honest, instant answer.
		document.addEventListener("canvist:theme", () => {
			rive.cleanup();
			setState("static");
		});
	};

	const begin = () => idle(() => void start().catch(() => setState("css")));
	if (document.readyState === "complete") begin();
	else addEventListener("load", begin, { once: true });
}
