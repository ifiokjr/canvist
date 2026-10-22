/**
 * How long the synchronous calls from JavaScript into the native editor
 * take: the TurboModule hop, the Rust call, and the JSON either side.
 * Keystrokes never cross into JavaScript, so this is the cost a toolbar or
 * an app's own commands add, not typing's.
 */
import type { Editor, StarterKit } from "@canvist/react-native";

/** One kind of call's timings, in microseconds. */
export interface BenchmarkRow {
	readonly name: string;
	readonly p50: number;
	readonly p95: number;
	readonly budget: number;
}

export interface BenchmarkResult {
	readonly rows: readonly BenchmarkRow[];
	readonly withinBudget: boolean;
}

const SAMPLES = 200;

/**
 * Budgets for p95, in microseconds, on a developer's simulator or emulator.
 * A toolbar reads several queries after every change, so a query must stay
 * well under a millisecond, and a command well under a frame.
 */
const BUDGETS = {
	query: 1_000,
	command: 4_000,
	insertText: 4_000,
	export: 4_000,
} as const;

/**
 * Time each kind of call on `editor`, then put its document back. Budgets
 * are multiplied by `slack`, which a shared CI simulator or emulator sets,
 * as the native performance tests read `CANVIST_PERF_SLACK`.
 */
export function benchmark(
	editor: Editor<StarterKit>,
	slack = 1,
): BenchmarkResult {
	const original = editor.export("json");
	editor.selectAll();

	const rows: BenchmarkRow[] = [
		row("query", BUDGETS.query * slack, () => editor.query.isBold()),
		row("command", BUDGETS.command * slack, () => editor.commands.toggleBold()),
		row("export", BUDGETS.export * slack, () => editor.export("markdown")),
	];

	editor.select({ path: [0], offset: 0 });
	rows.push(
		row("insertText", BUDGETS.insertText * slack, () => editor.insertText("x")),
	);
	editor.setContent({ format: "json", input: original });

	return {
		rows,
		withinBudget: rows.every((timing) => timing.p95 <= timing.budget),
	};
}

function row(name: string, budget: number, call: () => unknown): BenchmarkRow {
	const samples = new Float64Array(SAMPLES);

	for (let index = 0; index < SAMPLES; index++) {
		const start = performance.now();
		call();
		samples[index] = (performance.now() - start) * 1_000;
	}

	samples.sort();

	return {
		name,
		p50: samples[Math.floor(SAMPLES * 0.5)] ?? 0,
		p95: samples[Math.floor(SAMPLES * 0.95)] ?? 0,
		budget,
	};
}
