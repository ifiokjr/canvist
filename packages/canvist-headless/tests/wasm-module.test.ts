/**
 * Loading from a compiled `WebAssembly.Module`, as Cloudflare Workers do
 * when they import a `.wasm` file. Under `node --test` each file runs in
 * its own process, so this is the first load; a runtime that shares one
 * process across files may already have loaded the module from disk.
 *
 * @module
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

import { createEngine } from "../src/mod.ts";

test("loads from a compiled module, as Workers do", async () => {
	const bytes = await readFile(
		new URL("../wasm/canvist_headless_wasm_bg.wasm", import.meta.url),
	);
	const module = await WebAssembly.compile(bytes);
	const engine = await createEngine(undefined, { wasm: module });

	assert.equal(
		engine.parse("markdown", "**ok**").export("html"),
		"<p><strong>ok</strong></p>",
	);
});
