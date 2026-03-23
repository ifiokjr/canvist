/**
 * WASM initialisation helpers.
 *
 * The WASM module must be initialised once before any editor instances can be
 * created. {@link initWasm} handles this and is idempotent — calling it
 * multiple times is safe.
 */

import type { CanvistEditor as GeneratedCanvistEditor } from "../wasm/canvist_wasm.d.ts";

let _initialised = false;
let _initPromise: Promise<void> | null = null;
let _wasmModule: CanvistWasmModule | null = null;

/**
 * The WASM editor instance type. Uses the auto-generated type definitions
 * from `wasm-pack build`, ensuring all WASM methods are available with
 * correct signatures.
 */
export type WasmCanvistEditor = GeneratedCanvistEditor;

export interface CanvistWasmModule {
	default(input?: unknown): Promise<unknown>;
	CanvistEditor: {
		create(canvasId: string): WasmCanvistEditor;
		diff_texts(a: string, b: string): string[];
	};
}

/**
 * Dynamically import the WASM glue module.
 *
 * This is split out so the import path works in both Deno and Node/bundler
 * environments.
 */
async function loadWasmModule(): Promise<CanvistWasmModule> {
	if (_wasmModule) return _wasmModule;
	// The wasm glue JS is co-located in the package.
	_wasmModule = (await import("../wasm/canvist_wasm.js")) as CanvistWasmModule;
	return _wasmModule;
}

/**
 * Initialise the canvist WASM module.
 *
 * Must be called (and awaited) before creating any editor instances. Safe to
 * call multiple times — subsequent calls are no-ops.
 *
 * ```ts
 * import { initWasm } from "@canvist/canvist";
 * await initWasm();
 * ```
 */
export async function initWasm(): Promise<void> {
	if (_initialised) return;
	if (_initPromise) return _initPromise;

	_initPromise = (async () => {
		const mod = await loadWasmModule();
		// wasm-pack --target web exposes a default init function.
		await mod.default();
		_initialised = true;
	})();

	return _initPromise;
}

/**
 * Return the raw WASM module exports (for internal use).
 * @internal
 */
export async function getWasmModule(): Promise<CanvistWasmModule> {
	await initWasm();
	return loadWasmModule();
}
