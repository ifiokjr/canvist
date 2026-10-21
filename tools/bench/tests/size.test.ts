import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";

import { sizeReport } from "../lib/report.ts";
import {
	failed,
	grewTooMuch,
	judgeSizes,
	measureArtifact,
	type SizeConfig,
	withNeeds,
} from "../lib/size.ts";

const config: SizeConfig = {
	growthPercent: 1,
	growthBytes: 1024,
	approvalLabel: "size-increase-approved",
	globalPaths: [],
	groups: {
		headless: {},
		web: { needs: ["headless"] },
		react: { needs: ["web"] },
	},
	artifacts: [
		{ name: "web", group: "web", files: ["web.js"], budget: { gzip: 300_000 } },
		{ name: "react", group: "react", files: ["react.js"] },
	],
};

const root = mkdtempSync(join(tmpdir(), "canvist-size-"));
after(() => rmSync(root, { recursive: true, force: true }));

test("an artifact is the total of its files, each compressed alone", () => {
	writeFileSync(join(root, "a.js"), "a".repeat(10_000));
	writeFileSync(join(root, "b.js"), "b".repeat(10_000));
	const sizes = measureArtifact(root, {
		name: "js",
		group: "web",
		files: ["*.js"],
	})!;

	assert.equal(sizes.raw, 20_000);
	assert.ok(sizes.gzip! > 0 && sizes.gzip! < 200);
	assert.ok(sizes.brotli! > 0 && sizes.brotli! < 100);
	assert.equal(
		measureArtifact(root, { name: "none", group: "web", files: ["*.wasm"] }),
		undefined,
	);
	assert.deepEqual(
		Object.keys(
			measureArtifact(root, {
				name: "raw",
				group: "web",
				files: ["a.js"],
				metrics: ["raw"],
			})!,
		),
		["raw"],
	);
});

test("growth must clear both the percentage and the byte floor", () => {
	assert.equal(grewTooMuch({ gzip: 100_000 }, { gzip: 101_500 }, config), true);
	assert.equal(
		grewTooMuch({ gzip: 100_000 }, { gzip: 100_900 }, config),
		false,
	);
	assert.equal(
		grewTooMuch({ gzip: 200_000 }, { gzip: 201_500 }, config),
		false,
	);
	assert.equal(grewTooMuch({ raw: 1000 }, { raw: 9000 }, config), true);
});

test("budgets fail even with the approval label, growth fails without it", () => {
	const base = {
		groups: ["web", "react"],
		artifacts: { web: { gzip: 290_000 }, react: { gzip: 2000 } },
	};
	const head = {
		groups: ["web", "react"],
		artifacts: { web: { gzip: 310_000 }, react: { gzip: 4000 } },
	};
	const unapproved = judgeSizes(base, head, config, false);
	const approved = judgeSizes(base, head, config, true);

	assert.deepEqual(unapproved.map((row) => row.verdict), [
		"over budget",
		"grew",
	]);
	assert.deepEqual(approved.map((row) => row.verdict), [
		"over budget",
		"approved",
	]);
	assert.equal(failed(approved), true);
	assert.match(
		sizeReport(unapproved, { title: "t", notes: [] }),
		/❌ 2 of 2 artifacts failed/,
	);
});

test("a group the head didn't rebuild keeps the base's sizes", () => {
	const base = {
		groups: ["web", "react"],
		artifacts: { web: { gzip: 290_000 }, react: { gzip: 2000 } },
	};
	const rows = judgeSizes(
		base,
		{ groups: ["react"], artifacts: { react: { gzip: 2000 } } },
		config,
		false,
	);

	assert.deepEqual(rows.map((row) => [row.name, row.reused, row.verdict]), [[
		"web",
		true,
		"ok",
	], ["react", false, "ok"]]);
	assert.equal(failed(rows), false);
});

test("a rebuilt group that no longer produces its artifact fails", () => {
	const rows = judgeSizes(
		undefined,
		{ groups: ["web"], artifacts: {} },
		config,
		false,
	);

	assert.deepEqual(rows.map((row) => row.verdict), ["missing"]);
	assert.equal(failed(rows), true);
});

test("without a base, only budgets apply", () => {
	const rows = judgeSizes(
		undefined,
		{ groups: ["web"], artifacts: { web: { gzip: 1 } } },
		config,
		false,
	);

	assert.deepEqual(rows.map((row) => row.verdict), ["new"]);
});

test("groups come after the groups they need", () => {
	assert.deepEqual(withNeeds(config.groups, ["react"]), [
		"headless",
		"web",
		"react",
	]);
	assert.deepEqual(withNeeds(config.groups, ["web", "headless"]), [
		"headless",
		"web",
	]);
});
