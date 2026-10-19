/**
 * Generates the Rive project for the website hero: the Wet Caret paints the
 * canvist wordmark, lets its drip fall once, then blinks.
 *
 * Run from the repository root (paths resolve against this file, so any working
 * directory works):
 *
 * ```sh
 * deno run --allow-read --allow-write website/rive/generate.ts
 * ```
 *
 * then compile with `rive website/rive --once`, which writes
 * `website/public/rive/wet-caret.riv`.
 *
 * The brand lockups in `assets/brand/vector/` are the source of truth. This
 * script parses their path data, converts every segment to the cubic or
 * straight vertices Rive's `PointsPath` understands, and writes `scene.rml` and
 * `rive.yaml` next to itself. The final frame of `Paint` is the static lockup,
 * so a host can show the SVG first and swap to the canvas without a jump.
 */

const HERE = new URL("./", import.meta.url);
const BRAND = new URL("../../assets/brand/vector/", import.meta.url);

/** Artboard pixels per lockup unit: the 240.1 x 64 lockup at 4x. */
const SCALE = 4;
const ARTBOARD_WIDTH = 960;
const ARTBOARD_HEIGHT = 256;

const FPS = 60;
/** Frames of the `Paint` one-shot. */
const PAINT_FRAMES = 100;
/** The caret overshoots its resting place by this much (artboard px), then settles. */
const OVERSHOOT = 4;
const SWEEP_PEAK_FRAME = 56;
const SWEEP_SETTLE_FRAME = 68;
const DRIP_START_FRAME = 64;
const DRIP_END_FRAME = 96;
type Ease = readonly [number, number, number, number];
/** Fast off the mark, long settling arrival (CSS cubic-bezier control points). */
const SWEEP_EASE: Ease = [0.3, 0.6, 0.25, 1];
/** Back from the overshoot onto the resting place. */
const SETTLE_EASE: Ease = [0.45, 0, 0.55, 1];
/** The drop hangs for a moment, then runs and comes to rest. */
const DRIP_EASE: Ease = [0.55, 0, 0.3, 1];
/** Frames of the `Blink` loop, and the frame the caret hides on (0.53 s visible). */
const BLINK_FRAMES = 60;
const BLINK_HIDE_FRAME = 32;

// ---------------------------------------------------------------------------
// Geometry

interface Vec {
	readonly x: number;
	readonly y: number;
}

interface LineSegment {
	readonly kind: "line";
	readonly to: Vec;
}

interface CubicSegment {
	readonly kind: "cubic";
	readonly c1: Vec;
	readonly c2: Vec;
	readonly to: Vec;
}

type Segment = LineSegment | CubicSegment;

interface Subpath {
	readonly start: Vec;
	readonly segments: Segment[];
	closed: boolean;
}

const vec = (x: number, y: number): Vec => ({ x, y });
const add = (a: Vec, b: Vec): Vec => vec(a.x + b.x, a.y + b.y);
const sub = (a: Vec, b: Vec): Vec => vec(a.x - b.x, a.y - b.y);
const mul = (a: Vec, s: number): Vec => vec(a.x * s, a.y * s);
const lerp = (a: Vec, b: Vec, t: number): Vec => add(a, mul(sub(b, a), t));
const near = (a: Vec, b: Vec): boolean =>
	Math.hypot(a.x - b.x, a.y - b.y) < 1e-4;

// ---------------------------------------------------------------------------
// SVG path data parser

/**
 * Reads SVG path data: every command in absolute and relative form, implicit
 * repeated commands, compact numbers (`.6-1.41.72`) and compact arc flags.
 * Quadratics are raised to cubics and arcs are split into cubics of at most a
 * quarter turn, so the result holds only lines and cubics.
 */
class PathReader {
	readonly #d: string;
	#i = 0;

	constructor(d: string) {
		this.#d = d;
	}

	#skipSeparators(): void {
		while (this.#i < this.#d.length && /[\s,]/.test(this.#d[this.#i])) {
			this.#i++;
		}
	}

	/** The next command letter, or undefined when the next token is a number or the end. */
	peekCommand(): string | undefined {
		this.#skipSeparators();
		const c = this.#d[this.#i];
		return c !== undefined && /[MmLlHhVvCcSsQqTtAaZz]/.test(c) ? c : undefined;
	}

	atEnd(): boolean {
		this.#skipSeparators();
		return this.#i >= this.#d.length;
	}

	command(): string {
		const c = this.peekCommand();
		if (c === undefined) {
			throw new Error(`expected a command at ${this.#i} in path data`);
		}
		this.#i++;
		return c;
	}

	number(): number {
		this.#skipSeparators();
		const match = /^[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/.exec(
			this.#d.slice(this.#i),
		);
		if (match === null) {
			throw new Error(`expected a number at ${this.#i} in path data`);
		}
		this.#i += match[0].length;
		return Number(match[0]);
	}

	/** An arc flag is a single `0` or `1`, which may run straight into the next token. */
	flag(): boolean {
		this.#skipSeparators();
		const c = this.#d[this.#i];
		if (c !== "0" && c !== "1") {
			throw new Error(`expected an arc flag at ${this.#i} in path data`);
		}
		this.#i++;
		return c === "1";
	}
}

/** Converts one SVG elliptical arc to cubics (SVG 1.1 implementation notes, F.6.5). */
function arcToCubics(
	p0: Vec,
	radiusX: number,
	radiusY: number,
	rotationDegrees: number,
	largeArc: boolean,
	sweep: boolean,
	p1: Vec,
): Segment[] {
	if (near(p0, p1)) return [];
	let rx = Math.abs(radiusX);
	let ry = Math.abs(radiusY);
	if (rx === 0 || ry === 0) return [{ kind: "line", to: p1 }];

	const phi = (rotationDegrees * Math.PI) / 180;
	const cos = Math.cos(phi);
	const sin = Math.sin(phi);
	const dx = (p0.x - p1.x) / 2;
	const dy = (p0.y - p1.y) / 2;
	const x1 = cos * dx + sin * dy;
	const y1 = -sin * dx + cos * dy;

	const lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
	if (lambda > 1) {
		rx *= Math.sqrt(lambda);
		ry *= Math.sqrt(lambda);
	}

	const numerator = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
	const denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
	const coefficient = (largeArc === sweep ? -1 : 1) *
		Math.sqrt(Math.max(0, numerator / denominator));
	const cxPrime = (coefficient * rx * y1) / ry;
	const cyPrime = (-coefficient * ry * x1) / rx;
	const cx = cos * cxPrime - sin * cyPrime + (p0.x + p1.x) / 2;
	const cy = sin * cxPrime + cos * cyPrime + (p0.y + p1.y) / 2;

	const angle = (ux: number, uy: number, vx: number, vy: number): number =>
		Math.atan2(ux * vy - uy * vx, ux * vx + uy * vy);
	const theta = angle(1, 0, (x1 - cxPrime) / rx, (y1 - cyPrime) / ry);
	let delta = angle(
		(x1 - cxPrime) / rx,
		(y1 - cyPrime) / ry,
		(-x1 - cxPrime) / rx,
		(-y1 - cyPrime) / ry,
	);
	if (!sweep && delta > 0) delta -= 2 * Math.PI;
	if (sweep && delta < 0) delta += 2 * Math.PI;

	const count = Math.max(1, Math.ceil(Math.abs(delta) / (Math.PI / 2) - 1e-9));
	const step = delta / count;
	const k = (4 / 3) * Math.tan(step / 4);
	const map = (ux: number, uy: number): Vec =>
		vec(cx + rx * cos * ux - ry * sin * uy, cy + rx * sin * ux + ry * cos * uy);

	const segments: Segment[] = [];
	for (let n = 0; n < count; n++) {
		const a = theta + n * step;
		const b = a + step;
		segments.push({
			kind: "cubic",
			c1: map(Math.cos(a) - k * Math.sin(a), Math.sin(a) + k * Math.cos(a)),
			c2: map(Math.cos(b) + k * Math.sin(b), Math.sin(b) - k * Math.cos(b)),
			to: n === count - 1 ? p1 : map(Math.cos(b), Math.sin(b)),
		});
	}
	return segments;
}

function parsePath(d: string): Subpath[] {
	const reader = new PathReader(d);
	const subpaths: Subpath[] = [];
	let current: Subpath | undefined;
	let point = vec(0, 0);
	let start = vec(0, 0);
	/** The last control point, for the reflection in S and T. */
	let lastCubicControl: Vec | undefined;
	let lastQuadControl: Vec | undefined;
	let command = "";

	const open = (): Subpath => {
		if (current === undefined) {
			current = { start: point, segments: [], closed: false };
			subpaths.push(current);
		}
		return current;
	};
	const line = (to: Vec): void => {
		open().segments.push({ kind: "line", to });
		point = to;
	};
	const cubic = (c1: Vec, c2: Vec, to: Vec): void => {
		open().segments.push({ kind: "cubic", c1, c2, to });
		lastCubicControl = c2;
		point = to;
	};
	const quad = (q: Vec, to: Vec): void => {
		cubic(lerp(point, q, 2 / 3), lerp(to, q, 2 / 3), to);
		lastQuadControl = q;
	};

	while (!reader.atEnd()) {
		const explicit = reader.peekCommand();
		if (explicit !== undefined) {
			command = reader.command();
		} else if (command === "" || command === "Z" || command === "z") {
			throw new Error("path data has numbers with no command");
		} else if (command === "M") {
			command = "L";
		} else if (command === "m") {
			command = "l";
		}

		const relative = command === command.toLowerCase();
		const at = (
			x: number,
			y: number,
		): Vec => (relative ? vec(point.x + x, point.y + y) : vec(x, y));
		const upper = command.toUpperCase();
		const cubicControl = lastCubicControl;
		const quadControl = lastQuadControl;
		lastCubicControl = undefined;
		lastQuadControl = undefined;

		switch (upper) {
			case "M": {
				point = at(reader.number(), reader.number());
				start = point;
				current = undefined;
				open();
				break;
			}
			case "L":
				line(at(reader.number(), reader.number()));
				break;
			case "H": {
				const x = reader.number();
				line(vec(relative ? point.x + x : x, point.y));
				break;
			}
			case "V": {
				const y = reader.number();
				line(vec(point.x, relative ? point.y + y : y));
				break;
			}
			case "C": {
				const c1 = at(reader.number(), reader.number());
				const c2 = at(reader.number(), reader.number());
				cubic(c1, c2, at(reader.number(), reader.number()));
				break;
			}
			case "S": {
				const c1 = cubicControl === undefined
					? point
					: sub(mul(point, 2), cubicControl);
				const c2 = at(reader.number(), reader.number());
				cubic(c1, c2, at(reader.number(), reader.number()));
				break;
			}
			case "Q": {
				const q = at(reader.number(), reader.number());
				quad(q, at(reader.number(), reader.number()));
				break;
			}
			case "T": {
				const q = quadControl === undefined
					? point
					: sub(mul(point, 2), quadControl);
				quad(q, at(reader.number(), reader.number()));
				break;
			}
			case "A": {
				const rx = reader.number();
				const ry = reader.number();
				const rotation = reader.number();
				const largeArc = reader.flag();
				const sweep = reader.flag();
				const to = at(reader.number(), reader.number());
				for (
					const segment of arcToCubics(
						point,
						rx,
						ry,
						rotation,
						largeArc,
						sweep,
						to,
					)
				) {
					open().segments.push(segment);
				}
				point = to;
				break;
			}
			case "Z": {
				open().closed = true;
				point = start;
				current = undefined;
				break;
			}
			default:
				throw new Error(`unsupported path command ${command}`);
		}
	}
	return subpaths;
}

function scaleSubpath(path: Subpath, s: number): Subpath {
	const scale = (p: Vec): Vec => mul(p, s);
	return {
		start: scale(path.start),
		closed: path.closed,
		segments: path.segments.map((segment): Segment =>
			segment.kind === "line" ? { kind: "line", to: scale(segment.to) } : {
				kind: "cubic",
				c1: scale(segment.c1),
				c2: scale(segment.c2),
				to: scale(segment.to),
			}
		),
	};
}

// ---------------------------------------------------------------------------
// Rive vertices

type RiveVertex =
	| { readonly kind: "straight"; readonly at: Vec }
	| {
		readonly kind: "detached";
		readonly at: Vec;
		readonly inRotation: number;
		readonly inDistance: number;
		readonly outRotation: number;
		readonly outDistance: number;
	};

interface RivePath {
	readonly vertices: RiveVertex[];
	readonly isClosed: boolean;
	readonly isClockwise: boolean;
}

/** A handle as Rive stores it: the control point's angle and distance from its vertex. */
function polar(
	from: Vec,
	to: Vec | undefined,
): { rotation: number; distance: number } {
	if (to === undefined) return { rotation: 0, distance: 0 };
	const d = sub(to, from);
	const distance = Math.hypot(d.x, d.y);
	return distance < 1e-9
		? { rotation: 0, distance: 0 }
		: { rotation: Math.atan2(d.y, d.x), distance };
}

/**
 * Signed area of the contour, positive when it runs clockwise on screen (y
 * grows downward). Cubics are sampled finely enough for the sign to be exact
 * for any non-degenerate outline.
 */
function signedArea(path: Subpath): number {
	const points: Vec[] = [path.start];
	let from = path.start;
	for (const segment of path.segments) {
		if (segment.kind === "line") {
			points.push(segment.to);
		} else {
			for (let n = 1; n <= 16; n++) {
				const t = n / 16;
				const u = 1 - t;
				points.push(
					add(
						add(mul(from, u * u * u), mul(segment.c1, 3 * u * u * t)),
						add(mul(segment.c2, 3 * u * t * t), mul(segment.to, t * t * t)),
					),
				);
			}
		}
		from = segment.to;
	}
	let area = 0;
	for (let n = 0; n < points.length; n++) {
		const a = points[n];
		const b = points[(n + 1) % points.length];
		area += a.x * b.y - b.x * a.y;
	}
	return area / 2;
}

/**
 * Turns a subpath into Rive vertices. Vertex `i` takes its in handle from the
 * segment ending on it and its out handle from the segment leaving it; a line
 * contributes a zero-length handle, and a vertex between two lines is a plain
 * `StraightVertex`. A closed subpath whose last segment already returns to the
 * start drops that duplicate point instead of adding a zero-length closing line.
 */
function toRivePath(path: Subpath): RivePath {
	const points: Vec[] = [
		path.start,
		...path.segments.map((segment) => segment.to),
	];
	const segments: Segment[] = [...path.segments];
	if (path.closed) {
		if (near(points[points.length - 1], path.start)) {
			points.pop();
		} else {
			segments.push({ kind: "line", to: path.start });
		}
	}

	const count = points.length;
	const vertices = points.map((at, i): RiveVertex => {
		const incoming = i > 0
			? segments[i - 1]
			: path.closed
			? segments[segments.length - 1]
			: undefined;
		const outgoing = i < segments.length ? segments[i] : undefined;
		const inHandle = incoming?.kind === "cubic" ? incoming.c2 : undefined;
		const outHandle = outgoing?.kind === "cubic" ? outgoing.c1 : undefined;
		if (inHandle === undefined && outHandle === undefined) {
			return { kind: "straight", at };
		}
		const handleIn = polar(at, inHandle);
		const handleOut = polar(at, outHandle);
		return {
			kind: "detached",
			at,
			inRotation: handleIn.rotation,
			inDistance: handleIn.distance,
			outRotation: handleOut.rotation,
			outDistance: handleOut.distance,
		};
	});
	if (path.closed && segments.length !== count) {
		throw new Error(
			`closed contour has ${segments.length} segments for ${count} vertices`,
		);
	}
	return { vertices, isClosed: path.closed, isClockwise: signedArea(path) > 0 };
}

// ---------------------------------------------------------------------------
// Brand sources

interface Lockup {
	readonly wordmark: { readonly fill: string; readonly d: string };
	readonly caret: { readonly fill: string; readonly d: string };
}

async function readLockup(file: string): Promise<Lockup> {
	const svg = await Deno.readTextFile(new URL(file, BRAND));
	const paths = [
		...svg.matchAll(/<path\s+fill="(#[0-9A-Fa-f]{6})"\s+d="([^"]+)"/g),
	].map((m) => ({
		fill: m[1],
		d: m[2],
	}));
	if (paths.length !== 2) {
		throw new Error(
			`${file}: expected the wordmark and caret paths, found ${paths.length}`,
		);
	}
	return { wordmark: paths[0], caret: paths[1] };
}

const light = await readLockup("lockup.svg");
const dark = await readLockup("lockup-dark.svg");
if (light.wordmark.d !== dark.wordmark.d || light.caret.d !== dark.caret.d) {
	throw new Error("lockup.svg and lockup-dark.svg must share geometry");
}

const wordmarkPaths = parsePath(light.wordmark.d).map((p) =>
	toRivePath(scaleSubpath(p, SCALE))
);
const caretSubpaths = parsePath(light.caret.d);
if (caretSubpaths.length !== 1) {
	throw new Error("the caret is expected to be one contour");
}
const caretPath = toRivePath(scaleSubpath(caretSubpaths[0], SCALE));

/**
 * Extent of the caret's vertices in artboard px. The drop's curve bulges a
 * little past its lowest vertex, which the masks' margin covers.
 */
const caretXs = caretPath.vertices.map((v) => v.at.x);
const caretYs = caretPath.vertices.map((v) => v.at.y);
const caretLeft = Math.min(...caretXs);
const caretRight = Math.max(...caretXs);
/** The stem's centre line runs through the apex of its rounded top. */
const caretCenterX =
	caretPath.vertices.reduce((top, v) => (v.at.y < top.at.y ? v : top)).at.x;
const caretBottom = Math.max(...caretYs);
/** The stem's straight right side ends here; below it is the drip. */
const stemFootY = 50 * SCALE;

const wordmarkLeft = Math.min(
	...wordmarkPaths.flatMap((p) => p.vertices.map((v) => v.at.x)),
);
/** The caret starts with its centre a little left of the "c", so nothing is painted yet. */
const sweepStartX = wordmarkLeft - 1.3 * SCALE - caretCenterX;

// ---------------------------------------------------------------------------
// RML

type Attributes = Record<string, string | number | boolean | undefined>;

interface Element {
	readonly tag: string;
	readonly attributes: Attributes;
	readonly children: Element[];
}

const el = (
	tag: string,
	attributes: Attributes = {},
	children: Element[] = [],
): Element => ({
	tag,
	attributes,
	children,
});

/** Shortest exact-enough decimal: 4 places in artboard px is far below a device pixel. */
function num(value: number): string {
	const rounded = Math.round(value * 1e4) / 1e4;
	return Object.is(rounded, -0) ? "0" : String(rounded);
}

function serialize(element: Element, depth = 0): string {
	const indent = "    ".repeat(depth);
	const attributes = Object.entries(element.attributes)
		.filter((entry): entry is [string, string | number | boolean] =>
			entry[1] !== undefined
		)
		.map(([key, value]) =>
			` ${key}="${typeof value === "number" ? num(value) : String(value)}"`
		)
		.join("");
	if (element.children.length === 0) {
		return `${indent}<${element.tag}${attributes}/>`;
	}
	const children = element.children.map((child) => serialize(child, depth + 1))
		.join("\n");
	return `${indent}<${element.tag}${attributes}>\n${children}\n${indent}</${element.tag}>`;
}

/** Ids are `0:n`; each artboard gets its own block so the two never collide. */
class Ids {
	#next: number;
	constructor(base: number) {
		this.#next = base;
	}
	next(): string {
		return `0:${this.#next++}`;
	}
}

const argb = (hex: string): string => `FF${hex.slice(1).toUpperCase()}`;

function pointsPath(path: RivePath, name: string, ids: Ids): Element {
	return el(
		"PointsPath",
		{
			isClosed: path.isClosed,
			isClockwise: path.isClockwise,
			name,
			id: ids.next(),
		},
		path.vertices.map((v) =>
			v.kind === "straight"
				? el("StraightVertex", { x: v.at.x, y: v.at.y, id: ids.next() })
				: el("CubicDetachedVertex", {
					x: v.at.x,
					y: v.at.y,
					inRotation: v.inRotation,
					inDistance: v.inDistance,
					outRotation: v.outRotation,
					outDistance: v.outDistance,
					id: ids.next(),
				})
		),
	);
}

interface Key {
	readonly frame: number;
	readonly value: number;
	/** Easing of the segment that starts at this key; omitted means hold. */
	readonly ease?: Ease;
}

function keyed(
	objectId: string,
	propertyKey: number,
	keys: readonly Key[],
	ids: Ids,
): Element {
	return el("KeyedObject", { objectId, id: ids.next() }, [
		el(
			"KeyedProperty",
			{ propertyKey, id: ids.next() },
			keys.map((key) =>
				el(
					"KeyFrameDouble",
					{
						frame: key.frame,
						value: key.value,
						interpolationType: key.ease === undefined ? "hold" : "cubic",
						id: ids.next(),
					},
					key.ease === undefined ? [] : [
						el("CubicEaseInterpolator", {
							x1: key.ease[0],
							y1: key.ease[1],
							x2: key.ease[2],
							y2: key.ease[3],
							id: ids.next(),
						}),
					],
				)
			),
		),
	]);
}

// Property keys, from `rive schema`.
const KEY_X = 13; // Node.x
const KEY_SCALE_Y = 17; // TransformComponent.scaleY
const KEY_OPACITY = 18; // WorldTransformComponent.opacity

/** The drip piece overlaps the stem by this much so no hairline opens between them. */
const DRIP_OVERLAP = 1 * SCALE;
/** Masks extend past the caret on every side by this margin so anti-aliasing is never cut. */
const MASK_MARGIN = 4 * SCALE;
const caretPadLeft = caretLeft - MASK_MARGIN;
const caretPadWidth = caretRight - caretLeft + 2 * MASK_MARGIN;

/** The same contour moved vertically; handles are polar, so only positions change. */
function translatePath(path: RivePath, dy: number): RivePath {
	return {
		...path,
		vertices: path.vertices.map((v) => ({
			...v,
			at: vec(v.at.x, v.at.y + dy),
		})),
	};
}

/** A paint-less rectangle used only as a clipping source. */
function mask(
	name: string,
	id: string,
	left: number,
	top: number,
	width: number,
	height: number,
	ids: Ids,
): Element {
	return el("Shape", { name, id }, [
		el("Rectangle", {
			x: left,
			y: top,
			width,
			height,
			originX: 0,
			originY: 0,
			name: `${name} Path`,
			id: ids.next(),
		}),
	]);
}

function artboard(
	name: string,
	theme: Lockup,
	base: number,
	stageY: number,
): Element {
	const ids = new Ids(base);
	const artboardId = ids.next();
	const styleId = ids.next();
	const stateMachineId = ids.next();
	const caretId = ids.next();
	const caretPaintId = ids.next();
	const piecesId = ids.next();
	const stemMaskId = ids.next();
	const dripId = ids.next();
	const dripMaskId = ids.next();
	const revealMaskId = ids.next();
	const paintId = ids.next();
	const blinkId = ids.next();
	const repaintId = ids.next();
	const paintStateId = ids.next();
	const repaintStateId = ids.next();
	const blinkStateId = ids.next();

	const fill = (hex: string): Element =>
		el("Fill", { fillRule: "nonZero", name: "Fill", id: ids.next() }, [
			el("SolidColor", {
				colorValue: argb(hex),
				name: "Color",
				id: ids.next(),
			}),
		]);

	// The caret group moves as one. `Caret Paint` is the brand caret as a single
	// exact contour: it is the resting pose and the one that blinks. While the
	// caret is painting, `Caret Pieces` stands in for it: the stem cut at its
	// foot, and the drip (the same contour cut just above the foot) in a group
	// pivoting on the foot so it can grow downward. The reveal mask's right edge
	// is the stem's centre line, so the wordmark appears under the caret. Masks
	// carry no paint and never draw.
	const caret = el("Node", { name: "Caret", id: caretId }, [
		el("Shape", { name: "Caret Paint", id: caretPaintId }, [
			pointsPath(caretPath, "Caret Path", ids),
			fill(theme.caret.fill),
		]),
		el("Node", { opacity: 0, name: "Caret Pieces", id: piecesId }, [
			el("Shape", { name: "Stem", id: ids.next() }, [
				pointsPath(caretPath, "Stem Path", ids),
				fill(theme.caret.fill),
				el("ClippingShape", {
					sourceId: stemMaskId,
					name: "Stem Clip",
					id: ids.next(),
				}),
			]),
			el("Node", { y: stemFootY, name: "Drip", id: dripId }, [
				el("Shape", { name: "Drop", id: ids.next() }, [
					pointsPath(translatePath(caretPath, -stemFootY), "Drop Path", ids),
					fill(theme.caret.fill),
					el("ClippingShape", {
						sourceId: dripMaskId,
						name: "Drop Clip",
						id: ids.next(),
					}),
				]),
				mask(
					"Drop Mask",
					dripMaskId,
					caretPadLeft,
					-DRIP_OVERLAP,
					caretPadWidth,
					caretBottom - stemFootY + DRIP_OVERLAP + MASK_MARGIN,
					ids,
				),
			]),
			mask(
				"Stem Mask",
				stemMaskId,
				caretPadLeft,
				0,
				caretPadWidth,
				stemFootY,
				ids,
			),
		]),
		mask(
			"Reveal Mask",
			revealMaskId,
			caretCenterX - 3 * ARTBOARD_WIDTH,
			-ARTBOARD_HEIGHT,
			3 * ARTBOARD_WIDTH,
			3 * ARTBOARD_HEIGHT,
			ids,
		),
	]);

	const wordmark = el("Shape", { name: "Wordmark", id: ids.next() }, [
		...wordmarkPaths.map((path, i) =>
			pointsPath(path, `Contour ${i + 1}`, ids)
		),
		fill(theme.wordmark.fill),
		el("ClippingShape", {
			sourceId: revealMaskId,
			name: "Reveal Clip",
			id: ids.next(),
		}),
	]);

	const exitAtEnd = {
		enableExitTime: true,
		exitTimeIsPercetange: true,
		exitTime: 100,
	};
	const stateMachine = el("StateMachine", {
		name: "Lockup",
		id: stateMachineId,
	}, [
		el("StateMachineTrigger", { name: "repaint", id: repaintId }),
		el("StateMachineLayer", { name: "Caret", id: ids.next() }, [
			el("AnyState", { x: 0, y: 160, id: ids.next() }, [
				el("StateTransition", { stateToId: paintStateId, id: ids.next() }, [
					el("TransitionTriggerCondition", {
						inputId: repaintId,
						id: ids.next(),
					}),
				]),
			]),
			el("ExitState", { x: 640, y: 0, id: ids.next() }),
			el("EntryState", { x: 0, y: 0, id: ids.next() }, [
				el("StateTransition", { stateToId: paintStateId, id: ids.next() }),
			]),
			// `repaint` while already painting cannot re-enter the same state, so it
			// hops to a twin state playing the same timeline from its first frame.
			el("AnimationState", {
				x: 160,
				y: 0,
				animationId: paintId,
				reset: true,
				stateName: "Paint",
				id: paintStateId,
			}, [
				el("StateTransition", {
					stateToId: blinkStateId,
					...exitAtEnd,
					id: ids.next(),
				}),
				el("StateTransition", { stateToId: repaintStateId, id: ids.next() }, [
					el("TransitionTriggerCondition", {
						inputId: repaintId,
						id: ids.next(),
					}),
				]),
			]),
			el("AnimationState", {
				x: 160,
				y: 160,
				animationId: paintId,
				reset: true,
				stateName: "Repaint",
				id: repaintStateId,
			}, [
				el("StateTransition", {
					stateToId: blinkStateId,
					...exitAtEnd,
					id: ids.next(),
				}),
			]),
			el("AnimationState", {
				x: 400,
				y: 0,
				animationId: blinkId,
				reset: true,
				stateName: "Blink",
				id: blinkStateId,
			}),
		]),
	]);

	// Both timelines key the same four properties, so whichever one is playing
	// fully decides the pose. Opacity only ever switches (hold keys), never fades.
	const paint = el("LinearAnimation", {
		name: "Paint",
		fps: FPS,
		duration: PAINT_FRAMES,
		loopValue: "oneShot",
		id: paintId,
	}, [
		keyed(caretId, KEY_X, [
			{ frame: 0, value: sweepStartX, ease: SWEEP_EASE },
			{ frame: SWEEP_PEAK_FRAME, value: OVERSHOOT, ease: SETTLE_EASE },
			{ frame: SWEEP_SETTLE_FRAME, value: 0 },
		], ids),
		keyed(dripId, KEY_SCALE_Y, [
			{ frame: 0, value: 0 },
			{ frame: DRIP_START_FRAME, value: 0, ease: DRIP_EASE },
			{ frame: DRIP_END_FRAME, value: 1 },
		], ids),
		keyed(piecesId, KEY_OPACITY, [
			{ frame: 0, value: 1 },
			{ frame: DRIP_END_FRAME, value: 0 },
		], ids),
		keyed(caretPaintId, KEY_OPACITY, [
			{ frame: 0, value: 0 },
			{ frame: DRIP_END_FRAME, value: 1 },
		], ids),
	]);

	const blink = el("LinearAnimation", {
		name: "Blink",
		fps: FPS,
		duration: BLINK_FRAMES,
		loopValue: "loop",
		id: blinkId,
	}, [
		keyed(caretId, KEY_X, [{ frame: 0, value: 0 }], ids),
		keyed(dripId, KEY_SCALE_Y, [{ frame: 0, value: 1 }], ids),
		keyed(piecesId, KEY_OPACITY, [{ frame: 0, value: 0 }], ids),
		keyed(caretPaintId, KEY_OPACITY, [
			{ frame: 0, value: 1 },
			{ frame: BLINK_HIDE_FRAME, value: 0 },
		], ids),
	]);

	return el("Artboard", {
		x: 0,
		y: stageY,
		width: ARTBOARD_WIDTH,
		height: ARTBOARD_HEIGHT,
		defaultStateMachineId: stateMachineId,
		styleId,
		name,
		id: artboardId,
	}, [
		el("LayoutComponentStyle", { name: `${name} Style`, id: styleId }),
		// First sibling draws on top: the caret paints over the letters.
		caret,
		wordmark,
		stateMachine,
		paint,
		blink,
	]);
}

const scene = el("Rive", { version: 1, kind: "fragment" }, [
	artboard("Light", light, 100, 0),
	artboard("Dark", dark, 5000, ARTBOARD_HEIGHT + 100),
]);

const header =
	"<!-- Generated by generate.ts from assets/brand/vector/lockup*.svg. Do not edit by hand. -->\n";
await Deno.writeTextFile(
	new URL("scene.rml", HERE),
	header + serialize(scene) + "\n",
);
await Deno.writeTextFile(
	new URL("rive.yaml", HERE),
	[
		"# Generated by generate.ts. Compile with `rive website/rive --once`.",
		"name: wet-caret",
		"main: Light",
		"exclude:",
		"  - generate.ts",
		"output:",
		"  dir: ../public/rive",
		"",
	].join("\n"),
);

const holes = wordmarkPaths.filter((p) => !p.isClockwise).length;
console.log(
	`wordmark: ${wordmarkPaths.length} contours (${holes} counter-clockwise), ` +
		`${
			wordmarkPaths.reduce((n, p) => n + p.vertices.length, 0)
		} vertices; caret: ${caretPath.vertices.length} vertices`,
);
console.log(
	`sweep starts at x ${num(sweepStartX)}, caret centre ${
		num(caretCenterX)
	}, stem foot ${stemFootY}, lowest caret vertex ${num(caretBottom)}`,
);
