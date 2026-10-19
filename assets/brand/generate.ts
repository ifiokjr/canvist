/**
 * Generates every raster, icon container and print vector for the canvist brand
 * from the hand-authored masters in `vector/`.
 *
 * Run from the repository root:
 *
 * ```sh
 * deno run --allow-read --allow-write assets/brand/generate.ts
 * ```
 *
 * The masters are the source of truth. Everything outside `vector/`, except
 * `readme.md` and this script, is generated and safe to delete and regenerate.
 * The script also writes the docs site's favicons (`docs/theme/`) and logo
 * copies (`docs/src/images/brand/`).
 */

import { initWasm, Resvg } from "npm:@resvg/resvg-wasm@2.6.2";

const ROOT = new URL("./", import.meta.url);
const INK = "#17141F";
const PAPER = "#FFFFFF";
const BLACK = "#000000";

/** Below this pixel size the drip is noise, so icons switch to the simplified small mark. */
const SMALL_MARK_BELOW = 64;

await initWasm(
	Deno.readFile(
		new URL(import.meta.resolve("npm:@resvg/resvg-wasm@2.6.2/index_bg.wasm")),
	),
);

const written: string[] = [];

// ---------------------------------------------------------------------------
// Masters

interface Art {
	/** Child elements of the master SVG (everything after `<title>`). */
	readonly body: string;
	/** Side of the master's square view box. */
	readonly box: number;
}

function readMaster(name: string): string {
	return Deno.readTextFileSync(new URL(`vector/${name}`, ROOT));
}

function loadArt(name: string): Art {
	const source = readMaster(name);
	const box = Number(viewBox(source)[2]);
	const body = source.slice(
		source.indexOf("</title>") + "</title>".length,
		source.lastIndexOf("</svg>"),
	).trim();
	return { body, box };
}

function viewBox(source: string): number[] {
	const match = source.match(/viewBox="([^"]+)"/);
	if (!match) throw new Error("master SVG has no viewBox");
	return match[1].split(/\s+/).map(Number);
}

const ART = {
	light: loadArt("mark.svg"),
	dark: loadArt("mark-dark.svg"),
	monoWhite: loadArt("mark-mono-white.svg"),
	small: loadArt("mark-small.svg"),
} as const;

// ---------------------------------------------------------------------------
// Icon composition

type Shape = "none" | "full" | "rounded" | "circle" | "macos";

interface IconSpec {
	readonly art: Art;
	/** Side of the art's view box as a fraction of the tile. */
	readonly scale: number;
	readonly shape: Shape;
	readonly fill?: string;
}

/** Builds a square icon SVG in a 1024 unit space: an optional tile with the mark centred on it. */
function iconSvg(spec: IconSpec): string {
	const S = 1024;
	const tile = spec.shape === "macos"
		? { x: 100, side: 824 }
		: { x: 0, side: S };
	const fill = spec.fill ?? PAPER;
	const shape = {
		none: "",
		full: `<rect width="${S}" height="${S}" fill="${fill}"/>`,
		rounded: `<rect width="${S}" height="${S}" rx="${
			S * 0.225
		}" fill="${fill}"/>`,
		circle: `<circle cx="${S / 2}" cy="${S / 2}" r="${S / 2}" fill="${fill}"/>`,
		macos:
			`<rect x="${tile.x}" y="${tile.x}" width="${tile.side}" height="${tile.side}" rx="185" fill="${fill}"/>`,
	}[spec.shape];
	const side = tile.side * spec.scale;
	const offset = tile.x + (tile.side - side) / 2;
	return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${S} ${S}">${shape}<svg x="${offset}" y="${offset}" width="${side}" height="${side}" viewBox="0 0 ${spec.art.box} ${spec.art.box}">${spec.art.body}</svg></svg>`;
}

/** The rounded white tile used by desktop and PWA icons; picks the small mark for small sizes. */
function tileIcon(px: number, shape: Shape = "rounded"): string {
	const small = px < SMALL_MARK_BELOW;
	return iconSvg({
		art: small ? ART.small : ART.light,
		scale: small ? 0.7 : 0.78,
		shape,
	});
}

// ---------------------------------------------------------------------------
// Rasterising

interface Raster {
	readonly png: Uint8Array;
	readonly rgba: Uint8Array;
	readonly width: number;
	readonly height: number;
}

function render(svg: string, width: number): Raster {
	const image = new Resvg(svg, { fitTo: { mode: "width", value: width } })
		.render();
	return {
		png: image.asPng(),
		rgba: image.pixels,
		width: image.width,
		height: image.height,
	};
}

async function writeFile(
	path: string,
	data: Uint8Array | string,
): Promise<void> {
	const url = new URL(path, ROOT);
	await Deno.mkdir(new URL("./", url), { recursive: true });
	if (typeof data === "string") await Deno.writeTextFile(url, data);
	else await Deno.writeFile(url, data);
	written.push(path);
}

async function png(
	path: string,
	svg: string,
	width: number,
	options: { opaque?: boolean } = {},
): Promise<void> {
	const raster = render(svg, width);
	await writeFile(
		path,
		options.opaque ? await encodeOpaquePng(raster) : raster.png,
	);
}

// ---------------------------------------------------------------------------
// PNG without an alpha channel (App Store icons must not have one)

const CRC_TABLE = Uint32Array.from({ length: 256 }, (_, n) => {
	let c = n;
	for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
	return c >>> 0;
});

function crc32(bytes: Uint8Array): number {
	let c = 0xffffffff;
	for (const byte of bytes) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
	return (c ^ 0xffffffff) >>> 0;
}

function chunk(type: string, data: Uint8Array): Uint8Array {
	const out = new Uint8Array(12 + data.length);
	const view = new DataView(out.buffer);
	view.setUint32(0, data.length);
	out.set(new TextEncoder().encode(type), 4);
	out.set(data, 8);
	view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
	return out;
}

async function zlib(data: Uint8Array<ArrayBuffer>): Promise<Uint8Array> {
	const stream = new Blob([data]).stream().pipeThrough(
		new CompressionStream("deflate"),
	);
	return new Uint8Array(await new Response(stream).arrayBuffer());
}

/** Re-encodes a raster as 8-bit RGB, failing loudly if any pixel is not fully opaque. */
async function encodeOpaquePng(
	{ rgba, width, height }: Raster,
): Promise<Uint8Array> {
	const rows = new Uint8Array((width * 3 + 1) * height);
	for (let y = 0; y < height; y++) {
		const rowStart = y * (width * 3 + 1);
		for (let x = 0; x < width; x++) {
			const i = (y * width + x) * 4;
			if (rgba[i + 3] !== 255) {
				throw new Error(`opaque PNG has a transparent pixel at ${x},${y}`);
			}
			rows.set(rgba.subarray(i, i + 3), rowStart + 1 + x * 3);
		}
	}
	const header = new Uint8Array(13);
	const view = new DataView(header.buffer);
	view.setUint32(0, width);
	view.setUint32(4, height);
	header.set([8, 2, 0, 0, 0], 8);
	const signature = Uint8Array.of(
		0x89,
		0x50,
		0x4e,
		0x47,
		0x0d,
		0x0a,
		0x1a,
		0x0a,
	);
	return concat([
		signature,
		chunk("IHDR", header),
		chunk("IDAT", await zlib(rows)),
		chunk("IEND", new Uint8Array()),
	]);
}

function concat(parts: Uint8Array[]): Uint8Array {
	const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
	let offset = 0;
	for (const part of parts) {
		out.set(part, offset);
		offset += part.length;
	}
	return out;
}

// ---------------------------------------------------------------------------
// Icon containers

/** Windows ICO with PNG-compressed entries (supported since Windows Vista). */
function ico(images: { size: number; png: Uint8Array }[]): Uint8Array {
	const header = new Uint8Array(6 + images.length * 16);
	const view = new DataView(header.buffer);
	view.setUint16(2, 1, true);
	view.setUint16(4, images.length, true);
	let offset = header.length;
	images.forEach(({ size, png }, i) => {
		const at = 6 + i * 16;
		header[at] = size >= 256 ? 0 : size;
		header[at + 1] = size >= 256 ? 0 : size;
		view.setUint16(at + 4, 1, true);
		view.setUint16(at + 6, 32, true);
		view.setUint32(at + 8, png.length, true);
		view.setUint32(at + 12, offset, true);
		offset += png.length;
	});
	return concat([header, ...images.map((image) => image.png)]);
}

/** Apple ICNS. Entries are PNG except the 16 and 32 px 1x slots, which macOS reads as RLE ARGB. */
function icns(entries: { type: string; data: Uint8Array }[]): Uint8Array {
	const encoded = entries.map(({ type, data }) => {
		const head = new Uint8Array(8);
		head.set(new TextEncoder().encode(type));
		new DataView(head.buffer).setUint32(4, data.length + 8);
		return concat([head, data]);
	});
	const head = new Uint8Array(8);
	head.set(new TextEncoder().encode("icns"));
	new DataView(head.buffer).setUint32(
		4,
		encoded.reduce((sum, e) => sum + e.length, 8),
	);
	return concat([head, ...encoded]);
}

/** The `ic04`/`ic05` payload: "ARGB" then each straight-alpha channel run-length encoded. */
function icnsArgb({ rgba, width, height }: Raster): Uint8Array {
	const channels = [0, 1, 2, 3].map(() => new Uint8Array(width * height));
	for (let p = 0; p < width * height; p++) {
		const alpha = rgba[p * 4 + 3];
		channels[0][p] = alpha;
		// resvg pixels are premultiplied; ICNS wants straight alpha.
		for (let c = 0; c < 3; c++) {
			channels[c + 1][p] = alpha === 0
				? 0
				: Math.min(255, Math.round((rgba[p * 4 + c] * 255) / alpha));
		}
	}
	return concat([new TextEncoder().encode("ARGB"), ...channels.map(icnsRle)]);
}

/** Apple's icon run-length encoding: `n < 0x80` copies n + 1 bytes, `n >= 0x80` repeats the next byte n - 0x7D times. */
function icnsRle(channel: Uint8Array): Uint8Array {
	const out: number[] = [];
	let i = 0;
	while (i < channel.length) {
		let run = 1;
		while (
			i + run < channel.length && run < 130 && channel[i + run] === channel[i]
		) run++;
		if (run >= 3) {
			out.push(0x80 + run - 3, channel[i]);
			i += run;
			continue;
		}
		const start = i;
		while (i < channel.length && i - start < 128) {
			if (
				i + 2 < channel.length && channel[i] === channel[i + 1] &&
				channel[i] === channel[i + 2]
			) break;
			i++;
		}
		out.push(i - start - 1, ...channel.subarray(start, i));
	}
	return Uint8Array.from(out);
}

// ---------------------------------------------------------------------------
// Print vectors: SVG path data to PDF and EPS

type Point = readonly [number, number];
type Segment =
	| { readonly op: "M"; readonly to: Point }
	| {
		readonly op: "C";
		readonly c1: Point;
		readonly c2: Point;
		readonly to: Point;
	}
	| { readonly op: "Z" };

interface FilledPath {
	readonly rgb: readonly [number, number, number];
	readonly segments: Segment[];
}

/** Parses SVG path data into absolute moves and cubic curves. */
function parsePath(d: string): Segment[] {
	const tokens = d.match(
		/[MmLlHhVvCcSsQqTtAaZz]|[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/g,
	) ?? [];
	const out: Segment[] = [];
	let i = 0;
	let command = "";
	let cur: Point = [0, 0];
	let start: Point = [0, 0];
	let lastControl: Point | null = null;
	let lastQuad: Point | null = null;
	const num = () => {
		const token = tokens[i++];
		if (token === undefined || /[a-z]/i.test(token)) {
			throw new Error(`malformed path data near token ${i}`);
		}
		return Number(token);
	};
	const line = (to: Point) => {
		out.push({
			op: "C",
			c1: lerp(cur, to, 1 / 3),
			c2: lerp(cur, to, 2 / 3),
			to,
		});
		cur = to;
	};
	while (i < tokens.length) {
		if (/[a-z]/i.test(tokens[i])) command = tokens[i++];
		const rel = command === command.toLowerCase();
		const at = (x: number, y: number): Point =>
			rel ? [cur[0] + x, cur[1] + y] : [x, y];
		const upper = command.toUpperCase();
		if (upper !== "C" && upper !== "S") lastControl = null;
		if (upper !== "Q" && upper !== "T") lastQuad = null;
		switch (upper) {
			case "M": {
				cur = start = at(num(), num());
				out.push({ op: "M", to: cur });
				command = rel ? "l" : "L";
				break;
			}
			case "L":
				line(at(num(), num()));
				break;
			case "H":
				line([rel ? cur[0] + num() : num(), cur[1]]);
				break;
			case "V":
				line([cur[0], rel ? cur[1] + num() : num()]);
				break;
			case "C": {
				const c1 = at(num(), num()),
					c2 = at(num(), num()),
					to = at(num(), num());
				out.push({ op: "C", c1, c2, to });
				lastControl = c2;
				cur = to;
				break;
			}
			case "S": {
				const c1: Point = lastControl
					? [2 * cur[0] - lastControl[0], 2 * cur[1] - lastControl[1]]
					: cur;
				const c2 = at(num(), num()), to = at(num(), num());
				out.push({ op: "C", c1, c2, to });
				lastControl = c2;
				cur = to;
				break;
			}
			case "Q":
			case "T": {
				const q: Point = upper === "Q"
					? at(num(), num())
					: lastQuad
					? [2 * cur[0] - lastQuad[0], 2 * cur[1] - lastQuad[1]]
					: cur;
				const to = at(num(), num());
				out.push({
					op: "C",
					c1: lerp(cur, q, 2 / 3),
					c2: lerp(to, q, 2 / 3),
					to,
				});
				lastQuad = q;
				cur = to;
				break;
			}
			case "A": {
				const rx = num(),
					ry = num(),
					rotation = num(),
					large = num(),
					sweep = num();
				const to = at(num(), num());
				out.push(
					...arcToCubics(cur, to, rx, ry, rotation, large !== 0, sweep !== 0),
				);
				cur = to;
				break;
			}
			case "Z":
				out.push({ op: "Z" });
				cur = start;
				break;
			default:
				throw new Error(`unsupported path command ${command}`);
		}
	}
	return out;
}

function lerp(a: Point, b: Point, t: number): Point {
	return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
}

/** Converts an SVG elliptical arc to cubic curves (SVG 1.1 implementation notes, F.6). */
function arcToCubics(
	from: Point,
	to: Point,
	rx: number,
	ry: number,
	degrees: number,
	large: boolean,
	sweep: boolean,
): Segment[] {
	if (rx === 0 || ry === 0) return [{ op: "C", c1: from, c2: to, to }];
	const phi = (degrees * Math.PI) / 180,
		cos = Math.cos(phi),
		sin = Math.sin(phi);
	const dx = (from[0] - to[0]) / 2, dy = (from[1] - to[1]) / 2;
	const x1 = cos * dx + sin * dy, y1 = -sin * dx + cos * dy;
	rx = Math.abs(rx);
	ry = Math.abs(ry);
	const lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
	if (lambda > 1) {
		rx *= Math.sqrt(lambda);
		ry *= Math.sqrt(lambda);
	}
	const numerator = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
	const factor = (large === sweep ? -1 : 1) *
		Math.sqrt(Math.max(0, numerator / (rx * rx * y1 * y1 + ry * ry * x1 * x1)));
	const cx1 = (factor * rx * y1) / ry, cy1 = (-factor * ry * x1) / rx;
	const cx = cos * cx1 - sin * cy1 + (from[0] + to[0]) / 2;
	const cy = sin * cx1 + cos * cy1 + (from[1] + to[1]) / 2;
	const angle = (ux: number, uy: number, vx: number, vy: number) =>
		Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
	const theta = angle(1, 0, (x1 - cx1) / rx, (y1 - cy1) / ry);
	let delta = angle(
		(x1 - cx1) / rx,
		(y1 - cy1) / ry,
		(-x1 - cx1) / rx,
		(-y1 - cy1) / ry,
	);
	if (!sweep && delta > 0) delta -= 2 * Math.PI;
	if (sweep && delta < 0) delta += 2 * Math.PI;
	const parts = Math.ceil(Math.abs(delta) / (Math.PI / 2));
	const step = delta / parts, k = (4 / 3) * Math.tan(step / 4);
	const point = (t: number): Point => [
		cx + rx * Math.cos(t) * cos - ry * Math.sin(t) * sin,
		cy + rx * Math.cos(t) * sin + ry * Math.sin(t) * cos,
	];
	const tangent = (t: number): Point => [
		-rx * Math.sin(t) * cos - ry * Math.cos(t) * sin,
		-rx * Math.sin(t) * sin + ry * Math.cos(t) * cos,
	];
	const segments: Segment[] = [];
	for (let n = 0; n < parts; n++) {
		const a = theta + n * step, b = a + step;
		const p0 = point(a),
			p1 = n === parts - 1 ? to : point(b),
			t0 = tangent(a),
			t1 = tangent(b);
		segments.push({
			op: "C",
			c1: [p0[0] + k * t0[0], p0[1] + k * t0[1]],
			c2: [p1[0] - k * t1[0], p1[1] - k * t1[1]],
			to: p1,
		});
	}
	return segments;
}

/** Reads the filled paths of a master. Masters for print may only contain `<path fill d>`. */
function filledPaths(source: string): FilledPath[] {
	const unsupported = source.match(
		/<(rect|circle|ellipse|line|polyline|polygon|g|use|image|text)\b/,
	);
	if (unsupported) {
		throw new Error(`print export does not support <${unsupported[1]}>`);
	}
	return [...source.matchAll(/<path\b([^>]*)\/>/g)].map(([, attributes]) => {
		const fill = attributes.match(/fill="#([0-9a-fA-F]{6})"/)?.[1];
		const d = attributes.match(/\bd="([^"]+)"/)?.[1];
		if (!fill || !d) {
			throw new Error("every printable path needs a hex fill and path data");
		}
		const channel = (at: number) => parseInt(fill.slice(at, at + 2), 16) / 255;
		return {
			rgb: [channel(0), channel(2), channel(4)],
			segments: parsePath(d),
		};
	});
}

const fmt = (n: number) => String(Math.round(n * 1000) / 1000);
/** Print vectors are drawn at 4 pt per master unit, matching the masters' width and height. */
const PT_PER_UNIT = 4;

function pdf(source: string): string {
	const [, , w, h] = viewBox(source);
	const W = w * PT_PER_UNIT, H = h * PT_PER_UNIT;
	const ops = [`q ${PT_PER_UNIT} 0 0 -${PT_PER_UNIT} 0 ${fmt(H)} cm`];
	for (const { rgb, segments } of filledPaths(source)) {
		ops.push(`${rgb.map(fmt).join(" ")} rg`);
		for (const s of segments) {
			if (s.op === "M") ops.push(`${fmt(s.to[0])} ${fmt(s.to[1])} m`);
			else if (s.op === "C") {
				ops.push(`${[...s.c1, ...s.c2, ...s.to].map(fmt).join(" ")} c`);
			} else ops.push("h");
		}
		ops.push("f");
	}
	ops.push("Q");
	const content = ops.join("\n");
	const objects = [
		"<< /Type /Catalog /Pages 2 0 R >>",
		"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
		`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${fmt(W)} ${
			fmt(H)
		}] /Resources << >> /Contents 4 0 R >>`,
		`<< /Length ${content.length} >>\nstream\n${content}\nendstream`,
		"<< /Title (canvist) /Producer (canvist assets/brand/generate.ts) >>",
	];
	let body = "%PDF-1.4\n";
	const offsets: number[] = [];
	objects.forEach((object, i) => {
		offsets.push(body.length);
		body += `${i + 1} 0 obj\n${object}\nendobj\n`;
	});
	const xref = body.length;
	body += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
	body += offsets.map((offset) =>
		`${String(offset).padStart(10, "0")} 00000 n \n`
	).join("");
	body += `trailer\n<< /Size ${
		objects.length + 1
	} /Root 1 0 R /Info 5 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
	return body;
}

function eps(source: string): string {
	const [, , w, h] = viewBox(source);
	const W = w * PT_PER_UNIT, H = h * PT_PER_UNIT;
	const lines = [
		"%!PS-Adobe-3.0 EPSF-3.0",
		`%%BoundingBox: 0 0 ${Math.ceil(W)} ${Math.ceil(H)}`,
		`%%HiResBoundingBox: 0 0 ${fmt(W)} ${fmt(H)}`,
		"%%Title: canvist",
		"%%Creator: canvist assets/brand/generate.ts",
		"%%EndComments",
		"/m { moveto } bind def /c { curveto } bind def /h { closepath } bind def",
		"gsave",
		`0 ${fmt(H)} translate ${PT_PER_UNIT} -${PT_PER_UNIT} scale`,
	];
	for (const { rgb, segments } of filledPaths(source)) {
		lines.push(`${rgb.map(fmt).join(" ")} setrgbcolor newpath`);
		for (const s of segments) {
			if (s.op === "M") lines.push(`${fmt(s.to[0])} ${fmt(s.to[1])} m`);
			else if (s.op === "C") {
				lines.push(`${[...s.c1, ...s.c2, ...s.to].map(fmt).join(" ")} c`);
			} else lines.push("h");
		}
		lines.push("fill");
	}
	lines.push("grestore", "showpage", "%%EOF", "");
	return lines.join("\n");
}

// ---------------------------------------------------------------------------
// Targets

const json = (value: unknown) => JSON.stringify(value, null, "\t") + "\n";

async function printVectors() {
	for (
		const name of [
			"mark",
			"mark-dark",
			"mark-mono",
			"lockup",
			"lockup-dark",
			"lockup-mono",
		]
	) {
		const source = readMaster(`${name}.svg`);
		await writeFile(`print/${name}.pdf`, pdf(source));
		await writeFile(`print/${name}.eps`, eps(source));
	}
}

async function web() {
	const smallMark = iconSvg({ art: ART.small, scale: 1, shape: "none" });
	await writeFile(
		"web/favicon.ico",
		ico(
			[16, 32, 48].map((size) => ({ size, png: render(smallMark, size).png })),
		),
	);
	await writeFile("web/favicon.svg", readMaster("favicon.svg"));
	// The app icon as a vector: rustdoc's sidebar logo and anywhere else a square tile is needed.
	await writeFile("web/icon.svg", tileIcon(1024) + "\n");
	await png(
		"web/apple-touch-icon.png",
		iconSvg({ art: ART.light, scale: 0.76, shape: "full" }),
		180,
		{ opaque: true },
	);
	await png("web/icon-192.png", tileIcon(192), 192);
	await png("web/icon-512.png", tileIcon(512), 512);
	// Maskable: the mark stays inside the central 80% safe circle.
	await png(
		"web/icon-maskable-512.png",
		iconSvg({ art: ART.light, scale: 0.68, shape: "full" }),
		512,
		{ opaque: true },
	);
	await png("web/og-image.png", readMaster("social-og.svg"), 1200, {
		opaque: true,
	});
	await png(
		"web/github-social-preview.png",
		readMaster("social-github.svg"),
		1280,
		{ opaque: true },
	);
	await writeFile(
		"web/site.webmanifest",
		json({
			name: "canvist",
			short_name: "canvist",
			icons: [
				{ src: "icon-192.png", sizes: "192x192", type: "image/png" },
				{ src: "icon-512.png", sizes: "512x512", type: "image/png" },
				{
					src: "icon-maskable-512.png",
					sizes: "512x512",
					type: "image/png",
					purpose: "maskable",
				},
			],
			theme_color: INK,
			background_color: PAPER,
			display: "standalone",
		}),
	);
}

async function ios() {
	const dir = "ios/AppIcon.appiconset";
	await png(
		`${dir}/AppIcon-1024.png`,
		iconSvg({ art: ART.light, scale: 0.76, shape: "full" }),
		1024,
		{ opaque: true },
	);
	// Dark: transparent so iOS draws its own dark background behind the mark.
	await png(
		`${dir}/AppIcon-1024-dark.png`,
		iconSvg({ art: ART.dark, scale: 0.76, shape: "none" }),
		1024,
	);
	// Tinted: grayscale on black; iOS applies the user's tint by luminance.
	await png(
		`${dir}/AppIcon-1024-tinted.png`,
		iconSvg({ art: ART.monoWhite, scale: 0.76, shape: "full", fill: BLACK }),
		1024,
		{ opaque: true },
	);
	const image = (filename: string, appearance?: string) => ({
		...(appearance
			? { appearances: [{ appearance: "luminosity", value: appearance }] }
			: {}),
		filename,
		idiom: "universal",
		platform: "ios",
		size: "1024x1024",
	});
	await writeFile(
		`${dir}/Contents.json`,
		json({
			images: [
				image("AppIcon-1024.png"),
				image("AppIcon-1024-dark.png", "dark"),
				image("AppIcon-1024-tinted.png", "tinted"),
			],
			info: { author: "xcode", version: 1 },
		}),
	);
}

async function macos() {
	const at = (px: number) => {
		const small = px < SMALL_MARK_BELOW;
		return render(
			iconSvg({
				art: small ? ART.small : ART.light,
				scale: small ? 0.7 : 0.76,
				shape: "macos",
			}),
			px,
		);
	};
	const png: [string, number][] = [
		["ic11", 32],
		["ic12", 64],
		["ic07", 128],
		["ic13", 256],
		["ic08", 256],
		["ic14", 512],
		["ic09", 512],
		["ic10", 1024],
	];
	const entries = [
		{ type: "ic04", data: icnsArgb(at(16)) },
		{ type: "ic05", data: icnsArgb(at(32)) },
		...png.map(([type, px]) => ({ type, data: at(px).png })),
	];
	await writeFile("macos/canvist.icns", icns(entries));
}

async function windows() {
	const sizes = [16, 24, 32, 48, 64, 128, 256];
	await writeFile(
		"windows/canvist.ico",
		ico(sizes.map((size) => ({ size, png: render(tileIcon(size), size).png }))),
	);
}

async function android() {
	const densities: [string, number][] = [
		["mdpi", 1],
		["hdpi", 1.5],
		["xhdpi", 2],
		["xxhdpi", 3],
		["xxxhdpi", 4],
	];
	// Adaptive layers are 108 dp; the mark stays inside the 66 dp safe zone.
	const foreground = iconSvg({ art: ART.light, scale: 0.56, shape: "none" });
	const monochrome = iconSvg({
		art: ART.monoWhite,
		scale: 0.56,
		shape: "none",
	});
	const background =
		`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><rect width="1" height="1" fill="${PAPER}"/></svg>`;
	for (const [name, factor] of densities) {
		const dir = `android/res/mipmap-${name}`;
		const legacy = 48 * factor, layer = 108 * factor;
		await png(`${dir}/ic_launcher.png`, tileIcon(legacy), legacy);
		await png(
			`${dir}/ic_launcher_round.png`,
			tileIcon(legacy, "circle"),
			legacy,
		);
		await png(`${dir}/ic_launcher_foreground.png`, foreground, layer);
		await png(`${dir}/ic_launcher_background.png`, background, layer);
		await png(`${dir}/ic_launcher_monochrome.png`, monochrome, layer);
	}
	const adaptive = `<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@color/ic_launcher_background" />
    <foreground android:drawable="@mipmap/ic_launcher_foreground" />
    <monochrome android:drawable="@mipmap/ic_launcher_monochrome" />
</adaptive-icon>
`;
	await writeFile("android/res/mipmap-anydpi-v26/ic_launcher.xml", adaptive);
	await writeFile(
		"android/res/mipmap-anydpi-v26/ic_launcher_round.xml",
		adaptive,
	);
	await writeFile(
		"android/res/values/ic_launcher_background.xml",
		`<?xml version="1.0" encoding="utf-8"?>
<resources>
    <color name="ic_launcher_background">${PAPER}</color>
</resources>
`,
	);
	await png(
		"android/play-store-512.png",
		iconSvg({ art: ART.light, scale: 0.76, shape: "full" }),
		512,
		{ opaque: true },
	);
}

async function flutter() {
	await png(
		"flutter/icon.png",
		iconSvg({ art: ART.light, scale: 0.76, shape: "full" }),
		1024,
		{ opaque: true },
	);
	await png(
		"flutter/icon-foreground.png",
		iconSvg({ art: ART.light, scale: 0.56, shape: "none" }),
		1024,
	);
	await png(
		"flutter/icon-monochrome.png",
		iconSvg({ art: ART.monoWhite, scale: 0.56, shape: "none" }),
		1024,
	);
	await png(
		"flutter/icon-ios-dark.png",
		iconSvg({ art: ART.dark, scale: 0.76, shape: "none" }),
		1024,
	);
	await png(
		"flutter/icon-ios-tinted.png",
		iconSvg({ art: ART.monoWhite, scale: 0.76, shape: "full", fill: BLACK }),
		1024,
		{ opaque: true },
	);
	await png("flutter/icon-desktop.png", tileIcon(1024), 1024);
	await writeFile(
		"flutter/flutter_launcher_icons.yaml",
		`# flutter_launcher_icons (0.14+) configuration for a canvist Flutter app.
# Copy the flutter/ folder into the app and adjust the paths if it lives elsewhere.
flutter_launcher_icons:
  image_path: "assets/brand/flutter/icon.png"
  android: true
  ios: true
  remove_alpha_ios: true
  image_path_ios_dark_transparent: "assets/brand/flutter/icon-ios-dark.png"
  image_path_ios_tinted_grayscale: "assets/brand/flutter/icon-ios-tinted.png"
  adaptive_icon_background: "${PAPER}"
  adaptive_icon_foreground: "assets/brand/flutter/icon-foreground.png"
  adaptive_icon_monochrome: "assets/brand/flutter/icon-monochrome.png"
  web:
    generate: true
    image_path: "assets/brand/flutter/icon-desktop.png"
    background_color: "${PAPER}"
    theme_color: "${INK}"
  windows:
    generate: true
    image_path: "assets/brand/flutter/icon-desktop.png"
    icon_size: 256
  macos:
    generate: true
    image_path: "assets/brand/flutter/icon-desktop.png"
`,
	);
}

async function linux() {
	for (const size of [16, 22, 24, 32, 48, 64, 128, 256, 512]) {
		await png(
			`linux/hicolor/${size}x${size}/apps/canvist.png`,
			tileIcon(size),
			size,
		);
	}
	await writeFile(
		"linux/hicolor/scalable/apps/canvist.svg",
		tileIcon(1024) + "\n",
	);
}

/** The mdBook docs site: favicons in its theme, and logo copies under `src/` so the book can serve them. */
async function docs() {
	await writeFile("../../docs/theme/favicon.svg", readMaster("favicon.svg"));
	await png(
		"../../docs/theme/favicon.png",
		iconSvg({ art: ART.small, scale: 1, shape: "none" }),
		32,
	);
	for (
		const name of [
			"lockup.svg",
			"lockup-dark.svg",
			"mark-small.svg",
			"mark-small-dark.svg",
		]
	) {
		await writeFile(`../../docs/src/images/brand/${name}`, readMaster(name));
	}
}

await printVectors();
await web();
await ios();
await macos();
await windows();
await android();
await flutter();
await linux();
await docs();

console.log(written.sort().join("\n"));
console.log(`\n${written.length} files written`);
