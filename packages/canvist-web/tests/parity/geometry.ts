/**
 * Comparing where the canvas put things with where the browser put the
 * same document as server-rendered HTML.
 *
 * Everything is in document coordinates: relative to the top-left of the
 * canvas editor's content box and of `.cv-doc`.
 *
 * @module
 */
import type { Geometry } from "../../src/editor.ts";

/** The canvas editor's layout. */
export type CanvasGeometry = Geometry;

/** One measured difference between the canvas and the DOM. */
export interface Delta {
	readonly kind: "height" | "block" | "run" | "baseline" | "grapheme" | "line";
	readonly block: string;
	readonly what: string;
	readonly canvas: number;
	readonly dom: number;
}

/** The result of holding the canvas to the DOM. */
export interface GeometryReport {
	/** The largest difference of each kind, in CSS pixels. */
	readonly max: Readonly<Record<Delta["kind"], number>>;
	/** How many comparisons of each kind were made. */
	readonly counts: Readonly<Record<Delta["kind"], number>>;
	/** The worst differences, largest first. */
	readonly worst: readonly Delta[];
	/**
	 * Per block, the first caret stop in reading order that is off by more
	 * than an eighth of a pixel: where a drift starts, which the worst list
	 * hides once the drift carries on to the end of the line.
	 */
	readonly onsets: readonly Delta[];
}

/** A text position in the DOM for a UTF-16 offset in a block's model text. */
interface Segment {
	readonly node: Node;
	readonly start: number;
	readonly length: number;
	/** Whether the node is an atomic embed standing for one U+FFFC. */
	readonly atom: boolean;
}

/** Above this share of whole-pixel character edges, text rectangles count as enclosing. */
const ENCLOSING_SHARE = 0.9;
/** How many characters to sample when telling whether text rectangles are enclosing. */
const ENCLOSING_SAMPLE = 400;
/** Differences up to this many CSS px are float noise, left out of the worst list. */
const NOISE_PX = 1 / 64;
/** Caret stops off by more than this many CSS px start a drift. */
const ONSET_PX = 1 / 8;
/** How many of the worst differences a report keeps. */
const WORST_KEPT = 40;
/** How much of a run's text labels its deltas. */
const RUN_LABEL_LENGTH = 24;

type DeltaKind = Delta["kind"];
type BlockGeometry = CanvasGeometry["blocks"][number];
type RunGeometry = CanvasGeometry["runs"][number];
/** `[utf16, downstream x, upstream x, line top, line height]` at one grapheme boundary. */
type Stop = NonNullable<BlockGeometry["stops"]>[number];

/** What every comparison in one {@link compare} call shares. */
interface Comparison {
	/** The DOM rendering, `.cv-doc`. */
	readonly root: Element;
	/** The root's client rectangle, which DOM rectangles are made relative to. */
	readonly origin: DOMRect;
	/** Whether this browser rounds text rectangles out to whole pixels. */
	readonly enclosing: boolean;
	readonly record: (
		kind: DeltaKind,
		block: string,
		what: string,
		canvas: number,
		dom: number,
	) => void;
}

/** How a text block's element maps to model text: one atomic character, nothing, or its children's text. */
function elementRole(element: Element): "atom" | "skip" | "children" {
	const synthetic = element.classList.contains("cv-br");

	if (
		element.classList.contains("cv-e") ||
		(element.tagName === "BR" && !synthetic)
	) {
		return "atom";
	}

	return element.tagName === "INPUT" || synthetic ? "skip" : "children";
}

/**
 * The DOM pieces of a text block in model order. Text nodes map one to
 * one; embeds (`.cv-e`) and hard breaks (`<br>`) are one U+FFFC or newline
 * each; the synthetic trailing `<br class="cv-br">` and task checkboxes
 * are nothing.
 */
function segments(block: Element): Segment[] {
	const result: Segment[] = [];
	let offset = 0;
	const walk = (node: Node) => {
		if (node.nodeType === Node.TEXT_NODE) {
			const length = node.textContent?.length ?? 0;
			result.push({ node, start: offset, length, atom: false });
			offset += length;

			return;
		}

		if (!(node instanceof Element)) {
			return;
		}

		const role = elementRole(node);

		if (role === "atom") {
			result.push({ node, start: offset, length: 1, atom: true });
			offset += 1;

			return;
		}

		if (role === "skip") {
			return;
		}

		for (const child of node.childNodes) {
			walk(child);
		}
	};

	walk(block);

	return result;
}

/** The client rectangles of model text `start..end` in a block. */
function rangeRects(
	parts: readonly Segment[],
	start: number,
	end: number,
): DOMRect[] {
	const rects: DOMRect[] = [];

	for (const part of parts) {
		const from = Math.max(start, part.start);
		const to = Math.min(end, part.start + part.length);

		if (from >= to) {
			continue;
		}

		const range = document.createRange();

		if (part.atom) {
			range.selectNode(part.node);
		} else {
			range.setStart(part.node, from - part.start);
			range.setEnd(part.node, to - part.start);
		}

		rects.push(...range.getClientRects());
	}

	return rects.filter((rect) => rect.width > 0 || rect.height > 0);
}

/**
 * Whether this browser reports text ranges as enclosing whole-pixel
 * rectangles, as WebKit does, rather than with layout's fractions: true
 * when nearly every character's edges are whole numbers. (WebKit clips
 * characters at a line's end to the line, which can leave a fraction.)
 */
function enclosingTextRects(root: Element): boolean {
	const range = document.createRange();
	const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
	let whole = 0;
	let total = 0;

	for (
		let node = walker.nextNode();
		node !== null && total < ENCLOSING_SAMPLE;
		node = walker.nextNode()
	) {
		const length = node.textContent?.length ?? 0;

		for (
			let offset = 0;
			offset < length && total < ENCLOSING_SAMPLE;
			offset += 1
		) {
			range.setStart(node, offset);
			range.setEnd(node, offset + 1);
			const rect = range.getBoundingClientRect();
			whole += Number(
				Number.isInteger(rect.left) && Number.isInteger(rect.right),
			);
			total += 1;
		}
	}

	return total > 0 && whole / total > ENCLOSING_SHARE;
}

/**
 * The DOM's edge as a canvas value can be compared with: itself, or where
 * the browser rounds text rectangles out to whole pixels, the nearest point
 * of the pixel the edge must lie in.
 */
function edge(
	canvas: number,
	dom: number,
	side: "left" | "right",
	enclosing: boolean,
): number {
	if (!enclosing) {
		return dom;
	}

	const [low, high] = side === "left" ? [dom, dom + 1] : [dom - 1, dom];

	return Math.min(Math.max(canvas, low), high);
}

/** Hold the canvas `geometry` to the DOM rendering under `root` (`.cv-doc`). */
export function compare(
	geometry: CanvasGeometry,
	root: Element,
): GeometryReport {
	const enclosing = enclosingTextRects(root);
	const origin = root.getBoundingClientRect();
	const deltas: Delta[] = [];
	const counts = zeroes();
	const record = (
		kind: DeltaKind,
		block: string,
		what: string,
		canvas: number,
		dom: number,
	) => {
		counts[kind] += 1;
		deltas.push({ kind, block, what, canvas, dom });
	};
	const comparison: Comparison = { root, origin, enclosing, record };

	record("height", "", "document height", geometry.height, origin.height);

	for (const block of geometry.blocks) {
		compareBlock(block, comparison);
	}

	for (const run of geometry.runs) {
		compareRun(run, comparison);
	}

	return {
		max: largest(deltas),
		counts,
		worst: worst(deltas),
		onsets: onsets(deltas),
	};
}

/** Compare a block's box, then each of its graphemes, with the DOM's. */
function compareBlock(block: BlockGeometry, comparison: Comparison): void {
	const { root, origin, record } = comparison;
	const element = root.querySelector(`[data-id="${block.id}"]`);

	if (element === null || block.rect === null) {
		return;
	}

	const box = element.getBoundingClientRect();
	const [x, y, width, height] = block.rect;
	record("block", block.id, "left", x, box.left - origin.left);
	record("block", block.id, "top", y, box.top - origin.top);
	record("block", block.id, "width", width, box.width);
	record("block", block.id, "height", height, box.height);

	const stops = block.stops ?? [];
	const parts = segments(element);

	for (let index = 0; index + 1 < stops.length; index += 1) {
		const from = stops[index] ?? [0, 0, 0, 0, 0];
		const to = stops[index + 1] ?? [0, 0, 0, 0, 0];
		compareGrapheme(block, parts, from, to, comparison);
	}
}

/**
 * Compare the edges of the grapheme between the stops `from` and `to` with
 * its DOM box, and check the box sits on the line the canvas put it on.
 */
function compareGrapheme(
	block: BlockGeometry,
	parts: readonly Segment[],
	from: Stop,
	to: Stop,
	comparison: Comparison,
): void {
	const [start, downX, , lineTop, lineHeight] = from;
	const [end, , upX] = to;
	const grapheme = (block.text ?? "").slice(start, end);

	// Hanging and collapsed white space has no stable box.
	if (grapheme === "\n" || grapheme === "\t" || /^\s$/u.test(grapheme)) {
		return;
	}

	const rect = rangeRects(parts, start, end)[0];

	if (rect === undefined) {
		return;
	}

	const { origin, enclosing, record } = comparison;
	const label = `“${grapheme}” @${start}`;
	const left = Math.min(downX, upX);
	const right = Math.max(downX, upX);
	const domLeft = rect.left - origin.left;
	const domRight = rect.right - origin.left;
	record(
		"grapheme",
		block.id,
		`${label} left`,
		left,
		edge(left, domLeft, "left", enclosing),
	);
	record(
		"grapheme",
		block.id,
		`${label} right`,
		right,
		edge(right, domRight, "right", enclosing),
	);

	const middle = (rect.top + rect.bottom) / 2 - origin.top;
	const inside = middle >= lineTop && middle <= lineTop + lineHeight;
	record(
		"line",
		block.id,
		`${label} on its line`,
		0,
		inside ? 0 : Math.abs(middle - lineTop),
	);
}

/** Compare a run's edges and content top with the DOM line its text sits on. */
function compareRun(run: RunGeometry, comparison: Comparison): void {
	const { root, origin, record } = comparison;
	const element = root.querySelector(`[data-id="${run.block}"]`);

	if (element === null || run.text.trim() === "") {
		return;
	}

	const line = lineRects(run, element, origin);

	if (line.length === 0) {
		record(
			"run",
			run.block,
			`“${run.text}” is on no DOM line`,
			run.baseline,
			-1,
		);

		return;
	}

	compareRunEdges(run, line, comparison);
}

/** Compare a run's left and right edges and its content top with `line`, the DOM rectangles of its text. */
function compareRunEdges(
	run: RunGeometry,
	line: readonly DOMRect[],
	comparison: Comparison,
): void {
	const { origin, enclosing, record } = comparison;
	const left = Math.min(...line.map((rect) => rect.left)) - origin.left;
	const right = Math.max(...line.map((rect) => rect.right)) - origin.left;
	const label = `“${run.text.slice(0, RUN_LABEL_LENGTH)}”`;
	// Trailing spaces were left out of the DOM range: they sit after the
	// text, on the right of a left-to-right run and the left of a
	// right-to-left one, so only the other edge is comparable.
	const trailing = /\s$/u.test(run.text);

	if (!run.rtl || !trailing) {
		record(
			"run",
			run.block,
			`${label} left`,
			run.x,
			edge(run.x, left, "left", enclosing),
		);
	}

	if (run.rtl || !trailing) {
		const end = run.x + run.width;
		record(
			"run",
			run.block,
			`${label} right`,
			end,
			edge(end, right, "right", enclosing),
		);
	}

	// The content area's top: the baseline less the font's ascent.
	const top = Math.min(...line.map((rect) => rect.top)) - origin.top;
	const contentTop = run.baseline - run.ascent;
	record(
		"baseline",
		run.block,
		`${label} content top`,
		contentTop,
		edge(contentTop, top, "left", enclosing),
	);
}

/** The DOM rectangles of `run`'s text, less trailing spaces, on the line through its baseline. */
function lineRects(
	run: RunGeometry,
	element: Element,
	origin: DOMRect,
): DOMRect[] {
	const end = run.start + run.text.trimEnd().length;
	const rects = rangeRects(segments(element), run.start, end);

	return rects.filter((rect) =>
		rect.top - origin.top <= run.baseline &&
		rect.bottom - origin.top >= run.baseline
	);
}

/** A count or maximum of zero for every kind of delta. */
function zeroes(): Record<DeltaKind, number> {
	return { height: 0, block: 0, run: 0, baseline: 0, grapheme: 0, line: 0 };
}

/** How far apart the canvas and the DOM are. */
function size(delta: Delta): number {
	return Math.abs(delta.canvas - delta.dom);
}

/** The largest difference of each kind. */
function largest(deltas: readonly Delta[]): Record<DeltaKind, number> {
	const max = zeroes();

	for (const delta of deltas) {
		max[delta.kind] = Math.max(max[delta.kind], size(delta));
	}

	return max;
}

/** Each block's first caret stop off by more than {@link ONSET_PX}, in the order they were compared. */
function onsets(deltas: readonly Delta[]): Delta[] {
	const first = new Map<string, Delta>();

	for (const delta of deltas) {
		if (
			delta.kind === "grapheme" && size(delta) > ONSET_PX &&
			!first.has(delta.block)
		) {
			first.set(delta.block, delta);
		}
	}

	return [...first.values()];
}

/** The largest differences above float noise, largest first. */
function worst(deltas: readonly Delta[]): Delta[] {
	return deltas
		.filter((delta) => size(delta) > NOISE_PX)
		.sort((first, second) => size(second) - size(first))
		.slice(0, WORST_KEPT);
}
