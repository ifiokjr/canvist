/**
 * Measures shipped artifacts and judges them against budgets and the base.
 *
 * Builds are deterministic, so unlike timings a size needs one measurement
 * and no noise margin. An artifact fails when it is over its budget, or when
 * it grew by more than the growth threshold since the base and the pull
 * request has no approval label. Budgets need an edit to `size.json`; the
 * label only waives growth.
 *
 * @module
 */
import { globSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";

import type { Inputs } from "./affected.ts";

export const METRICS = ["raw", "gzip", "brotli"] as const;
export type Metric = (typeof METRICS)[number];
export type Sizes = Partial<Record<Metric, number>>;

/** How an artifact group is built. */
export interface Group extends Inputs {
	/** Groups whose builds this one's build reads. */
	readonly needs?: readonly string[];
	/**
	 * Shell commands, each run from `cwd` relative to the checkout. A group
	 * without any is built elsewhere (the native workflow) and only measured.
	 */
	readonly build?: readonly { readonly cwd: string; readonly run: string }[];
}

/** One shipped artifact: the files an app or a platform downloads. */
export interface Artifact {
	readonly name: string;
	readonly group: string;
	/** Globs relative to the checkout; the artifact is their total. */
	readonly files: readonly string[];
	/** Defaults to every metric. Brotli is slow on multi-megabyte files. */
	readonly metrics?: readonly Metric[];
	readonly budget?: Sizes;
}

/** `tools/bench/size.json`. */
export interface SizeConfig {
	/** Gzipped growth fails when over both this percentage and `growthBytes`. */
	readonly growthPercent: number;
	readonly growthBytes: number;
	readonly approvalLabel: string;
	/** Changes to these paths rebuild every group. */
	readonly globalPaths: readonly string[];
	readonly groups: Readonly<Record<string, Group>>;
	readonly artifacts: readonly Artifact[];
}

/** What one checkout measured: the groups it built and every artifact. */
export interface Measurement {
	readonly groups: readonly string[];
	readonly artifacts: Readonly<Record<string, Sizes>>;
}

export type SizeVerdict =
	| "over budget"
	| "grew"
	| "approved"
	| "ok"
	| "new"
	| "missing";

export interface SizeRow {
	readonly name: string;
	readonly base: Sizes | undefined;
	readonly head: Sizes | undefined;
	readonly budget: Sizes | undefined;
	/** Whether the head reuses the base's sizes because nothing changed. */
	readonly reused: boolean;
	readonly verdict: SizeVerdict;
}

const GZIP_LEVEL = 9;
const BROTLI_QUALITY = 11;
const PERCENT = 100;
const FAILING: readonly SizeVerdict[] = ["over budget", "grew", "missing"];

function compressedSize(bytes: Buffer, metric: Metric): number {
	if (metric === "gzip") return gzipSync(bytes, { level: GZIP_LEVEL }).length;

	if (metric === "brotli") {
		const params = { [constants.BROTLI_PARAM_QUALITY]: BROTLI_QUALITY };

		return brotliCompressSync(bytes, { params }).length;
	}

	return bytes.length;
}

/**
 * An artifact's total size per metric, each file compressed on its own as a
 * server or an archive would. Undefined when no file matches.
 */
export function measureArtifact(
	root: string,
	artifact: Artifact,
): Sizes | undefined {
	const files = artifact.files
		.flatMap((pattern) => globSync(pattern, { cwd: root, withFileTypes: true }))
		.filter((entry) => entry.isFile())
		.map((entry) => join(entry.parentPath, entry.name));
	const unique = [...new Set(files)].sort();

	if (unique.length === 0) return undefined;

	const sizes: Sizes = {};
	const contents = unique.map((file) => readFileSync(file));

	for (const metric of artifact.metrics ?? METRICS) {
		const total = contents.reduce(
			(sum, bytes) => sum + compressedSize(bytes, metric),
			0,
		);
		sizes[metric] = total;
	}

	return sizes;
}

/** Measures every artifact of `groups` in the checkout at `root`. */
export function measure(
	root: string,
	config: SizeConfig,
	groups: readonly string[],
): Measurement {
	const artifacts: Record<string, Sizes> = {};

	for (const artifact of config.artifacts) {
		if (!groups.includes(artifact.group)) continue;

		const sizes = measureArtifact(root, artifact);

		if (sizes !== undefined) artifacts[artifact.name] = sizes;
	}

	return { groups, artifacts };
}

function overBudget(sizes: Sizes, budget: Sizes | undefined): boolean {
	return METRICS.some((metric) => {
		const limit = budget?.[metric];
		const size = sizes[metric];

		return limit !== undefined && size !== undefined && size > limit;
	});
}

/** Whether gzipped growth from `base` to `head` is over the threshold. */
export function grewTooMuch(
	base: Sizes,
	head: Sizes,
	config: SizeConfig,
): boolean {
	const metric: Metric = head.gzip === undefined ? "raw" : "gzip";
	const before = base[metric];
	const after = head[metric];

	if (before === undefined || after === undefined) return false;

	const growth = after - before;

	return growth > config.growthBytes &&
		growth > (before * config.growthPercent) / PERCENT;
}

/**
 * The first rule that applies wins: a missing artifact, then a budget, which
 * no label waives, then growth, which the approval label waives.
 */
function verdictFor(
	row: Omit<SizeRow, "verdict">,
	config: SizeConfig,
	approved: boolean,
): SizeVerdict {
	if (row.head === undefined) return "missing";

	if (overBudget(row.head, row.budget)) return "over budget";

	if (row.base === undefined) return "new";

	if (!grewTooMuch(row.base, row.head, config)) return "ok";

	return approved ? "approved" : "grew";
}

/**
 * Judges every artifact the head or the base measured. An artifact whose
 * group the head didn't rebuild, because nothing it's built from changed,
 * keeps the base's sizes.
 */
export function judgeSizes(
	base: Measurement | undefined,
	head: Measurement,
	config: SizeConfig,
	approved: boolean,
): SizeRow[] {
	const rows: SizeRow[] = [];

	for (const artifact of config.artifacts) {
		const baseSizes = base?.artifacts[artifact.name];
		const rebuilt = head.groups.includes(artifact.group);

		if (!rebuilt && baseSizes === undefined) continue;

		const reused = !rebuilt;
		const headSizes = reused ? baseSizes : head.artifacts[artifact.name];

		const row = {
			name: artifact.name,
			base: baseSizes,
			head: headSizes,
			budget: artifact.budget,
			reused,
		};
		rows.push({ ...row, verdict: verdictFor(row, config, approved) });
	}

	return rows;
}

/**
 * Whether any row fails the check. A configured artifact that its group's build
 * no longer produces fails too, so `size.json` stays in step with the builds.
 */
export function failed(rows: readonly SizeRow[]): boolean {
	return rows.some((row) => FAILING.includes(row.verdict));
}

/** `names` and every group they need, each after the groups it needs. */
export function withNeeds(
	groups: Readonly<Record<string, Group>>,
	names: readonly string[],
): string[] {
	const ordered: string[] = [];
	const visit = (name: string): void => {
		if (ordered.includes(name)) return;

		for (const need of groups[name]?.needs ?? []) visit(need);

		ordered.push(name);
	};

	for (const name of names) visit(name);

	return ordered;
}
