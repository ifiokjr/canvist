/**
 * Reads the tree table divan prints after a bench binary runs.
 *
 * divan 0.1 has no machine-readable output, so this parses the table it
 * prints with `--color never`:
 *
 * ```text
 * model             fastest   │ slowest   │ median    │ mean      │ samples │ iters
 * ├─ type_char                │           │           │           │         │
 * │  ├─ 100         254.8 ns  │ 700.2 ns  │ 273.1 ns  │ 284.8 ns  │ 100     │ 1600
 * ╰─ create         54.87 µs  │ 84.54 µs  │ 55.62 µs  │ 56.53 µs  │ 100     │ 100
 * ```
 *
 * The name column is padded to one width for the whole table, so the header's
 * `fastest` marks where every row's statistics start. Tree prefixes are three
 * characters per level, which gives each row's depth.
 *
 * @module
 */

const NANOSECONDS_PER_UNIT: Readonly<Record<string, number>> = {
	ps: 1e-3,
	ns: 1,
	"µs": 1e3,
	us: 1e3,
	ms: 1e6,
	s: 1e9,
};

const HEADER = /\bfastest\s+│\s+slowest\s+│\s+median\b/;
const BRANCHES = ["├─ ", "╰─ "] as const;
const TREE_INDENT = 3;
const MEDIAN_COLUMN = 2;

/** A divan duration such as `273.1 ns` or `1.2 µs`, in nanoseconds. */
export function parseDuration(text: string): number | undefined {
	const match = /^(\d+(?:\.\d+)?) (ps|ns|µs|us|ms|s)$/.exec(text.trim());

	if (match === null) return undefined;

	const [, value, unit] = match;
	const scale = unit === undefined ? undefined : NANOSECONDS_PER_UNIT[unit];

	return scale === undefined ? undefined : Number(value) * scale;
}

/** Where a row's tree branch starts, or -1 for rows that name nothing. */
function branchIndex(name: string): number {
	const indices = BRANCHES.map((branch) => name.indexOf(branch));

	return Math.max(...indices);
}

/**
 * Every benchmark's median time in one divan run, keyed by
 * `<suite>/<function>/<argument>`.
 *
 * Rows without a tree branch are throughput or allocation details of the row
 * above them, and are skipped.
 */
export function parseDivan(suite: string, output: string): Map<string, number> {
	const lines = output.split("\n");
	const header = lines.find((line) => HEADER.test(line));

	if (header === undefined) {
		throw new Error(`${suite} printed no divan table`);
	}

	const nameWidth = header.indexOf("fastest");
	const path: string[] = [];
	const medians = new Map<string, number>();

	for (const line of lines.slice(lines.indexOf(header) + 1)) {
		const name = line.slice(0, nameWidth);
		const branch = branchIndex(name);

		if (branch === -1) continue;

		path.length = branch / TREE_INDENT;
		path.push(name.slice(branch + TREE_INDENT).trim());
		const columns = line.slice(nameWidth).split("│");
		const median = parseDuration(columns[MEDIAN_COLUMN] ?? "");

		if (median !== undefined) medians.set([suite, ...path].join("/"), median);
	}

	return medians;
}
