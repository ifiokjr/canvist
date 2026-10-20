/**
 * An editor for the editing tests: `?input=textarea` forces the textarea
 * fallback, `?markdown=` sets the document, `?editable=false` makes it
 * read-only, `?placeholder=` sets its placeholder, and `?menus` attaches
 * a bubble menu and a floating menu. `window.editor` is the editor, once
 * it has painted, and `window.modifier` the key `Mod` means.
 */
import { bubbleMenu, floatingMenu } from "../../src/menus.ts";
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
	editable: params.get("editable") !== "false",
	placeholder: params.get("placeholder") ?? "",
	...(input === "textarea" || input === "editcontext" || input === "auto"
		? { input: input satisfies InputMode }
		: {}),
});
await editor.ready();

if (params.has("menus")) {
	const bubble = document.querySelector<HTMLElement>("#bubble")!;
	const floating = document.querySelector<HTMLElement>("#floating")!;
	bubbleMenu(editor, bubble);
	floatingMenu(editor, floating);
	bubble.querySelector("button")!.addEventListener("click", () => {
		editor.commands.toggleBold();
	});
	floating.querySelector("button")!.addEventListener("click", () => {
		editor.commands.toggleHeading({ level: 2 });
	});
}

Object.assign(globalThis, {
	editor,
	modifier: detectPlatform(navigator) === "apple" ? "Meta" : "Control",
});
