import { assert, assertEquals, assertStringIncludes } from "@std/assert";
import { parseSummary } from "../../src/lib/docs-nav.ts";
import {
	absoluteLinks,
	llmsFull,
	llmsIndex,
	pagesFrom,
} from "../../src/lib/llms.ts";

const SITE = new URL("https://docs.example");

const SUMMARY = parseSummary(
	[
		"# Summary",
		"",
		"# Get started",
		"",
		"- [Introduction](./readme.md)",
		"",
		"# Guides",
		"",
		"- [Collaboration](./guides/collaboration.md)",
	].join("\n"),
);

const FILES: Record<string, string> = {
	"readme.md":
		'<h1 class="brand-title">\n  <img src="images/brand/lockup.svg" alt="canvist">\n</h1>\n\n**canvist is an editor.** Read [the guide](./guides/collaboration.md#share).',
	"guides/collaboration.md":
		"# Collaboration\n\nShare documents. See [errors](../reference/errors.md) and [Yjs](https://yjs.dev).",
};

const pages = pagesFrom(SUMMARY, (path) => FILES[path] ?? "");

Deno.test("llms.txt lists every summary page under its part, with absolute links", () => {
	const index = llmsIndex(pages, SITE);
	assert(index.startsWith("# canvist\n\n> "));
	assertStringIncludes(
		index,
		"## Get started\n\n- [Introduction](https://docs.example/docs): canvist is an editor.",
	);
	assertStringIncludes(
		index,
		"## Guides\n\n- [Collaboration](https://docs.example/docs/guides/collaboration): Share documents.",
	);
	assertStringIncludes(index, "(https://docs.example/llms-full.txt)");
});

Deno.test("llms-full.txt joins every page, drops the logo HTML, and makes links absolute", () => {
	const full = llmsFull(pages, SITE);
	assert(!full.includes("<img"), "the logo HTML is gone");
	assertStringIncludes(
		full,
		"[the guide](https://docs.example/docs/guides/collaboration#share)",
	);
	assertStringIncludes(
		full,
		"[errors](https://docs.example/docs/reference/errors)",
	);
	assertStringIncludes(full, "[Yjs](https://yjs.dev)");
	assertStringIncludes(
		full,
		"Source: https://docs.example/docs/guides/collaboration",
	);
});

Deno.test("absoluteLinks leaves anchors and external links alone", () => {
	assertEquals(
		absoluteLinks("[a](#x) [b](https://a.b)", "guides/x.md", SITE),
		"[a](#x) [b](https://a.b)",
	);
	assertEquals(
		absoluteLinks("[c](/changelog)", "readme.md", SITE),
		"[c](https://docs.example/changelog)",
	);
	assertEquals(
		absoluteLinks("![p](images/editor.png)", "readme.md", SITE),
		"![p](https://docs.example/docs/images/editor.png)",
	);
});
