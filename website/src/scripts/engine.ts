/**
 * The live editor behind "Owns every pixel".
 *
 * The section talks to this interface, not to a specific engine, so the v2
 * web host (milestone M3) replaces `loadV1Engine` without touching the page.
 */
export interface LiveEditor {
	/** Which engine is painting, for the readout. */
	readonly label: string;
	/** Inserts text at the caret, as if typed. */
	type(text: string): void;
	focus(): void;
	setTheme(theme: EditorTheme): void;
	destroy(): void;
}

/** Colours as computed CSS colour strings, for example `rgb(59, 44, 245)`. */
export interface EditorTheme {
	background: string;
	text: string;
	caret: string;
	selection: string;
}

export type EngineLoader = (
	canvas: HTMLCanvasElement,
	theme: EditorTheme,
) => Promise<LiveEditor>;

/** A computed CSS colour (`rgb()` or `rgba()`, as browsers serialise them) to channel bytes. */
export function channels(color: string): [number, number, number, number] {
	const parts = /rgba?\(([^)]+)\)/.exec(color)?.[1]?.split(/[\s,/]+/).filter(
		Boolean,
	).map(Number);
	if (!parts || parts.length < 3 || parts.some(Number.isNaN)) {
		throw new Error(`Cannot read colour channels from "${color}"`);
	}
	const [r = 0, g = 0, b = 0, a = 1] = parts;
	return [r, g, b, Math.round(a * 255)];
}

/** The v1 editor from packages/canvist, compiled to wasm. */
export const loadV1Engine: EngineLoader = async (canvas, theme) => {
	const { createEditor } = await import("../../../packages/canvist/src/mod.ts");

	const box = canvas.getBoundingClientRect();
	const dpr = Math.min(devicePixelRatio, 2);
	canvas.width = Math.round(box.width * dpr);
	canvas.height = Math.round(box.height * dpr);
	canvas.getContext("2d")?.scale(dpr, dpr);

	const editor = await createEditor(canvas.id, { title: "Owns every pixel" });
	editor.setSize(box.width, box.height);
	editor.setFontSize(20);
	editor.setWordWrap(true);
	editor.setHighlightCurrentLine(false);

	const apply = (next: EditorTheme) => {
		editor.setThemeColor("background", ...channels(next.background));
		editor.setThemeColor("text", ...channels(next.text));
		editor.setThemeColor("caret", ...channels(next.caret));
		editor.setThemeColor("selection", ...channels(next.selection));
		editor.setCursorColor(...channels(next.caret));
		editor.render();
	};
	apply(theme);

	return {
		label: "v1 engine · wasm",
		type(text) {
			editor.insertText(text);
			editor.render();
		},
		focus() {
			canvas.focus();
		},
		setTheme: apply,
		destroy() {
			editor.destroy();
		},
	};
};
