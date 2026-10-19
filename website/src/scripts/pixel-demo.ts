import {
	channels,
	type EditorTheme,
	type LiveEditor,
	loadV1Engine,
} from "./engine.ts";
import { prefersReducedMotion, resolveColor, whenNear } from "./motion.ts";

/**
 * "Owns every pixel": the v1 engine paints into the page and a readout
 * times each keystroke from its input event to the next painted frame.
 *
 * The engine loads on its own as the section approaches (unless the visitor
 * asked to save data or reduce motion, when the button loads it), types a
 * sentence by itself, and hands over the moment the visitor clicks in.
 */

const GHOST_TEXT =
	"Every glyph here was measured, laid out and painted by canvist, not by the browser. Click in and type to try it.";
const HISTORY = 48;

type Source = "auto" | "you";

interface NetworkInformationLike {
	saveData?: boolean;
}

function savesData(): boolean {
	const connection =
		(navigator as Navigator & { connection?: NetworkInformationLike })
			.connection;
	return connection?.saveData === true;
}

function percentile(sorted: number[], p: number): number {
	if (sorted.length === 0) return 0;
	const index = Math.min(sorted.length - 1, Math.ceil(p * sorted.length) - 1);
	return sorted[Math.max(0, index)] ?? 0;
}

/** Calls back after the frame that follows now has been painted. */
function afterNextPaint(callback: (paintedAt: number) => void): void {
	requestAnimationFrame(() => {
		const channel = new MessageChannel();
		channel.port1.onmessage = () => callback(performance.now());
		channel.port2.postMessage(undefined);
	});
}

export function mountPixelDemo(root: HTMLElement): void {
	const canvas = root.querySelector<HTMLCanvasElement>("#pixel-canvas");
	const paper = root.querySelector<HTMLElement>(".sheet__paper");
	const start = root.querySelector<HTMLButtonElement>("[data-pixel-start]");
	const startLabel = root.querySelector<HTMLElement>(
		"[data-pixel-start-label]",
	);
	const status = root.querySelector<HTMLElement>("[data-pixel-status]");
	const spark = root.querySelector<HTMLCanvasElement>("[data-pixel-spark]");
	const out = {
		last: root.querySelector<HTMLElement>("[data-pixel-last]"),
		p50: root.querySelector<HTMLElement>("[data-pixel-p50]"),
		p95: root.querySelector<HTMLElement>("[data-pixel-p95]"),
		count: root.querySelector<HTMLElement>("[data-pixel-count]"),
		source: root.querySelector<HTMLElement>("[data-pixel-source]"),
	};
	if (!canvas || !paper || !start || !startLabel || !status || !spark) {
		throw new Error("Every-pixel section markup is incomplete");
	}

	const samples: number[] = [];
	let source: Source = "auto";
	let editor: LiveEditor | undefined;
	let loading: Promise<void> | undefined;
	let ghostTimer: number | undefined;

	function theme(): EditorTheme {
		const caret = resolveColor(root, "var(--accent)");
		const [r, g, b] = channels(caret);
		return {
			background: getComputedStyle(paper as HTMLElement).backgroundColor,
			text: resolveColor(root, "var(--text)"),
			caret,
			selection: `rgba(${r}, ${g}, ${b}, 0.22)`,
		};
	}

	function format(ms: number): string {
		return `${ms.toFixed(1)} ms`;
	}

	function record(ms: number): void {
		samples.push(ms);
		if (samples.length > HISTORY) samples.shift();
		const sorted = [...samples].sort((a, b) => a - b);
		if (out.last) out.last.textContent = format(ms);
		if (out.p50) out.p50.textContent = format(percentile(sorted, 0.5));
		if (out.p95) out.p95.textContent = format(percentile(sorted, 0.95));
		if (out.count) out.count.textContent = String(samples.length);
		drawSpark();
	}

	function drawSpark(): void {
		const ctx = spark?.getContext("2d");
		if (!spark || !ctx) return;
		const { width, height } = spark;
		ctx.clearRect(0, 0, width, height);
		const max = Math.max(16.7, ...samples);
		const barWidth = width / HISTORY;
		// The 8 ms budget line.
		const budgetY = height - (8 / max) * height;
		ctx.fillStyle = "rgba(245, 243, 255, 0.35)";
		ctx.fillRect(0, budgetY, width, 1);
		for (const [index, value] of samples.entries()) {
			const barHeight = Math.max(2, (value / max) * height);
			ctx.fillStyle = value > 8 ? "#ff7a93" : "#8a80ff";
			ctx.fillRect(
				index * barWidth + 1,
				height - barHeight,
				Math.max(1, barWidth - 2),
				barHeight,
			);
		}
	}

	function setSource(next: Source): void {
		if (source === next) return;
		source = next;
		samples.length = 0;
		if (out.source) {
			out.source.textContent = next === "you"
				? "Your keystrokes to the next painted frame, measured in this browser"
				: "Auto-typing to the next painted frame, measured in this browser";
		}
	}

	function stopGhost(): void {
		clearTimeout(ghostTimer);
		ghostTimer = undefined;
	}

	function ghostType(index = 0): void {
		if (!editor || source === "you" || index >= GHOST_TEXT.length) return;
		const char = GHOST_TEXT[index] ?? "";
		const startedAt = performance.now();
		editor.type(char);
		afterNextPaint((paintedAt) => record(paintedAt - startedAt));
		const pause = char === " "
			? 70
			: /[.,]/.test(char)
			? 260
			: 38 + Math.random() * 55;
		ghostTimer = window.setTimeout(() => ghostType(index + 1), pause);
	}

	function listenForKeys(): void {
		// The v1 engine reads keys from its own hidden textarea.
		document.addEventListener(
			"keydown",
			(event) => {
				if (
					!(event.target instanceof HTMLTextAreaElement) ||
					event.target.id !== "canvist-input"
				) return;
				if (
					event.key === "Shift" || event.key === "Meta" ||
					event.key === "Control" || event.key === "Alt"
				) return;
				setSource("you");
				stopGhost();
				const pressedAt = event.timeStamp;
				afterNextPaint((paintedAt) => record(paintedAt - pressedAt));
			},
			true,
		);
		paper?.addEventListener("pointerdown", () => {
			setSource("you");
			stopGhost();
		});
	}

	async function load(): Promise<void> {
		if (!canvas) return;
		root.dataset["state"] = "loading";
		if (startLabel) startLabel.textContent = "Loading the engine…";
		const loaded = performance.now();
		try {
			editor = await loadV1Engine(canvas, theme());
		} catch (error) {
			root.dataset["state"] = "error";
			if (startLabel) startLabel.textContent = "Try loading again";
			if (status) {
				status.textContent =
					"The engine didn't load. Check your connection and try again.";
			}
			loading = undefined;
			throw error;
		}
		root.dataset["state"] = "ready";
		root.dataset["loadMs"] = String(Math.round(performance.now() - loaded));
		listenForKeys();
		document.addEventListener(
			"canvist:theme",
			() => requestAnimationFrame(() => editor?.setTheme(theme())),
		);
		if (prefersReducedMotion()) {
			setSource("you");
			editor.focus();
		} else {
			ghostType();
		}
	}

	function ensureLoaded(): Promise<void> {
		loading ??= load();
		return loading;
	}

	start.addEventListener("click", () => {
		ensureLoaded()
			.then(() => {
				setSource("you");
				stopGhost();
				editor?.focus();
			})
			.catch((error: unknown) =>
				console.warn("canvist v1 engine failed to load", error)
			);
	});

	if (!prefersReducedMotion() && !savesData()) {
		whenNear(root, () => {
			ensureLoaded().catch((error: unknown) =>
				console.warn("canvist v1 engine failed to load", error)
			);
		}, "0px");
	}
}
