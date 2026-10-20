# Quick start: Web

In this guide you put a live canvist editor on a web page with `@canvist/web`, add a toolbar that follows the selection, and set up a bundler. It takes about five minutes.

**Status: available.** `@canvist/web` works in Chromium, Firefox, and Safari, and its tests run in all three on every change: pixel parity with server-rendered HTML, editing and input methods, accessibility, collaboration, and a 350 KB bundle budget. Using React or Vue? See [React](./react.md) and [Vue](./vue.md).

## 1. Install

```sh
npm install @canvist/web
```

## 2. Create an editor

Give the editor an element with a width. Without a height, it grows with its document and the page scrolls:

```html
<div class="toolbar"><button id="bold" type="button">Bold</button></div>
<div id="editor" style="max-width: 640px"></div>
```

```ts no_run
import { createEditor } from "@canvist/web";

const element = document.querySelector<HTMLElement>("#editor");
const bold = document.querySelector<HTMLButtonElement>("#bold");

if (element !== null && bold !== null) {
	const editor = await createEditor(element, {
		content: { format: "markdown", input: "# Hello\n\nStart typing." },
		label: "Notes",
	});

	// Keep the button in step with the selection.
	const refresh = () => {
		bold.disabled = !editor.can.toggleBold();
		bold.setAttribute("aria-pressed", String(editor.query.isBold()));
	};
	editor.subscribe(refresh);
	refresh();

	bold.addEventListener("click", () => {
		editor.commands.toggleBold();
		editor.focus();
	});
}
```

The editor paints on a canvas, but it types, selects, copies, pastes, and composes text from input methods as a text field does, and screen readers read it. [Text input on the web](../guides/web-input.md) explains how.

Commands, queries, and `can` checks are typed from the extensions you pass (TipTap's StarterKit by default). Calling one you didn't register is a type error.

## 3. Set up your bundler

`@canvist/web` loads a WebAssembly module from next to its own files, and bundlers that follow `new URL(…, import.meta.url)` copy it for you. In Vite, exclude canvist from dependency pre-bundling, which would move the modules away from it:

```ts no_run
// vite.config.ts
export default {
	optimizeDeps: { exclude: ["@canvist/web", "@canvist/headless"] },
};
```

To serve the module yourself, pass its URL as `wasm` in the options.

## 4. Hydrate server-rendered HTML

canvist's [server renderer](./ssr.md) (`@canvist/headless/ssr`) sends the document as HTML that wraps exactly where the editor does. Render it into the editor's element, and `createEditor` reads the document from it. It keeps the HTML on screen until your fonts have loaded, then swaps in the canvas in one frame, so nothing moves:

```ts no_run
import { createEditor } from "@canvist/web";

// The element already holds the server's `<div class="cv-doc">…</div>`.
const element = document.querySelector<HTMLElement>("#editor");

if (element !== null) {
	const editor = await createEditor(element, {
		// The theme the server rendered with.
		theme: { bodyFamily: "Inter, sans-serif" },
	});
	await editor.ready(); // the canvas is showing now
}
```

The server and the browser must lay text out with the same font files. [Fonts and text rendering](../guides/fonts.md#render-the-same-fonts-on-your-server) shows how.

## 5. Read-only editors and placeholders

```ts no_run
import { createEditor } from "@canvist/web";

const element = document.querySelector<HTMLElement>("#editor");

if (element !== null) {
	const editor = await createEditor(element, {
		content: { format: "markdown", input: "" },
		placeholder: "Write a comment…",
		editable: false,
	});

	// Later, when the person may edit:
	editor.setEditable(true);
}
```

- **`editable: false`** makes the editor read-only. People can still select text with the pointer and navigation keys, and copy it, but typing, pasting, cutting, and dropping change nothing, and no caret shows. Commands and the API still change the document, as in TipTap. Screen readers hear `aria-readonly`.
- **`placeholder`** shows dimmed text where the first line starts while the document is empty (`editor.isEmpty`), and is the field's `aria-placeholder`. Change it with `editor.setPlaceholder`.
- **`editor.subscribe`** also calls back when focus or `editable` changes, and `editor.focused` and `editor.editable` read them.

## 6. Menus that follow the selection

`@canvist/web/menus` keeps an element you build next to the selection: `bubbleMenu` over selected text, and `floatingMenu` beside the caret on an empty line. They follow edits, scrolling, and resizing, flip to the other side when there's no room, and hide when they don't apply. Pressing inside a menu keeps the editor's focus, so its buttons act on the selection:

```html
<div id="bubble" role="toolbar" aria-label="Formatting">
	<button id="italic" type="button">Italic</button>
</div>
```

```ts no_run
import { createEditor } from "@canvist/web";
import { bubbleMenu } from "@canvist/web/menus";

const element = document.querySelector<HTMLElement>("#editor");
const bubble = document.querySelector<HTMLElement>("#bubble");

if (element !== null && bubble !== null) {
	const editor = await createEditor(element);
	const menu = bubbleMenu(editor, bubble, { placement: "top", offset: 8 });

	bubble.querySelector("#italic")?.addEventListener("click", () => {
		editor.commands.toggleItalic();
	});
	// menu.destroy() stops following the editor.
}
```

By default a bubble menu shows while the editor is editable and focused and the selection isn't a caret, and a floating menu while the caret is in an empty text block. Pass `shouldShow(editor)` to decide yourself. For your own positioning, `editor.selectionRect()` is the selection's bounding box in document coordinates, `editor.toClientRect(rect)` converts it to viewport coordinates, and `placeMenu(element, rect, { placement })` does the flipping and clamping.

## 7. Collaborate

`@canvist/web/collab` puts editors in a room on any y-websocket server, with no Yjs code of your own:

```ts no_run
import { connect, createEditor } from "@canvist/web/collab";

const element = document.querySelector<HTMLElement>("#editor");

if (element !== null) {
	const editor = await createEditor(element);
	await connect(editor, { url: "wss://sync.example.com", room: "notes" })
		.whenSynced();
}
```

[Collaboration](../guides/collaboration.md#put-editors-in-a-room) covers servers, reconnection, and undo.

## Run the examples

`examples/web` in the repository has a plain page, React, Vue, and two server-rendered pages (plain and React), each with a Playwright test:

```sh
cd examples/web
npm install
npm run dev   # http://localhost:5180
```
