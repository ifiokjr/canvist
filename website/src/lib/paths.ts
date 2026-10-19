import { join } from "node:path";

/**
 * Absolute paths into the rest of the repository, for build-time reads. The
 * root comes from `astro.config.ts`, because bundling moves `import.meta.url`.
 */
export const REPO_ROOT: string = import.meta.env.REPO_ROOT;

export function repoPath(...segments: string[]): string {
	return join(REPO_ROOT, ...segments);
}
