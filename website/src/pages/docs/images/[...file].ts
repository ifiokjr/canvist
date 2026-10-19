import type { APIRoute, GetStaticPaths } from "astro";
import { readdir, readFile } from "node:fs/promises";
import { extname, join, relative } from "node:path";
import { repoPath } from "../../../lib/paths.ts";

/** Serves the images the docs reference from `docs/src/images` at `/docs/images/...`. */
const IMAGES = repoPath("docs/src/images");

const TYPES: Record<string, string> = {
	".svg": "image/svg+xml",
	".png": "image/png",
	".jpg": "image/jpeg",
	".jpeg": "image/jpeg",
	".gif": "image/gif",
	".webp": "image/webp",
};

async function walk(directory: string): Promise<string[]> {
	const entries = await readdir(directory, { withFileTypes: true });
	const nested = await Promise.all(
		entries.map((
			entry,
		) => (entry.isDirectory()
			? walk(join(directory, entry.name))
			: Promise.resolve([join(directory, entry.name)]))
		),
	);
	return nested.flat();
}

export const getStaticPaths = (async () => {
	const files = await walk(IMAGES);
	return files
		.filter((file) => TYPES[extname(file).toLowerCase()] !== undefined)
		.map((file) => ({ params: { file: relative(IMAGES, file) } }));
}) satisfies GetStaticPaths;

export const GET: APIRoute = async ({ params }) => {
	const file = params["file"];
	if (file === undefined) return new Response(null, { status: 404 });
	const type = TYPES[extname(file).toLowerCase()];
	if (type === undefined) return new Response(null, { status: 404 });
	return new Response(await readFile(join(IMAGES, file)), {
		headers: { "Content-Type": type },
	});
};
