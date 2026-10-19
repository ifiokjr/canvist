import { unified } from "@astrojs/markdown-remark";
import { defineConfig } from "astro/config";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { rehypeDocsLinks } from "./src/lib/rehype-docs-links.ts";

const repoRoot = fileURLToPath(new URL("..", import.meta.url));

/**
 * The public origin, used for canonical URLs and Open Graph images. Railway
 * passes `RAILWAY_PUBLIC_DOMAIN` to the build; `SITE_URL` overrides it.
 */
function siteUrl(): string {
	const explicit = process.env["SITE_URL"];
	if (explicit) return explicit;
	const railway = process.env["RAILWAY_PUBLIC_DOMAIN"];
	if (railway) return `https://${railway}`;
	return "http://localhost:4321";
}

export default defineConfig({
	site: siteUrl(),
	output: "static",
	trailingSlash: "never",
	build: {
		format: "file",
		inlineStylesheets: "always",
	},
	prefetch: {
		prefetchAll: false,
		defaultStrategy: "hover",
	},
	markdown: {
		shikiConfig: {
			themes: { light: "github-light", dark: "github-dark-dimmed" },
			defaultColor: false,
			wrap: false,
		},
		processor: unified({ rehypePlugins: [rehypeDocsLinks] }),
	},
	vite: {
		define: {
			// Build-time file reads resolve from here; bundling moves import.meta.url.
			"import.meta.env.REPO_ROOT": JSON.stringify(repoRoot),
		},
		server: {
			// The site reads the docs, the brand assets, and the v1 engine
			// from the rest of the repository.
			fs: { allow: [repoRoot] },
		},
		build: {
			assetsInlineLimit: 0,
		},
	},
});
