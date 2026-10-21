/**
 * The canvas demo in a real browser: frame times while panning and
 * dragging, on WebGPU and on the CPU fallback, and two tabs editing one
 * scene.
 *
 * Frame time is the time `CanvasApp.render` takes on the main thread: the
 * scene walk, encoding, and submitting (GPU) or rasterising and
 * `putImageData` (CPU). Budgets match the canvas design doc and scale
 * with `CANVIST_PERF_SLACK` on slow machines.
 *
 * Needs `deno task build` first, and Chromium from `npx playwright install
 * chromium`.
 */
import { assert, assertEquals, assertNotEquals } from "@std/assert";
import { type BrowserContext, chromium, type Page } from "playwright";
import "../demo/globals.ts";
import { type DemoServer, startServer } from "./server.ts";

/** The viewport frames are measured at, in CSS pixels at 1x. */
const VIEWPORT = { width: 1280, height: 720 };
/** Extra rectangles, for a scene with real work in it. */
const STRESS = 2_000;
/** Frames each measured gesture runs for. */
const GESTURE_FRAMES = 120;
/** Budgets for the 95th percentile frame, in milliseconds. */
const BUDGETS: Readonly<Record<string, number>> = { webgpu: 8, cpu: 33 };
/** How much slower than the budget a slow machine may be. */
const SLACK = Number(Deno.env.get("CANVIST_PERF_SLACK") ?? 1);
/** Seconds to wait for the demo to start. */
const START_TIMEOUT = 30_000;
/** A screen point on the demo's title text frame. */
const ON_TITLE = { x: 300, y: 66 };
/** A screen point on the demo's star. */
const ON_STAR = { x: 640, y: 290 };

/** Percentiles of one gesture's frame times. */
interface FrameStats {
	frames: number;
	p50: number;
	p95: number;
	max: number;
}

/** What the page reports about itself. */
interface PageState {
	backend: string;
	nodes: number;
	zoom: number;
	json: string;
	editing: boolean;
}

async function withBrowser(
	run: (context: BrowserContext, server: DemoServer) => Promise<void>,
): Promise<void> {
	const server = startServer();
	const browser = await chromium.launch({
		headless: true,
		args: [
			"--enable-unsafe-webgpu",
			"--enable-features=Vulkan",
			"--ignore-gpu-blocklist",
		],
	});

	// One context, so its tabs share a BroadcastChannel.
	const context = await browser.newContext({
		viewport: VIEWPORT,
		deviceScaleFactor: 1,
	});

	try {
		await run(context, server);
	} finally {
		await browser.close();
		await server.server.shutdown();
	}
}

async function open(context: BrowserContext, url: string): Promise<Page> {
	const page = await context.newPage();
	page.on(
		"pageerror",
		(error) => console.error(`page error: ${error.message}`),
	);
	await page.goto(url);
	await page.waitForFunction(() => globalThis.__canvas !== undefined, null, {
		timeout: START_TIMEOUT,
	});

	return page;
}

function state(page: Page): Promise<PageState> {
	return page.evaluate(() => {
		const handle = globalThis.__canvas;
		if (!handle) throw new Error("the canvas did not start");
		return {
			backend: handle.backend,
			nodes: handle.app.node_count(),
			zoom: handle.app.zoom(),
			json: handle.app.scene_json(),
			editing: handle.app.editing(),
		};
	});
}

/** Run `gesture`, then summarise the frames painted during it. */
async function measure(
	page: Page,
	gesture: () => Promise<void>,
): Promise<FrameStats> {
	await page.evaluate(() => globalThis.__canvas?.resetFrameTimes());
	await gesture();
	const work = await page.evaluate(() =>
		globalThis.__canvas?.frameTimes().map((frame) => frame.work) ?? []
	);
	work.sort((a, b) => a - b);
	const at = (quantile: number) =>
		work[Math.min(work.length - 1, Math.floor(quantile * work.length))] ?? 0;

	return {
		frames: work.length,
		p50: at(0.5),
		p95: at(0.95),
		max: work.at(-1) ?? 0,
	};
}

/** Wait for the next painted frame. */
function nextFrame(page: Page): Promise<unknown> {
	return page.evaluate(() =>
		new Promise((resolve) => requestAnimationFrame(resolve))
	);
}

async function pan(page: Page): Promise<void> {
	await page.mouse.move(VIEWPORT.width / 2, VIEWPORT.height / 2);

	for (let frame = 0; frame < GESTURE_FRAMES; frame++) {
		// Back and forth, ending where it started.
		const direction = frame % 40 < 20 ? 1 : -1;
		await page.mouse.wheel(14 * direction, 10 * direction);
		await nextFrame(page);
	}
}

async function drag(page: Page): Promise<void> {
	await page.mouse.move(ON_STAR.x, ON_STAR.y);
	await page.mouse.down();

	for (let frame = 1; frame <= GESTURE_FRAMES; frame++) {
		const angle = (frame / GESTURE_FRAMES) * Math.PI * 2;
		await page.mouse.move(
			ON_STAR.x + Math.sin(angle) * 220,
			ON_STAR.y + (1 - Math.cos(angle)) * 120,
		);
		await nextFrame(page);
	}

	await page.mouse.up();
}

function report(backend: string, gesture: string, stats: FrameStats): void {
	const budget = (BUDGETS[backend] ?? BUDGETS.cpu) * SLACK;
	console.log(
		`CANVAS_PERF ${JSON.stringify({ backend, gesture, budget, ...stats })}`,
	);
	assert(
		stats.frames >= GESTURE_FRAMES / 2,
		`${gesture} painted only ${stats.frames} frames`,
	);
	assert(
		stats.p95 <= budget,
		`${backend} ${gesture}: p95 frame ${
			stats.p95.toFixed(2)
		} ms over the ${budget} ms budget`,
	);
}

for (const variant of ["gpu", "cpu"] as const) {
	Deno.test(`frame time while panning and dragging (${variant})`, async () => {
		await withBrowser(async (context, server) => {
			const query = `?room=perf-${variant}&stress=${STRESS}${
				variant === "cpu" ? "&cpu" : ""
			}`;
			const page = await open(context, `${server.url}${query}`);
			const before = await state(page);
			if (variant === "cpu") assertEquals(before.backend, "cpu");
			assert(before.nodes > STRESS, "the stress rectangles were added");

			const panning = await measure(page, () => pan(page));
			const panned = await state(page);
			// Panning back and forth ends near where it started, but zoom
			// stays put: the wheel pans.
			assertEquals(panned.zoom, before.zoom);
			report(before.backend, "pan", panning);

			const dragging = await measure(page, () => drag(page));
			const dragged = await state(page);
			assertNotEquals(dragged.json, panned.json, "the drag moved something");
			report(before.backend, "drag", dragging);
		});
	});
}

Deno.test("two tabs edit one scene", async () => {
	await withBrowser(async (context, server) => {
		// Collaboration, not rendering, so the quick-starting CPU backend.
		const url = `${server.url}?cpu&room=pair-${crypto.randomUUID()}`;
		const ada = await open(context, url);
		const grace = await open(context, url);
		const start = await state(grace);

		// Ada draws a rectangle.
		await ada.keyboard.press("r");
		await ada.mouse.move(420, 520);
		await ada.mouse.down();
		await ada.mouse.move(520, 600, { steps: 8 });
		await ada.mouse.up();
		await grace.waitForFunction(
			(nodes) => (globalThis.__canvas?.app.node_count() ?? 0) > nodes,
			start.nodes,
		);

		// Grace edits the title on the real text editor.
		await grace.mouse.dblclick(ON_TITLE.x, ON_TITLE.y);
		assert(
			(await state(grace)).editing,
			"a double click opens the text editor",
		);
		await grace.keyboard.press("End");
		await grace.keyboard.type(" Together!");
		await ada.waitForFunction(() =>
			globalThis.__canvas?.app.scene_json().includes("Together!") ?? false
		);

		await grace.keyboard.press("Escape");
		await ada.waitForFunction(
			(json) => globalThis.__canvas?.app.scene_json() === json,
			(await state(grace)).json,
		);
		assertEquals((await state(ada)).json, (await state(grace)).json);
	});
});

Deno.test("a canvas starts from the scene it is given, the same in every tab", async () => {
	const scene = {
		type: "canvist-scene",
		version: 1,
		nodes: [{
			type: "page",
			children: [{
				type: "rectangle",
				props: {
					name: "Card",
					size: [120, 80],
					transform: [1, 0, 0, 1, 40, 40],
					fills: [{ type: "solid", color: "#3b2cf5" }],
				},
			}],
		}],
	};

	await withBrowser(async (context, server) => {
		const url = `${server.url}?cpu&room=scene&scene=${
			encodeURIComponent(JSON.stringify(scene))
		}`;
		const first = await open(context, url);
		const second = await open(context, url);
		await nextFrame(second);

		const [one, two] = await Promise.all([state(first), state(second)]);
		assertEquals(one.nodes, 2, "a page and its rectangle");
		assertEquals(
			two.nodes,
			one.nodes,
			"tabs that share a scene merge it without duplicates",
		);
		assertEquals(two.json, one.json);

		const saved = await first.evaluate(() => globalThis.__canvas?.toJson());
		assert(JSON.stringify(saved).includes('"Card"'));
		const stateBytes = await first.evaluate(() =>
			globalThis.__canvas?.encodeState().length ?? 0
		);
		assert(stateBytes > 0, "the shared state encodes");
	});
});
