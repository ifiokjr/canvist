/**
 * Every extension canvist ships, in a second WebAssembly module, so
 * `@canvist/headless` stays small with only the StarterKit. The API is the
 * same; `createEngine` here also takes the extensions beyond the StarterKit.
 *
 * ```ts
 * import { createEngine, highlight, starterKit } from "@canvist/headless/full";
 *
 * const engine = await createEngine([...starterKit, highlight]);
 * const doc = engine.parse("markdown", "Hello world");
 * doc.selectAll();
 * doc.commands.toggleHighlight();
 * doc.export("html"); // "<p><mark>Hello world</mark></p>"
 * ```
 *
 * The emoji table is data rather than code: the first engine with `emoji`
 * loads it.
 *
 * @module
 */
import init, {
	loadEmoji,
	WasmEngine,
} from "../wasm-full/canvist_headless_full.js";
import { Engine, type EngineOptions } from "./engine.ts";
import { starterKit } from "./extensions.generated.ts";
import { bundled, type WasmSource } from "./load.ts";
import type { Extensions, StarterKit } from "./types.ts";

export {
	type CanvistError,
	Doc,
	Engine,
	type EngineOptions,
	isCanvistError,
	type SyncReply,
	type Validation,
} from "./engine.ts";
export * from "./extensions.generated.ts";
export * from "./extras.generated.ts";
export type { WasmSource } from "./load.ts";
export type * from "./types.ts";

const BUNDLED = new URL(
	"../wasm-full/canvist_headless_full_bg.wasm",
	import.meta.url,
);

let loading: Promise<void> | undefined;
let loadingEmoji: Promise<void> | undefined;

/**
 * Load the full WebAssembly module once. Later calls return the first
 * call's promise. Without a source, Node, Deno, and Bun read the bundled
 * file; runtimes without a file system pass the module they imported.
 */
export function load(source?: WasmSource): Promise<void> {
	loading ??= (async () => {
		await init({ module_or_path: source ?? await bundled(BUNDLED) });
	})();
	return loading;
}

/** Give the module the emoji table, once. */
function emojiTable(): Promise<void> {
	loadingEmoji ??= import("./gemoji.generated.ts").then(({ gemoji }) => {
		loadEmoji(gemoji);
	});
	return loadingEmoji;
}

/**
 * Load canvist and create an engine with the given extensions, the
 * StarterKit by default. The engine's documents only offer the registered
 * extensions' commands, queries, blocks, and marks, so using another one
 * is a type error.
 */
export async function createEngine<const E extends Extensions = StarterKit>(
	extensions?: E,
	options: EngineOptions = {},
): Promise<Engine<E>> {
	await load(options.wasm);
	const registered = (extensions ?? starterKit) as E;
	const names = registered.map((extension) => extension.name);

	if (names.includes("emoji")) {
		await emojiTable();
	}

	return new Engine(new WasmEngine(names, options.client ?? 1), registered);
}
