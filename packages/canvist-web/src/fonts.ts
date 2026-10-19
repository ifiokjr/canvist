/**
 * The fonts plugin: load fonts the way the web does, without jank.
 *
 * The editor already uses every font the page can: installed fonts by
 * family name, and every face in `document.fonts`, whether the page
 * declared it with `@font-face` or added a `FontFace`. This plugin adds:
 *
 * - **Loading** faces from URLs, bytes, or `local()` names with CSS
 *   descriptors (`weight`, `style`, `unicodeRange`, `display`, …). Faces
 *   with a `unicodeRange` load only once the document uses a character in
 *   it.
 * - **`font-display`.** Text in a face that is still loading is laid out
 *   but not drawn for the block period, then drawn in the fallback until the
 *   face arrives. A face that arrives after its swap period (`fallback`,
 *   `optional`) is never swapped in, as in CSS.
 * - **Metric-compatible fallbacks.** Each face gets a `"<family> Fallback"`
 *   face over a local font, scaled with `size-adjust` and
 *   `ascent-override`/`descent-override` to the real face's metrics, so
 *   text laid out in the fallback wraps almost where the real face will.
 *   Metrics come from `metrics`, or from measuring the face once it loads
 *   (remembered for the next visit). Use {@link stack} to put it in a
 *   family list.
 * - **Discovery**: whether a family is available, and the installed fonts
 *   through the Local Font Access API where the browser has it.
 * - **Server rendering**: {@link fontFaceCss} writes the same faces as
 *   `@font-face` rules, with no DOM.
 *
 * ```ts
 * import { createEditor } from "@canvist/web";
 * import { fonts, stack } from "@canvist/web/fonts";
 *
 * const editor = await createEditor(element, {
 * 	theme: { bodyFamily: stack("Inter", "sans-serif") },
 * 	plugins: [fonts({ faces: [{ family: "Inter", source: "/fonts/Inter.woff2", weight: "100 900" }] })],
 * });
 * const families = await editor.plugins.fonts.local();
 * ```
 *
 * @module
 */
import type { Plugin, PluginContext } from "./plugin.ts";

declare global {
	interface FontFaceDescriptors {
		/** CSS Fonts 5 `size-adjust`, shipped in every engine but missing from TypeScript's DOM types. */
		sizeAdjust?: string;
	}
}

/** The suffix of a face's metric-compatible fallback family. */
const FALLBACK_SUFFIX = " Fallback";
/** Text whose average advance stands for a face's average character width. */
const WIDTH_SAMPLE = "The quick brown fox jumps over the lazy dog 0123456789";
/** The size faces are measured at, large enough that rounding doesn't matter. */
const PROBE_SIZE = 100;
/** Where measured metrics are remembered between visits. */
const STORAGE_PREFIX = "canvist:font-metrics:";
/** Local fonts to build fallbacks over when a face names none, tried in order. */
const DEFAULT_FALLBACKS = ["Arial", "Helvetica", "Liberation Sans", "Roboto"];

/** CSS Fonts 4's recommended short block and short swap periods, in ms. */
const SHORT_PERIOD_MS = 3000;
/** CSS Fonts 4's recommended extremely small block period, in ms. */
const EXTREMELY_SMALL_PERIOD_MS = 100;

/** How long text in a loading face stays invisible, and how long a late face may still swap in, per CSS Fonts 4. */
const DISPLAY_PERIODS: Record<FontDisplay, { block: number; swap: number }> = {
	auto: { block: SHORT_PERIOD_MS, swap: Infinity },
	block: { block: SHORT_PERIOD_MS, swap: Infinity },
	swap: { block: 0, swap: Infinity },
	fallback: { block: EXTREMELY_SMALL_PERIOD_MS, swap: SHORT_PERIOD_MS },
	optional: {
		block: EXTREMELY_SMALL_PERIOD_MS,
		swap: EXTREMELY_SMALL_PERIOD_MS,
	},
};

/** A face's vertical metrics and average width, as fractions of its em. */
export interface FaceMetrics {
	readonly ascent: number;
	readonly descent: number;
	/** The average advance of {@link WIDTH_SAMPLE}'s characters. */
	readonly averageWidth: number;
}

/** A font face to load, described as `@font-face` describes one. */
export interface FontSource {
	/** The family name to register the face under. */
	readonly family: string;
	/** A URL, several URLs (tried in order), or the font file's bytes. */
	readonly source:
		| string
		| URL
		| readonly (string | URL)[]
		| ArrayBuffer
		| ArrayBufferView;
	/** CSS `font-weight`, or a range such as `"100 900"` for variable fonts. */
	readonly weight?: string;
	/** CSS `font-style`: `"normal"`, `"italic"`, or `"oblique 0deg 10deg"`. */
	readonly style?: string;
	readonly stretch?: string;
	/** Load the face only for text in these ranges, e.g. `"U+0000-00FF"`. */
	readonly unicodeRange?: string;
	/** What to show while the face loads. Default `"swap"`. */
	readonly display?: FontDisplay;
	readonly featureSettings?: string;
	readonly variationSettings?: string;
	/**
	 * Local fonts to build the metric-compatible fallback over, best first,
	 * or `false` for none. Default Arial, Helvetica, Liberation Sans, Roboto.
	 */
	readonly fallback?: readonly string[] | false;
	/** The face's metrics, so the fallback is right on the first visit too. */
	readonly metrics?: FaceMetrics;
}

/** Options for {@link fonts}. */
export interface FontsOptions {
	/** Faces to load as the editor starts. */
	readonly faces?: readonly FontSource[];
}

/** One installed family, from {@link FontsApi.local}. */
export interface LocalFamily {
	readonly family: string;
	/** Its faces' style names, such as `"Regular"` and `"Bold Italic"`. */
	readonly styles: readonly string[];
}

/** `editor.plugins.fonts`. */
export interface FontsApi {
	/** Load a face, as `@font-face` would declare it. Resolves once it is usable or has failed over to its fallback. */
	load(face: FontSource): Promise<FontFace>;
	/** Whether text in `family` would render in that family rather than a fallback. */
	available(family: string): boolean;
	/**
	 * The installed font families, through the Local Font Access API. Ask in
	 * response to a click: browsers require a user gesture and permission.
	 * `null` where the API is missing (only Chromium has it) or permission is
	 * denied.
	 */
	local(): Promise<readonly LocalFamily[] | null>;
	/** Every face this plugin added to the page. */
	faces(): readonly FontFace[];
	/** `@font-face` rules for every face loaded from a URL, for server rendering. */
	css(): string;
}

/** The fonts plugin. */
export function fonts(options: FontsOptions = {}): Plugin<"fonts", FontsApi> {
	return {
		name: "fonts",
		setup(context) {
			const loader = new FaceLoader(context);

			for (const face of options.faces ?? []) {
				context.waitFor(loader.load(face));
			}

			return {
				load: (face) => loader.load(face),
				available: (family) =>
					isAvailable(context.element.ownerDocument, family),
				local: () => localFamilies(),
				faces: () => loader.added(),
				css: () => fontFaceCss(loader.sources()),
			};
		},
	};
}

/**
 * A CSS `font-family` list that puts `family`'s metric-compatible fallback
 * right after it: `stack("Inter", "sans-serif")` is
 * `"Inter", "Inter Fallback", sans-serif`.
 */
export function stack(family: string, ...fallbacks: readonly string[]): string {
	return [quote(family), quote(family + FALLBACK_SUFFIX), ...fallbacks].join(
		", ",
	);
}

/**
 * `@font-face` rules for `faces` loaded from URLs, so server-rendered HTML
 * loads the same files the editor does. Runs anywhere, with no DOM. Faces
 * given as bytes have no URL to point at and are skipped.
 */
export function fontFaceCss(faces: readonly FontSource[]): string {
	return faces.flatMap((face) => {
		const urls = sourceUrls(face.source);

		if (urls === null) {
			return [];
		}

		const declarations = [
			`font-family:${quote(face.family)}`,
			`src:${
				urls.map((url) => `url(${JSON.stringify(String(url))})`).join(",")
			}`,
			face.weight === undefined ? "" : `font-weight:${face.weight}`,
			face.style === undefined ? "" : `font-style:${face.style}`,
			face.stretch === undefined ? "" : `font-stretch:${face.stretch}`,
			face.unicodeRange === undefined
				? ""
				: `unicode-range:${face.unicodeRange}`,
			`font-display:${face.display ?? "swap"}`,
		].filter((declaration) => declaration !== "");
		return [`@font-face{${declarations.join(";")}}`];
	}).join("\n");
}

/** Loads faces into one document for one editor. */
class FaceLoader {
	readonly #context: PluginContext;
	readonly #document: Document;
	readonly #added: FontFace[] = [];
	readonly #sources: FontSource[] = [];
	readonly #blocked = new Map<string, number>();

	constructor(context: PluginContext) {
		this.#context = context;
		this.#document = context.element.ownerDocument;
	}

	added(): readonly FontFace[] {
		return this.#added;
	}

	sources(): readonly FontSource[] {
		return this.#sources;
	}

	/**
	 * Add `source` the way CSS `font-display` would: subsets are added
	 * unloaded for the font watcher to load on demand; whole faces hide their
	 * text for the block period, and are only swapped in if they arrive within
	 * the swap period. A network failure keeps the fallback, like CSS.
	 */
	async load(source: FontSource): Promise<FontFace> {
		this.#sources.push(source);
		const display = source.display ?? "swap";
		const periods = DISPLAY_PERIODS[display];
		const face = new FontFace(source.family, faceSource(source.source), {
			display,
			...descriptors(source),
		});
		const fallback = source.fallback === false
			? undefined
			: this.#fallback(source, source.fallback ?? DEFAULT_FALLBACKS);

		if (source.unicodeRange !== undefined) {
			// Subsets load when text needs them: the editor's font watcher
			// asks for exactly the characters the document uses.
			this.#add(face);
			await fallback;

			return face;
		}

		const started = performance.now();
		this.#block(source.family, periods.block);

		try {
			await face.load();
		} catch (error) {
			if (!(error instanceof DOMException && error.name === "NetworkError")) {
				throw error;
			}

			// A face that fails to load leaves its fallback in place, as CSS does.
			this.#unblock(source.family);
			await fallback;

			return face;
		}

		this.#unblock(source.family);

		if (performance.now() - started <= periods.swap) {
			this.#add(face);
			this.#remember(source);
			this.#context.fontsLoaded([source.family]);
		}

		await fallback;

		return face;
	}

	#add(face: FontFace): void {
		this.#document.fonts.add(face);
		this.#added.push(face);
		this.#context.signal.addEventListener(
			"abort",
			() => this.#document.fonts.delete(face),
			{ once: true },
		);
	}

	/** Hide `family`'s text for `period` milliseconds, or until it loads. */
	#block(family: string, period: number): void {
		if (period <= 0) {
			return;
		}

		const timer = window.setTimeout(() => this.#unblock(family), period);
		this.#blocked.set(family, timer);
		this.#context.hideFamilies([...this.#blocked.keys()]);
	}

	#unblock(family: string): void {
		const timer = this.#blocked.get(family);

		if (timer === undefined) {
			return;
		}

		clearTimeout(timer);
		this.#blocked.delete(family);
		this.#context.hideFamilies([...this.#blocked.keys()]);
	}

	/** Add a metric-compatible fallback for `source` over the first of `locals` installed. */
	async #fallback(
		source: FontSource,
		locals: readonly string[],
	): Promise<void> {
		const target = source.metrics ?? recall(source.family);

		if (target === undefined) {
			return;
		}

		for (const local of locals) {
			const probe = new FontFace(
				`${source.family}${FALLBACK_SUFFIX}`,
				`local(${quote(local)})`,
			);

			if (!(await installed(probe))) {
				continue;
			}

			const base = measureFace(this.#document, quote(local));
			const sizeAdjust = target.averageWidth / base.averageWidth;
			const face = new FontFace(
				`${source.family}${FALLBACK_SUFFIX}`,
				`local(${quote(local)})`,
				{
					...descriptors(source),
					sizeAdjust: percent(sizeAdjust),
					ascentOverride: percent(target.ascent / sizeAdjust),
					descentOverride: percent(target.descent / sizeAdjust),
					lineGapOverride: "0%",
				},
			);
			await face.load();
			this.#add(face);
			this.#context.fontsLoaded([face.family]);

			return;
		}
	}

	/** Measure a loaded face and keep its metrics for the next visit's fallback. */
	#remember(source: FontSource): void {
		if (source.metrics !== undefined) {
			return;
		}

		const metrics = measureFace(this.#document, quote(source.family));

		try {
			localStorage.setItem(
				STORAGE_PREFIX + source.family,
				JSON.stringify(metrics),
			);
		} catch (error) {
			// Storage can be full or disabled (private browsing); the next
			// visit then measures again, and nothing else depends on it.
			if (!(error instanceof DOMException)) {
				throw error;
			}
		}
	}
}

/** Whether the `local()` face `probe` loads: it fails with a NetworkError when the font is not installed. */
async function installed(probe: FontFace): Promise<boolean> {
	try {
		await probe.load();

		return true;
	} catch (error) {
		if (error instanceof DOMException && error.name === "NetworkError") {
			return false;
		}

		throw error;
	}
}

/** Metrics measured on a previous visit, if any were stored. */
function recall(family: string): FaceMetrics | undefined {
	let stored: string | null;

	try {
		stored = localStorage.getItem(STORAGE_PREFIX + family);
	} catch (error) {
		if (error instanceof DOMException) {
			return undefined;
		}

		throw error;
	}

	if (stored === null) {
		return undefined;
	}

	const parsed: unknown = JSON.parse(stored);

	return isFaceMetrics(parsed) ? parsed : undefined;
}

function isFaceMetrics(value: unknown): value is FaceMetrics {
	return typeof value === "object" && value !== null &&
		["ascent", "descent", "averageWidth"].every((key) =>
			typeof (value as Record<string, unknown>)[key] === "number"
		);
}

/** The metrics of `family` as the canvas renders it now. */
export function measureFace(document: Document, family: string): FaceMetrics {
	const context = document.createElement("canvas").getContext("2d");

	if (context === null) {
		throw new Error("canvist needs a 2D canvas context to measure fonts");
	}

	context.font = `${PROBE_SIZE}px ${family}`;
	const box = context.measureText(WIDTH_SAMPLE);

	return {
		ascent: box.fontBoundingBoxAscent / PROBE_SIZE,
		descent: box.fontBoundingBoxDescent / PROBE_SIZE,
		averageWidth: box.width / PROBE_SIZE / WIDTH_SAMPLE.length,
	};
}

/**
 * Whether `family` renders as itself: a probe string measures differently
 * from both the serif and the monospace generic when the family falls
 * back to neither.
 */
export function isAvailable(document: Document, family: string): boolean {
	const context = document.createElement("canvas").getContext("2d");

	if (context === null) {
		throw new Error("canvist needs a 2D canvas context to measure fonts");
	}

	const probe = "mmmmmmmmmmlli10OoWw@#";
	return ["monospace", "serif"].some((generic) => {
		context.font = `${PROBE_SIZE}px ${generic}`;
		const base = context.measureText(probe).width;
		context.font = `${PROBE_SIZE}px ${quote(family)}, ${generic}`;
		return context.measureText(probe).width !== base;
	});
}

/** The Local Font Access API, which only Chromium ships. */
interface LocalFontData {
	readonly family: string;
	readonly style: string;
}

type QueryLocalFonts = () => Promise<readonly LocalFontData[]>;

/**
 * The installed families and their styles, from the Local Font Access API.
 * `null` when the browser has no such API or the user denies permission,
 * so callers can tell "unknown" apart from "no fonts".
 */
async function localFamilies(): Promise<readonly LocalFamily[] | null> {
	const query =
		(globalThis as { queryLocalFonts?: QueryLocalFonts }).queryLocalFonts;

	if (query === undefined) {
		return null;
	}

	let fonts: readonly LocalFontData[];

	try {
		fonts = await query();
	} catch (error) {
		if (
			error instanceof DOMException &&
			(error.name === "NotAllowedError" || error.name === "SecurityError")
		) {
			return null;
		}

		throw error;
	}

	const families = new Map<string, string[]>();

	for (const font of fonts) {
		const styles = families.get(font.family) ?? [];
		styles.push(font.style);
		families.set(font.family, styles);
	}

	return [...families].map(([family, styles]) => ({ family, styles }));
}

function descriptors(source: FontSource): FontFaceDescriptors {
	const result: Record<string, string> = {};
	const entries = {
		weight: source.weight,
		style: source.style,
		stretch: source.stretch,
		unicodeRange: source.unicodeRange,
		featureSettings: source.featureSettings,
		variationSettings: source.variationSettings,
	};

	for (const [key, value] of Object.entries(entries)) {
		if (value !== undefined) {
			result[key] = value;
		}
	}

	return result;
}

function faceSource(source: FontSource["source"]): string | BufferSource {
	const urls = sourceUrls(source);

	if (urls === null) {
		return source as BufferSource;
	}

	return urls.map((url) => `url(${JSON.stringify(String(url))})`).join(", ");
}

function sourceUrls(
	source: FontSource["source"],
): readonly (string | URL)[] | null {
	if (typeof source === "string" || source instanceof URL) {
		return [source];
	}

	if (Array.isArray(source)) {
		return source;
	}

	return null;
}

function quote(family: string): string {
	return JSON.stringify(family);
}

function percent(ratio: number): string {
	return `${(ratio * 100).toFixed(4)}%`;
}
