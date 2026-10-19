/**
 * Summarises Lighthouse CI reports as a Markdown table: the median of each
 * page's runs, per preset. Prints it, and appends it to the GitHub Actions
 * job summary when `GITHUB_STEP_SUMMARY` is set.
 *
 *   deno run --allow-read --allow-env --allow-write scripts/lighthouse-summary.ts
 *
 * Reads `lighthouse-reports/desktop` and `lighthouse-reports/mobile`, which the
 * `lighthouse:desktop` and `lighthouse:mobile` tasks fill.
 */
import { walk } from "@std/fs/walk";
import { fromFileUrl, join } from "@std/path";

const ROOT = join(
	fromFileUrl(new URL("..", import.meta.url)),
	"lighthouse-reports",
);
const PRESETS = ["desktop", "mobile"] as const;
const CATEGORIES = [
	"performance",
	"accessibility",
	"best-practices",
	"seo",
] as const;

/** The targets from the website brief, also asserted in the lighthouserc files. */
const TARGET = { score: 95, lcpMs: 1500, cls: 0 };

/** The parts of a Lighthouse result this script reads. */
interface LighthouseResult {
	requestedUrl: string;
	categories: Record<(typeof CATEGORIES)[number], { score: number | null }>;
	audits: Record<string, { numericValue?: number }>;
}

interface Row {
	page: string;
	scores: number[];
	lcpMs: number;
	cls: number;
	tbtMs: number;
}

function median(values: number[]): number {
	const sorted = [...values].sort((a, b) => a - b);
	const middle = Math.floor(sorted.length / 2);
	const value = sorted.length % 2 === 0
		? ((sorted[middle - 1] ?? 0) + (sorted[middle] ?? 0)) / 2
		: sorted[middle];
	return value ?? Number.NaN;
}

function audit(result: LighthouseResult, id: string): number {
	const value = result.audits[id]?.numericValue;
	if (value === undefined) {
		throw new Error(`${result.requestedUrl} has no ${id} audit`);
	}
	return value;
}

async function readPreset(preset: string): Promise<Row[]> {
	const byPage = new Map<string, LighthouseResult[]>();
	try {
		for await (
			const entry of walk(join(ROOT, preset), {
				exts: [".json"],
				match: [/lhr-\d+\.json$/],
			})
		) {
			const result = JSON.parse(
				await Deno.readTextFile(entry.path),
			) as LighthouseResult;
			const page = new URL(result.requestedUrl).pathname;
			byPage.set(page, [...(byPage.get(page) ?? []), result]);
		}
	} catch (error) {
		if (error instanceof Deno.errors.NotFound) return [];
		throw error;
	}
	return [...byPage.entries()]
		.sort(([a], [b]) => a.localeCompare(b))
		.map(([page, runs]) => ({
			page,
			scores: CATEGORIES.map((category) =>
				median(
					runs.map((run) =>
						Math.round((run.categories[category].score ?? 0) * 100)
					),
				)
			),
			lcpMs: median(runs.map((run) => audit(run, "largest-contentful-paint"))),
			cls: median(runs.map((run) => audit(run, "cumulative-layout-shift"))),
			tbtMs: median(runs.map((run) => audit(run, "total-blocking-time"))),
		}));
}

const mark = (ok: boolean) => (ok ? "" : " ⚠️");

function table(preset: string, rows: Row[]): string {
	const lines = [
		`### Lighthouse: ${preset}`,
		"",
		"| Page | Performance | Accessibility | Best practices | SEO | LCP | CLS | TBT |",
		"| --- | --: | --: | --: | --: | --: | --: | --: |",
		...rows.map((row) =>
			[
				`\`${row.page}\``,
				...row.scores.map((score) => `${score}${mark(score >= TARGET.score)}`),
				`${Math.round(row.lcpMs)} ms${mark(row.lcpMs <= TARGET.lcpMs)}`,
				`${row.cls.toFixed(3)}${mark(row.cls <= TARGET.cls)}`,
				`${Math.round(row.tbtMs)} ms`,
			].join(" | ")
		).map((line) => `| ${line} |`),
		"",
	];
	return lines.join("\n");
}

const sections: string[] = [];
for (const preset of PRESETS) {
	const rows = await readPreset(preset);
	if (rows.length > 0) sections.push(table(preset, rows));
}
if (sections.length === 0) {
	throw new Error(
		`No Lighthouse reports in ${ROOT}; run the lighthouse task first.`,
	);
}

const markdown = [
	...sections,
	`Medians of each page's runs. ⚠️ marks a value outside the target: scores ${TARGET.score}+, LCP under ${
		TARGET.lcpMs / 1000
	} s, CLS ${TARGET.cls}. Mobile is Lighthouse's default profile (slow 4G, 4× slower CPU).`,
	"",
].join("\n");

console.log(markdown);
const summary = Deno.env.get("GITHUB_STEP_SUMMARY");
if (summary) await Deno.writeTextFile(summary, markdown, { append: true });
