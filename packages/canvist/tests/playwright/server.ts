/**
 * A minimal static file server for the Playwright tests.
 *
 * Serves the demo HTML and WASM files so the browser can load them.
 */

import { extname, join } from "@std/path";

const MIME_TYPES: Record<string, string> = {
	".html": "text/html",
	".js": "application/javascript",
	".mjs": "application/javascript",
	".ts": "application/typescript",
	".wasm": "application/wasm",
	".css": "text/css",
	".json": "application/json",
	".png": "image/png",
	".svg": "image/svg+xml",
};

/** Start a local HTTP server for the demo. Returns the server and its URL. */
export function startServer(
	root: string,
	port = 0,
): { server: Deno.HttpServer; url: string; port: number } {
	const server = Deno.serve({ port, hostname: "127.0.0.1" }, async (req) => {
		const url = new URL(req.url);
		let path = decodeURIComponent(url.pathname);
		if (path === "/") path = "/demo/index.html";

		const filePath = join(root, path);

		try {
			const file = await Deno.open(filePath, { read: true });
			const ext = extname(filePath);
			const contentType = MIME_TYPES[ext] ?? "application/octet-stream";

			return new Response(file.readable, {
				headers: {
					"content-type": contentType,
					"cross-origin-opener-policy": "same-origin",
					"cross-origin-embedder-policy": "require-corp",
				},
			});
		} catch {
			return new Response("Not Found", { status: 404 });
		}
	});

	const addr = server.addr;
	const actualPort = addr.port;
	const urlStr = `http://127.0.0.1:${actualPort}`;
	return { server, url: urlStr, port: actualPort };
}
