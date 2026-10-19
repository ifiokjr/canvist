/**
 * A static server for the demo and its browser tests: the package root,
 * with `/` redirecting to the demo.
 *
 * Run it directly to try the demo: `deno task demo`.
 *
 * @module
 */
import { STATUS_CODE } from "@std/http/status";
import { dirname, extname, fromFileUrl, join, normalize } from "@std/path";

const ROOT = join(dirname(fromFileUrl(import.meta.url)), "..");

const MIME_TYPES: Readonly<Record<string, string>> = {
	".html": "text/html; charset=utf-8",
	".js": "text/javascript",
	".wasm": "application/wasm",
	".css": "text/css",
};

/** A running server and its address. */
export interface DemoServer {
	server: Deno.HttpServer;
	/** The demo page's address. */
	url: string;
}

/** Serve the package on `port` (any free port by default). */
export function startServer(port = 0): DemoServer {
	const server = Deno.serve(
		{ port, hostname: "127.0.0.1", onListen: () => {} },
		respond,
	);

	return { server, url: `http://127.0.0.1:${server.addr.port}/demo/` };
}

function respond(request: Request): Promise<Response> | Response {
	const url = new URL(request.url);

	if (url.pathname === "/") {
		return Response.redirect(
			new URL(`/demo/${url.search}`, url),
			STATUS_CODE.Found,
		);
	}

	// A directory serves its index page.
	const pathname = decodeURIComponent(url.pathname).slice(1);
	const relative = pathname === "" || pathname.endsWith("/")
		? `${pathname}index.html`
		: pathname;
	const path = normalize(join(ROOT, relative));

	if (!path.startsWith(ROOT)) {
		return new Response("Forbidden", { status: STATUS_CODE.Forbidden });
	}

	return serveFile(path);
}

async function serveFile(path: string): Promise<Response> {
	try {
		const file = await Deno.open(path, { read: true });
		return new Response(file.readable, {
			headers: {
				"content-type": MIME_TYPES[extname(path)] ?? "application/octet-stream",
				"cache-control": "no-store",
			},
		});
	} catch (error) {
		if (!(error instanceof Deno.errors.NotFound)) throw error;

		return new Response("Not Found", { status: STATUS_CODE.NotFound });
	}
}

if (import.meta.main) {
	const { url } = startServer(Number(Deno.env.get("PORT") ?? 8787));
	console.log(`canvist canvas demo: ${url}`);
}
