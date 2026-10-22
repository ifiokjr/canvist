/**
 * Markdown tables for the job summary and the pull request comment.
 *
 * @module
 */
import { METRICS, type SizeRow, type Sizes } from "./size.ts";
import type { SpeedRow } from "./speed.ts";

const PERCENT = 100;
const KIBIBYTE = 1024;
const TIME_UNITS = [
	{ unit: "s", nanoseconds: 1e9 },
	{ unit: "ms", nanoseconds: 1e6 },
	{ unit: "µs", nanoseconds: 1e3 },
	{ unit: "ns", nanoseconds: 1 },
] as const;
const SIGNIFICANT_DIGITS = 4;
const COUNT_UNITS = [
	{ suffix: "G", size: 1e9 },
	{ suffix: "M", size: 1e6 },
	{ suffix: "K", size: 1e3 },
] as const;

const SPEED_MARKS: Readonly<Record<SpeedRow["verdict"], string>> = {
	regressed: "❌ regressed",
	improved: "🚀 improved",
	unchanged: "",
	new: "new",
	removed: "removed",
};

const SIZE_MARKS: Readonly<Record<SizeRow["verdict"], string>> = {
	"over budget": "❌ over budget",
	grew: "❌ grew",
	approved: "⚠️ growth approved",
	ok: "",
	new: "new",
	missing: "❌ not built",
};

export function formatDuration(nanoseconds: number | undefined): string {
	if (nanoseconds === undefined) return "—";

	const scale = TIME_UNITS.find((unit) => nanoseconds >= unit.nanoseconds) ??
		TIME_UNITS[TIME_UNITS.length - 1];

	return `${
		Number((nanoseconds / scale.nanoseconds).toPrecision(SIGNIFICANT_DIGITS))
	} ${scale.unit}`;
}

/** An instruction count such as `965.3 M`, with no unit for small ones. */
export function formatCount(count: number | undefined): string {
	if (count === undefined) return "—";

	const scale = COUNT_UNITS.find((unit) => count >= unit.size);

	return scale === undefined
		? `${count}`
		: `${Number((count / scale.size).toPrecision(3))} ${scale.suffix}`;
}

export function formatChange(change: number | undefined): string {
	if (change === undefined) return "—";

	const percent = (change * PERCENT).toFixed(1);

	return change > 0 ? `+${percent}%` : `${percent}%`;
}

export function formatBytes(bytes: number | undefined): string {
	if (bytes === undefined) return "—";

	if (bytes < KIBIBYTE) return `${bytes} B`;

	return `${(bytes / KIBIBYTE).toFixed(1)} KiB`;
}

function row(cells: readonly string[]): string {
	return `| ${cells.join(" | ")} |`;
}

function table(header: readonly string[], rows: readonly string[][]): string {
	const divider = header.map(() => "---");

	return [row(header), row(divider), ...rows.map(row)].join("\n");
}

function speedCells(benchmark: SpeedRow): string[] {
	const format = benchmark.unit === "instructions"
		? formatCount
		: formatDuration;

	return [
		`\`${benchmark.id}\``,
		format(benchmark.baseNs),
		format(benchmark.headNs),
		formatChange(benchmark.change),
		`±${benchmark.threshold}%`,
		SPEED_MARKS[benchmark.verdict],
	];
}

const SPEED_HEADER = ["Benchmark", "Base", "Head", "Change", "Threshold", ""];

/** What the speed check measured, with notable rows first. */
export function speedReport(
	rows: readonly SpeedRow[],
	context: { readonly title: string; readonly notes: readonly string[] },
): string {
	const notable = rows.filter((benchmark) => benchmark.verdict !== "unchanged");
	const regressed = rows.filter((benchmark) =>
		benchmark.verdict === "regressed"
	);
	const headline = regressed.length === 0
		? "✅ No benchmark regressed beyond its threshold."
		: `❌ ${regressed.length} of ${rows.length} benchmarks regressed beyond their thresholds.`;
	const sections = [`## ${context.title}`, headline, ...context.notes];

	if (notable.length > 0) {
		sections.push(table(SPEED_HEADER, notable.map(speedCells)));
	}

	sections.push(
		`<details><summary>All ${rows.length} benchmarks</summary>\n\n${
			table(SPEED_HEADER, rows.map(speedCells))
		}\n\n</details>`,
	);

	return `${sections.join("\n\n")}\n`;
}

function sizeCell(
	head: Sizes | undefined,
	base: Sizes | undefined,
	metric: keyof Sizes,
): string {
	const after = head?.[metric];
	const before = base?.[metric];

	if (after === undefined || before === undefined || after === before) {
		return formatBytes(after);
	}

	const change = formatChange(after / before - 1);

	return `${formatBytes(after)} (${change})`;
}

function budgetCell(budget: Sizes | undefined): string {
	const limits = METRICS.flatMap((metric) =>
		budget?.[metric] === undefined
			? []
			: [`${formatBytes(budget[metric])} ${metric}`]
	);

	return limits.length === 0 ? "—" : limits.join(", ");
}

function sizeCells(artifact: SizeRow): string[] {
	const name = artifact.reused ? `${artifact.name} (unchanged)` : artifact.name;

	return [
		name,
		formatBytes(artifact.base?.gzip ?? artifact.base?.raw),
		...METRICS.map((metric) => sizeCell(artifact.head, artifact.base, metric)),
		budgetCell(artifact.budget),
		SIZE_MARKS[artifact.verdict],
	];
}

const SIZE_HEADER = [
	"Artifact",
	"Base (gzip)",
	"Raw",
	"Gzip",
	"Brotli",
	"Budget",
	"",
];

/** Every artifact's sizes, with the change since the base in brackets. */
export function sizeReport(
	rows: readonly SizeRow[],
	context: { readonly title: string; readonly notes: readonly string[] },
): string {
	const failing = rows.filter((artifact) =>
		SIZE_MARKS[artifact.verdict].startsWith("❌")
	);
	const headline = failing.length === 0
		? "✅ Every artifact is within its budget and growth threshold."
		: `❌ ${failing.length} of ${rows.length} artifacts failed.`;
	const sections = [
		`## ${context.title}`,
		headline,
		...context.notes,
		table(SIZE_HEADER, rows.map(sizeCells)),
	];

	return `${sections.join("\n\n")}\n`;
}
