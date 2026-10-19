/**
 * Renders the launch film's cuts and poster from one bundle.
 *
 * ```sh
 * npm run render                 # every cut, the poster, and out/report.json
 * npm run render:preview         # a 540p preview of the launch cut
 * node scripts/render.ts --stills 120,480,900   # inspection stills
 * ```
 *
 * Run `npm run prepare:footage` first (the npm scripts do).
 */

import { copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

import { bundle } from "@remotion/bundler";
import {
	renderMedia,
	renderStill,
	selectComposition,
} from "@remotion/renderer";

const VIDEO = join(dirname(fileURLToPath(import.meta.url)), "..");
const OUT = join(VIDEO, "out");
const POSTER = join(VIDEO, "..", "assets/brand/video-poster.png");
/** The poster frame: the formatting scene with every block type on the page. */
const POSTER_FRAME = 725;

interface Cut {
	readonly composition: string;
	readonly file: string;
	readonly scale: number;
	readonly crf: number;
}

const CUTS: readonly Cut[] = [
	{
		composition: "Launch",
		file: "canvist-launch-1080p.mp4",
		scale: 1,
		crf: 16,
	},
	{ composition: "Launch", file: "canvist-launch-4k.mp4", scale: 2, crf: 18 },
	{
		composition: "LaunchVertical",
		file: "canvist-launch-9x16.mp4",
		scale: 1,
		crf: 18,
	},
	{
		composition: "LaunchSquare",
		file: "canvist-launch-1x1.mp4",
		scale: 1,
		crf: 18,
	},
];

const PREVIEW: Cut = {
	composition: "Launch",
	file: "canvist-launch-preview.mp4",
	scale: 0.5,
	crf: 26,
};

const { values } = parseArgs({
	options: {
		preview: { type: "boolean", default: false },
		stills: { type: "string" },
		composition: { type: "string", default: "Launch" },
	},
});

mkdirSync(OUT, { recursive: true });
const started = performance.now();
const serveUrl = await bundle({ entryPoint: join(VIDEO, "src/index.ts") });
const bundled = performance.now();

async function still(
	composition: string,
	frame: number,
	output: string,
	scale = 1,
): Promise<void> {
	const selected = await selectComposition({ serveUrl, id: composition });
	await renderStill({
		serveUrl,
		composition: selected,
		frame,
		output,
		scale,
		imageFormat: "png",
	});
}

if (values.stills) {
	const dir = join(OUT, "stills");
	mkdirSync(dir, { recursive: true });

	for (const frame of values.stills.split(",").map(Number)) {
		const output = join(dir, `${values.composition}-${frame}.png`);
		await still(values.composition, frame, output, 0.5);
		console.error(output);
	}

	process.exit(0);
}

interface Rendered {
	readonly file: string;
	readonly width: number;
	readonly height: number;
	readonly seconds: number;
	readonly renderSeconds: number;
}

const report: Rendered[] = [];

for (const cut of values.preview ? [PREVIEW] : CUTS) {
	const composition = await selectComposition({
		serveUrl,
		id: cut.composition,
	});
	const output = join(OUT, cut.file);
	const begin = performance.now();
	let last = -1;

	await renderMedia({
		serveUrl,
		composition,
		codec: "h264",
		crf: cut.crf,
		pixelFormat: "yuv420p",
		scale: cut.scale,
		outputLocation: output,
		onProgress: ({ progress }) => {
			const percent = Math.floor(progress * 10) * 10;

			if (percent !== last) {
				last = percent;
				console.error(`${cut.file}: ${percent}%`);
			}
		},
	});

	report.push({
		file: `video/out/${cut.file}`,
		width: composition.width * cut.scale,
		height: composition.height * cut.scale,
		seconds: composition.durationInFrames / composition.fps,
		renderSeconds: Math.round((performance.now() - begin) / 100) / 10,
	});
}

if (!values.preview) {
	const poster = join(OUT, "poster.png");
	await still("Launch", POSTER_FRAME, poster);
	copyFileSync(poster, POSTER);
}

const summary = {
	bundleSeconds: Math.round((bundled - started) / 100) / 10,
	totalSeconds: Math.round((performance.now() - started) / 100) / 10,
	cuts: report,
};
writeFileSync(
	join(OUT, "report.json"),
	`${JSON.stringify(summary, null, "\t")}\n`,
);
console.log(JSON.stringify(summary, null, "\t"));
