/**
 * The example server's HTTP handler, using only web-standard `Request` and
 * `Response`, so any runtime with a fetch-style server can host it.
 *
 * - `POST /convert?from=markdown&to=json` converts the request body.
 * - `GET /rooms/:name` returns a room's document as JSON (`?format=` picks
 *   another format), as merged from every connected client.
 *
 * @module
 */
import {
	type Engine,
	type Format,
	isCanvistError,
	type StarterKit,
} from "../../src/mod.ts";
import type { Rooms } from "./rooms.ts";

const FORMATS: readonly Format[] = ["json", "html", "markdown", "text"];
const CONTENT_TYPES: Record<Format, string> = {
	json: "application/json",
	html: "text/html; charset=utf-8",
	markdown: "text/markdown; charset=utf-8",
	text: "text/plain; charset=utf-8",
};

function format(value: string | null, fallback: Format): Format | undefined {
	const name = value ?? fallback;
	return FORMATS.find((candidate) => candidate === name);
}

function problem(status: number, code: string, message: string): Response {
	return Response.json({ code, message }, { status });
}

/** Handle the HTTP routes; WebSocket upgrades are the host's job. */
export function handle(
	request: Request,
	engine: Engine<StarterKit>,
	rooms: Rooms,
): Promise<Response> {
	const url = new URL(request.url);
	const room = url.pathname.match(/^\/rooms\/([\w-]+)$/)?.[1];

	if (request.method === "POST" && url.pathname === "/convert") {
		return convert(request, url, engine);
	}

	if (request.method === "GET" && room !== undefined) {
		const to = format(url.searchParams.get("format"), "json") ?? "json";
		const body = rooms.get(room).doc.export(to);

		return Promise.resolve(
			new Response(body, { headers: { "content-type": CONTENT_TYPES[to] } }),
		);
	}

	return Promise.resolve(
		problem(404, "notFound", `no route for ${request.method} ${url.pathname}`),
	);
}

/** Convert the request body between formats. */
async function convert(
	request: Request,
	url: URL,
	engine: Engine<StarterKit>,
): Promise<Response> {
	const from = format(url.searchParams.get("from"), "markdown");
	const to = format(url.searchParams.get("to"), "json");

	if (from === undefined || to === undefined) {
		return problem(400, "parse", `formats are ${FORMATS.join(", ")}`);
	}

	try {
		using doc = engine.parse(from, await request.text());

		return new Response(doc.export(to), {
			headers: { "content-type": CONTENT_TYPES[to] },
		});
	} catch (error) {
		if (isCanvistError(error)) {
			return problem(422, error.code, error.message);
		}

		throw error;
	}
}
