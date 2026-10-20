/**
 * What the server and the browser must agree on for server-rendered HTML
 * to hydrate without moving: the document, the theme, and the width the
 * document is laid out at.
 */
import type { ThemeOptions } from "@canvist/web";

/** The width the pages give the editor, in CSS px. */
export const WIDTH = 640;

/** The theme both the renderer and the editor use. The pages load Inter with `@font-face`. */
export const THEME = {
	bodyFamily: "Inter, sans-serif",
} as const satisfies ThemeOptions;

/** The document the server renders. */
export const CONTENT = {
	format: "markdown",
	input: [
		"# Rendered on the server",
		"",
		"This page arrived as **HTML**. Once Inter has loaded, the live editor takes its place without moving a pixel: it measures text with the same font file, so every line wraps at the *same character*.",
		"",
		"- Click anywhere and start typing.",
		"- Press `Mod-b` for **bold**.",
		"",
		"> Server rendering gives readers the text before any JavaScript runs.",
	].join("\n"),
} as const;
