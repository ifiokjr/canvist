import type { APIRoute } from "astro";
import { llmsFull, readLlmsPages } from "../lib/llms.ts";
import { repoPath } from "../lib/paths.ts";

/** Every docs page in one markdown file, for AI agents that read it all at once. */
export const GET: APIRoute = ({ site }) => {
	if (site === undefined) {
		throw new Error("llms-full.txt needs `site` set in astro.config.ts");
	}
	return new Response(llmsFull(readLlmsPages(repoPath("docs/src")), site), {
		headers: { "Content-Type": "text/plain; charset=utf-8" },
	});
};
