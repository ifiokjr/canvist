# Collaboration

In this guide you make documents collaborative: put live editors in a room, share a document, merge edits from several people in any order, run a sync server that standard Yjs clients connect to, and use canvist Cloud instead of running one.

canvist documents are [Yjs](https://yjs.dev) documents underneath. Every edit becomes a Yjs update, and every Yjs update becomes an edit, so canvist works with y-websocket, Hocuspocus, and the rest of the Yjs ecosystem.

## Put editors in a room

`@canvist/web/collab` connects a browser editor to any y-websocket server: y-websocket's own, the server below, or [canvist Cloud Sync](../cloud/sync.md). The editor is the Yjs peer itself, so there's no `Y.Doc` to keep in step, and you don't need the `yjs` package:

```ts no_run
import { connect, createEditor } from "@canvist/web/collab";

const element = document.querySelector<HTMLElement>("#editor");

if (element !== null) {
	const editor = await createEditor(element, {
		placeholder: "Write together…",
	});
	const connection = connect(editor, {
		url: "wss://sync.example.com",
		room: "team-notes",
		params: { token: "…" },
	});

	connection.subscribe(() => console.log(connection.status, connection.synced));
	await connection.whenSynced();
}
```

- **`createEditor` from `@canvist/web/collab`** takes the same options as `@canvist/web`'s, and loads a second WebAssembly module with Yjs inside (about 140 KB more, gzipped). Editors that don't collaborate never download it.
- **The room's document replaces the editor's** once it arrives. An empty room gets one empty paragraph.
- **Undo is per person.** `Mod-z`, `editor.undo()`, and the `undo` command revert only this peer's changes, grouped by word as when typing alone.
- **It reconnects** with backoff when the connection drops, and catches up both ways. `connection.disconnect()` closes it for good. Call `connect` again to resume.
- **JavaScript Yjs peers in the room see the same document.** Read it with `engine.fromYjs(Y.encodeStateAsUpdate(ydoc), clientId)` from `@canvist/headless`.

The tests in `packages/canvist-web/tests/browser/collab.spec.ts` run two editors and a JavaScript `WebsocketProvider` peer against y-websocket's server in Chromium, Firefox, and WebKit. Presence (other people's carets) is coming soon.

## Share a document and merge edits

One peer, usually your server, creates the shared document. Others join from its state. Each peer needs its own **client id**, a number no other peer uses.

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();

// The server owns the document and shares it first.
const server = engine.parse("markdown", "Attendees:");
server.share(1);
const state = server.encodeState();

// Two people join and type at the same moment.
const alice = engine.fromYjs(state, 2);
const bob = engine.fromYjs(state, 3);
alice.select({ path: [0], offset: 10 });
alice.insertText(" Alice");
bob.select({ path: [0], offset: 10 });
bob.insertText(" Bob");

// Updates can arrive in any order. Every peer ends up the same.
for (const update of [...bob.takeUpdates(), ...alice.takeUpdates()]) {
	server.applyUpdate(update);
}
console.log(server.export("text"));
// Attendees: Alice Bob
```

- `share(clientId)` turns an existing document into a shared one.
- `encodeState()` is the whole shared state, to store or send to a new peer.
- `fromYjs(state, clientId)` joins from a state.
- `takeUpdates()` returns the local edits made since the last call, as Yjs updates.
- `applyUpdate(update)` merges a remote update.

Have the server seed the document before anyone connects. A canvist document always has at least one block, so a peer that joins an empty shared document writes one, and two peers doing that at once end up with two.

## Run a sync server

`receiveSync` speaks the y-websocket protocol. Hand it each binary message from a connection, send `reply` back to that connection, and send `broadcast` to everyone else in the room. This is a complete Deno server:

```ts no_run
import { createEngine, type Doc, type StarterKit } from "@canvist/headless";

const engine = await createEngine();
const rooms = new Map<
	string,
	{ doc: Doc<StarterKit>; sockets: Set<WebSocket> }
>();

function room(name: string) {
	let found = rooms.get(name);
	if (found === undefined) {
		const doc = engine.parse("markdown", "# New document");
		doc.share(1);
		found = { doc, sockets: new Set() };
		rooms.set(name, found);
	}
	return found;
}

Deno.serve({ port: 8787 }, (request) => {
	const { socket, response } = Deno.upgradeWebSocket(request);
	const { doc, sockets } = room(new URL(request.url).pathname.slice(1));
	socket.binaryType = "arraybuffer";

	socket.onopen = () => {
		sockets.add(socket);
		socket.send(doc.syncStart());
	};
	socket.onmessage = (event) => {
		const { reply, broadcast } = doc.receiveSync(new Uint8Array(event.data));
		reply.forEach((message) => socket.send(message));
		for (const other of sockets) {
			if (other !== socket) broadcast.forEach((message) => other.send(message));
		}
	};
	socket.onclose = () => sockets.delete(socket);
	return response;
});
```

The server holds a real canvist document, so it can read, export, or transform the room at any time. After it runs commands itself, it sends the changes to everyone with `doc.flushSync()`.

The Rust (`receive_sync`) and Dart (`receiveSync`) packages have the same calls. The repository has complete servers for [TypeScript](https://github.com/canvist/canvist/tree/main/packages/canvist-headless/examples/server) and [Dart](https://github.com/canvist/canvist/blob/main/platforms/dart/example/server.dart), both tested against real Yjs clients.

## Connect clients

Any y-websocket client can join a room, because it's the standard protocol:

```ts no_run
import { WebsocketProvider } from "y-websocket";
import * as Y from "yjs";

const ydoc = new Y.Doc();
const provider = new WebsocketProvider(
	"ws://localhost:8787",
	"team-notes",
	ydoc,
);
provider.on("sync", (synced: boolean) => console.log("synced:", synced));
```

canvist's browser editor joins the same rooms with [`@canvist/web/collab`](#put-editors-in-a-room), as a live editor instead of a bare `Y.Doc`.

## Check what clients send

A Yjs client can write anything into a shared document, including blocks your schema doesn't have. canvist hides content it doesn't understand instead of deleting it, and `validate()` tells you about it:

```ts
import { createEngine } from "@canvist/headless";

const engine = await createEngine();
const doc = engine.parse("markdown", "# Shared");
doc.share(1);

console.log(doc.validate());
// { rejectedBlocks: 0, waiting: false }
```

- `rejectedBlocks` counts shared blocks the schema rejected. They stay in the Yjs state but not in the document.
- `waiting` is `true` while updates wait for earlier updates that haven't arrived.

## Use canvist Cloud Sync

Don't want to run a server? [canvist Cloud Sync](../cloud/sync.md) is a hosted y-websocket and Hocuspocus relay with persistence, short-lived client tokens, presence, and webhooks. Clients connect with the same providers:

```ts no_run
import { WebsocketProvider } from "y-websocket";
import * as Y from "yjs";

// Your project's sync host, from canvist Cloud.
const SYNC_HOST = "sync.example.com";
// Your server mints a short-lived token with your API key.
const token = await fetch("/api/canvist-token").then((response) =>
	response.text()
);

const ydoc = new Y.Doc();
new WebsocketProvider(`wss://${SYNC_HOST}/v1/sync`, "team/notes", ydoc, {
	params: { token },
});
```

## Things to know

- **Splitting a block while someone edits its end.** If one person presses Enter in a paragraph while another edits the text after the caret, the inserted text survives but moves to the end of the first half, and deletions and formatting there are lost. Every Yjs-based editor shares this.
- **Moving blocks.** Yjs has no move operation, so moving a block deletes and re-inserts it. Text typed into the block at the same moment is lost.
- **Undo is per person.** Each peer's undo only undoes its own changes.

## Next

- [canvist Cloud Sync](../cloud/sync.md): hosted sync, tokens, and presence.
- [Server-side transforms](./server-transforms.md): edit shared documents from your server.
