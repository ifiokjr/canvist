import { existsSync } from "node:fs";
import type { ServiceId } from "../data/services.ts";
import { repoPath } from "./paths.ts";
import { docsRoute } from "./rehype-docs-links.ts";

export interface CloudDocsLink {
	href: string;
	/** False until canvist Cloud's docs land in `docs/src/cloud/`. */
	published: boolean;
}

/**
 * The docs page for a hosted service. canvist Cloud's docs are being written
 * alongside the service; until `docs/src/cloud/` exists the link goes to the
 * roadmap and the page says the docs are coming.
 */
export function cloudDocsFor(service: ServiceId): CloudDocsLink {
	for (const candidate of [`cloud/${service}.md`, "cloud/readme.md"]) {
		if (existsSync(repoPath("docs/src", candidate))) {
			return { href: docsRoute(candidate), published: true };
		}
	}
	return { href: "/docs/v2/roadmap", published: false };
}
