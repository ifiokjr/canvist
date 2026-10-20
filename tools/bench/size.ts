/**
 * `bench / bundle size`: measures every shipped artifact raw, gzipped, and
 * brotli-compressed, and fails on a budget breach or on growth since the base.
 *
 *   node tools/bench/size.ts plan --base <rev> [--all]
 *   node tools/bench/size.ts build --root <checkout> --groups "<names>"
 *   node tools/bench/size.ts measure --root <checkout> --groups "<names>" --out <file>
 *   node tools/bench/size.ts compare --head <file> [--base <file>]
 *
 * `plan` picks the artifact groups the change can affect, `build` builds them
 * and the groups they need, `measure` records their sizes, and `compare`
 * judges the head against the base and the budgets in `size.json`. Without
 * `--base`, only budgets apply. `docs/src/guides/performance.md` explains how
 * to read the report and raise a budget.
 *
 * @module
 */
import { execSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

import { changedFiles, isAffected, readWorkspace } from "./lib/affected.ts";
import { loadSizeConfig } from "./lib/config.ts";
import {
	pullRequestLabels,
	setOutput,
	upsertComment,
	writeSummary,
} from "./lib/github.ts";
import { sizeReport } from "./lib/report.ts";
import {
	failed,
	judgeSizes,
	measure,
	type Measurement,
	withNeeds,
} from "./lib/size.ts";

const config = loadSizeConfig(new URL("./size.json", import.meta.url));
const repository = fileURLToPath(new URL("../..", import.meta.url));

const { positionals, values } = parseArgs({
	allowPositionals: true,
	options: {
		all: { type: "boolean", default: false },
		base: { type: "string" },
		groups: { type: "string", default: "" },
		head: { type: "string" },
		out: { type: "string" },
		comment: { type: "string" },
		root: { type: "string" },
		title: { type: "string", default: "bench / bundle size" },
	},
});

function option(name: "base" | "head" | "out" | "root"): string {
	const value = values[name];

	if (value === undefined) throw new Error(`--${name} is required`);

	return value;
}

/** The groups CI builds: those with build commands. */
function buildable(): string[] {
	return Object.keys(config.groups).filter((name) =>
		(config.groups[name]?.build ?? []).length > 0
	);
}

function requestedGroups(): string[] {
	return withNeeds(
		config.groups,
		values.groups.split(/\s+/).filter((name) => name in config.groups),
	);
}

/** The buildable groups a change since `--base` can affect, or all with `--all`. */
function plan(): void {
	const workspace = readWorkspace(repository);
	const changed = values.all
		? []
		: changedFiles(repository, option("base"), "HEAD");
	const affected = buildable().filter((name) => {
		const group = config.groups[name]!;
		const paths = [...config.globalPaths, ...(group.paths ?? [])];

		return values.all ||
			isAffected(workspace, changed, { crates: group.crates ?? [], paths });
	});

	setOutput("groups", affected.join(" "));
	console.log(`affected: ${affected.join(", ") || "none"}`);
}

/** Runs each requested group's build commands in the checkout at `--root`. */
function build(): void {
	const root = resolve(option("root"));

	for (const name of requestedGroups()) {
		for (const step of config.groups[name]?.build ?? []) {
			console.log(`::group::${name}: ${step.run}`);
			execSync(step.run, { cwd: join(root, step.cwd), stdio: "inherit" });
			console.log("::endgroup::");
		}
	}
}

function measureCommand(): void {
	const measurement = measure(
		resolve(option("root")),
		config,
		requestedGroups(),
	);

	mkdirSync(dirname(option("out")), { recursive: true });
	writeFileSync(option("out"), `${JSON.stringify(measurement, null, "\t")}\n`);
	console.log(JSON.stringify(measurement.artifacts, null, 2));
}

function readMeasurement(path: string): Measurement {
	return JSON.parse(readFileSync(path, "utf8")) as Measurement;
}

async function compare(): Promise<void> {
	const head = readMeasurement(option("head"));
	const base = values.base === undefined
		? undefined
		: readMeasurement(values.base);
	const approved = pullRequestLabels().includes(config.approvalLabel);
	const rows = judgeSizes(base, head, config, approved);
	const notes = [
		base === undefined
			? "No base measurement, so only budgets apply."
			: `Gzipped growth over ${config.growthPercent}% and ${config.growthBytes} bytes fails unless the pull request has the \`${config.approvalLabel}\` label. Budgets apply either way.`,
	];
	const markdown = sizeReport(rows, { title: values.title, notes });

	writeSummary(markdown);

	if (values.comment !== undefined) {
		await upsertComment(values.comment, markdown);
	}

	if (failed(rows)) process.exitCode = 1;
}

const commands: Record<string, () => void | Promise<void>> = {
	plan,
	build,
	measure: measureCommand,
	compare,
};
const command = commands[positionals[0] ?? ""];

if (command === undefined) {
	throw new Error(
		`usage: size.ts <${Object.keys(commands).join("|")}> [options]`,
	);
}

await command();
