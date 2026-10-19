/**
 * Turn the parity test results in `report/parity/` into one HTML page:
 * for every case, the canvas, the server-rendered DOM, and a heatmap of
 * where their pixels differ, with the geometry and pixel numbers against
 * the platform's thresholds.
 *
 *   node scripts/parity-report.ts [--inline]
 *
 * `--inline` embeds the images as data URLs, for a single shareable file.
 *
 * @module
 */
import { readdir, readFile, writeFile } from "node:fs/promises";

interface Result {
	readonly host: string;
	readonly slug: string;
	readonly fixture: string;
	readonly title: string;
	readonly scale: number;
	readonly scheme: string;
	readonly vertical: number;
	readonly horizontal: number;
	readonly pixels: number;
	readonly antialiasing: number;
	readonly ssim: number;
	readonly svgPixels: number;
	readonly svgSsim: number;
	readonly limits: {
		vertical: number;
		horizontal: number;
		pixels: number;
		ssim: number;
		reason?: string;
	};
	readonly worst: readonly {
		kind: string;
		what: string;
		canvas: number;
		dom: number;
	}[];
}

const root = new URL("../report/parity/", import.meta.url);
const inline = process.argv.includes("--inline");
const hosts = (await readdir(root, { withFileTypes: true }))
	.filter((entry) => entry.isDirectory())
	.map((entry) => entry.name)
	.sort();
const results: Result[] = [];

for (const host of hosts) {
	for (
		const file of (await readdir(new URL(`${host}/`, root))).filter((name) =>
			name.endsWith(".json")
		).sort()
	) {
		results.push(
			JSON.parse(await readFile(new URL(`${host}/${file}`, root), "utf8")),
		);
	}
}

async function image(host: string, file: string): Promise<string> {
	if (!inline) {
		return `${host}/${file}`;
	}

	const bytes = await readFile(new URL(`${host}/${file}`, root));

	return `data:image/png;base64,${bytes.toString("base64")}`;
}

const escape = (text: string) =>
	text.replace(/[&<>"]/g, (character) => `&#${character.codePointAt(0)};`);
const px = (value: number) => `${value.toFixed(4)} px`;
/** How many of each case's largest differences the report lists. */
const WORST_SHOWN = 5;

const ok = (pass: boolean) => (pass ? "pass" : "fail");

const rows: string[] = [];

for (const result of results) {
	const checks = {
		vertical: result.vertical <= result.limits.vertical,
		horizontal: result.horizontal <= result.limits.horizontal,
		pixels: result.pixels <= result.limits.pixels,
		ssim: result.ssim >= result.limits.ssim,
	};
	const passed = Object.values(checks).every(Boolean);
	const worst = result.worst
		.slice(0, WORST_SHOWN)
		.map((delta) =>
			`<li>${escape(delta.kind)} ${escape(delta.what)}: canvas ${
				delta.canvas.toFixed(3)
			}, DOM ${delta.dom.toFixed(3)}</li>`
		)
		.join("");
	rows.push(`<section class="case ${ok(passed)}" data-host="${result.host}">
	<h2>${
		escape(result.title)
	} <small>${result.host} · ${result.scale}× · ${result.scheme}</small> <b>${
		passed ? "pass" : "fail"
	}</b></h2>
	<dl>
		<dt>Lines, blocks, baselines</dt><dd class="${ok(checks.vertical)}">${
		px(result.vertical)
	} ≤ ${px(result.limits.vertical)}</dd>
		<dt>Runs and caret stops</dt><dd class="${ok(checks.horizontal)}">${
		px(result.horizontal)
	} ≤ ${px(result.limits.horizontal)}</dd>
		<dt>Edge shading difference</dt><dd>${
		(result.antialiasing * 100).toFixed(2)
	}% mean where pixels differ</dd>
		<dt>Differing pixels (not antialiasing)</dt><dd class="${
		ok(checks.pixels)
	}">${(result.pixels * 100).toFixed(3)}% ≤ ${
		(result.limits.pixels * 100).toFixed(3)
	}%</dd>
		<dt>SSIM</dt><dd class="${ok(checks.ssim)}">${
		result.ssim.toFixed(5)
	} ≥ ${result.limits.ssim}</dd>
		<dt>SVG vs DOM</dt><dd>${
		(result.svgPixels * 100).toFixed(3)
	}% pixels, SSIM ${result.svgSsim.toFixed(4)}</dd>
	</dl>
	${
		result.limits.reason === undefined
			? ""
			: `<p class="reason">Known difference: ${
				escape(result.limits.reason)
			}</p>`
	}
	${
		worst === ""
			? ""
			: `<details><summary>Largest geometry differences</summary><ul>${worst}</ul></details>`
	}
	<div class="images">
		<figure><img loading="lazy" src="${await image(
		result.host,
		`${result.slug}-canvas.png`,
	)}"><figcaption>Canvas</figcaption></figure>
		<figure><img loading="lazy" src="${await image(
		result.host,
		`${result.slug}-dom.png`,
	)}"><figcaption>DOM (SSR)</figcaption></figure>
		<figure><img loading="lazy" src="${await image(
		result.host,
		`${result.slug}-diff.png`,
	)}"><figcaption>Difference</figcaption></figure>
	</div>
</section>`);
}

const failed =
	results.filter((result) =>
		result.vertical > result.limits.vertical ||
		result.horizontal > result.limits.horizontal ||
		result.pixels > result.limits.pixels || result.ssim < result.limits.ssim
	).length;

const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width">
<title>canvist pixel parity</title>
<style>
:root { color-scheme: light dark; --pass: #15803d; --fail: #b91c1c; --line: #d6d3d1; }
body { margin: 0 auto; max-width: 1600px; padding: 24px 16px; font: 14px/1.5 system-ui, sans-serif; }
h1 { margin: 0 0 4px; } .case { border-top: 1px solid var(--line); padding: 16px 0; }
h2 { font-size: 16px; margin: 0 0 8px; } h2 small { font-weight: 400; opacity: 0.7; }
h2 b { font-size: 12px; text-transform: uppercase; } .pass b, dd.pass { color: var(--pass); } .fail b, dd.fail { color: var(--fail); }
dl { display: grid; grid-template-columns: max-content 1fr; gap: 2px 12px; margin: 0 0 8px; } dt { opacity: 0.7; } dd { margin: 0; font-variant-numeric: tabular-nums; }
.images { display: flex; gap: 8px; overflow-x: auto; } figure { margin: 0; } figcaption { font-size: 12px; opacity: 0.7; }
img { display: block; max-width: none; height: auto; width: calc(var(--w, 480px)); image-rendering: pixelated; border: 1px solid var(--line); }
nav label { margin-right: 12px; } .reason { margin: 0 0 8px; padding: 6px 10px; border-left: 3px solid #a16207; background: color-mix(in srgb, #a16207 10%, transparent); }
</style>
</head>
<body>
<h1>canvist pixel parity</h1>
<p>${results.length} cases, ${failed} over threshold. Each case: the canvas editor, the server-rendered HTML it hydrates from, and where their pixels differ (red to yellow by size).</p>
<nav>${
	hosts.map((host) =>
		`<label><input type="checkbox" checked data-toggle="${host}"> ${host}</label>`
	).join("")
}</nav>
${rows.join("\n")}
<script>
for (const toggle of document.querySelectorAll("[data-toggle]")) {
	toggle.addEventListener("change", () => {
		for (const section of document.querySelectorAll('[data-host="' + toggle.dataset.toggle + '"]')) section.hidden = !toggle.checked;
	});
}
</script>
</body>
</html>
`;

await writeFile(new URL("index.html", root), html);
console.log(
	`report/parity/index.html: ${results.length} cases, ${failed} over threshold`,
);

for (const result of results) {
	console.log(
		`${result.host.padEnd(16)} ${result.slug.padEnd(28)} v ${
			result.vertical.toFixed(3)
		} h ${result.horizontal.toFixed(3)} px ${
			(result.pixels * 100).toFixed(3)
		}% aa ${(result.antialiasing * 100).toFixed(1)}% ssim ${
			result.ssim.toFixed(4)
		}`,
	);
}
