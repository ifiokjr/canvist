import { glob } from "astro/loaders";
import { defineCollection } from "astro:content";
import { SUMMARY_PATH } from "./lib/docs-nav.ts";
import { repoPath } from "./lib/paths.ts";

/**
 * The mdBook source in `docs/src`, rendered at `/docs`. The files carry no
 * frontmatter, so there is no schema; titles come from `summary.md` and each
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

export const collections = { docs };
