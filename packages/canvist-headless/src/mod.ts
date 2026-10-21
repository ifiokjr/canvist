/**
 * canvist's document model and editing engine with no DOM, for Node, Deno,
 * Bun, and Workers: load and export ProseMirror JSON, HTML, Markdown, and
 * text, run typed commands, and serve Yjs collaboration.
 *
 * ```ts
 * import { createEngine } from "@canvist/headless";
 *
 * const engine = await createEngine();
 * const doc = engine.parse("markdown", "Hello world");
 * doc.select({ path: [0], offset: 6 }, { path: [0], offset: 11 });
 * doc.commands.toggleBold();
 * doc.export("html"); // "<p>Hello <strong>world</strong></p>"
 * ```
 *
 * @module
 */
export {
	type CanvistError,
	type Content,
	createEngine,
	Doc,
	Engine,
	type EngineOptions,
	isCanvistError,
	type Registered,
	type StarterKit,
	type SyncReply,
	type Validation,
} from "./engine.ts";
export * from "./extensions.generated.ts";
export { load, type WasmSource } from "./load.ts";
export type * from "./types.ts";
