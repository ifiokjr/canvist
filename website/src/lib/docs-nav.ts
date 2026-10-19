import { readFileSync } from "node:fs";
import { repoPath } from "./paths.ts";
import { docsRoute } from "./rehype-docs-links.ts";

/**
 * The `/docs` sidebar, built from the mdBook `summary.md` so the book and the
 * site share one table of contents.
 *
 * `summary.md` is a flat list, so the sidebar groups its entries by top-level
 * folder, keeping summary order inside each group. Pages that exist in
 * `docs/src` but are missing from the summary still render: they join the end
 * of their folder's group, titled by their first `# Heading`.
 */

/** One page as the docs collection sees it. */
export interface DocsPage {
	/** Path inside `docs/src`, such as `v2/architecture.md`. */
	path: string;
	/** The markdown body, used for the title and description fallbacks. */
	body: string;
}

export interface DocsLink {
	path: string;
	route: string;
	/** The full title, used for `<title>` and the pager. */
	title: string;
	/** The sidebar label, with the group's redundant prefix trimmed. */
	label: string;
	group: string;
}

export interface DocsGroup {
	title: string;
	links: DocsLink[];
}

interface GroupRule {
	title: string;
	/** Top-level folders whose pages belong here; `""` is the docs root. */
	folders: readonly string[];
	/** A summary-title prefix that repeats the group name, such as `v2 `. */
	trim?: RegExp;
}

/** Sidebar order. A folder not listed here gets a group named after it, last. */
const GROUP_RULES: readonly GroupRule[] = [
	{ title: "Guides", folders: ["", "guide"], trim: /^Guide:\s*/ },
	{ title: "v2 design", folders: ["v2"], trim: /^v2\s+/ },
	{ title: "Platforms", folders: ["platforms"] },
	{
		title: "canvist Cloud",
		folders: ["cloud"],
		trim: /^(canvist\s+)?Cloud:?\s+/i,
	},
	{ title: "Research", folders: ["research"], trim: /^Research:\s*/ },
];

const SUMMARY_ENTRY = /^\s*[-*]\s+\[([^\]]+)\]\(\.?\/?([^)\s]+\.md)\)/;

export const SUMMARY_PATH = "summary.md";

/** Reads `[Title](./path.md)` entries from mdBook's summary, in order. */
export function parseSummary(markdown: string): Map<string, string> {
	const entries = new Map<string, string>();
	for (const line of markdown.split("\n")) {
		const match = SUMMARY_ENTRY.exec(line);
		if (!match) continue;
		const [, title = "", path = ""] = match;
		if (!entries.has(path)) entries.set(path, title.trim());
	}
	return entries;
}

/** Strips the inline markdown a heading or paragraph might carry. */
function plainText(markdown: string): string {
	return markdown
		.replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
		.replace(/[`*_]/g, "")
		.replace(/\s+/g, " ")
		.trim();
}

/** The text of the page's first `# Heading`, if it has one. */
export function firstHeading(body: string): string | undefined {
	const match = /^#\s+(.+?)\s*#*\s*$/m.exec(body);
	return match?.[1] === undefined ? undefined : plainText(match[1]);
}

/** The first prose paragraph, trimmed to a meta-description length. */
export function firstParagraph(
	body: string,
	maxLength = 160,
): string | undefined {
	const withoutFences = body.replace(/^```[\s\S]*?^```/gm, "");
	const paragraph = withoutFences
		.split(/\n\s*\n/)
		.map((block) => block.trim())
		.find((block) =>
			block !== "" && !/^(#|\||>|[-*+]\s|\d+\.\s|<|!\[)/.test(block)
		);
	if (paragraph === undefined) return undefined;
	const text = plainText(paragraph);
	if (text.length <= maxLength) return text;
	const cut = text.slice(0, maxLength - 1);
	return `${cut.slice(0, cut.lastIndexOf(" "))}…`;
}

function folderOf(path: string): string {
	const slash = path.indexOf("/");
	return slash === -1 ? "" : path.slice(0, slash);
}

function titleCase(folder: string): string {
	return folder.replace(/[-_]+/g, " ").replace(
		/^\w/,
		(letter) => letter.toUpperCase(),
	);
}

function ruleFor(folder: string): GroupRule {
	return GROUP_RULES.find((rule) => rule.folders.includes(folder)) ??
		{ title: titleCase(folder), folders: [folder] };
}

function isReadme(path: string): boolean {
	return /(^|\/)readme\.md$/i.test(path);
}

/**
 * Groups every page into sidebar sections. Summary entries without a page are
 * dropped rather than linked, so the sidebar never points at a 404.
 */
export function buildDocsNav(
	pages: readonly DocsPage[],
	summary: Map<string, string>,
): DocsGroup[] {
	const byPath = new Map(pages.map((page) => [page.path, page]));
	const ordered = [
		...[...summary.keys()].filter((path) => byPath.has(path)),
		...pages.map((page) => page.path).filter((path) => !summary.has(path))
			.sort(),
	];

	const groups = new Map<string, DocsGroup>();
	const order = (title: string): number => {
		const index = GROUP_RULES.findIndex((rule) => rule.title === title);
		return index === -1 ? GROUP_RULES.length : index;
	};

	for (const path of ordered) {
		const page = byPath.get(path);
		if (page === undefined) continue;
		const rule = ruleFor(folderOf(path));
		const title = summary.get(path) ?? firstHeading(page.body) ??
			titleCase(path.replace(/\.md$/, ""));
		const label = rule.trim === undefined
			? title
			: title.replace(rule.trim, "") || title;
		const group = groups.get(rule.title) ?? { title: rule.title, links: [] };
		const link: DocsLink = {
			path,
			route: docsRoute(path),
			title,
			label,
			group: rule.title,
		};
		// The book's introduction leads its group, wherever the summary lists it.
		if (isReadme(path) && folderOf(path) === "") group.links.unshift(link);
		else group.links.push(link);
		groups.set(rule.title, group);
	}

	return [...groups.values()].sort((a, b) => order(a.title) - order(b.title));
}

/** Reads `docs/src/summary.md`, the book's table of contents. */
export function readSummary(): Map<string, string> {
	return parseSummary(readFileSync(repoPath("docs/src", SUMMARY_PATH), "utf8"));
}

/** The sidebar in reading order, for previous and next links. */
export function flatten(groups: readonly DocsGroup[]): DocsLink[] {
	return groups.flatMap((group) => group.links);
}

export const EDIT_BASE =
	"https://github.com/ifiokjr/canvist/edit/main/docs/src/";
