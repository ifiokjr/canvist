/**
 * Optimize `wasm/canvist_web_bg.wasm` for size on the wire.
 *
 * wasm-pack's default `wasm-opt` pass inlines small functions. That makes
 * the module smaller raw but larger gzipped (374 KB against 355 KB
 * unoptimized), because inlined copies compress worse than calls. These
 * passes optimize without inlining, which measured smallest gzipped:
 * `-O3` 337.0 KB, `-O2` 337.7 KB, `-Os` 337.1 KB, `-Oz` 338.2 KB, and
 * `--converge` (repeat until nothing shrinks) saves another 0.4 KB.
 * `--low-memory-unused` and `--zero-filled-memory` are safe for
 * Rust's wasm32 output, which never reads below 1 KiB and starts with
 * zeroed memory.
 *
 * @module
 */
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const module = fileURLToPath(
	new URL("../wasm/canvist_web_bg.wasm", import.meta.url),
);
const wasmOpt = fileURLToPath(
	new URL("../node_modules/.bin/wasm-opt", import.meta.url),
);

execFileSync(wasmOpt, [
	module,
	"-o",
	module,
	"-O3",
	"--converge",
	// No inlining of any kind: see above.
	"--flexible-inline-max-function-size=0",
	"--always-inline-max-function-size=0",
	"--one-caller-inline-max-function-size=0",
	"--low-memory-unused",
	"--zero-filled-memory",
	"--strip-debug",
	"--strip-producers",
	// The features Rust's wasm32-unknown-unknown target emits.
	"--enable-bulk-memory",
	"--enable-nontrapping-float-to-int",
	"--enable-sign-ext",
	"--enable-mutable-globals",
	"--enable-reference-types",
	"--enable-multivalue",
], { stdio: "inherit" });
