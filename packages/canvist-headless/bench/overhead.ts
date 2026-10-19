/**
 * What a call through the WebAssembly boundary costs, measured on the same
 * operations as `crates/canvist_headless/tests/perf_budgets.rs`, so the
 * difference is the binding's overhead.
 *
 * ```sh
 * node bench/overhead.ts           # print the numbers
 * node bench/overhead.ts --check   # also fail when one is over budget
 * ```
 *
 * `CANVIST_PERF_SLACK` multiplies every budget, as it does for the Rust
 * budgets.
 *
 * @module
 */
import { readFileSync } from "node:fs";

import { createEngine } from "../src/mod.ts";

const PARAGRAPH =
	"The quick brown fox jumps over the lazy dog, again and again, until the line wraps.";
const PARAGRAPHS = 10_000;
const SAMPLES = 15;
const CARET = { path: [PARAGRAPHS / 2], offset: 10 };
const WASM = new URL("../wasm/canvist_headless_wasm_bg.wasm", import.meta.url);

const slack = Number(process.env.CANVIST_PERF_SLACK ?? 1);
const check = process.argv.includes("--check");
const failures: string[] = [];

/** The median time of one call, in microseconds, over `SAMPLES` batches. */
function perCall(batch: number, run: () => unknown): number {
	run();
	const samples = Array.from({ length: SAMPLES }, () => {
		const start = performance.now();

		for (let index = 0; index < batch; index += 1) {
			run();
		}

		return ((performance.now() - start) * 1000) / batch;
	});

	return samples.sort((a, b) => a - b)[Math.floor(SAMPLES / 2)] ?? Number.NaN;
}

function report(
	name: string,
	measured: number,
	budget: number,
	unit: "µs" | "ms" | "KB",
): void {
	const allowed = budget * slack;
	const over = measured > allowed;
	console.log(
		`${name}: ${
			measured.toFixed(unit === "µs" ? 2 : 1)
		} ${unit} (budget ${budget} ${unit})${over ? "  OVER" : ""}`,
	);

	if (over) {
		failures.push(name);
	}
}

const engine = await createEngine();
const markdown = Array.from(
	{ length: PARAGRAPHS },
	(_, index) => `${PARAGRAPH} ${index}`,
).join("\n\n");

const doc = engine.parse("markdown", markdown);
doc.select(CARET);
report(
	"run a typed command @10k paragraphs",
	perCall(2_000, () => doc.commands.toggleBold()),
	5,
	"µs",
);
report(
	"answer a query @10k paragraphs",
	perCall(2_000, () => doc.query.isBold()),
	5,
	"µs",
);

let typing = false;
report(
	"type or delete a character @10k paragraphs",
	perCall(2_000, () => {
		typing = !typing;
		return typing ? doc.insertText("x") : doc.press("Backspace");
	}),
	5,
	"µs",
);

const server = engine.parse("markdown", markdown);
server.share(1);
const peer = engine.fromYjs(server.encodeState(), 2);
const updates: Uint8Array[] = [];

for (let index = 0; index < (SAMPLES + 1) * 200; index += 1) {
	peer.select(CARET);
	peer.insertText("y");
	peer.press("Backspace");
	updates.push(...peer.takeUpdates());
}

let next = 0;
report(
	"apply one Yjs keystroke update @10k paragraphs",
	perCall(200, () => server.applyUpdate(updates[next++] ?? new Uint8Array())),
	30,
	"µs",
);

report(
	"load 10k paragraphs from Markdown",
	perCall(1, () => engine.parse("markdown", markdown)) / 1000,
	100,
	"ms",
);
report(
	"export 10k paragraphs as JSON",
	perCall(1, () => doc.export("json")) / 1000,
	100,
	"ms",
);

const size = readFileSync(WASM).byteLength / 1024;
report("WebAssembly module (wasm-opt, raw)", size, 1_200, "KB");

if (check && failures.length > 0) {
	console.error(`over budget: ${failures.join(", ")}`);
	process.exit(1);
}
