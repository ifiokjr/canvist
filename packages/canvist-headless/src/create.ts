/**
 * Creating engines from the StarterKit WebAssembly module.
 *
 * @module
 */
import { WasmEngine } from "../wasm/canvist_headless_wasm.js";
import { Engine, type EngineOptions, type StarterKit } from "./engine.ts";
import { starterKit } from "./extensions.generated.ts";
import { load } from "./load.ts";

/**
 * Load canvist and create an engine with the given StarterKit extensions,
 * all of them by default. The engine's documents only offer the registered
 * extensions' commands, queries, blocks, and marks, so using another one
 * is a type error. Every other extension is in `@canvist/headless/full`.
 *
 * ```ts
 * const engine = await createEngine([bold, heading]);
 * const doc = engine.parse("markdown", "Hello");
 * doc.selectAll();
 * doc.commands.toggleBold();
 * doc.commands.toggleItalic(); // type error: italic is not registered
 * ```
 */
export async function createEngine<
	const E extends readonly StarterKit[number][] = StarterKit,
>(extensions?: E, options: EngineOptions = {}): Promise<Engine<E>> {
	await load(options.wasm);
	const registered = (extensions ?? starterKit) as E;
	const names = registered.map((extension) => extension.name);
	return new Engine(new WasmEngine(names, options.client ?? 1), registered);
}
