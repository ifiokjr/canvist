/**
 * The examples' server: Vite in middleware mode, which renders the two
 * server-rendered pages on every request, as Next.js and Nuxt do, and
 * serves every other page as Vite's dev server would.
 *
 * - `/ssr.html`: canvist's HTML in a plain page, hydrated by `src/ssr.ts`.
 * - `/ssr-react.html`: a React tree rendered with `renderToString`, holding
 *   canvist's HTML, hydrated by `src/ssr-react/client.tsx`.
 *
 * ```sh
 * node server.ts   # http://localhost:5180
 * ```
 */
import { createRenderer } from "@canvist/headless/ssr";
import { readFile } from "node:fs/promises";
import { createServer as createHttpServer } from "node:http";
import { createServer as createViteServer } from "vite";
import { CONTENT, THEME, WIDTH } from "./src/ssr-content.ts";
import type { render as renderReact } from "./src/ssr-react/server.tsx";

const PORT = 5180;
const ROOT = new URL(".", import.meta.url);

const vite = await createViteServer({
	server: { middlewareMode: true },
	appType: "mpa",
});
// One renderer for the server's lifetime: it caches text measurements.
const renderer = await createRenderer({ theme: THEME });

/** The pages this server renders, by path. */
const rendered: Record<string, (url: string) => Promise<string>> = {
	"/ssr.html": async (url) => {
		const { html, css } = renderer.html(CONTENT, { width: WIDTH });

		return page("ssr.html", url, {
			"<!-- canvist:css -->": css,
			"<!-- canvist:html -->": html,
		});
	},
	"/ssr-react.html": async (url) => {
		const doc = renderer.html(CONTENT, { width: WIDTH });
		const { render } = await vite.ssrLoadModule(
			"/src/ssr-react/server.tsx",
		) as { render: typeof renderReact };

		return page("ssr-react.html", url, {
			"<!-- react:app -->": render(doc),
			// The browser hydrates with the same props, as Next.js sends its own.
			"<!-- react:props -->": scriptJson(doc),
		});
	},
};

const server = createHttpServer((request, response) => {
	const path = new URL(request.url ?? "/", "http://localhost").pathname;
	const render = rendered[path];

	if (render === undefined) {
		vite.middlewares(request, response);
		return;
	}

	render(request.url ?? path).then(
		(html) => {
			response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
			response.end(html);
		},
		(error: unknown) => {
			if (error instanceof Error) {
				vite.ssrFixStacktrace(error);
			}

			console.error(error);
			response.writeHead(500, { "content-type": "text/plain" });
			response.end(String(error));
		},
	);
});

server.listen(PORT, () => {
	console.log(`canvist examples on http://localhost:${PORT}/`);
});

/**
 * The page template `name` with each placeholder replaced by its value,
 * then Vite's client and transforms applied.
 */
async function page(
	name: string,
	url: string,
	values: Record<string, string>,
): Promise<string> {
	const template = await readFile(new URL(name, ROOT), "utf8");
	const html = Object.entries(values).reduce(
		(filled, [placeholder, value]) => filled.replace(placeholder, () => value),
		template,
	);

	return vite.transformIndexHtml(url, html);
}

/** `value` as JSON that is safe inside a `<script>` element. */
function scriptJson(value: unknown): string {
	return JSON.stringify(value).replaceAll("<", "\\u003c");
}
