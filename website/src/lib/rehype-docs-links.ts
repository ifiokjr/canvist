import type { Element, ElementContent, Root, RootContent } from "hast";
import { dirname, join, normalize, relative } from "node:path";
import { fileURLToPath } from "node:url";

/** The mdBook source the `/docs` section renders. */
export const DOCS_SOURCE = fileURLToPath(
	new URL("../../../docs/src/", import.meta.url),
);
const REPO_ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const REPO_BLOB = "https://github.com/ifiokjr/canvist/blob/main/";

/** Crate and package readmes, which `/docs` renders under "Rust crates" and "Packages". */
const README = /^(crates|packages)\/([^/]+)(?:\/readme\.md)?\/?$/i;

/**
 * Maps a markdown file inside `docs/src` to its route: `guides/performance.md`
 * becomes `/docs/guides/performance`, and a `readme.md` becomes its folder.
 */
export function docsRoute(pathInDocs: string): string {
	const withoutExt = pathInDocs.replace(/\.md$/, "");
	const route = withoutExt.replace(/(^|\/)readme$/, "");
	return route === "" ? "/docs" : `/docs/${route}`;
}

/**
 * The docs path a crate or package readme renders at, such as
 * `crates/canvist_core.md` for `crates/canvist_core/readme.md`. `undefined`
 * for anything that isn't one.
 */
export function readmeDocsPath(pathInRepo: string): string | undefined {
	const match = README.exec(pathInRepo);
	if (!match) return undefined;
	const [, kind = "", name = ""] = match;
	return `${kind}/${name}.md`;
}

/** Whether a source file is one the docs render, and so needs its links rewritten. */
function rendersInDocs(file: string): boolean {
	if (file.startsWith(DOCS_SOURCE)) return true;
	return readmeDocsPath(relative(REPO_ROOT, file)) !== undefined;
}

function rewrite(href: string, sourceFile: string): string {
	if (/^[a-z]+:|^#|^\//i.test(href)) return href;
	const [path = "", hash] = href.split("#", 2);
	const suffix = hash === undefined ? "" : `#${hash}`;
	const target = normalize(join(dirname(sourceFile), path));
	const inDocs = relative(DOCS_SOURCE, target);
	if (!inDocs.startsWith("..") && target.endsWith(".md")) {
		return docsRoute(inDocs) + suffix;
	}
	const inRepo = relative(REPO_ROOT, target);
	const readme = readmeDocsPath(inRepo);
	if (readme !== undefined) return docsRoute(readme) + suffix;
	// Anything else in the repository (sources, workflows) lives on GitHub.
	return REPO_BLOB + inRepo + suffix;
}

/** Images inside the book are served from `/docs/...` by `pages/docs/images/[...file].ts`. */
function rewriteImage(src: string, sourceFile: string): string {
	if (/^[a-z]+:|^\//i.test(src)) return src;
	const target = normalize(join(dirname(sourceFile), src));
	const inDocs = relative(DOCS_SOURCE, target);
	return inDocs.startsWith("..")
		? REPO_BLOB + relative(REPO_ROOT, target) + "?raw=true"
		: `/docs/${inDocs}`;
}

/** Inline HTML in markdown (mdBook pages use it for images) stays a `raw` string node. */
interface RawNode {
	type: "raw";
	value: string;
}

function isRaw(node: unknown): node is RawNode {
	return typeof node === "object" && node !== null &&
		(node as { type?: unknown }).type === "raw";
}

function rewriteRaw(node: RawNode, sourceFile: string): void {
	node.value = node.value
		.replace(
			/(<img\b[^>]*\bsrc=")([^"]+)(")/gi,
			(_, before: string, src: string, after: string) =>
				before + rewriteImage(src, sourceFile) + after,
		)
		.replace(
			/(<a\b[^>]*\bhref=")([^"]+)(")/gi,
			(_, before: string, href: string, after: string) =>
				before + rewrite(href, sourceFile) + after,
		);
}

function rewriteElement(element: Element, sourceFile: string): void {
	const attribute = element.tagName === "a"
		? "href"
		: element.tagName === "img"
		? "src"
		: undefined;
	if (attribute === undefined) return;
	const value = element.properties[attribute];
	if (typeof value !== "string") return;
	element.properties[attribute] = attribute === "href"
		? rewrite(value, sourceFile)
		: rewriteImage(value, sourceFile);
}

function visit(
	node: Root | ElementContent | RootContent,
	sourceFile: string,
): void {
	if (!("children" in node)) return;
	for (const child of node.children) {
		if (isRaw(child)) rewriteRaw(child, sourceFile);
		else if (child.type === "element") rewriteElement(child, sourceFile);
		visit(child, sourceFile);
	}
}

function isBlankText(node: RootContent): boolean {
	return node.type === "text" && "value" in node && node.value.trim() === "";
}

/**
 * Readmes open with a centred header for GitHub, crates.io, npm, and pub.dev:
 * the logo, then links and badges. On the site the page header already
 * carries all of that, so every leading centred block is dropped.
 */
function dropReadmeHeader(tree: Root): void {
	for (;;) {
		const index = tree.children.findIndex((child) => !isBlankText(child));
		// `unknown`: raw nodes are not in hast's own union, so narrowing one is `never`.
		const first: unknown = tree.children[index];
		if (
			!isRaw(first) || !/^<p align="center">[\s\S]*<\/p>\s*$/.test(first.value)
		) return;
		tree.children.splice(0, index + 1);
	}
}

/**
 * Rewrites the relative `.md` links and image paths mdBook and GitHub
 * understand into site routes, so the same markdown works in the book, on
 * GitHub, and here. Applies to `docs/src` and to crate and package readmes.
 */
export function rehypeDocsLinks() {
	return (tree: Root, file: { path?: string }) => {
		if (file.path === undefined || !rendersInDocs(file.path)) return;
		if (!file.path.startsWith(DOCS_SOURCE)) dropReadmeHeader(tree);
		visit(tree, file.path);
	};
}
