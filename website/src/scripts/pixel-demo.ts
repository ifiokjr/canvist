import { PIXEL_GHOST_TEXT } from "../lib/pixel-document.ts";
import { type LiveEditor, loadEditor } from "./engine.ts";
import {
	colorScheme,
	onColorSchemeChange,
	prefersReducedMotion,
	trackVisibility,
	whenScrollSettles,
} from "./motion.ts";

/**
 * "Type into the real engine": the page arrives with the document as
 * server-rendered HTML, `@canvist/web` takes its place without moving it,
 * and a readout times each keystroke from its input event to the next
 * painted frame.
 *
 * The editor loads on its own as the section comes on screen (unless the
 * visitor asked to save data or reduce motion, when clicking the document
 * or the button loads it), types a sentence by itself, and hands over the
 * moment the visitor clicks in.
 */

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

const MODIFIERS = new Set(["Shift", "Meta", "Control", "Alt"]);

export function mountPixelDemo(root: HTMLElement): void {
	const element = root.querySelector<HTMLElement>("[data-pixel-editor]");
	const paper = root.querySelector<HTMLElement>(".sheet__paper");
	const start = root.querySelector<HTMLButtonElement>("[data-pixel-start]");
	const startLabel = root.querySelector<HTMLElement>(
		"[data-pixel-start-label]",
	);
	const status = root.querySelector<HTMLElement>("[data-pixel-status]");
	const out = {
		last: root.querySelector<HTMLElement>("[data-pixel-last]"),
		p50: root.querySelector<HTMLElement>("[data-pixel-p50]"),
		source: root.querySelector<HTMLElement>("[data-pixel-source]"),
	};
	if (!element || !paper || !start || !startLabel || !status) {
		throw new Error("Every-pixel section markup is incomplete");
	}

	const samples: number[] = [];
	let source: Source = "auto";
	let editor: LiveEditor | undefined;
	let loading: Promise<LiveEditor> | undefined;
	let ghostTimer: number | undefined;
	let onScreen = false;

	function format(ms: number): string {
		return `${ms.toFixed(1)} ms`;
	}

	function record(ms: number): void {
		samples.push(ms);
		if (samples.length > HISTORY) samples.shift();
		const sorted = [...samples].sort((a, b) => a - b);
		if (out.last) out.last.textContent = format(ms);
		if (out.p50) out.p50.textContent = format(percentile(sorted, 0.5));
	}

	function setSource(next: Source): void {
		if (source === next) return;
		source = next;
		samples.length = 0;
		if (out.source) {
			out.source.textContent = next === "you"
				? "Your keystroke to paint"
				: "Keystroke to paint";
		}
	}

	function stopGhost(): void {
		clearTimeout(ghostTimer);
		ghostTimer = undefined;
	}

	function handOver(): void {
		setSource("you");
		stopGhost();
	}

	/** The ghost's caret blinks while the section is on screen, until the visitor takes over. */
	function showGhostCaret(): void {
		if (source === "auto") editor?.showCaret(onScreen);
	}

	function ghostType(index = 0): void {
		if (!editor || source === "you" || index >= PIXEL_GHOST_TEXT.length) {
			return;
		}
		const char = PIXEL_GHOST_TEXT[index] ?? "";
		const startedAt = performance.now();
		editor.type(char);
		afterNextPaint((paintedAt) => record(paintedAt - startedAt));
		const pause = char === " "
			? 70
			: /[.,:]/.test(char)
			? 260
			: 38 + Math.random() * 55;
		ghostTimer = window.setTimeout(() => ghostType(index + 1), pause);
	}

	function listenForKeys(): void {
		element?.addEventListener(
			"keydown",
			(event) => {
				if (MODIFIERS.has(event.key)) return;
				handOver();
				const pressedAt = event.timeStamp;
				afterNextPaint((paintedAt) => record(paintedAt - pressedAt));
			},
			true,
		);
	}

	async function load(): Promise<LiveEditor> {
		if (!element) throw new Error("Every-pixel section has no editor");
		root.dataset["state"] = "loading";
		const loaded = performance.now();
		let live: LiveEditor;
		try {
			live = await loadEditor(element, colorScheme());
		} catch (error) {
			root.dataset["state"] = "error";
			if (startLabel) startLabel.textContent = "Try loading again";
			if (status) {
				status.textContent =
					"The editor didn't load. Check your connection and try again.";
			}
			loading = undefined;
			throw error;
		}
		editor = live;
		root.dataset["state"] = "ready";
		root.dataset["loadMs"] = String(Math.round(performance.now() - loaded));
		listenForKeys();
		onColorSchemeChange(() => void live.setScheme(colorScheme()));
		live.caretToEnd();
		if (prefersReducedMotion()) {
			setSource("you");
		} else if (source === "auto") {
			showGhostCaret();
			ghostType();
		}
		return live;
	}

	function ensureLoaded(): Promise<LiveEditor> {
		loading ??= load();
		return loading;
	}

	/** Loads the editor if it hasn't yet, and gives it to the visitor. */
	function takeOver(): void {
		handOver();
		// Only here, in answer to the visitor: the longer label moves the
		// button, which counts as a layout shift when nobody asked for it.
		if (!editor && startLabel) startLabel.textContent = "Loading the editor…";
		ensureLoaded()
			.then((live) => live.focus())
			.catch((error: unknown) =>
				console.warn("canvist editor failed to load", error)
			);
	}

	start.addEventListener("click", takeOver);
	// Before the editor has loaded, the document is HTML: pressing it loads
	// the editor. After, the editor handles the press itself.
	paper.addEventListener("pointerdown", () => {
		if (editor) handOver();
		else takeOver();
	});

	// Start the editor when the section is on screen and scrolling has
	// paused: its start-up is main-thread work that would otherwise drop
	// frames mid-scroll on slower phones. A visitor who scrolls straight past
	// never pays for it.
	const loadsItself = !prefersReducedMotion() && !savesData();
	let waiting = false;
	const attempt = () => {
		if (waiting || loading !== undefined) return;
		waiting = true;
		void whenScrollSettles().then(() => {
			waiting = false;
			if (!onScreen) return;
			ensureLoaded().catch((error: unknown) =>
				console.warn("canvist editor failed to load", error)
			);
		});
	};
	trackVisibility(root, (visible) => {
		onScreen = visible;
		showGhostCaret();
		if (visible && loadsItself) attempt();
	});
}
