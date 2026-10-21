import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { parseDivan, parseDuration } from "../lib/divan.ts";

const headless = readFileSync(
	new URL("./fixtures/headless.txt", import.meta.url),
	"utf8",
);

test("durations in every unit divan prints", () => {
	assert.equal(parseDuration("10 ps"), 0.01);
	assert.equal(parseDuration("273.1 ns"), 273.1);
	assert.equal(parseDuration("1.2 µs"), 1200);
	assert.equal(parseDuration("24.08 ms"), 24_080_000);
	assert.equal(parseDuration("1.5 s"), 1_500_000_000);
	assert.equal(parseDuration(""), undefined);
	assert.equal(parseDuration("72.1 Mitem/s"), undefined);
});

test("every leaf of a real divan table, nested or not", () => {
	const medians = parseDivan("headless", headless);

	assert.equal(medians.size, 12);
	assert.equal(medians.get("headless/apply_update/100"), 2290);
	assert.equal(medians.get("headless/create"), 55_620);
	assert.equal(medians.get("headless/export/markdown"), 6_759_000);
	assert.equal(medians.get("headless/run_command/10000"), 219.6);
	assert.equal(medians.has("headless/export"), false);
});

test("throughput rows belong to the leaf above them", () => {
	const output = [
		"layout          fastest       │ slowest       │ median        │ mean          │ samples │ iters",
		"╰─ shape                      │               │               │               │         │",
		"   ├─ 100       1 µs          │ 2 µs          │ 1.5 µs        │ 1.5 µs        │ 100     │ 100",
		"   │            100 Mitem/s   │ 50 Mitem/s    │ 66.6 Mitem/s  │ 66.6 Mitem/s  │         │",
		"   ╰─ 1000      10 µs         │ 20 µs         │ 15 µs         │ 15 µs         │ 100     │ 100",
		"                100 Mitem/s   │ 50 Mitem/s    │ 66.6 Mitem/s  │ 66.6 Mitem/s  │         │",
	].join("\n");

	assert.deepEqual(
		[...parseDivan("layout", output)],
		[["layout/shape/100", 1500], ["layout/shape/1000", 15_000]],
	);
});

test("output without a table is an error, not an empty result", () => {
	assert.throws(
		() => parseDivan("model", "thread 'main' panicked"),
		/printed no divan table/,
	);
});
