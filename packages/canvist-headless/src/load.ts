/**
 * Loading the WebAssembly module in any JavaScript runtime.
 *
 * @module
 */
import init from "../wasm/canvist_headless_wasm.js";

/**
 * Where the WebAssembly module comes from: its bytes, a compiled module (as
 * Cloudflare Workers import `.wasm` files), a URL, or a response.
 */
export type WasmSource =
	| ArrayBuffer
	| ArrayBufferView
	| WebAssembly.Module
	| URL
	| Response
	| Promise<Response>;

const BUNDLED = new URL(
	"../wasm/canvist_headless_wasm_bg.wasm",
	import.meta.url,
);

let loading: Promise<void> | undefined;

/**
 * Load the WebAssembly module once. Later calls return the first call's
 * promise, so passing a source only matters the first time.
 *
 * Without a source, Node, Deno, and Bun read the bundled file from disk.
 * Runtimes without a file system, such as Workers, pass the module they
 * imported: `await load(wasmModule)`.
 */
export function load(source?: WasmSource): Promise<void> {
	loading ??= instantiate(source);
	return loading;
}

async function instantiate(source: WasmSource | undefined): Promise<void> {
	await init({ module_or_path: source ?? await bundled() });
}

/** The bundled module: read from disk when it is a file, fetched otherwise. */
async function bundled(): Promise<WasmSource> {
	if (BUNDLED.protocol !== "file:") {
		return BUNDLED;
	}

	const [{ readFile }, { fileURLToPath }] = await Promise.all([
		import("node:fs/promises"),
		import("node:url"),
	]);
	// A path, not the URL: Node's `URL` type and the global one differ.
	return readFile(fileURLToPath(BUNDLED));
}
