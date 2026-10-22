/**
 * Judges benchmark timings from the base and head of a pull request.
 *
 * Each bench binary runs several times per revision, alternating base and
 * head on the same runner. One run gives one median per benchmark, so each
 * benchmark ends up with a handful of medians per revision. A benchmark
 * regresses only when both hold:
 *
 * - the median of the head's medians is slower than the base's by more than
 *   the benchmark's threshold, and
 * - every head run is slower than every base run, so the two revisions'
 *   ranges don't overlap.
 *
 * The second rule is what keeps runner noise from failing a build: a noisy
 * neighbour slows one run, not all five of one revision and none of the
 * other.
 *
 * @module
 */

/** One bench binary: a `[[bench]]` target and the features it needs. */
export interface Suite {
	readonly package: string;
	readonly bench: string;
	readonly features?: readonly string[];
	/**
	 * `instructions` compares callgrind instruction counts, which a
	 * bench's own code layout cannot move, instead of wall time. The
	 * default, wall time, is what divan measures.
	 */
	readonly metric?: "instructions";
}

/** `tools/bench/speed.json`. */
export interface SpeedConfig {
	/** How many times each bench binary runs per revision. */
	readonly rounds: number;
	/** How many more times a suite with a suspected regression runs. */
	readonly confirmRounds: number;
	/** The slowdown, in percent, a benchmark may show before it fails. */
	readonly threshold: number;
	/** Per-benchmark thresholds, keyed by id or by a pattern with `*`. */
	readonly thresholds: Readonly<Record<string, number>>;
	/** Changes to these paths run every suite. */
	readonly globalPaths: readonly string[];
	readonly suites: readonly Suite[];
}

/** Every run's median for one benchmark, in its suite's metric: one
 * nanosecond count for a wall-time suite, one instruction count for an
 * instruction-count suite. */
export interface Samples {
	readonly base: readonly number[];
	readonly head: readonly number[];
}

export type SpeedVerdict =
	| "regressed"
	| "improved"
	| "unchanged"
	| "new"
	| "removed";

/** What a suite's numbers mean, for the report's columns. */
export type SpeedUnit = "time" | "instructions";

export interface SpeedRow {
	readonly id: string;
	readonly unit: SpeedUnit;
	readonly baseNs: number | undefined;
	readonly headNs: number | undefined;
	/** Head over base, minus one: `0.1` is 10% more. */
	readonly change: number | undefined;
	/** In percent. */
	readonly threshold: number;
	readonly verdict: SpeedVerdict;
}

const PERCENT = 100;

export function median(values: readonly number[]): number {
	const sorted = [...values].sort((left, right) => left - right);
	const middle = Math.floor(sorted.length / 2);

	if (sorted.length === 0) throw new Error("the median of nothing");

	return sorted.length % 2 === 1
		? sorted[middle]!
		: (sorted[middle - 1]! + sorted[middle]!) / 2;
}

/** Whether `pattern`, where `*` matches anything, matches all of `id`. */
function matches(pattern: string, id: string): boolean {
	const escaped = pattern.split("*").map((part) =>
		part.replace(/[.+?^${}()|[\]\\]/g, "\\$&")
	);

	return new RegExp(`^${escaped.join(".*")}$`).test(id);
}

/**
 * The threshold for one benchmark: its own entry, else the longest pattern
 * that matches it, else the default.
 */
export function thresholdFor(id: string, config: SpeedConfig): number {
	const patterns = Object.keys(config.thresholds)
		.filter((pattern) => matches(pattern, id))
		.sort((left, right) => right.length - left.length);
	const best = patterns[0];

	return best === undefined ? config.threshold : config.thresholds[best]!;
}

function verdictFor(
	samples: Samples,
	change: number,
	threshold: number,
): SpeedVerdict {
	const limit = threshold / PERCENT;
	const separated = Math.min(...samples.head) > Math.max(...samples.base);
	const improvedApart = Math.max(...samples.head) < Math.min(...samples.base);

	if (change > limit && separated) return "regressed";

	if (change < -limit && improvedApart) return "improved";

	return "unchanged";
}

/** Compares one benchmark's runs on the base and the head. */
export function judge(
	id: string,
	unit: SpeedUnit,
	samples: Samples,
	threshold: number,
): SpeedRow {
	const baseNs = samples.base.length === 0 ? undefined : median(samples.base);
	const headNs = samples.head.length === 0 ? undefined : median(samples.head);

	if (baseNs === undefined || headNs === undefined) {
		const verdict = baseNs === undefined ? "new" : "removed";

		return {
			id,
			unit,
			baseNs,
			headNs,
			change: undefined,
			threshold,
			verdict,
		};
	}

	const change = headNs / baseNs - 1;
	const verdict = verdictFor(samples, change, threshold);

	return { id, unit, baseNs, headNs, change, threshold, verdict };
}

/** Judges every benchmark, sorted by id. */
export function judgeAll(
	samples: ReadonlyMap<string, Samples>,
	config: SpeedConfig,
): SpeedRow[] {
	const units = new Map(config.suites.map((suite): [string, SpeedUnit] => [
		suite.bench,
		suite.metric === "instructions" ? "instructions" : "time",
	]));

	return [...samples.keys()].sort().map((id) =>
		judge(
			id,
			units.get(id.split("/")[0] ?? "") ?? "time",
			samples.get(id)!,
			thresholdFor(id, config),
		)
	);
}
