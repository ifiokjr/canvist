/**
 * How far the canvas may differ from server-rendered HTML, per browser and
 * platform, and why. Every number here was measured; see
 * `docs/src/guides/rendering-parity.md` for the screenshots.
 *
 * Geometry is held to the browser's own layout quantum: the canvas is laid
 * out with the browser's measurements and line boxes (read from the DOM),
 * so lines, runs, and caret stops land where the DOM puts them. Pixels can
 * differ only where canvas and DOM text rasterise differently.
 *
 * @module
 */
/** What one case may differ by. */
export interface Thresholds {
	/** Largest block, line, or baseline difference, in CSS px. */
	readonly vertical: number;
	/** Largest run or caret-stop difference along the line, in CSS px. */
	readonly horizontal: number;
	/** Largest share of pixels pixelmatch finds different, antialiasing aside (0 to 1). */
	readonly pixels: number;
	/** Smallest structural similarity (SSIM, 0 to 1). */
	readonly ssim: number;
}

/** A documented difference: the limits for one fixture, and the reason. */
interface Exception extends Partial<Thresholds> {
	readonly reason: string;
}

/** How far blocks, lines, and baselines may move in every engine, in CSS px. */
const VERTICAL_LIMIT = 1 / 32;

/** Chromium's run and caret-stop limit, in CSS px: above the 0.115 px LayoutUnit drift measured below. */
const CHROMIUM_HORIZONTAL_LIMIT = 1 / 8;

/**
 * Chromium, the reference: lines and baselines within a 64th of a pixel.
 * Blink rounds each inline item's width up to a 1/64 px LayoutUnit, while
 * layout sums floats, so a line with many style runs drifts by up to a
 * 64th per run (0.115 px measured on the marks paragraph).
 */
const CHROMIUM: Thresholds = {
	vertical: VERTICAL_LIMIT,
	horizontal: CHROMIUM_HORIZONTAL_LIMIT,
	pixels: 0.001,
	ssim: 0.99,
};

/**
 * WebKit reports text ranges as whole-pixel rectangles, so the harness can
 * only place a DOM caret stop within a pixel; layout positions are compared
 * against that pixel. Emoji bitmaps scale through a different path in
 * canvas, which shades their edges differently (0.52% of pixels, geometry
 * exact).
 */
const WEBKIT: Thresholds = {
	vertical: VERTICAL_LIMIT,
	horizontal: 1 / 4,
	pixels: 0.0075,
	ssim: 0.985,
};

/**
 * Firefox draws fractional font sizes and optical sizes in canvas at
 * quantized steps its DOM does not use, so layout takes those fonts'
 * widths from the DOM and glyphs inside a word can sit a fraction of a
 * pixel from the DOM's (SSIM 0.90 to 0.97, geometry exact). Its character
 * rectangles for some glyphs end 19/60 px past where the next character
 * starts, which caps how exactly the harness can read a caret stop.
 */
const FIREFOX: Thresholds = {
	vertical: VERTICAL_LIMIT,
	horizontal: 1 / 3,
	pixels: 0.004,
	ssim: 0.9,
};

const ENGINES: Readonly<Record<string, Thresholds>> = {
	chromium: CHROMIUM,
	webkit: WEBKIT,
	firefox: FIREFOX,
};

/**
 * Per-platform adjustments, by `${browser}-${platform}`. Linux runners
 * render with different system fonts and FreeType; their limits are
 * recorded here once CI has measured them.
 */
const PLATFORMS: Readonly<Record<string, Partial<Thresholds>>> = {};

/** Known differences, by `${browser}-${fixture}`. */
const EXCEPTIONS: Readonly<Record<string, Exception>> = {
	"webkit-cjk": {
		horizontal: 1.5,
		pixels: 0.02,
		ssim: 0.95,
		reason:
			"Latin text falling back from a CJK web font measures up to 1.2 px apart in WebKit's canvas and DOM; lines still wrap identically.",
	},
	"webkit-rtl": {
		horizontal: 2.5,
		pixels: 0.0075,
		ssim: 0.96,
		reason:
			"Caret stops inside Arabic words come from DOM grapheme boxes, which WebKit rounds to whole pixels.",
	},
	"firefox-cjk": {
		vertical: 24,
		horizontal: 480,
		pixels: 0.025,
		ssim: 0.9,
		reason:
			"Firefox applies contextual half-width spacing (OpenType chws) to full-width punctuation by its neighbours on both sides; layout measures CJK one character at a time, so a line holding 「。」 before Latin can wrap one character differently.",
	},
};

/** The thresholds for `fixture` in `browser` on `platform` (`darwin`, `linux`, …). */
export function thresholds(
	browser: string,
	platform: string,
	fixture: string,
): Thresholds & { reason?: string } {
	return {
		...(ENGINES[browser] ?? CHROMIUM),
		...PLATFORMS[`${browser}-${platform}`],
		...EXCEPTIONS[`${browser}-${fixture}`],
	};
}
