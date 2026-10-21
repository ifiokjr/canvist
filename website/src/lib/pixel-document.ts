/**
 * The document in "Type into the real engine", and the theme it is laid out
 * with. The page is built with this document rendered to HTML by
 * `@canvist/headless/ssr`, and the browser editor (`@canvist/web`) starts
 * from the same content with the same theme, so the canvas lines up with
 * that HTML when it takes its place.
 */
import type { ThemeOptions } from "@canvist/headless";

export type Scheme = "light" | "dark";

/** What the page shows before the editor loads, and what the editor starts from. */
export const PIXEL_CONTENT = {
	format: "markdown",
	input:
		"Every glyph here is laid out and painted by **canvist**, not the browser.",
} as const;

/** Typed in after {@link PIXEL_CONTENT} once the editor is live, a key at a time. */
export const PIXEL_GHOST_TEXT = " Click in and type.";

/**
 * The theme for each colour scheme. The families are the page's own
 * `@font-face` faces (`src/styles/tokens.css`), so the server-rendered HTML
 * and the canvas measure text with the same font files.
 */
export function pixelTheme(scheme: Scheme): ThemeOptions {
	return {
		scheme,
		bodyFamily: "Bricolage, sans-serif",
		codeFamily: '"Geist Mono", monospace',
		size: 20,
	};
}
