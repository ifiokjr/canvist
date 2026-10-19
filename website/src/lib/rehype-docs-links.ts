import type { Element, ElementContent, Root, RootContent } from "hast";
import { dirname, join, normalize, relative } from "node:path";
import { fileURLToPath } from "node:url";

/** The mdBook source the `/docs` section renders. */
export const DOCS_SOURCE = fileURLToPath(
	new URL("../../../docs/src/", import.meta.url),
);
const REPO_ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const REPO_BLOB = "https://github.com/ifiokjr/canvist/blob/main/";

/**
 * Maps a markdown file inside `docs/src` to its route: `v2/architecture.md`
 * becomes `/docs/v2/architecture`, and a `readme.md` becomes its folder.
 */
export function docsRoute(pathInDocs: string): string {
	const withoutExt = pathInDocs.replace(/\.md$/, "");
	const route = withoutExt.replace(/(^|\/)readme$/, "");
	return route === "" ? "/docs" : `/docs/${route}`;
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
	// Anything outside the book (crates, packages, workflows) lives on GitHub.
	return REPO_BLOB + relative(REPO_ROOT, target) + suffix;
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

/** Images inside the book are served from `/docs/...` by `pages/docs/images/[...file].ts`. */
function rewriteImage(src: string, sourceFile: string): string {
	if (/^[a-z]+:|^\//i.test(src)) return src;
	const target = normalize(join(dirname(sourceFile), src));
	const inDocs = relative(DOCS_SOURCE, target);
	return inDocs.startsWith("..")
		? REPO_BLOB + relative(REPO_ROOT, target) + "?raw=true"
		: `/docs/${inDocs}`;
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

/**
 * Rewrites the relative `.md` links and image paths mdBook understands into
 * site routes, so the same markdown works in the book, on GitHub, and here.
 */
export function rehypeDocsLinks() {
	return (tree: Root, file: { path?: string }) => {
		if (!file.path?.startsWith(DOCS_SOURCE)) return;
		visit(tree, file.path);
	};
}
