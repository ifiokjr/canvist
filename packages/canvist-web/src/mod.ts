/**
 * The canvist editor for the browser.
 *
 * canvist paints every pixel of the document on a canvas, measured by the
 * browser itself, so the canvas wraps each line exactly where canvist's
 * server-rendered HTML does and hydrating from that HTML moves nothing.
 * Commands, queries, and plugins are typed from what you register: calling
 * one you did not register is a type error.
 *
 * ```ts
 * import { createEditor } from "@canvist/web";
 *
 * const editor = await createEditor(document.querySelector("#editor")!, {
 * 	content: { format: "markdown", input: "Hello **world**" },
 * });
 * editor.select({ path: [0], offset: 0 }, { path: [0], offset: 5 });
 * editor.commands.toggleItalic();
 * ```
 *
 * @module
 */
export type {
	CanvistError,
	CommandName,
	CommandParams,
	Commands,
	DocJson,
	ErrorCode,
	ExtensionDescriptor,
	Extensions,
	Format,
	Position,
	Queries,
	QueryName,
	QueryParams,
	QueryResult,
	Range,
	Registered,
	StarterKit,
} from "@canvist/headless";
export * from "@canvist/headless/extensions";
export {
	borderScale,
	type Content,
	createEditor,
	detectBreaks,
	Editor,
	type EditorOptions,
	type EditorStats,
	type FrameReport,
	type Geometry,
	type LineBreaks,
	pixelGrid,
	type ThemeOptions,
} from "./editor.ts";
export { familyName, FontWatcher } from "./font-watcher.ts";
export { load, type WasmSource } from "./load.ts";
export { CanvasMeasurer, type MeasureListener } from "./measure.ts";
export type { Plugin, PluginApis, PluginContext, Plugins } from "./plugin.ts";
