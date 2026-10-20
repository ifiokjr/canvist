import { assertEquals } from "@std/assert";
import type { Root, RootContent, Text } from "hast";
import { fileURLToPath } from "node:url";
import { rehypeDocsLinks } from "../../src/lib/rehype-docs-links.ts";

const REPO_ROOT = fileURLToPath(new URL("../../../", import.meta.url));

// Raw HTML blocks are not in hast's own union, so they are built as content.
const raw = (value: string) =>
	({ type: "raw", value }) as unknown as RootContent;
const text = (value: string): Text => ({ type: "text", value });
const heading: RootContent = {
	type: "element",
	tagName: "h1",
	properties: {},
	children: [text("canvist_model")],
};

// The rendered nodes, without the blank text between blocks.
function render(path: string, children: RootContent[]): RootContent[] {
	const tree: Root = { type: "root", children };
	rehypeDocsLinks()(tree, { path: `${REPO_ROOT}${path}` });
	return tree.children.filter((node) =>
		node.type !== "text" || node.value.trim() !== ""
	);
}

Deno.test("a readme's centred header (logo, links, badges) is dropped on the site", () => {
	const children = render("crates/canvist_model/readme.md", [
		raw(
			`<p align="center">\n  <a href="https://github.com/ifiokjr/canvist"><picture></picture></a>\n</p>`,
		),
		text("\n"),
		raw(
			`<p align="center">\n  <a href="https://docs.rs"><strong>API</strong></a>\n</p>`,
		),
		text("\n"),
		raw(
			`<p align="center">\n  <a href="https://crates.io"><img src="badge.svg" alt="" /></a>\n</p>`,
		),
		text("\n"),
		heading,
	]);

	assertEquals(children, [heading]);
});

Deno.test("centred blocks after a readme's first heading stay", () => {
	const figure = raw(
		`<p align="center"><img src="https://example.com/shot.png" alt="" /></p>`,
	);
	const children = render("crates/canvist_model/readme.md", [heading, figure]);

	assertEquals(children, [heading, figure]);
});

Deno.test("book pages keep a leading centred block", () => {
	const figure = raw(
		`<p align="center"><img src="https://example.com/shot.png" alt="" /></p>`,
	);
	const children = render("docs/src/guides/performance.md", [figure, heading]);

	assertEquals(children, [figure, heading]);
});
