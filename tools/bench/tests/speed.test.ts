import assert from "node:assert/strict";
import { test } from "node:test";

import { speedReport } from "../lib/report.ts";
import {
	judge,
	judgeAll,
	median,
	type SpeedConfig,
	thresholdFor,
} from "../lib/speed.ts";

const config: SpeedConfig = {
	rounds: 5,
	confirmRounds: 5,
	threshold: 10,
	thresholds: {
		"scene/*": 25,
		"scene/to_json/*": 40,
		"model/type_char/100": 5,
	},
	globalPaths: [],
	suites: [],
};

test("median of odd and even counts", () => {
	assert.equal(median([3, 1, 2]), 2);
	assert.equal(median([4, 1, 3, 2]), 2.5);
	assert.throws(() => median([]));
});

test("an exact id beats a pattern, and a longer pattern beats a shorter one", () => {
	assert.equal(thresholdFor("model/type_char/100", config), 5);
	assert.equal(thresholdFor("model/type_char/10000", config), 10);
	assert.equal(thresholdFor("scene/to_json/1000", config), 40);
	assert.equal(thresholdFor("scene/bulk_insert/1000", config), 25);
});

test("a slowdown beyond the threshold with separated runs regresses", () => {
	const row = judge("model/type_char/100", "time", {
		base: [100, 101, 99, 100, 102],
		head: [120, 119, 121, 122, 118],
	}, 10);

	assert.equal(row.verdict, "regressed");
	assert.equal(row.baseNs, 100);
	assert.equal(row.headNs, 120);
	assert.ok(Math.abs(row.change! - 0.2) < 1e-9);
});

test("a large median change with overlapping runs is noise", () => {
	const row = judge("scene/to_json/1000", "time", {
		base: [100, 100, 100, 100, 300],
		head: [130, 130, 130, 130, 90],
	}, 10);

	assert.equal(row.verdict, "unchanged");
});

test("separated runs within the threshold are unchanged", () => {
	const row = judge("model/type_char/100", "time", {
		base: [100, 100, 100],
		head: [105, 105, 105],
	}, 10);

	assert.equal(row.verdict, "unchanged");
});

test("a speedup beyond the threshold with separated runs improves", () => {
	const row = judge("model/type_char/100", "time", {
		base: [100, 100, 100],
		head: [50, 51, 49],
	}, 10);

	assert.equal(row.verdict, "improved");
});

test("benchmarks on one side only are new or removed, and never fail", () => {
	assert.equal(judge("a", "time", { base: [], head: [1] }, 10).verdict, "new");
	assert.equal(
		judge("a", "time", { base: [1], head: [] }, 10).verdict,
		"removed",
	);
});

test("the report leads with what changed and keeps every row", () => {
	const samples = new Map([
		["model/type_char/100", { base: [100, 100, 100], head: [200, 200, 200] }],
		["model/type_char/10000", { base: [100, 100, 100], head: [100, 100, 100] }],
	]);
	const markdown = speedReport(judgeAll(samples, config), {
		title: "bench / speed",
		notes: [],
	});

	assert.match(markdown, /❌ 1 of 2 benchmarks regressed/);
	assert.match(
		markdown,
		/\| `model\/type_char\/100` \| 100 ns \| 200 ns \| \+100\.0% \| ±5% \| ❌ regressed \|/,
	);
	assert.match(markdown, /All 2 benchmarks/);
});
