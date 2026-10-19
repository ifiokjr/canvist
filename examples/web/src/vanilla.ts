/**
 * The editor in a plain page: `createEditor`, a toolbar that follows the
 * selection through `subscribe`, and nothing else.
 */
import { createEditor } from "@canvist/web";
import { CONTENT } from "./content.ts";

const app = document.querySelector<HTMLElement>("#app")!;
const toolbar = document.createElement("div");
toolbar.className = "toolbar";
const bold = document.createElement("button");
bold.type = "button";
bold.textContent = "Bold";
const element = document.createElement("div");
element.className = "editor";
app.append(toolbar, element);
toolbar.append(bold);

const editor = await createEditor(element, {
	content: CONTENT,
	label: "Example document",
});

function refresh(): void {
	bold.disabled = !editor.can.toggleBold();
	bold.setAttribute("aria-pressed", String(editor.query.isBold()));
}

bold.addEventListener("click", () => {
	editor.commands.toggleBold();
	editor.focus();
});
editor.subscribe(refresh);
refresh();

Object.assign(globalThis, { editor });
