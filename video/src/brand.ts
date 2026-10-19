/**
 * Wet Caret colours and type, from `assets/brand/readme.md`.
 *
 * The film's stage is Ink; documents sit on Paper sheets. Carets are Paint
 * on light surfaces and Paint on dark on the stage, and nothing else.
 */

import { loadFont as loadFile } from "@remotion/fonts";
import { loadFont as loadBricolage } from "@remotion/google-fonts/BricolageGrotesque";
import { loadFont as loadGeistMono } from "@remotion/google-fonts/GeistMono";
import { staticFile } from "remotion";

export const INK = "#17141F";
export const PAINT = "#3B2CF5";
export const PAPER = "#F5F3FF";
export const PAINT_ON_DARK = "#8A80FF";

/** Ink lifted a little, for panels on the stage. */
export const INK_RAISED = "#221E2D";
/** Paper text at reduced emphasis on the stage. */
export const PAPER_MUTED = "#A8A3BD";
/** Hairlines on the stage. */
export const LINE = "#3A3548";

/** The face the footage was measured with, declared under the same name. */
export const DOCUMENT_FAMILY = "Atkinson Hyperlegible";

export const DISPLAY = loadBricolage("normal", {
	weights: ["500", "700", "800"],
	subsets: ["latin"],
}).fontFamily;

export const MONO = loadGeistMono("normal", {
	weights: ["400", "500"],
	subsets: ["latin"],
}).fontFamily;

/** The faces the footage was measured with, as `prepare` copies them. */
const DOCUMENT_FACES = [
	{ file: "Regular", weight: "400", style: "normal" },
	{ file: "Bold", weight: "700", style: "normal" },
	{ file: "Italic", weight: "400", style: "italic" },
	{ file: "BoldItalic", weight: "700", style: "italic" },
] as const;

for (const face of DOCUMENT_FACES) {
	void loadFile({
		family: DOCUMENT_FAMILY,
		url: staticFile(`fonts/AtkinsonHyperlegible-${face.file}.ttf`),
		weight: face.weight,
		style: face.style,
	});
}
