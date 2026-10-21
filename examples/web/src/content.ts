/** The document every example starts with. */
export const CONTENT = {
	format: "markdown",
	input: [
		"# Hello, canvist",
		"",
		"Every pixel here is painted on a **canvas**, and every line wraps where the browser would wrap it.",
		"",
		"- Select a word and press **Bold**, or `Mod-b`.",
		"- Start a line with `##` and a space for a heading.",
	].join("\n"),
} as const;

/** What an empty document shows. */
export const PLACEHOLDER = "Start writing…";
