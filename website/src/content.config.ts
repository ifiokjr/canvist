import { createMarkdownProcessor } from "@astrojs/markdown-remark";
import { glob } from "astro/loaders";
import { z } from "astro/zod";
import { defineCollection } from "astro:content";
import {
	CHANGELOG_DIRECTORY,
	loadReleases,
	USER_CHANGE_TYPES,
} from "./lib/changelog.ts";
import { SUMMARY_PATH } from "./lib/docs-nav.ts";
import { repoPath } from "./lib/paths.ts";

/**
 * The mdBook source in `docs/src`, rendered at `/docs`. The files carry no
 * frontmatter, so there is no schema; titles come from `SUMMARY.md` and each
 * page's first heading (see `lib/docs-nav.ts`).
 */
const docs = defineCollection({
	loader: glob({
		base: repoPath("docs/src"),
		pattern: ["**/*.md", `!${SUMMARY_PATH}`],
		// Keep the source path as the id: routes and edit links derive from it.
		generateId: ({ entry }) => entry,
	}),
});

/**
 * Crate and package readmes, rendered at `/docs/crates/<name>` and
 * `/docs/packages/<name>` so the API docs sit beside the book.
 */
const readmes = defineCollection({
	loader: glob({
		base: repoPath(""),
		pattern: [
			"crates/*/readme.md",
			"packages/*/readme.md",
			"packages/*/README.md",
		],
		generateId: ({ entry }) => entry,
	}),
});

/**
 * The user-facing changelog at `/changelog`: one JSON file per app release,
 * written by monochange from the `user` release stream only.
 * `loadReleases` rejects any other stream, so a developer note fails the build
 * instead of reaching the page.
 */
const changelog = defineCollection({
	loader: async () => {
		const markdown = await createMarkdownProcessor();
		const releases = await loadReleases(repoPath(CHANGELOG_DIRECTORY));
		return Promise.all(releases.map(async (release) => ({
			id: release.version,
			version: release.version,
			date: release.date,
			sections: await Promise.all(release.sections.map(async (section) => ({
				title: section.title,
				entries: await Promise.all(section.entries.map(async (entry) => ({
					type: entry.type,
					summary: entry.summary,
					html: (await markdown.render(entry.details)).code,
				}))),
			}))),
		})));
	},
	schema: z.object({
		version: z.string(),
		date: z.string(),
		sections: z.array(z.object({
			title: z.string(),
			entries: z.array(z.object({
				type: z.enum(USER_CHANGE_TYPES),
				summary: z.string(),
				html: z.string(),
			})),
		})),
	}),
});

export const collections = { changelog, docs, readmes };
