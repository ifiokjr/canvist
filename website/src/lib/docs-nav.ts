import { readFileSync } from "node:fs";
import { docsRoute } from "./rehype-docs-links.ts";

/**
 * The `/docs` sidebar, built from the mdBook `SUMMARY.md` so the book and the
 * site share one table of contents.
 *
 * Groups come from the summary's part headings (`# Get started`), in summary
 * order, so the sidebar follows the book. Pages outside any part, and pages
 * missing from the summary, are grouped by top-level folder instead. Pages
 * that exist in `docs/src` but are missing from the summary still render:
 * they join the end of their group, titled by their first `# Heading`.
 */

export interface DocsPage {
	/**
	 * Docs path, such as `guides/performance.md`. Readmes use a virtual one:
	 * `crates/canvist_core.md` for `crates/canvist_core/readme.md`.
	 */
	path: string;
	/** The file in the repository, for edit links. */
	source: string;
	/** The markdown body, used for the title and description fallbacks. */
	body: string;
}

export interface DocsLink {
	path: string;
	/** The file in the repository, for edit links. */
	source: string;
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

/**
 * Groups for pages outside the summary's parts, by folder, in sidebar order.
 * A folder not listed here gets a group named after it, last.
 */
const GROUP_RULES: readonly GroupRule[] = [
	{ title: "Get started", folders: ["", "quick-start"] },
	{ title: "Core concepts", folders: ["concepts"] },
	{ title: "Extensions", folders: ["extensions"] },
	{ title: "Guides", folders: ["guides"] },
	{ title: "AI agents", folders: ["agents"] },
	{
		title: "canvist Cloud",
		folders: ["cloud"],
		trim: /^(canvist\s+)?Cloud:?\s+/i,
	},
	{ title: "Reference", folders: ["reference"] },
	{ title: "Rust crates", folders: ["crates"] },
	{ title: "Packages", folders: ["packages"] },
];

const SUMMARY_ENTRY = /^\s*[-*]\s+\[([^\]]+)\]\(\.?\/?([^)\s]+\.md)\)/;

// mdBook only reads this exact name; on case-sensitive systems `summary.md` fails.
export const SUMMARY_PATH = "SUMMARY.md";

/** One `[Title](./path.md)` entry of mdBook's summary. */
export interface SummaryEntry {
	title: string;
	/** The part heading (`# Guides`) the entry sits under, if any. */
	part?: string;
}

const SUMMARY_PART = /^#\s+(.+?)\s*$/;

/** Reads `[Title](./path.md)` entries from mdBook's summary, in order, with their parts. */
export function parseSummary(markdown: string): Map<string, SummaryEntry> {
	const entries = new Map<string, SummaryEntry>();
	let part: string | undefined;
	let sawHeading = false;
	for (const line of markdown.split("\n")) {
		const heading = SUMMARY_PART.exec(line);
		if (heading) {
			// The first heading is the book's title (`# Summary`), not a part.
			if (sawHeading) part = heading[1];
			sawHeading = true;
			continue;
		}
		const match = SUMMARY_ENTRY.exec(line);
		if (!match) continue;
		const [, title = "", path = ""] = match;
		if (entries.has(path)) continue;
		entries.set(
			path,
			part === undefined
				? { title: title.trim() }
				: { title: title.trim(), part },
		);
	}
	return entries;
}

/** Strips the inline markdown a heading or paragraph might carry. */
function plainText(markdown: string): string {
	return markdown
		.replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
		.replace(/[`*]/g, "")
		// Underscores mark emphasis only around words; `canvist_core` keeps its own.
		.replace(/(^|[^\w])_([^_]+)_(?=[^\w]|$)/g, "$1$2")
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

/** A page with no heading is named after its file; a readme after its crate or package. */
function fallbackTitle(path: string): string {
	const name = path.replace(/\.md$/, "");
	const folder = folderOf(path);
	if (folder === "crates" || folder === "packages") {
		return name.slice(folder.length + 1);
	}
	return titleCase(name);
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
	summary: Map<string, SummaryEntry>,
): DocsGroup[] {
	const byPath = new Map(pages.map((page) => [page.path, page]));
	const ordered = [
		...[...summary.keys()].filter((path) => byPath.has(path)),
		...pages.map((page) => page.path).filter((path) => !summary.has(path))
			.sort(),
	];

	// Parts lead, in summary order; folder groups follow in GROUP_RULES order.
	const parts = [
		...new Set(
			[...summary.values()].flatMap((entry) =>
				entry.part === undefined ? [] : [entry.part]
			),
		),
	];
	const order = (title: string): number => {
		const part = parts.indexOf(title);
		if (part !== -1) return part;
		const index = GROUP_RULES.findIndex((rule) => rule.title === title);
		return parts.length + (index === -1 ? GROUP_RULES.length : index);
	};

	const groups = new Map<string, DocsGroup>();
	for (const path of ordered) {
		const page = byPath.get(path);
		if (page === undefined) continue;
		const entry = summary.get(path);
		const rule = ruleFor(folderOf(path));
		const groupTitle = entry?.part ?? rule.title;
		const title = entry?.title ?? firstHeading(page.body) ??
			fallbackTitle(path);
		const label = rule.trim === undefined
			? title
			: title.replace(rule.trim, "") || title;
		const group = groups.get(groupTitle) ?? { title: groupTitle, links: [] };
		const link: DocsLink = {
			path,
			source: page.source,
			route: docsRoute(path),
			title,
			label,
			group: groupTitle,
		};
		// The book's introduction leads its group, wherever the summary lists it.
		if (isReadme(path) && folderOf(path) === "") group.links.unshift(link);
		else group.links.push(link);
		groups.set(groupTitle, group);
	}

	return [...groups.values()].sort((a, b) => order(a.title) - order(b.title));
}

/** Reads the book's table of contents from `docs/src/SUMMARY.md` at `path`. */
export function readSummary(path: string): Map<string, SummaryEntry> {
	return parseSummary(readFileSync(path, "utf8"));
}

/** The sidebar in reading order, for previous and next links. */
export function flatten(groups: readonly DocsGroup[]): DocsLink[] {
	return groups.flatMap((group) => group.links);
}

export const EDIT_BASE = "https://github.com/canvist/canvist/edit/main/";
