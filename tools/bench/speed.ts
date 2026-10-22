/**
 * `bench / speed`: runs the divan benches on a pull request's base and head
 * on one runner and fails when one regresses beyond its threshold.
 *
 *   node tools/bench/speed.ts plan --base <rev> [--all]
 *   node tools/bench/speed.ts build --root <checkout> --suites "<names>" --out <dir>
 *   node tools/bench/speed.ts compare --base <dir> --head <dir> --suites "<names>"
 *
 * `plan` picks the suites the change can affect, `build` compiles their bench
 * binaries in a checkout, and `compare` runs both sets of binaries, alternating
 * base and head, and reports. `docs/src/guides/performance.md` explains the
 * thresholds and how to read the report.
 *
 * @module
 */
import { execFileSync, spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

import {
	changedFiles,
	isAffected,
	readWorkspace,
	type WorkspaceCrate,
} from "./lib/affected.ts";
import { loadSpeedConfig } from "./lib/config.ts";
import { parseDivan } from "./lib/divan.ts";
import { setOutput, upsertComment, writeSummary } from "./lib/github.ts";
import { parseGungraun } from "./lib/gungraun.ts";
import { speedReport } from "./lib/report.ts";
import { judgeAll, type SpeedRow, type Suite } from "./lib/speed.ts";

const config = loadSpeedConfig(new URL("./speed.json", import.meta.url));
const repository = fileURLToPath(new URL("../..", import.meta.url));
const OUTPUT_BUFFER = 256 * 1024 * 1024;

const { positionals, values } = parseArgs({
	allowPositionals: true,
	options: {
		all: { type: "boolean", default: false },
		base: { type: "string" },
		comment: { type: "string" },
		head: { type: "string" },
		out: { type: "string" },
		root: { type: "string" },
		rounds: { type: "string" },
		suites: { type: "string", default: "" },
		title: { type: "string", default: "bench / speed" },
	},
});

function option(name: "base" | "head" | "out" | "root"): string {
	const value = values[name];

	if (value === undefined) throw new Error(`--${name} is required`);

	return value;
}

function selectedSuites(): Suite[] {
	const names = values.suites.split(/\s+/).filter((name) => name !== "");

	return config.suites.filter((suite) => names.includes(suite.bench));
}

/** The suites a change since `--base` can affect; every suite with `--all`. */
function plan(): void {
	const workspace = readWorkspace(repository);
	const changed = values.all
		? []
		: changedFiles(repository, option("base"), "HEAD");
	const affected = config.suites.filter((suite) =>
		values.all || isAffected(workspace, changed, {
			crates: [suite.package],
			paths: config.globalPaths,
		})
	);
	const skipped = config.suites.filter((suite) => !affected.includes(suite));

	setOutput("suites", affected.map((suite) => suite.bench).join(" "));
	setOutput("skipped", skipped.map((suite) => suite.bench).join(" "));
	console.log(
		`affected: ${affected.map((suite) => suite.bench).join(", ") || "none"}`,
	);
}

function hasBench(workspace: readonly WorkspaceCrate[], suite: Suite): boolean {
	return workspace.some((crate) =>
		crate.name === suite.package && crate.benches.includes(suite.bench)
	);
}

interface CargoMessage {
	reason: string;
	executable?: string | null;
	target?: { name: string; kind: string[] };
}

/** The bench binary `cargo --message-format=json` reports for `bench`. */
function benchExecutable(output: string, bench: string): string {
	const executable = output
		.split("\n")
		.filter((line) => line.startsWith("{"))
		.map((line) => JSON.parse(line) as CargoMessage)
		.filter((message) => message.reason === "compiler-artifact")
		.find((message) =>
			message.target?.name === bench && message.target.kind.includes("bench")
		)
		?.executable;

	if (typeof executable !== "string") {
		throw new Error(`cargo built no binary for ${bench}`);
	}

	return executable;
}

/** Compiles one suite with `cargo bench --no-run` and returns the binary. */
function compile(root: string, suite: Suite): string {
	const features = (suite.features ?? []).map((feature) =>
		`${suite.package}/${feature}`
	);
	const args = [
		"bench",
		"--locked",
		"--no-run",
		"--message-format=json",
		"-p",
		suite.package,
		"--bench",
		suite.bench,
	];

	if (features.length > 0) args.push("--features", features.join(","));

	const output = execFileSync("cargo", args, {
		cwd: root,
		encoding: "utf8",
		maxBuffer: OUTPUT_BUFFER,
		stdio: ["ignore", "pipe", "inherit"],
	});
	return benchExecutable(output, suite.bench);
}

/**
 * Builds each suite on its own, so features one suite enables never change
 * another suite's binary: the base and the head always build the same thing.
 * A suite the checkout doesn't have yet is skipped.
 */
function build(): void {
	const root = resolve(option("root"));
	const out = resolve(option("out"));
	const workspace = readWorkspace(root);
	mkdirSync(out, { recursive: true });

	for (const suite of selectedSuites()) {
		if (!hasBench(workspace, suite)) {
			console.log(
				`${root} has no ${suite.package} bench ${suite.bench}; skipping`,
			);
			continue;
		}

		copyFileSync(compile(root, suite), join(out, suite.bench));
	}
}

/**
 * Runs one bench binary from its crate's directory, as `cargo bench` does.
 *
 * A gungraun binary may exit 3 after printing its summaries, because it
 * compared them with the run before and found a regression by its own
 * rules; this gate judges regressions itself, so any output that parses
 * is accepted.
 */
function runSuite(
	binary: string,
	suite: Suite,
	cwd: string,
): Map<string, number> {
	const instructions = suite.metric === "instructions";
	const args = instructions
		? ["--bench", "--output-format=json"]
		: ["--bench", "--color", "never"];
	const run = spawnSync(binary, args, {
		cwd,
		encoding: "utf8",
		maxBuffer: OUTPUT_BUFFER,
		stdio: ["ignore", "pipe", "inherit"],
	});
	const output = run.stdout ?? "";
	// Only a gungraun run's exit 3 is tolerated, and only with output.
	const tolerated = instructions && run.status === 3 && output !== "";

	if (run.error !== undefined || (run.status !== 0 && !tolerated)) {
		throw new Error(
			`${suite.bench} failed with exit ${
				run.status ?? run.signal ?? "unknown"
			}: ${run.stderr}`,
		);
	}

	return instructions
		? parseGungraun(suite.bench, output)
		: parseDivan(suite.bench, output);
}

type Side = "base" | "head";
type Recorded = Map<string, { base: number[]; head: number[] }>;

function record(
	samples: Recorded,
	side: Side,
	medians: Map<string, number>,
): void {
	for (const [id, nanoseconds] of medians) {
		const entry = samples.get(id) ?? { base: [], head: [] };
		entry[side].push(nanoseconds);
		samples.set(id, entry);
	}
}

/**
 * Runs `suites` `rounds` more times per side, adding to `samples`. The order
 * flips each round (base then head, head then base) so a runner that slows
 * down or warms up over the job penalises both sides equally. A suite that
 * counts instructions runs once per side: callgrind's counts are exact, so
 * repeating them measures nothing.
 */
function sample(
	samples: Recorded,
	suites: readonly Suite[],
	rounds: number,
): void {
	const directories: Record<Side, string> = {
		base: resolve(option("base")),
		head: resolve(option("head")),
	};
	const workspace = readWorkspace(repository);
	const crateDirectory = (suite: Suite): string =>
		join(
			repository,
			workspace.find((crate) => crate.name === suite.package)?.dir ?? ".",
		);

	for (let round = 0; round < rounds; round++) {
		const order: Side[] = round % 2 === 0 ? ["base", "head"] : ["head", "base"];

		for (const suite of suites) {
			if (suite.metric === "instructions" && round > 0) continue;

			const binaries = order
				.map((side) => ({ side, binary: join(directories[side], suite.bench) }))
				.filter(({ binary }) => existsSync(binary));

			for (const { side, binary } of binaries) {
				record(samples, side, runSuite(binary, suite, crateDirectory(suite)));
			}
		}

		console.log(
			`${suites.map((suite) => suite.bench).join(", ")}: round ${
				round + 1
			} of ${rounds} done`,
		);
	}
}

/** The suites that hold a benchmark judged as regressed. */
function suspects(rows: readonly SpeedRow[]): Suite[] {
	const regressed = rows.filter((row) => row.verdict === "regressed").map((
		row,
	) => row.id.split("/")[0]);

	return config.suites.filter((suite) => regressed.includes(suite.bench));
}

function notes(rounds: number, confirmed: readonly Suite[]): string[] {
	const skipped = config.suites.filter((suite) =>
		!selectedSuites().includes(suite)
	);
	const counting = selectedSuites().filter((suite) =>
		suite.metric === "instructions"
	);
	const lines = [
		`Each suite ran ${rounds} times per revision on this runner, alternating base and head. A time is the median of the runs' divan medians. A benchmark regresses only when it is slower by more than its threshold **and** every head run is slower than every base run.`,
	];

	if (counting.length > 0) {
		lines.push(
			`${
				counting.map((suite) => `\`${suite.bench}\``).join(", ")
			} ran once per revision under callgrind (valgrind): instruction counts are exact, the same binary always reports the same one, so code layout and the runner's CPU cannot move them.`,
		);
	}

	if (confirmed.length > 0) {
		lines.push(
			`Suspected regressions were run ${config.confirmRounds} more times and judged on every run: ${
				confirmed.map((suite) => `\`${suite.bench}\``).join(", ")
			}.`,
		);
	}

	if (skipped.length > 0) {
		lines.push(
			`Not affected by this change, so not run: ${
				skipped.map((suite) => `\`${suite.bench}\``).join(", ")
			}.`,
		);
	}

	return lines;
}

async function publish(markdown: string): Promise<void> {
	writeSummary(markdown);

	if (values.comment !== undefined) {
		await upsertComment(values.comment, markdown);
	}
}

/**
 * Runs and judges the selected suites. A suite with a suspected regression
 * runs `confirmRounds` more times and is judged again on all its runs, so a
 * burst of runner noise in the first rounds can't fail the check alone.
 */
async function compare(): Promise<void> {
	const rounds = values.rounds === undefined
		? config.rounds
		: Number(values.rounds);
	const samples: Recorded = new Map();
	sample(samples, selectedSuites(), rounds);
	const confirmed = suspects(judgeAll(samples, config));
	sample(samples, confirmed, config.confirmRounds);
	const rows = judgeAll(samples, config);

	if (values.out !== undefined) {
		writeFileSync(
			values.out,
			JSON.stringify(Object.fromEntries(samples), null, "\t"),
		);
	}

	await publish(
		speedReport(rows, { title: values.title, notes: notes(rounds, confirmed) }),
	);

	if (rows.some((row) => row.verdict === "regressed")) process.exitCode = 1;
}

/** Writes the report when no suite was affected, so the check still passes visibly. */
async function skip(): Promise<void> {
	await publish(
		speedReport([], { title: values.title, notes: notes(0, []).slice(1) }),
	);
}

const commands: Record<string, () => void | Promise<void>> = {
	plan,
	build,
	compare,
	skip,
};
const command = commands[positionals[0] ?? ""];

if (command === undefined) {
	throw new Error(
		`usage: speed.ts <${Object.keys(commands).join("|")}> [options]`,
	);
}

await command();
