/**
 * Text measurement with the browser's own text engine.
 *
 * Layout asks for every segment it is missing in one batch per relayout,
 * and caches the answers by (font, segment). This side measures a batch
 * with `measureText` on a canvas whose text properties match the painting
 * canvas and the CSS that server-rendered HTML declares, so a word is the
 * same width in all three.
 *
 * @module
 */
import type { HostMeasurer } from "../wasm/canvist_web.js";

/**
 * Code units below this are never part of a multi-character grapheme
 * cluster: combining marks start at U+0300, and joiners, variation
 * selectors, and surrogates are all above it.
 */
const SIMPLE_TEXT_LIMIT = 0x300;

/** Scripts whose letters join: Arabic, Syriac, Thaana, N'Ko, Mandaic, and Mongolian. */
const JOINING =
	/[\u0600-\u07ff\u0800-\u08ff\u1800-\u18af\ufb50-\ufdff\ufe70-\ufeff]/u;

/** Text whose width tells which face a family list resolves to. */
const RESOLUTION_SAMPLE =
	"The quick brown fox jumps over the lazy dog 0123456789";
/** How far the DOM and canvas widths of the sample may differ and still be one face. */
const SAME_FACE_TOLERANCE = 0.05;

/** Letter sequences that fonts commonly draw as one ligature glyph, longest first. */
const LIGATURE_CANDIDATES = ["ffi", "ffl", "ff", "fi", "fl", "fj", "ft", "Th"];
/** A space then letters that fonts with Times's metrics kern together. */
const SPACE_PAIRS = [" A", " T", " Y", " V", " W"];
/** How far, in CSS px at the probe size, a space pair must kern to count. */
const SPACE_KERN_TOLERANCE = 0.5;

/** The font size ligatures are probed at, in CSS px: large, so whole-pixel DOM rectangles barely round. */
const LIGATURE_PROBE_PX = 200;
/**
 * How far, as a share of the sequence's width, the DOM's caret edge inside
 * a sequence may sit from the canvas's before the sequence counts as a
 * ligature. Ligatures move it by 5% or more; WebKit's rounding at the
 * probe size moves it under 0.5%.
 */
const LIGATURE_TOLERANCE = 0.02;

/** Called after each batch with the fonts it used and the text in each. */
export type MeasureListener = (used: ReadonlyMap<string, string>) => void;

/**
 * The browser half of layout's `TextMeasurer`.
 *
 * Per-character advances come from measuring prefixes that end on grapheme
 * boundaries, so they sum to exactly the width `fillText` paints for the
 * whole segment (kerning and ligatures included), and caret stops fall
 * where the browser puts them. Each grapheme's advance goes to its first
 * scalar value; the rest of the cluster advances nothing.
 */
export class CanvasMeasurer implements HostMeasurer {
	readonly #document: Document;
	readonly #context: CanvasRenderingContext2D;
	readonly #fonts: string[] = [];
	readonly #graphemes = new Intl.Segmenter(undefined, {
		granularity: "grapheme",
	});
	readonly #listeners = new Set<MeasureListener>();
	/** Widths of single graphemes, by font and grapheme. */
	readonly #singles = new Map<string, number>();
	/** How each font id is drawn and measured, once resolved. */
	readonly #resolved = new Map<number, Resolution>();
	#font = "";
	#measured = 0;
	#calls = 0;

	constructor(document: Document = globalThis.document) {
		const context = document.createElement("canvas").getContext("2d");

		if (context === null) {
			throw new Error("canvist needs a 2D canvas context to measure text");
		}

		context.fontKerning = "normal";
		context.fontVariantCaps = "normal";
		context.textRendering = "auto";
		context.letterSpacing = "0px";
		context.wordSpacing = "0px";
		this.#document = document;
		this.#context = context;
	}

	/** Segments measured so far. */
	get measured(): number {
		return this.#measured;
	}

	/** Batches measured so far. */
	get calls(): number {
		return this.#calls;
	}

	/** Hear about the fonts and text each batch measured, e.g. to load web fonts for that text. */
	listen(listener: MeasureListener): () => void {
		this.#listeners.add(listener);

		return () => this.#listeners.delete(listener);
	}

	/** Forget cached widths and resolved faces, which change when a web font finishes loading. */
	forget(): void {
		this.#singles.clear();
		this.#resolved.clear();
		this.#font = "";
	}

	paintFont(id: number): string {
		return this.#resolution(id).font;
	}

	#resolution(id: number): Resolution {
		let resolution = this.#resolved.get(id);

		if (resolution === undefined) {
			resolution = this.#resolve(this.#css(id));
			this.#resolved.set(id, resolution);
		}

		return resolution;
	}

	/**
	 * A canvas `font` string that draws the face the DOM uses for `css`.
	 *
	 * Canvas and DOM text resolve a family list through the same fonts, but
	 * not always to the same face: WebKit's DOM skips `ui-monospace` in a
	 * list that its canvas honours. When the two disagree on a sample's
	 * width, the shortest tail of the family list that canvas draws as
	 * wide as the DOM draws the whole list stands in for it.
	 *
	 * When no tail matches, the face is right but its size is not: Firefox's
	 * canvas snaps fractional font sizes to steps its DOM does not use. The
	 * font is then drawn as asked, and measured by the DOM.
	 */
	#resolve(css: string): Resolution {
		const match = /^(\S+ \S+ \S+px) (.+)$/.exec(css);

		if (match === null) {
			return { font: css, domWidths: false, ligatures: [], spacesApart: false };
		}

		const face = this.#face(css, match);

		return {
			...face,
			ligatures: this.#ligatures(face.font, css),
			spacesApart: this.#spacesApart(face.font, css),
		};
	}

	/** The canvas font that draws `css`'s face, and whether its widths come from the DOM. */
	#face(
		css: string,
		match: RegExpExecArray,
	): Pick<Resolution, "font" | "domWidths"> {
		const [, prefix = "", list = ""] = match;
		const dom = this.#domWidth(css, RESOLUTION_SAMPLE);
		const families = splitFamilies(list);

		for (let start = 0; start < families.length; start += 1) {
			const candidate = `${prefix} ${families.slice(start).join(", ")}`;
			this.#context.font = candidate;

			if (
				Math.abs(this.#context.measureText(RESOLUTION_SAMPLE).width - dom) <=
					SAME_FACE_TOLERANCE
			) {
				this.#font = "";

				return { font: candidate, domWidths: false };
			}
		}

		this.#font = "";

		return { font: css, domWidths: true };
	}

	/** The width the DOM lays `text` out at in `font`. */
	#domWidth(font: string, text: string): number {
		const span = this.#document.createElement("span");
		span.setAttribute("aria-hidden", "true");
		span.style.cssText =
			`position:absolute;left:0;top:0;visibility:hidden;white-space:pre;font:${font};font-kerning:normal;font-variant:normal;letter-spacing:0;word-spacing:0`;
		span.textContent = text;
		this.#document.body.append(span);
		const width = span.getBoundingClientRect().width;
		span.remove();

		return width;
	}

	addFont(id: number, css: string): void {
		this.#fonts[id] = css;
	}

	measure(
		metrics: Uint32Array,
		lineFonts: Uint32Array,
		lineHeights: Float32Array,
		fonts: Uint32Array,
		text: string,
		lengths: Uint32Array,
	): Float32Array {
		const out: number[] = [];
		const used = new Map<string, string>();
		const boxes = [
			...Array.from(
				metrics,
				(id) => ({ font: this.#css(id), lineHeight: null }),
			),
			...Array.from(lineFonts, (id, index) => ({
				font: this.#css(id),
				lineHeight: lineHeights[index] ?? 0,
			})),
		];

		for (const box of this.#probe(boxes)) {
			out.push(box[0], box[1]);
		}

		for (const { font } of boxes) {
			used.set(font, used.get(font) ?? "");
		}

		let start = 0;
		const fromDom: DomSegment[] = [];

		for (let index = 0; index < lengths.length; index += 1) {
			const length = lengths[index] ?? 0;
			const segment = text.slice(start, start + length);
			const id = fonts[index] ?? 0;
			const font = this.#select(id);
			const outIndex = out.length;
			const joined = JOINING.test(segment);
			const domWidths = this.#resolution(id).domWidths;

			this.#pushAdvances(segment, joined, out, this.#resolution(id));

			if (joined || domWidths) {
				const canvas = this.#context.measureText(segment).width;
				fromDom.push({
					font,
					segment,
					at: outIndex,
					joined,
					width: domWidths ? null : canvas,
				});
			}

			used.set(font, (used.get(font) ?? "") + segment);
			start += length;
		}

		this.#fromDom(fromDom, out);

		this.#measured += lengths.length;
		this.#calls += 1;

		for (const listener of this.#listeners) {
			listener(used);
		}

		return Float32Array.from(out);
	}

	/** Push `segment`'s advances, or zero placeholders for the DOM to replace when its letters join. */
	#pushAdvances(
		segment: string,
		joined: boolean,
		out: number[],
		resolution: Resolution,
	): void {
		if (!joined) {
			const pieces = resolution.spacesApart
				? splitAtSpaces(segment)
				: [segment];

			for (const piece of pieces) {
				this.#advances(piece, out, resolution.ligatures);
			}

			return;
		}

		const start = out.length;
		const scalars = countScalars(segment);
		out.length += scalars;
		out.fill(0, start, start + scalars);
	}

	/**
	 * Advances the DOM has to supply, in one layout pass.
	 *
	 * - In scripts whose letters join, a prefix of an Arabic word measured
	 *   alone ends in a final form, not the joined form the letter has in the
	 *   word, and canvas ignores a zero width joiner that would keep it
	 *   joined. The DOM places each grapheme by its glyph in context, so its
	 *   grapheme boxes give the advances.
	 * - Where canvas and DOM disagree on a font's size (see `#resolve`), the
	 *   DOM's width of the whole segment is the one layout must wrap by; the
	 *   canvas advances are scaled to it.
	 *
	 * `width` is the total the advances must sum to, or `null` to take the
	 * DOM's.
	 */
	#fromDom(segments: readonly DomSegment[], out: number[]): void {
		if (segments.length === 0) {
			return;
		}

		const document = this.#document;
		const host = document.createElement("div");
		host.setAttribute("aria-hidden", "true");
		host.style.cssText =
			"position:absolute;left:0;top:0;width:0;height:0;overflow:hidden;visibility:hidden;pointer-events:none";
		const spans = segments.map(({ font, segment }) => {
			const span = document.createElement("div");
			span.style.cssText =
				`font:${font};white-space:pre;width:max-content;font-kerning:normal;font-variant:normal;letter-spacing:0;word-spacing:0`;
			span.textContent = segment;
			host.append(span);
			return span;
		});
		document.body.append(host);
		const range = document.createRange();

		segments.forEach((entry, index) => {
			const span = spans[index];
			const node = span?.firstChild;

			if (span === undefined || node === null || node === undefined) {
				return;
			}

			const total = entry.width ?? span.getBoundingClientRect().width;

			if (entry.joined) {
				this.#joinedAdvances(entry, range, node, total, out);
			}

			scale(out, entry.at, countScalars(entry.segment), total);
		});

		host.remove();
	}

	/**
	 * Write the advances of a segment whose letters join, read from `node`,
	 * its laid-out text. Each edge is the width of the prefix that ends
	 * there, read in the word's context: a browser that rounds rectangles to
	 * whole pixels (WebKit) then misplaces an edge by under a pixel, rather
	 * than accumulating each grapheme's rounding.
	 */
	#joinedAdvances(
		{ segment, at }: DomSegment,
		range: Range,
		node: Node,
		total: number,
		out: number[],
	): void {
		let scalar = 0;
		let previous = 0;

		for (
			const { index: offset, segment: cluster } of this.#graphemes.segment(
				segment,
			)
		) {
			const end = offset + cluster.length;
			range.setStart(node, 0);
			range.setEnd(node, end);
			const edge = end === segment.length
				? total
				: range.getBoundingClientRect().width;
			out[at + scalar] = edge - previous;
			previous = edge;
			scalar += countScalars(cluster);
		}
	}

	/** The CSS of font `id`. */
	#css(id: number): string {
		const font = this.#fonts[id];

		if (font === undefined) {
			throw new Error(`canvist asked to measure unknown font ${id}`);
		}

		return font;
	}

	/**
	 * Read inline boxes from the DOM, in one layout pass: for a font alone,
	 * its content area's ascent and descent; with a unitless line height,
	 * the space its line box keeps above and below the baseline.
	 *
	 * Canvas reports font metrics rounded to whole CSS pixels, but browsers
	 * lay out lines with their own rounding: Blink rounds ascent and descent
	 * to device pixels and floors the half-leading, WebKit keeps 1/64 px,
	 * and Firefox keeps floats. Reading the boxes from real DOM text makes
	 * the canvas's line positions the browser's, whatever its rules.
	 */
	#probe(
		boxes: readonly { font: string; lineHeight: number | null }[],
	): [number, number][] {
		if (boxes.length === 0) {
			return [];
		}

		const document = this.#document;
		const host = document.createElement("div");
		host.setAttribute("aria-hidden", "true");
		host.style.cssText =
			"position:absolute;left:0;top:0;width:0;height:0;overflow:hidden;visibility:hidden;pointer-events:none";
		const rows = boxes.map(({ font, lineHeight }) => {
			const row = document.createElement("div");
			// Unitless, as the server-rendered CSS writes it: browsers round a
			// length and a multiple of the font size to layout units differently.
			row.style.cssText = `font:${font};line-height:${
				lineHeight ?? "normal"
			};white-space:pre;width:max-content;font-kerning:normal;font-variant:normal;letter-spacing:0;word-spacing:0`;
			const baseline = document.createElement("span");
			baseline.style.cssText = "display:inline-block;width:0;height:0";
			row.append(baseline, "x");
			host.append(row);
			return { row, baseline };
		});
		document.body.append(host);

		const result = boxes.map(({ lineHeight }, index): [number, number] => {
			const { row, baseline } = rows[index] ?? rows[0]!;
			const top = row.getBoundingClientRect().top;
			const base = baseline.getBoundingClientRect().bottom - top;

			if (lineHeight !== null) {
				return [base, row.getBoundingClientRect().height - base];
			}

			const range = document.createRange();
			range.selectNodeContents(row.lastChild ?? row);
			const content = range.getBoundingClientRect();
			return [base - (content.top - top), content.bottom - top - base];
		});

		host.remove();

		return result;
	}

	/** Make font `id` current, as it is drawn, and return its CSS. */
	#select(id: number): string {
		const font = this.paintFont(id);

		if (font !== this.#font) {
			this.#context.font = font;
			this.#font = font;
		}

		return this.#css(id);
	}

	/**
	 * Push one advance per scalar value of `segment`.
	 *
	 * A shaper adds a kerning pair's adjustment to the first glyph's advance,
	 * and the browser places carets and selection edges by those advances.
	 * So the edge after grapheme `k` is the width of the first `k + 1`
	 * graphemes less the next grapheme's own width, and the last edge is the
	 * whole segment's width: the advances sum to what `fillText` paints, and
	 * each edge sits where the DOM puts it. Inside a ligature the browser
	 * splits the glyph's width instead, so edges there follow `ligatures`.
	 */
	#advances(
		segment: string,
		out: number[],
		ligatures: readonly Ligature[],
	): void {
		const clusters = this.#clusters(segment);
		const edges: number[] = [];
		let end = 0;

		for (let index = 0; index < clusters.length; index += 1) {
			end += (clusters[index] ?? "").length;
			edges.push(this.#edge(segment, end, clusters[index + 1]));
		}

		splitLigatures(clusters, edges, ligatures);
		let previous = 0;

		clusters.forEach((cluster, index) => {
			const edge = edges[index] ?? previous;
			out.push(edge - previous);
			previous = edge;

			for (let scalars = countScalars(cluster); scalars > 1; scalars -= 1) {
				out.push(0);
			}
		});
	}

	/**
	 * Whether the DOM lays out `css` without the kerning `canvasFont`'s
	 * canvas applies between a space and the letter after it. Browsers
	 * shape a run across its spaces only when they see the font kern them,
	 * and engines disagree on that, so both are measured at a large size.
	 */
	#spacesApart(canvasFont: string, css: string): boolean {
		const texts = SPACE_PAIRS.flatMap((pair) => [pair, pair.slice(1)]);
		const dom = this.#domWidths(atSize(css, LIGATURE_PROBE_PX), [
			" ",
			...texts,
		]);
		this.#context.font = atSize(canvasFont, LIGATURE_PROBE_PX);
		this.#font = "";
		const canvas = (text: string) => this.#context.measureText(text).width;
		const space = dom[0] ?? 0;

		return SPACE_PAIRS.some((pair, index) => {
			const domKern = (dom[1 + 2 * index] ?? 0) - space -
				(dom[2 + 2 * index] ?? 0);
			const canvasKern = canvas(pair) - canvas(" ") - canvas(pair.slice(1));

			return Math.abs(canvasKern) > SPACE_KERN_TOLERANCE &&
				Math.abs(domKern) < SPACE_KERN_TOLERANCE;
		});
	}

	/** The widths the DOM lays each of `texts` out at in `font`, in one layout pass. */
	#domWidths(font: string, texts: readonly string[]): number[] {
		const document = this.#document;
		const host = document.createElement("div");
		host.setAttribute("aria-hidden", "true");
		host.style.cssText =
			"position:absolute;left:0;top:0;width:0;height:0;overflow:hidden;visibility:hidden;pointer-events:none";
		const rows = texts.map((text) => {
			const row = document.createElement("div");
			row.style.cssText =
				`font:${font};white-space:pre;width:max-content;font-kerning:normal;font-variant:normal;letter-spacing:0;word-spacing:0`;
			row.textContent = text;
			host.append(row);
			return row;
		});
		document.body.append(host);
		const widths = rows.map((row) => row.getBoundingClientRect().width);
		host.remove();

		return widths;
	}

	/**
	 * The sequences `canvasFont` draws as ligatures, found by comparing caret
	 * edges inside each candidate in the DOM (in `css`) with the canvas's, at
	 * a large size so whole-pixel rectangles barely round.
	 */
	#ligatures(canvasFont: string, css: string): Ligature[] {
		const document = this.#document;
		const host = document.createElement("div");
		host.setAttribute("aria-hidden", "true");
		host.style.cssText =
			"position:absolute;left:0;top:0;width:0;height:0;overflow:hidden;visibility:hidden;pointer-events:none";
		const rows = LIGATURE_CANDIDATES.map((text) => {
			const row = document.createElement("div");
			row.style.cssText = `font:${
				atSize(css, LIGATURE_PROBE_PX)
			};white-space:pre;width:max-content;font-kerning:normal;font-variant:normal;letter-spacing:0;word-spacing:0`;
			row.textContent = text;
			host.append(row);
			return row;
		});
		document.body.append(host);
		this.#context.font = atSize(canvasFont, LIGATURE_PROBE_PX);
		this.#font = "";
		const range = document.createRange();
		const found: Ligature[] = [];

		LIGATURE_CANDIDATES.forEach((text, index) => {
			const node = rows[index]?.firstChild;

			if (node === null || node === undefined) {
				return;
			}

			const domWidth = rows[index]?.getBoundingClientRect().width ?? 0;
			const canvasWidth = this.#context.measureText(text).width;
			const edges: number[] = [];
			let ligated = false;

			for (let split = 1; split < text.length; split += 1) {
				range.setStart(node, 0);
				range.setEnd(node, split);
				const dom = range.getBoundingClientRect().width / domWidth;
				const canvas = (this.#context.measureText(text.slice(0, split + 1))
					.width - this.#context.measureText(text.charAt(split)).width) /
					canvasWidth;
				ligated ||= Math.abs(dom - canvas) > LIGATURE_TOLERANCE;
				edges.push(dom);
			}

			if (ligated && domWidth > 0 && canvasWidth > 0) {
				found.push({ text, edges });
			}
		});

		host.remove();

		return found;
	}

	/**
	 * Where the caret edge after the cluster ending at `end` sits: the
	 * segment's whole width after the last cluster, otherwise the width up to
	 * and including `next` less `next` measured alone, so a kerning pair's
	 * adjustment stays with the first glyph.
	 */
	#edge(segment: string, end: number, next: string | undefined): number {
		if (next === undefined) {
			return this.#context.measureText(segment).width;
		}

		return this.#context.measureText(segment.slice(0, end + next.length))
			.width - this.#alone(next);
	}

	/** The grapheme clusters of `segment`. */
	#clusters(segment: string): string[] {
		if (isSimple(segment)) {
			return [...segment];
		}

		return Array.from(this.#graphemes.segment(segment), (part) => part.segment);
	}

	/** The width of `cluster` on its own in the current font, cached per font. */
	#alone(cluster: string): number {
		const key = `${this.#font}\u0000${cluster}`;
		let width = this.#singles.get(key);

		if (width === undefined) {
			width = this.#context.measureText(cluster).width;
			this.#singles.set(key, width);
		}

		return width;
	}
}

/** How one layout font is drawn and measured. */
interface Resolution {
	/** The canvas `font` string that draws the DOM's face. */
	readonly font: string;
	/** Whether segment widths must come from the DOM. */
	readonly domWidths: boolean;
	/** The sequences the face draws as ligatures, and where the DOM puts carets inside them. */
	readonly ligatures: readonly Ligature[];
	/**
	 * Whether the DOM shapes words apart from the spaces between them while
	 * canvas kerns across them: Firefox, with a font that kerns a space
	 * against the next letter. Segments are then measured in pieces split
	 * at spaces.
	 */
	readonly spacesApart: boolean;
}

/**
 * A sequence a face draws as one glyph. Browsers place caret stops inside
 * a ligature at fractions of its width (in equal parts, unless the font
 * says otherwise), not where each letter's own advance would end.
 */
export interface Ligature {
	readonly text: string;
	/** Each inner caret edge, as a share of the ligature's width. */
	readonly edges: readonly number[];
}

/** A segment whose advances the DOM supplies, at `at` in the output. */
interface DomSegment {
	readonly font: string;
	readonly segment: string;
	readonly at: number;
	readonly joined: boolean;
	readonly width: number | null;
}

/**
 * Move the edges inside each ligature in `clusters` to where the DOM puts
 * them: at the ligature's fractions of the width between its outer edges,
 * which stay where the canvas measured them.
 */
export function splitLigatures(
	clusters: readonly string[],
	edges: number[],
	ligatures: readonly Ligature[],
): void {
	for (let index = 0; index < clusters.length && ligatures.length > 0;) {
		const ligature = ligatures.find(({ text }) =>
			text.split("").every((letter, offset) =>
				clusters[index + offset] === letter
			)
		);

		if (ligature === undefined) {
			index += 1;
			continue;
		}

		const start = index === 0 ? 0 : edges[index - 1] ?? 0;
		const last = index + ligature.text.length - 1;
		const width = (edges[last] ?? start) - start;
		ligature.edges.forEach((share, offset) => {
			edges[index + offset] = start + share * width;
		});
		index = last + 1;
	}
}

/** `segment` in runs of spaces and runs of everything else, in order. */
export function splitAtSpaces(segment: string): string[] {
	return segment.match(/[ \u00a0]+|[^ \u00a0]+/gu) ?? [segment];
}

/** `font` (a CSS `font` shorthand) at `size` CSS px. */
function atSize(font: string, size: number): string {
	return font.replace(/^(\S+ \S+ )\S+px/, `$1${size}px`);
}

/** Scale `count` advances from `at` so they sum to `total`. */
function scale(out: number[], at: number, count: number, total: number): void {
	let sum = 0;

	for (let index = at; index < at + count; index += 1) {
		sum += out[index] ?? 0;
	}

	if (sum <= 0) {
		return;
	}

	const factor = total / sum;

	for (let index = at; index < at + count; index += 1) {
		out[index] = (out[index] ?? 0) * factor;
	}
}

/** The families of a CSS `font-family` list, splitting on commas outside quotes. */
export function splitFamilies(list: string): string[] {
	const families: string[] = [];
	let current = "";
	let quote: string | null = null;

	for (const character of list) {
		if (quote === null && character === ",") {
			families.push(current.trim());
			current = "";

			continue;
		}

		current += character;

		if (quote !== null) {
			quote = character === quote ? null : quote;
		} else if (character === '"' || character === "'") {
			quote = character;
		}
	}

	families.push(current.trim());

	return families.filter((family) => family !== "");
}

/** Whether every code unit of `text` is its own grapheme cluster. */
function isSimple(text: string): boolean {
	for (let index = 0; index < text.length; index += 1) {
		if (text.charCodeAt(index) >= SIMPLE_TEXT_LIMIT) {
			return false;
		}
	}

	return true;
}

/** The number of Unicode scalar values in `text`. */
function countScalars(text: string): number {
	let count = 0;

	for (const _ of text) {
		count += 1;
	}

	return count;
}
