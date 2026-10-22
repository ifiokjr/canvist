/**
 * Reads the JSON summaries a gungraun bench binary prints with
 * `--output-format=json`: one object per benchmark, with its callgrind
 * instruction count (`Ir`) under `profiles[0].data.total.metrics`.
 *
 * gungraun, iai-callgrind's new name, runs each benchmark exactly once
 * under callgrind, so its counts are exact: the same binary reports the
 * same number on every machine and every run. That is what makes it the
 * bench gate's metric where wall time is code-layout noise; see
 * `docs/src/guides/performance.md`.
 *
 * @module
 */

interface GungraunMetric {
	readonly values?: { readonly new?: unknown };
}

interface GungraunSummary {
	readonly group?: string;
	readonly id?: string;
	readonly profiles?: {
		readonly data?: {
			readonly total?: { readonly metrics?: Record<string, GungraunMetric> };
		};
	}[];
}

/** The JSON documents in gungraun's output: one summary per benchmark. */
function summaries(output: string): GungraunSummary[] {
	const text = output.trim();

	try {
		const parsed: unknown = JSON.parse(text);

		return Array.isArray(parsed) ? parsed : [parsed];
	} catch {
		// The summaries stream one object per line.
		return text
			.split("\n")
			.filter((line) => line.startsWith("{"))
			.map((line) => JSON.parse(line) as GungraunSummary);
	}
}

/**
 * Every benchmark's instruction count in one gungraun run, keyed by
 * `<suite>/<group>/<id>`, the same shape divan's ids take.
 */
export function parseGungraun(
	suite: string,
	output: string,
): Map<string, number> {
	const counts = new Map<string, number>();

	for (const summary of summaries(output)) {
		const id = summary.id;
		const group = summary.group;

		if (typeof id !== "string" || typeof group !== "string") {
			throw new Error(
				`${suite} printed a gungraun summary with no benchmark id`,
			);
		}

		const metrics = summary.profiles?.[0]?.data?.total?.metrics;
		const count = metrics?.["Ir"]?.values?.new;

		if (typeof count !== "number") {
			throw new Error(
				`${suite}/${group}/${id} printed no callgrind instruction count`,
			);
		}

		counts.set([suite, group, id].join("/"), count);
	}

	if (counts.size === 0) {
		throw new Error(`${suite} printed no gungraun summaries`);
	}

	return counts;
}
