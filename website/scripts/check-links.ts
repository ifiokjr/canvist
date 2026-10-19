/**
 * Fails when the built site has a broken internal link, image or anchor.
 *
 *   deno run --allow-read scripts/check-links.ts [dist]
 *
 * Checks every `href` and `src` in `dist/**.html`:
 * - site paths resolve to a built file (`/name` serves `name.html`, as Caddy does);
 * - `#fragments` exist as an `id` (or `name`) on the target page;
 * - links into this repository on GitHub (`…/canvist/blob/main/<path>`, which
 *   is where docs links to sources are rewritten) point at a file that exists.
 *
 * External sites are not fetched.
 */
import { walk } from "@std/fs/walk";
import { dirname, fromFileUrl, join, relative } from "@std/path";

const WEBSITE = fromFileUrl(new URL("..", import.meta.url));
const REPO = join(WEBSITE, "..");
const DIST = join(WEBSITE, Deno.args[0] ?? "dist");
const REPO_BLOB =
	/^https:\/\/github\.com\/ifiokjr\/canvist\/(?:blob|tree|edit)\/main\/([^?#]*)/;
const ATTRIBUTE = /\s(href|src)="([^"]*)"/g;

interface Problem {
	page: string;
	link: string;
	reason: string;
}

async function isFile(path: string): Promise<boolean> {
	try {
		return (await Deno.stat(path)).isFile;
	} catch (error) {
		if (error instanceof Deno.errors.NotFound) return false;
		throw error;
	}
}

async function exists(path: string): Promise<boolean> {
	try {
		await Deno.stat(path);
		return true;
	} catch (error) {
		if (error instanceof Deno.errors.NotFound) return false;
		throw error;
	}
}

/** The built file a site path serves, mirroring the Caddyfile's try_files. */
async function builtFile(sitePath: string): Promise<string | undefined> {
	const path = decodeURIComponent(sitePath);
	const candidates = path.endsWith("/") ? [join(DIST, path, "index.html")] : [
		join(DIST, path),
		`${join(DIST, path)}.html`,
		join(DIST, path, "index.html"),
	];
	for (const candidate of candidates) {
		if (await isFile(candidate)) return candidate;
	}
	return undefined;
}

const anchorCache = new Map<string, Set<string>>();

async function anchorsOf(file: string): Promise<Set<string>> {
	const cached = anchorCache.get(file);
	if (cached) return cached;
	const html = await Deno.readTextFile(file);
	const anchors = new Set(
		[...html.matchAll(/\s(?:id|name)="([^"]+)"/g)].map((match) =>
			match[1] ?? ""
		),
	);
	anchorCache.set(file, anchors);
	return anchors;
}

/** The site path a built file is served at. */
function sitePathOf(file: string): string {
	const path = `/${relative(DIST, file)}`.replace(/\.html$/, "").replace(
		/\/index$/,
		"/",
	);
	return path === "" ? "/" : path;
}

async function check(file: string): Promise<Problem[]> {
	const problems: Problem[] = [];
	const page = sitePathOf(file);
	const html = await Deno.readTextFile(file);
	const base = new URL(page, "https://site.invalid");

	for (const [, , raw = ""] of html.matchAll(ATTRIBUTE)) {
		const link = raw.replaceAll("&amp;", "&");
		if (link === "" || /^(mailto|tel|data|javascript):/i.test(link)) continue;

		const repo = REPO_BLOB.exec(link);
		if (repo) {
			const target = decodeURIComponent(repo[1] ?? "");
			if (!(await exists(join(REPO, target)))) {
				problems.push({ page, link, reason: `no ${target} in the repository` });
			}
			continue;
		}
		if (/^[a-z]+:|^\/\//i.test(link)) continue;

		const url = new URL(link, base);
		const target = url.pathname === base.pathname && link.startsWith("#")
			? file
			: await builtFile(url.pathname);
		if (target === undefined) {
			problems.push({ page, link, reason: "no built page or file" });
			continue;
		}
		const fragment = decodeURIComponent(url.hash.slice(1));
		if (
			fragment !== "" && target.endsWith(".html") &&
			!(await anchorsOf(target)).has(fragment)
		) {
			problems.push({
				page,
				link,
				reason: `no #${fragment} on ${sitePathOf(target)}`,
			});
		}
	}
	return problems;
}

const files: string[] = [];
for await (const entry of walk(DIST, { exts: [".html"], includeDirs: false })) {
	// Pagefind's UI fragments are not pages.
	if (!entry.path.includes(`${dirname(DIST)}/dist/pagefind/`)) {
		files.push(entry.path);
	}
}
if (files.length === 0) {
	throw new Error(`No HTML in ${DIST}; run \`deno task build\` first.`);
}

const problems = (await Promise.all(files.map(check))).flat();
if (problems.length > 0) {
	console.error(`${problems.length} broken link(s) in ${files.length} pages:`);
	for (const problem of problems) {
		console.error(`  ${problem.page}: ${problem.link} (${problem.reason})`);
	}
	Deno.exit(1);
}
console.log(
	`Checked ${files.length} pages: no broken internal links, images or anchors.`,
);
