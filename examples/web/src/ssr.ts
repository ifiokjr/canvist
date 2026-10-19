/**
 * Hydrate server-rendered HTML. The page arrived with the document as
 * canvist's HTML, laid out by the Rust renderer with the same Inter file
 * the page declares in `@font-face`. The editor reads the document from
 * that HTML, keeps it on screen until Inter has loaded, then swaps in the
 * canvas in one frame.
 */
import { createEditor } from "@canvist/web";

const element = document.querySelector<HTMLElement>("#editor")!;
const editor = await createEditor(element, {
	// The theme the server rendered with.
	theme: { bodyFamily: "Inter, sans-serif" },
	label: "Server-rendered document",
});
await editor.ready();

Object.assign(globalThis, { editor });
