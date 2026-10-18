/**
 * A plain JavaScript Yjs peer for `js_compat.rs`.
 *
 * The Rust test drives it over stdin and stdout, one JSON message per line:
 *
 * - `{ "update": "<hex>" }` applies a Yjs update (v1).
 * - `{ "edit": "<name>" }` runs one of the scripted edits in `canvist.ts`.
 * - `{ "dump": true }` replies with `{ "json": <ProseMirror JSON> }`.
 *
 * Every local change is sent back as `{ "update": "<hex>" }`, and every
 * message is acknowledged with `{ "done": true }`.
 */

import { edit, fromHex, messages, send, toHex, toJson, Y } from "./canvist.ts";

/** A fixed client id keeps runs reproducible. */
const CLIENT_ID = 4242;

const doc = new Y.Doc();
doc.clientID = CLIENT_ID;

doc.on("update", (update: Uint8Array, origin: unknown) => {
	if (origin !== "remote") {
		send({ update: toHex(update) });
	}
});

for await (const message of messages()) {
	if (typeof message.update === "string") {
		Y.applyUpdate(doc, fromHex(message.update), "remote");
	} else if (typeof message.edit === "string") {
		edit(doc, message.edit);
	} else if (message.dump) {
		send({ json: toJson(doc) });
	}

	send({ done: true });
}
