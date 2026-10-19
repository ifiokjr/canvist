/**
 * The example server end to end, under Deno: HTTP conversion, and two plain
 * JavaScript Yjs clients editing one room through the y-sync endpoint.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as decoding from "lib0/decoding";
import * as encoding from "lib0/encoding";
import * as sync from "y-protocols/sync";
import * as Y from "yjs";

import { start } from "./main.ts";

const MESSAGE_SYNC = 0;

/** A Yjs client speaking y-websocket's protocol, as `WebsocketProvider` does. */
class Client {
	readonly doc = new Y.Doc();
	readonly #socket: WebSocket;

	constructor(url: string) {
		this.#socket = new WebSocket(url);
		this.#socket.binaryType = "arraybuffer";
		this.#socket.onopen = () =>
			this.#send((encoder) => sync.writeSyncStep1(encoder, this.doc));
		this.#socket.onmessage = (event: MessageEvent<ArrayBuffer>) =>
			this.#receive(new Uint8Array(event.data));
		this.doc.on("update", (update: Uint8Array, origin: unknown) => {
			if (origin !== this) {
				this.#send((encoder) => sync.writeUpdate(encoder, update));
			}
		});
	}

	/** The text of top-level block `index`. */
	text(index: number): string {
		const block = this.doc.getArray<Y.Map<unknown>>("blocks").get(index);
		return String(block?.get("text") ?? "");
	}

	type(index: number, at: number, text: string): void {
		const block = this.doc.getArray<Y.Map<Y.Text>>("blocks").get(index);
		block?.get("text")?.insert(at, text);
	}

	close(): void {
		this.#socket.close();
	}

	#receive(data: Uint8Array): void {
		const decoder = decoding.createDecoder(data);

		if (decoding.readVarUint(decoder) !== MESSAGE_SYNC) {
			return;
		}

		const encoder = encoding.createEncoder();
		encoding.writeVarUint(encoder, MESSAGE_SYNC);
		sync.readSyncMessage(decoder, encoder, this.doc, this);

		if (encoding.length(encoder) > 1) {
			this.#socket.send(encoding.toUint8Array(encoder));
		}
	}

	#send(write: (encoder: encoding.Encoder) => void): void {
		const encoder = encoding.createEncoder();
		encoding.writeVarUint(encoder, MESSAGE_SYNC);
		write(encoder);
		this.#socket.send(encoding.toUint8Array(encoder));
	}
}

async function until(condition: () => boolean, what: string): Promise<void> {
	const deadline = Date.now() + 5_000;

	while (!condition()) {
		assert.ok(Date.now() < deadline, `timed out waiting for ${what}`);
		await new Promise((resolve) => setTimeout(resolve, 10));
	}
}

test("converts Markdown over HTTP and syncs Yjs clients through a room", async () => {
	const server = await start(0);
	const base = `http://localhost:${server.addr.port}`;
	const clients: Client[] = [];

	try {
		const converted = await fetch(`${base}/convert?from=markdown&to=json`, {
			method: "POST",
			body: "# Hi **there**",
		});
		const json = await converted.json() as { content: { type: string }[] };
		assert.equal(json.content[0]?.type, "heading");

		const unknown = await fetch(`${base}/convert?from=docx`, {
			method: "POST",
			body: "",
		});
		assert.equal(unknown.status, 400);
		await unknown.body?.cancel();

		const alice = new Client(`ws://localhost:${server.addr.port}/sync/notes`);
		const bob = new Client(`ws://localhost:${server.addr.port}/sync/notes`);
		clients.push(alice, bob);
		await until(
			() => alice.text(0) === "Shared notes" && bob.text(0) === "Shared notes",
			"both clients to sync",
		);

		alice.type(1, 0, "Alice says hi. ");
		await until(
			() => bob.text(1).startsWith("Alice says hi."),
			"bob to see alice's edit",
		);

		const room = await fetch(`${base}/rooms/notes?format=markdown`);
		assert.equal(
			await room.text(),
			"# Shared notes\n\nAlice says hi. Edit me together.",
		);
	} finally {
		clients.forEach((client) => client.close());
		await server.shutdown();
	}
});
