/**
 * A real Hocuspocus server with a JavaScript client, for `hocuspocus.rs`.
 *
 * The server runs `@hocuspocus/server`'s core on `Deno.serve`. A stock
 * `HocuspocusProvider` connects to it over a WebSocket, and the Rust
 * `SyncSession` connects through this script: frames it writes go straight
 * into the server's connection handler, and frames the server sends back
 * come out on stdout.
 *
 * Messages, one JSON object per line:
 *
 * - `{ "frame": "<hex>" }` delivers a frame from the Rust session.
 * - `{ "edit": "<name>" }` makes a scripted edit on the JavaScript client.
 * - `{ "cursor": { "path": [0], "index": 3 } }` publishes the JavaScript
 *   client's caret, y-prosemirror style, at a UTF-16 index of a block.
 * - `{ "dump": true }` replies with `{ "json": …, "cursors": […] }`: the
 *   client's document and the carets of other peers it can resolve.
 *
 * After each message the script waits for the server to settle, writes any
 * frames for the Rust session as `{ "frame": "<hex>" }`, then `{ "done": true }`.
 */

import { HocuspocusProvider } from "npm:@hocuspocus/provider@4.7.0";
import { Hocuspocus } from "npm:@hocuspocus/server@4.7.0";
import crossws from "npm:crossws@0.4.15/adapters/deno";

import {
	edit,
	fromHex,
	messages,
	send,
	text,
	toHex,
	toJson,
	Y,
} from "./canvist.ts";

const DOCUMENT = "canvist";
/** How long to let the server and client exchange messages. */
const SETTLE_MS = 100;
/** The WebSocket `OPEN` ready state. */
const OPEN = 1;

const hocuspocus = new Hocuspocus({ quiet: true });
type Peer = { websocket: unknown; request: Request; _connection?: Connection };
type Connection = ReturnType<typeof hocuspocus.handleConnection>;

const websockets = crossws({
	hooks: {
		open(peer) {
			const client = peer as unknown as Peer;
			client._connection = hocuspocus.handleConnection(
				client.websocket as never,
				client.request,
			);
		},
		message(peer, message) {
			(peer as unknown as Peer)._connection?.handleMessage(
				message.uint8Array(),
			);
		},
		close(peer, event) {
			(peer as unknown as Peer)._connection?.handleClose({
				code: event.code ?? 1000,
				reason: event.reason ?? "",
			});
		},
	},
});

const server = Deno.serve(
	{ port: 0, hostname: "127.0.0.1", onListen() {} },
	(request, info) => websockets.handleUpgrade(request, info),
);
const url = `ws://127.0.0.1:${server.addr.port}`;

// The Rust session's connection: a socket whose frames go to stdout.
const pending: Uint8Array[] = [];
const rust = hocuspocus.handleConnection(
	{
		readyState: OPEN,
		send(data: ArrayBufferLike | ArrayBufferView) {
			pending.push(new Uint8Array(data as ArrayBuffer));
		},
		close() {},
	} as never,
	new Request(url),
);

const doc = new Y.Doc();
const provider = new HocuspocusProvider({
	url,
	name: DOCUMENT,
	document: doc,
	token: "",
});
await new Promise((resolve) => provider.on("synced", resolve));

const blocks = doc.getArray<Y.Map<unknown>>("blocks");

/** Publish a caret like y-prosemirror's cursor plugin does. */
function setCursor(path: number[], index: number) {
	const position = Y.relativePositionToJSON(
		Y.createRelativePositionFromTypeIndex(text(blocks, path), index),
	);
	provider.setAwarenessField("user", { name: "js", color: "#6366f1" });
	provider.setAwarenessField("cursor", { anchor: position, head: position });
}

/** Other peers' carets as block ids and UTF-16 indices. */
function cursors() {
	const states = [...provider.awareness!.getStates().entries()];

	return states
		.filter(([client, state]) => client !== doc.clientID && state.cursor)
		.map(([, state]) => {
			const head = Y.createAbsolutePositionFromRelativePosition(
				Y.createRelativePositionFromJSON(state.cursor.head),
				doc,
			);
			const owner = (head?.type as Y.Text | undefined)?.parent as Y.Map<
				unknown
			>;

			return {
				name: state.user?.name,
				block: owner?.get("id"),
				index: head?.index,
			};
		});
}

/** Act on one message from the Rust test: a frame, an edit, a caret, or a dump. */
function handle(message: Record<string, unknown>) {
	if (typeof message.frame === "string") {
		rust.handleMessage(fromHex(message.frame));
	} else if (typeof message.edit === "string") {
		edit(doc, message.edit);
	} else if (message.cursor) {
		const { path, index } = message.cursor as { path: number[]; index: number };
		setCursor(path, index);
	} else if (message.dump) {
		send({ json: toJson(doc), cursors: cursors() });
	}
}

for await (const message of messages()) {
	handle(message);
	await new Promise((resolve) => setTimeout(resolve, SETTLE_MS));

	for (const frame of pending.splice(0)) {
		send({ frame: toHex(frame) });
	}

	send({ done: true });
}

provider.destroy();
await server.shutdown();
