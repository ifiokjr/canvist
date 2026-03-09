import { assert, assertInstanceOf, assertRejects } from "@std/assert";
import { getWasmModule, initWasm } from "../src/wasm.ts";

Deno.test("initWasm is idempotent across repeated calls", async () => {
	await initWasm();
	await initWasm();
	const module = await getWasmModule();
	assert(module);
	assertInstanceOf(module.CanvistEditor, Function);
});

Deno.test("initWasm surfaces module-load failures", async () => {
	await assertRejects(
		() => import("./fixtures/wasm_fail.ts").then((m) => m.initWasmFixture()),
		Error,
	);
});
