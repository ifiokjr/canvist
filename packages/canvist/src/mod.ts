/**
 * @module canvist
 *
 * A canvas-based text editor for the browser — build your own Google Docs.
 *
 * canvist renders text through a custom WASM-powered canvas engine, giving you
 * full control over every pixel. Instead of relying on `contenteditable`, it
 * manages its own rendering, selection, input handling, and accessibility.
 *
 * ## Quick start
 *
 * ```ts
 * import { createEditor } from "@canvist/canvist";
 *
 * const editor = await createEditor("my-canvas");
 * editor.insertText("Hello, canvist!");
 * editor.render();
 * ```
 *
 * @license Unlicense
 */

export { createEditor, type EditorOptions } from "./editor.ts";
export { initWasm } from "./wasm.ts";
export type { CanvistEditor } from "./types.ts";
