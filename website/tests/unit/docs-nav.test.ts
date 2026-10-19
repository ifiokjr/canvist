import { assertEquals } from "@std/assert";
import { buildDocsNav, flatten, parseSummary } from "../../src/lib/docs-nav.ts";
import { docsRoute, readmeDocsPath } from "../../src/lib/rehype-docs-links.ts";

const page = (path: string, body = "") => ({
	path,
	source: `docs/src/${path}`,
	body,
});

Deno.test("pages missing from summary.md still land in the sidebar, in their folder's group", () => {
	const summary = parseSummary(
		"- [Introduction](./readme.md)\n- [v2 Roadmap](./v2/roadmap.md)\n",
	);
	const nav = buildDocsNav(
		[
			page("readme.md", "# canvist"),
			page("v2/roadmap.md", "# Roadmap"),
			page("cloud/self-hosting.md", "# Self-hosting canvist Cloud"),
			page("cloud/api.md", "# Cloud API"),
			page("brand-new/notes.md", "# Notes"),
		],
		summary,
	);
	assertEquals(nav.map((group) => group.title), [
		"Guides",
		"v2 design",
		"canvist Cloud",
		"Brand new",
	]);
	const cloud = nav.find((group) => group.title === "canvist Cloud");
	assertEquals(cloud?.links.map((link) => [link.route, link.label]), [
		["/docs/cloud/api", "API"],
		["/docs/cloud/self-hosting", "Self-hosting canvist Cloud"],
	]);
});

Deno.test("crate and package readmes render under virtual docs paths", () => {
	assertEquals(
		readmeDocsPath("crates/canvist_core/readme.md"),
		"crates/canvist_core.md",
	);
	assertEquals(
		readmeDocsPath("crates/canvist_model"),
		"crates/canvist_model.md",
	);
	assertEquals(
		readmeDocsPath("packages/canvist/README.md"),
		"packages/canvist.md",
	);
	assertEquals(readmeDocsPath("crates/canvist_core/src/lib.rs"), undefined);
	assertEquals(
		docsRoute("crates/canvist_core.md"),
		"/docs/crates/canvist_core",
	);

	const nav = buildDocsNav(
		[
			{
				path: "crates/canvist_core.md",
				source: "crates/canvist_core/readme.md",
				body: "# canvist_core",
			},
			{
				path: "packages/canvist.md",
				source: "packages/canvist/README.md",
				body: "# @canvist/canvist",
			},
		],
		new Map(),
	);
	assertEquals(
		flatten(nav).map((link) => [link.group, link.title, link.source]),
		[
			["Rust crates", "canvist_core", "crates/canvist_core/readme.md"],
			["Packages", "@canvist/canvist", "packages/canvist/README.md"],
		],
	);
});
