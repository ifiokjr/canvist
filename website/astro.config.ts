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
			themes: { light: "github-light", dark: "github-dark" },
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
			// The site reads the docs and the brand assets from the rest of the
			// repository, and links the canvist packages from `packages/`.
			fs: { allow: [repoRoot] },
		},
		// `@canvist/web` finds its WebAssembly next to its own modules, and
		// pre-bundling in development would move the modules away from it.
		optimizeDeps: {
			exclude: ["@canvist/web", "@canvist/headless"],
		},
		// The packages are linked from `packages/`, where `@canvist/web` links
		// its own `@canvist/headless`; the site resolves one copy, its own.
		resolve: {
			dedupe: ["@canvist/headless"],
		},
		build: {
			assetsInlineLimit: 0,
		},
	},
});
