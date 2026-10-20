/**
 * The editor in a plain page: `createEditor`, a toolbar that follows the
 * selection through `subscribe`, a read-only switch, and a bubble menu
 * from `@canvist/web/menus` over selected text.
 */
import { createEditor } from "@canvist/web";
import { bubbleMenu } from "@canvist/web/menus";
import { CONTENT, PLACEHOLDER } from "./content.ts";

const app = document.querySelector<HTMLElement>("#app")!;
const toolbar = document.createElement("div");
toolbar.className = "toolbar";
const bold = button("Bold");
const readOnly = document.createElement("label");
const readOnlyBox = document.createElement("input");
readOnlyBox.type = "checkbox";
readOnly.append(readOnlyBox, " Read-only");
const element = document.createElement("div");
element.className = "editor";
const bubble = document.createElement("div");
bubble.className = "bubble";
bubble.setAttribute("role", "toolbar");
bubble.setAttribute("aria-label", "Selection");
const italic = button("Italic");
bubble.append(italic);
toolbar.append(bold, readOnly);
app.append(toolbar, element, bubble);

const editor = await createEditor(element, {
	content: CONTENT,
	label: "Example document",
	placeholder: PLACEHOLDER,
});
bubbleMenu(editor, bubble);

function refresh(): void {
	bold.disabled = !editor.editable || !editor.can.toggleBold();
	bold.setAttribute("aria-pressed", String(editor.query.isBold()));
	italic.setAttribute("aria-pressed", String(editor.query.isItalic()));
}

bold.addEventListener("click", () => {
	editor.commands.toggleBold();
	editor.focus();
});
italic.addEventListener("click", () => editor.commands.toggleItalic());
readOnlyBox.addEventListener("change", () => {
	editor.setEditable(!readOnlyBox.checked);
});
editor.subscribe(refresh);
refresh();

function button(label: string): HTMLButtonElement {
	const created = document.createElement("button");
	created.type = "button";
	created.textContent = label;
	return created;
}

Object.assign(globalThis, { editor });
