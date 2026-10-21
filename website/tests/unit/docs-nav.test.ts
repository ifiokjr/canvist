import { assertEquals } from "@std/assert";
import { buildDocsNav, flatten, parseSummary } from "../../src/lib/docs-nav.ts";
import { docsRoute, readmeDocsPath } from "../../src/lib/rehype-docs-links.ts";

const page = (path: string, body = "") => ({
	path,
	source: `docs/src/${path}`,
	body,
});

Deno.test("SUMMARY.md part headings become sidebar groups, in summary order", () => {
	const summary = parseSummary(
		[
			"# Summary",
			"",
			"# Get started",
			"",
			"- [Introduction](./readme.md)",
			"- [Pick your platform](./quick-start/readme.md)",
			"  - [Server: Rust](./quick-start/server-rust.md)",
			"",
			"# canvist Cloud",
			"",
			"- [Overview](./cloud/readme.md)",
			"  - [Sync](./cloud/sync.md)",
			"",
			"# Reference",
			"",
			"- [Errors](./reference/errors.md)",
		].join("\n"),
	);
	assertEquals(summary.get("readme.md"), {
		title: "Introduction",
		part: "Get started",
	});

	const nav = buildDocsNav(
		[
			page("reference/errors.md", "# Errors"),
			page("cloud/sync.md", "# canvist Cloud Sync"),
			page("cloud/readme.md", "# canvist Cloud"),
			page("quick-start/server-rust.md", "# Quick start: Rust"),
			page("quick-start/readme.md", "# Pick your platform"),
			page("readme.md", "# canvist"),
		],
		summary,
	);
	assertEquals(
		nav.map((group) => [group.title, group.links.map((link) => link.label)]),
		[
			["Get started", ["Introduction", "Pick your platform", "Server: Rust"]],
			["canvist Cloud", ["Overview", "Sync"]],
			["Reference", ["Errors"]],
		],
	);
});

Deno.test("pages missing from SUMMARY.md still land in the sidebar, in their folder's group", () => {
	const summary = parseSummary(
		"# Summary\n\n- [Introduction](./readme.md)\n- [Collaboration](./guides/collaboration.md)\n",
	);
	const nav = buildDocsNav(
		[
			page("readme.md", "# canvist"),
			page("guides/collaboration.md", "# Collaboration"),
			page("cloud/self-hosting.md", "# Self-hosting canvist Cloud"),
			page("cloud/api.md", "# canvist Cloud API reference"),
			page("brand-new/notes.md", "# Notes"),
		],
		summary,
	);
	assertEquals(nav.map((group) => group.title), [
		"Get started",
		"Guides",
		"canvist Cloud",
		"Brand new",
	]);
	const cloud = nav.find((group) => group.title === "canvist Cloud");
	assertEquals(cloud?.links.map((link) => [link.route, link.label]), [
		["/docs/cloud/api", "API reference"],
		["/docs/cloud/self-hosting", "Self-hosting canvist Cloud"],
	]);
});

Deno.test("crate and package readmes render under virtual docs paths", () => {
	assertEquals(
		readmeDocsPath("crates/canvist_headless/readme.md"),
		"crates/canvist_headless.md",
	);
	assertEquals(
		readmeDocsPath("crates/canvist_model"),
		"crates/canvist_model.md",
	);
	assertEquals(
		readmeDocsPath("packages/canvist-headless/readme.md"),
		"packages/canvist-headless.md",
	);
	assertEquals(readmeDocsPath("crates/canvist_headless/src/lib.rs"), undefined);
	assertEquals(
		docsRoute("crates/canvist_headless.md"),
		"/docs/crates/canvist_headless",
	);

	const nav = buildDocsNav(
		[
			{
				path: "crates/canvist_headless.md",
				source: "crates/canvist_headless/readme.md",
				body: "# canvist_headless",
			},
			{
				path: "packages/canvist-headless.md",
				source: "packages/canvist-headless/readme.md",
				body: "# @canvist/headless",
			},
		],
		new Map(),
	);
	assertEquals(
		flatten(nav).map((link) => [link.group, link.title, link.source]),
		[
			["Rust crates", "canvist_headless", "crates/canvist_headless/readme.md"],
			["Packages", "@canvist/headless", "packages/canvist-headless/readme.md"],
		],
	);
});
