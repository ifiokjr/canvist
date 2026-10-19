import type { APIRoute } from "astro";
import { llmsIndex, readLlmsPages } from "../lib/llms.ts";
import { repoPath } from "../lib/paths.ts";

/** The docs index for AI agents (https://llmstxt.org), from `docs/src/SUMMARY.md`. */
export const GET: APIRoute = ({ site }) => {
	if (site === undefined) {
		throw new Error("llms.txt needs `site` set in astro.config.ts");
	}
	return new Response(llmsIndex(readLlmsPages(repoPath("docs/src")), site), {
		headers: { "Content-Type": "text/plain; charset=utf-8" },
	});
};
