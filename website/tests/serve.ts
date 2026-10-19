/**
 * Serves `dist/` the way the production Caddyfile does: `/name` serves
 * `name.html`, and unknown paths get `404.html` with a 404 status.
 *
 *   deno run --allow-net --allow-read tests/serve.ts [port]
 */
import { serveDir } from "@std/http/file-server";

const root = new URL("../dist/", import.meta.url).pathname;
const port = Number(Deno.args[0] ?? "4600");

async function exists(path: string): Promise<boolean> {
	try {
		return (await Deno.stat(path)).isFile;
	} catch (error) {
		if (error instanceof Deno.errors.NotFound) return false;
		throw error;
	}
}

Deno.serve({ port, hostname: "127.0.0.1" }, async (request) => {
	const url = new URL(request.url);
	const path = decodeURIComponent(url.pathname);
	if (
		!path.endsWith("/") && !(await exists(root + path)) &&
		(await exists(`${root}${path}.html`))
	) {
		url.pathname = `${path}.html`;
		return serveDir(new Request(url, request), { fsRoot: root, quiet: true });
	}
	const response = await serveDir(request, { fsRoot: root, quiet: true });
	if (response.status !== 404) return response;
	const body = await Deno.readFile(`${root}404.html`);
	return new Response(body, {
		status: 404,
		headers: { "Content-Type": "text/html; charset=utf-8" },
	});
});
