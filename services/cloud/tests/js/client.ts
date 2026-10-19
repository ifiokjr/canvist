/**
 * A stock JavaScript Yjs client for the relay's integration tests.
 *
 * It connects with the real `y-websocket` `WebsocketProvider` or the real
 * `@hocuspocus/provider` `HocuspocusProvider`, edits a canvist document by
 * the Yjs schema in the `canvist_collab` crate docs, and reports what it
 * sees. The Rust test drives it over stdin and stdout, one JSON message per
 * line, and every message is acknowledged with `{ "done": true }`:
 *
 * - `{ "connect": { "transport", "url", "document", "token", "timeout_ms" } }`
 *   connects and waits up to `timeout_ms` (default five seconds) for the
 *   first sync; replies
 *   `{ "synced": bool, "denied": string | null }`.
 * - `{ "append": "text" }` appends a paragraph block holding `text`.
 * - `{ "presence": "name" }` publishes a user in awareness.
 * - `{ "wait": { "texts": [...], "peers": n } }` waits up to ten seconds
 *   for the document to hold exactly those paragraph texts and for `n`
 *   other peers to be present; replies `{ "texts", "peers" }`.
 * - `{ "wait_synced": true }` waits up to ten seconds for a sync; replies
 *   `{ "synced": bool }`.
 * - `{ "disconnect": true }` closes the connection.
 */

import { HocuspocusProvider } from "npm:@hocuspocus/provider@4.7.0";
import { WebsocketProvider } from "npm:y-websocket@3.0.0";
import * as Y from "npm:yjs@13.6.33";

type Message = Record<string, unknown>;
type Awareness = {
	clientID: number;
	getStates(): Map<number, Record<string, unknown>>;
	setLocalStateField(field: string, value: unknown): void;
};
type Provider = {
	awareness: Awareness | null;
	destroy(): void;
};

const encoder = new TextEncoder();
const doc = new Y.Doc();
const blocks = doc.getArray<Y.Map<unknown>>("blocks");
let provider: Provider | null = null;
let denied: string | null = null;
let synced = false;
let blockCounter = 0;

function send(message: unknown) {
	Deno.stdout.writeSync(encoder.encode(`${JSON.stringify(message)}\n`));
}

async function* messages(): AsyncGenerator<Message> {
	const decoder = new TextDecoder();
	let buffered = "";

	for await (const chunk of Deno.stdin.readable) {
		buffered += decoder.decode(chunk, { stream: true });
		let newline;

		while ((newline = buffered.indexOf("\n")) >= 0) {
			const line = buffered.slice(0, newline);
			buffered = buffered.slice(newline + 1);
			yield JSON.parse(line);
		}
	}
}

function until(check: () => boolean, timeoutMs: number): Promise<boolean> {
	return new Promise((resolve) => {
		const started = Date.now();
		const poll = () => {
			if (check()) return resolve(true);
			if (Date.now() - started > timeoutMs) return resolve(false);
			setTimeout(poll, 10);
		};
		poll();
	});
}

function texts(): string[] {
	return blocks.toArray().map((block) => String(block.get("text") ?? ""));
}

function peers(): number {
	const awareness = provider?.awareness;
	if (!awareness) return 0;

	return [...awareness.getStates().entries()].filter(
		([client, state]) => client !== awareness.clientID && state.user,
	).length;
}

async function connect(options: Record<string, string>): Promise<Message> {
	synced = false;

	if (options.transport === "hocuspocus") {
		const hocuspocus = new HocuspocusProvider({
			url: options.url,
			name: options.document,
			document: doc,
			token: options.token,
			onSynced: () => {
				synced = true;
			},
			onAuthenticationFailed: ({ reason }: { reason: string }) => {
				denied = reason;
			},
		});
		provider = hocuspocus as unknown as Provider;
	} else {
		const websocket = new WebsocketProvider(
			options.url,
			options.document,
			doc,
			{
				params: { token: options.token },
			},
		);
		websocket.on("sync", (isSynced: boolean) => {
			synced = synced || isSynced;
		});
		provider = websocket as unknown as Provider;
	}

	await until(
		() => synced || denied !== null,
		Number(options.timeout_ms ?? 5000),
	);

	return { synced, denied };
}

function append(text: string) {
	doc.transact(() => {
		const block = new Y.Map<unknown>();
		blockCounter += 1;
		block.set("id", `js${doc.clientID % 1000}.${blockCounter}`);
		block.set("type", "paragraph");
		block.set("attrs", new Y.Map());
		block.set("text", new Y.Text(text));
		blocks.push([block]);
	});
}

async function handle(message: Message): Promise<Message | null> {
	if (message.connect) {
		return await connect(message.connect as Record<string, string>);
	}

	if (typeof message.append === "string") {
		append(message.append);
	} else if (typeof message.presence === "string") {
		provider?.awareness?.setLocalStateField("user", {
			name: message.presence,
			color: "#f97316",
		});
	} else if (message.wait) {
		const wanted = message.wait as { texts?: string[]; peers?: number };
		const expected = JSON.stringify(wanted.texts ?? texts());
		await until(
			() =>
				JSON.stringify(texts()) === expected &&
				peers() === (wanted.peers ?? peers()),
			10000,
		);
		return { texts: texts(), peers: peers() };
	} else if (message.wait_synced) {
		await until(() => synced, 10000);
		return { synced };
	} else if (message.disconnect) {
		provider?.destroy();
		provider = null;
	}

	return null;
}

for await (const message of messages()) {
	const reply = await handle(message);
	if (reply) send(reply);
	send({ done: true });
}

Deno.exit(0);
