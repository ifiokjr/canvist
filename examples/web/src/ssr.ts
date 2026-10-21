/**
 * Hydrate server-rendered HTML. `server.ts` rendered the document into the
 * page with `@canvist/headless/ssr`, using the theme this editor uses. The
 * editor reads the document from that HTML, keeps it on screen until Inter
 * has loaded, then swaps in the canvas in one frame.
 */
import { createEditor } from "@canvist/web";
import { THEME } from "./ssr-content.ts";

const element = document.querySelector<HTMLElement>("#editor")!;
const editor = await createEditor(element, {
	theme: THEME,
	label: "Server-rendered document",
});
await editor.ready();

Object.assign(globalThis, { editor });
