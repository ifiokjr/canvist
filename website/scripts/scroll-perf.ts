/// <reference lib="dom" />

/**
 * Measures scrolling the landing page end to end, the way a reader does,
 * with real wheel events, on desktop and with the CPU throttled 4×.
 *
 *   deno run -A scripts/scroll-perf.ts [url] [out-dir]
 *
 * For each run it records, during the scroll only:
 * - every frame's interval, from requestAnimationFrame (p50, p95, max, and
 *   how many frames took longer than one 60 Hz frame);
 * - long tasks (over 50 ms) and long animation frames;
 * - a Chrome performance trace, saved to `<out-dir>/<run>.trace.json`, which
 *   opens in the DevTools Performance panel.
 *
 * Prints a Markdown table and exits non-zero when the budget is missed: p95
 * frame interval within one frame, and no long tasks while scrolling.
 */
import { chromium, type Page } from "@playwright/test";

const url = Deno.args[0] ?? "http://127.0.0.1:4321/";
const outDir = Deno.args[1] ?? "perf-reports";
const FRAME_MS = 1000 / 60;
/** rAF timestamps jitter by a fraction of a millisecond; a missed frame is ~33 ms. */
const FRAME_BUDGET_MS = FRAME_MS + 1;

interface Run {
	name: string;
	cpuSlowdown: number;
}

const RUNS: Run[] = [
	{ name: "desktop", cpuSlowdown: 1 },
	{ name: "desktop-4x-cpu", cpuSlowdown: 4 },
];

interface Result {
	run: string;
	frames: number;
	p50: number;
	p95: number;
	max: number;
	slowFrames: number;
	longTasks: number;
	longestTask: number;
	longFrames: number;
}

interface PageSamples {
	intervals: number[];
	longTasks: number[];
	longFrames: number[];
}

/** Starts sampling inside the page: frame intervals, long tasks, long animation frames. */
async function startSampling(page: Page): Promise<void> {
	await page.evaluate(() => {
		const state = {
			intervals: [] as number[],
			longTasks: [] as number[],
			longFrames: [] as number[],
			running: true,
			// Long work counts only while the page is being scrolled.
			end: Number.POSITIVE_INFINITY,
		};
		(globalThis as unknown as { __perf: typeof state }).__perf = state;
		let last = performance.now();
		const frame = (now: number) => {
			if (now >= state.end) return;
			state.intervals.push(now - last);
			last = now;
			if (state.running) requestAnimationFrame(frame);
		};
		requestAnimationFrame((now) => {
			last = now;
			requestAnimationFrame(frame);
		});
		new PerformanceObserver((list) => {
			for (const entry of list.getEntries()) {
				if (entry.startTime < state.end) state.longTasks.push(entry.duration);
			}
		}).observe({ type: "longtask" });
		try {
			new PerformanceObserver((list) => {
				for (const entry of list.getEntries()) {
					if (entry.startTime < state.end) {
						state.longFrames.push(entry.duration);
					}
				}
			}).observe({ type: "long-animation-frame" });
		} catch {
			// Long animation frames are Chromium-only; the other samples still stand.
		}
	});
}

async function stopSampling(page: Page): Promise<PageSamples> {
	return await page.evaluate(() => {
		const state =
			(globalThis as unknown as { __perf: PageSamples & { running: boolean } })
				.__perf;
		state.running = false;
		return {
			intervals: state.intervals,
			longTasks: state.longTasks,
			longFrames: state.longFrames,
		};
	});
}

function percentile(values: number[], p: number): number {
	const sorted = [...values].sort((a, b) => a - b);
	return sorted[
		Math.min(sorted.length - 1, Math.max(0, Math.ceil(p * sorted.length) - 1))
	] ?? 0;
}

async function measure(run: Run): Promise<Result> {
	const browser = await chromium.launch({ channel: "chromium" });
	const page = await browser.newPage({
		viewport: { width: 1440, height: 900 },
	});
	const cdp = await page.context().newCDPSession(page);
	await page.goto(url, { waitUntil: "networkidle" });
	// Let the hero finish painting before scrolling, as a reader would.
	await page.waitForTimeout(3000);
	if (run.cpuSlowdown > 1) {
		await cdp.send("Emulation.setCPUThrottlingRate", { rate: run.cpuSlowdown });
	}

	const chunks: string[] = [];
	cdp.on("Tracing.dataCollected", (event: { value: unknown[] }) => {
		for (const value of event.value) chunks.push(JSON.stringify(value));
	});
	const tracingDone = new Promise<void>((resolve) =>
		cdp.once("Tracing.tracingComplete", () => resolve())
	);
	await cdp.send("Tracing.start", {
		categories:
			"devtools.timeline,disabled-by-default-devtools.timeline,disabled-by-default-devtools.timeline.frame,v8.execute,blink.user_timing",
	});

	await startSampling(page);
	const bottom = await page.evaluate(() =>
		document.documentElement.scrollHeight - innerHeight
	);
	let y = 0;
	while (y < bottom - 1) {
		await page.mouse.wheel(0, 80);
		await page.waitForTimeout(16);
		y = await page.evaluate(() => scrollY);
	}
	// The scroll is over: later work is not scroll jank. Wait so observers
	// deliver entries that started before this point.
	await page.evaluate(() => {
		(globalThis as unknown as { __perf: { end: number } }).__perf.end =
			performance.now();
	});
	await page.waitForTimeout(500);
	const samples = await stopSampling(page);

	await cdp.send("Tracing.end");
	await tracingDone;
	await Deno.mkdir(outDir, { recursive: true });
	await Deno.writeTextFile(
		`${outDir}/${run.name}.trace.json`,
		`{"traceEvents":[${chunks.join(",")}]}`,
	);
	await browser.close();

	// The first interval spans the start of sampling, not a frame.
	const intervals = samples.intervals.slice(1);
	return {
		run: run.name,
		frames: intervals.length,
		p50: percentile(intervals, 0.5),
		p95: percentile(intervals, 0.95),
		max: Math.max(...intervals),
		slowFrames:
			intervals.filter((interval) => interval > FRAME_BUDGET_MS).length,
		longTasks: samples.longTasks.length,
		longestTask: Math.max(0, ...samples.longTasks),
		longFrames: samples.longFrames.length,
	};
}

const results: Result[] = [];
for (const run of RUNS) results.push(await measure(run));

const ms = (value: number) => `${value.toFixed(1)} ms`;
const table = [
	`Scrolling ${url} top to bottom with wheel events, 1440 × 900.`,
	"",
	"| Run | Frames | p50 frame | p95 frame | Worst frame | Frames over 16.7 ms | Long tasks | Long animation frames |",
	"| --- | --: | --: | --: | --: | --: | --: | --: |",
	...results.map((result) =>
		`| ${result.run} | ${result.frames} | ${ms(result.p50)} | ${
			ms(result.p95)
		} | ${ms(result.max)} | ${result.slowFrames} (${
			(100 * result.slowFrames / result.frames).toFixed(1)
		}%) | ${result.longTasks}${
			result.longTasks > 0 ? ` (longest ${ms(result.longestTask)})` : ""
		} | ${result.longFrames} |`
	),
	"",
	`Traces: ${
		RUNS.map((run) => `\`${outDir}/${run.name}.trace.json\``).join(", ")
	}.`,
].join("\n");
console.log(table);
const summary = Deno.env.get("GITHUB_STEP_SUMMARY");
if (summary) {
	await Deno.writeTextFile(summary, `### Scroll performance\n\n${table}\n`, {
		append: true,
	});
}

const failed = results.filter((result) =>
	result.p95 > FRAME_BUDGET_MS || result.longTasks > 0
);
if (failed.length > 0) {
	console.error(
		`Over budget: ${failed.map((result) => result.run).join(", ")}`,
	);
	Deno.exit(1);
}
