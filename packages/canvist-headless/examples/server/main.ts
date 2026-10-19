/**
 * An example canvist server for Deno: Markdown conversion over HTTP and a
 * y-websocket-compatible sync endpoint at `/sync/:room`.
 *
 * ```sh
 * deno run --allow-read --allow-net examples/server/main.ts
 * curl -X POST 'localhost:8787/convert?from=markdown&to=html' -d '# Hello **world**'
 * ```
 *
 * Point a `WebsocketProvider` from `y-websocket` at `ws://localhost:8787/sync`
 * with any room name, then read the merged document at `/rooms/<name>`.
 *
 * @module
 */
import { createEngine } from "../../src/mod.ts";
import { handle } from "./app.ts";
import { Rooms } from "./rooms.ts";

/** Start the server and return it; `port` 0 picks a free port. */
export async function start(
	port = 8787,
): Promise<Deno.HttpServer<Deno.NetAddr>> {
	const engine = await createEngine();
	const rooms = new Rooms(engine, "# Shared notes\n\nEdit me together.");

	return Deno.serve({ port, onListen: () => {} }, (request) => {
		const room = new URL(request.url).pathname.match(/^\/sync\/([\w-]+)$/)?.[1];

		if (room === undefined || request.headers.get("upgrade") !== "websocket") {
			return handle(request, engine, rooms);
		}

		const { socket, response } = Deno.upgradeWebSocket(request);
		const connection = { send: (message: Uint8Array) => socket.send(message) };
		socket.binaryType = "arraybuffer";
		socket.onopen = () => rooms.get(room).join(connection);
		socket.onmessage = (event) =>
			rooms.get(room).receive(connection, new Uint8Array(event.data));
		socket.onclose = () => rooms.get(room).leave(connection);
		return response;
	});
}

if (import.meta.main) {
	const server = await start(Number(Deno.env.get("PORT") ?? 8787));
	console.log(`canvist example server on http://localhost:${server.addr.port}`);
}
