import assert from "node:assert/strict";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
	closure,
	crateOf,
	isAffected,
	readWorkspace,
	within,
	type WorkspaceCrate,
} from "../lib/affected.ts";
import { loadSizeConfig, loadSpeedConfig } from "../lib/config.ts";

const workspace: WorkspaceCrate[] = [
	{ name: "model", dir: "crates/model", dependencies: [], benches: ["model"] },
	{
		name: "layout",
		dir: "crates/layout",
		dependencies: ["model"],
		benches: ["layout"],
	},
	{
		name: "paint",
		dir: "crates/paint",
		dependencies: ["layout"],
		benches: ["paint"],
	},
	{
		name: "paint_extra",
		dir: "crates/paint/extra",
		dependencies: [],
		benches: [],
	},
	{ name: "docs", dir: "docs", dependencies: [], benches: [] },
];

test("prefixes match whole path segments", () => {
	assert.equal(within("crates/model/src/lib.rs", "crates/model"), true);
	assert.equal(within("crates/model", "crates/model"), true);
	assert.equal(within("crates/model_extra/lib.rs", "crates/model"), false);
});

test("the closure follows path dependencies transitively", () => {
	assert.deepEqual([...closure(workspace, ["paint"])].sort(), [
		"layout",
		"model",
		"paint",
	]);
});

test("a file belongs to the innermost crate", () => {
	assert.equal(
		crateOf(workspace, "crates/paint/extra/src/lib.rs"),
		"paint_extra",
	);
	assert.equal(crateOf(workspace, "crates/paint/src/lib.rs"), "paint");
	assert.equal(crateOf(workspace, "readme.md"), undefined);
});

test("a change reaches the crates that depend on it, not the ones it depends on", () => {
	const model = ["crates/model/src/lib.rs"];
	const paint = ["crates/paint/src/lib.rs"];

	assert.equal(isAffected(workspace, model, { crates: ["paint"] }), true);
	assert.equal(isAffected(workspace, paint, { crates: ["model"] }), false);
	assert.equal(
		isAffected(workspace, ["docs/src/index.md"], { crates: ["paint"] }),
		false,
	);
	assert.equal(
		isAffected(workspace, ["Cargo.lock"], {
			crates: ["model"],
			paths: ["Cargo.lock"],
		}),
		true,
	);
	assert.equal(
		isAffected(workspace, ["packages/web/x.ts"], { paths: ["packages/web"] }),
		true,
	);
});

test("the checked-in configs load, and every suite names a real bench", () => {
	const speed = loadSpeedConfig(new URL("../speed.json", import.meta.url));
	const size = loadSizeConfig(new URL("../size.json", import.meta.url));
	const real = readWorkspace(
		fileURLToPath(new URL("../../..", import.meta.url)),
	);

	for (const suite of speed.suites) {
		const crate = real.find((candidate) => candidate.name === suite.package);

		assert.ok(
			crate?.benches.includes(suite.bench),
			`${suite.package} has no bench ${suite.bench}`,
		);
	}

	for (const group of Object.values(size.groups)) {
		for (const name of group.crates ?? []) {
			assert.ok(real.some((crate) => crate.name === name), `no crate ${name}`);
		}
	}
});
