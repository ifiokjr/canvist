import { readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path/posix";
import {
	firstHeading,
	firstParagraph,
	readSummary,
	SUMMARY_PATH,
	type SummaryEntry,
} from "./docs-nav.ts";
import { docsRoute } from "./rehype-docs-links.ts";

/**
 * `/llms.txt` and `/llms-full.txt` (https://llmstxt.org): the docs for AI
 * agents, built from `docs/src/SUMMARY.md` so they list exactly the book's
 * pages, in its order.
 *
 * - `llms.txt` is an index: one link per page with its first paragraph,
 *   grouped by the summary's parts.
 * - `llms-full.txt` is every page's markdown in one file, with relative
 *   links made absolute, for agents that read the whole thing at once.
 */

const SUMMARY_LINE =
	"canvist is a rich-text editor you build from plugins. A Rust core paints every pixel to a canvas, the same engine runs on servers (Rust, TypeScript, Dart) and in Flutter today, documents are Yjs-compatible, and typed APIs make calling an unregistered extension a compile error.";

const NOTES = [
	"canvist has not had its first release. Install from GitHub, as each quick start shows; registry packages do not exist yet.",
	"Available today: `canvist_headless` (Rust), `@canvist/headless` (Node, Deno, Bun, Workers), `canvist` (Dart), `canvist_desktop` (macOS, Windows, and Linux), and `canvist_flutter` (engine only). React, Vue, iOS, Android, React Native, and Lynx are coming soon: do not invent their packages.",
	"Read the agent guide first: it has the minimal integration per platform, a checklist, and common pitfalls.",
];

/** One page of the book, in summary order. */
export interface LlmsPage {
	path: string;
	title: string;
	part: string | undefined;
	markdown: string;
}

/** The book's pages in summary order, read from `docsRoot` (`docs/src`). */
export function readLlmsPages(docsRoot: string): LlmsPage[] {
	return pagesFrom(
		readSummary(join(docsRoot, SUMMARY_PATH)),
		(path) => readFileSync(join(docsRoot, path), "utf8"),
	);
}

export function pagesFrom(
	summary: Map<string, SummaryEntry>,
	read: (path: string) => string,
): LlmsPage[] {
	return [...summary].map(([path, entry]) => ({
		path,
		title: entry.title,
		part: entry.part,
		markdown: read(path),
	}));
}

function absolute(route: string, site: URL): string {
	return new URL(route, site).href;
}

/** `llms.txt`: the index. */
export function llmsIndex(pages: readonly LlmsPage[], site: URL): string {
	const lines = [
		"# canvist",
		"",
		`> ${SUMMARY_LINE}`,
		"",
		...NOTES.map((note) => `- ${note}`),
	];
	let part: string | undefined;
	for (const page of pages) {
		const heading = page.part ?? "Docs";
		if (heading !== part) {
			lines.push("", `## ${heading}`, "");
			part = heading;
		}
		const summary = firstParagraph(withoutHtml(page.markdown), 200);
		const link = `- [${page.title}](${absolute(docsRoute(page.path), site)})`;
		lines.push(summary === undefined ? link : `${link}: ${summary}`);
	}
	lines.push(
		"",
		"## Optional",
		"",
		`- [Every page in one file](${
			absolute("/llms-full.txt", site)
		}): the full docs as markdown`,
		"- [Source and examples](https://github.com/canvist/canvist): the repository",
		"",
	);
	return lines.join("\n");
}

/** `llms-full.txt`: every page's markdown, with links made absolute. */
export function llmsFull(pages: readonly LlmsPage[], site: URL): string {
	const sections = pages.map((page) => {
		const body = absoluteLinks(withoutHtml(page.markdown), page.path, site)
			.trim();
		const source = absolute(docsRoute(page.path), site);
		const title = firstHeading(body) === undefined ? `# ${page.title}\n\n` : "";
		return `${title}${body}\n\nSource: ${source}`;
	});
	return `# canvist\n\n> ${SUMMARY_LINE}\n\n${sections.join("\n\n---\n\n")}\n`;
}

/** Drops the HTML the book uses for its logo, which means nothing as text. */
function withoutHtml(markdown: string): string {
	return markdown.replace(
		/^<h1 class="brand-title">[\s\S]*?<\/h1>\s*/m,
		"# canvist\n\n",
	);
}

const LINK = /(\]\()([^)\s]+)(\))/g;

/** Rewrites `./other.md#part` links, relative to `path`, to absolute site URLs. */
export function absoluteLinks(
	markdown: string,
	path: string,
	site: URL,
): string {
	return markdown.replace(
		LINK,
		(whole, open: string, href: string, close: string) => {
			if (/^[a-z]+:|^#/i.test(href)) return whole;
			if (href.startsWith("/")) return `${open}${absolute(href, site)}${close}`;
			const [target = "", hash] = href.split("#", 2);
			const resolved = normalize(join(dirname(path), target));
			if (resolved.startsWith("..")) return whole;
			const route = resolved.endsWith(".md")
				? docsRoute(resolved)
				: `/docs/${resolved}`;
			return `${open}${absolute(route, site)}${
				hash === undefined ? "" : `#${hash}`
			}${close}`;
		},
	);
}
