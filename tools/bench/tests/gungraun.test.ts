import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { parseGungraun } from "../lib/gungraun.ts";
import { speedReport } from "../lib/report.ts";
import { judgeAll, type SpeedConfig } from "../lib/speed.ts";

const config: SpeedConfig = {
	rounds: 5,
	confirmRounds: 5,
	threshold: 10,
	thresholds: {},
	globalPaths: [],
	suites: [{
		package: "canvist_headless",
		bench: "counts",
		metric: "instructions",
	}],
};

/** A real run of the `counts` suite, as the gate invokes it. */
const fixture = readFileSync(
	new URL("./fixtures/counts.json", import.meta.url),
	"utf8",
);

test("real gungraun output: one summary per line, counts keyed by suite, group, and id", () => {
	assert.deepEqual(
		parseGungraun("counts", fixture),
		new Map([
			["counts/export/html", 42_429_691],
			["counts/export/json", 215_996_486],
			["counts/export/markdown", 113_717_870],
		]),
	);
});

test("a single summary as one JSON document parses too", () => {
	const output =
		`{"version":"7","function_name":"write","group":"export","id":"html","profiles":[{"data":{"total":{"metrics":{"Ir":{"values":{"new":42}}}}}}]}`;

	assert.deepEqual(
		parseGungraun("counts", output),
		new Map([
			["counts/export/html", 42],
		]),
	);
});

test("a summary without an id or a count fails loudly", () => {
	assert.throws(() =>
		parseGungraun(
			"counts",
			`{"group":"export","function_name":"write","profiles":[]}`,
		)
	);
	assert.throws(() =>
		parseGungraun(
			"counts",
			`{"group":"export","id":"html","profiles":[{"data":{"total":{"metrics":{}}}}]}`,
		)
	);
	assert.throws(() => parseGungraun("counts", ""));
});

test("counts suites judge and report in instruction units", () => {
	const samples = new Map([
		["counts/export/html", { base: [966_000_000], head: [1_120_000_000] }],
	]);
	const rows = judgeAll(samples, config);

	assert.equal(rows[0].unit, "instructions");
	assert.equal(rows[0].verdict, "regressed");

	const markdown = speedReport(rows, { title: "bench / speed", notes: [] });

	assert.match(markdown, /966 M/);
	assert.match(markdown, /1.12 G/);
	assert.match(markdown, /\+15\.9%/);
});
