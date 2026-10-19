import type { APIRoute, GetStaticPaths } from "astro";
import { readFile } from "node:fs/promises";
import { repoPath } from "../lib/paths.ts";

/**
 * Serves the generated web icons from `assets/brand/web` at the site root,
 * so the brand folder stays the only copy.
 */
const BRAND_WEB = repoPath("assets/brand/web");

const FILES = {
	"favicon.ico": "image/x-icon",
	"favicon.svg": "image/svg+xml",
	"apple-touch-icon.png": "image/png",
	"icon-192.png": "image/png",
	"icon-512.png": "image/png",
	"icon-maskable-512.png": "image/png",
	"og-image.png": "image/png",
	"site.webmanifest": "application/manifest+json",
} as const satisfies Record<string, string>;

type BrandFile = keyof typeof FILES;

export const getStaticPaths =
	(() =>
		(Object.keys(FILES) as BrandFile[]).map((brandFile) => ({
			params: { brandFile },
		}))) satisfies GetStaticPaths;

export const GET: APIRoute = async ({ params }) => {
	const name = params["brandFile"] as BrandFile;
	const body = await readFile(`${BRAND_WEB}/${name}`);
	return new Response(body, { headers: { "Content-Type": FILES[name] } });
};
