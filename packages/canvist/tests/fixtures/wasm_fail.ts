export async function initWasmFixture(): Promise<void> {
	const mod = await import("./missing_wasm_glue.js");
	await mod.default();
}
