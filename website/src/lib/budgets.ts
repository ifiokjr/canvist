/**
 * Reads the "Performance budgets" table from `docs/src/v2/architecture.md` at
 * build time, so the site's numbers are the repository's numbers. The table
 * is enforced by `tests/perf_budgets.rs` in each crate.
 */
import { readFileSync } from "node:fs";

/** A duration in seconds, with the text it was parsed from. */
export interface Measure {
	seconds: number;
	text: string;
}

export interface Budget {
	metric: string;
	budgetText: string;
	todayText: string;
	/** Present when both cells hold one duration each. */
	budget?: Measure;
	today?: Measure;
}

const UNITS: Record<string, number> = {
	"µs": 1e-6,
	"us": 1e-6,
	"ms": 1e-3,
	"s": 1,
	"ns": 1e-9,
};

/** Parses `0.29 µs`, `< 2 ms` or `24 ms`; anything else is not a single duration. */
export function parseDuration(cell: string): Measure | undefined {
	const match = /^<?\s*([\d.,]+)\s*(µs|us|ms|ns|s)$/.exec(cell.trim());
	if (!match) return undefined;
	const [, value = "", unit = ""] = match;
	const factor = UNITS[unit];
	if (factor === undefined) return undefined;
	return {
		seconds: Number(value.replace(/,/g, "")) * factor,
		text: `${value} ${unit}`,
	};
}

const TRAILING_UNIT = /(µs|us|ms|ns|s)\s*$/;

/**
 * The first duration in a cell that may hold several: `83 µs / 1.4 ms` gives
 * 83 µs, and `19 / 7 / 2.4 ms` gives 19 ms, borrowing the unit written once
 * at the end.
 */
export function firstDuration(cell: string): Measure | undefined {
	const parts = cell.split("/").map((part) => part.trim());
	const first = parts[0] ?? "";
	const unit = TRAILING_UNIT.exec(parts.at(-1) ?? "")?.[1];
	return parseDuration(
		unit !== undefined && !TRAILING_UNIT.test(first)
			? `${first} ${unit}`
			: first,
	);
}

function stripFootnotes(cell: string): string {
	return cell.replace(/[¹²³⁴]/g, "").trim();
}

export function parseBudgetTable(markdown: string): Budget[] {
	const section = markdown.split(/^## Performance budgets\s*$/m)[1];
	if (section === undefined) {
		throw new Error("architecture.md has no '## Performance budgets' section");
	}
	const rows = section
		.split("\n")
		.filter((line) => line.startsWith("|"))
		.slice(2); // header and separator
	if (rows.length === 0) {
		throw new Error("architecture.md's budget table is empty");
	}

	return rows.map((row) => {
		const cells = row.split("|").slice(1, -1).map((cell) => cell.trim());
		const [metric = "", budgetText = "", todayText = ""] = cells;
		const entry: Budget = {
			metric: stripFootnotes(metric),
			budgetText,
			todayText,
		};
		// Paired cells ("83 µs / 1.4 ms") chart their first value; the text keeps all.
		const budget = firstDuration(budgetText);
		const today = firstDuration(todayText);
		if (budget && today) {
			entry.budget = budget;
			entry.today = today;
		}
		return entry;
	});
}

export type MeasuredBudget =
	& Budget
	& Required<Pick<Budget, "budget" | "today">>;

export interface BudgetTable {
	/** Rows with a measured duration today, ready to chart. */
	measured: MeasuredBudget[];
	/** Rows waiting on a milestone, such as the web host (M3). */
	pending: Budget[];
	/** Budgets that aren't durations, such as sizes and stress tests. */
	other: Budget[];
}

const WAITING = /\(M\d+\)/;

export function splitBudgets(rows: Budget[]): BudgetTable {
	const measured = rows.filter((row): row is MeasuredBudget =>
		row.budget !== undefined && row.today !== undefined
	);
	const rest = rows.filter((row) => !measured.includes(row as MeasuredBudget));
	return {
		measured,
		pending: rest.filter((row) => WAITING.test(row.todayText)),
		other: rest.filter((row) => !WAITING.test(row.todayText)),
	};
}

/** Reads the table from `docs/src/v2/architecture.md` at `path`. Called at build time. */
export function loadBudgets(path: string): BudgetTable {
	return splitBudgets(parseBudgetTable(readFileSync(path, "utf8")));
}

/** One frame at 60 Hz, the unit a reader already feels. */
export const FRAME_SECONDS = 1 / 60;

export function findBudget(table: BudgetTable, prefix: string): MeasuredBudget {
	const row = table.measured.find((entry) => entry.metric.startsWith(prefix));
	if (!row) throw new Error(`No measured budget row starts with "${prefix}"`);
	return row;
}
