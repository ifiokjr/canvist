/**
 * An editor for the editing tests: `?input=textarea` forces the textarea
 * fallback, and `?markdown=` sets the document. `window.editor` is the
 * editor, once it has painted, and `window.modifier` the key `Mod` means.
 */
import { createEditor, detectPlatform, type InputMode } from "../../src/mod.ts";

const params = new URLSearchParams(location.search);
const input = params.get("input");
const element = document.querySelector<HTMLElement>("#host")!;
const editor = await createEditor(element, {
	content: {
		format: "markdown",
		input: params.get("markdown") ?? "Hello world\n\nSecond paragraph",
	},
	label: "Test editor",
	...(input === "textarea" || input === "editcontext" || input === "auto"
		? { input: input satisfies InputMode }
		: {}),
});
await editor.ready();

Object.assign(globalThis, {
	editor,
	modifier: detectPlatform(navigator) === "apple" ? "Meta" : "Control",
});
