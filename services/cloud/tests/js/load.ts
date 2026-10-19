/**
 * A load test for a deployed canvist Cloud.
 *
 *   CANVIST_ADMIN_TOKEN=… deno run --allow-net --allow-env load.ts \
 *     https://sync.example.com [rooms=20] [peers=5] [seconds=30] [keys-per-second=5]
 *
 * Every peer is a stock y-websocket client in this one process, so one
 * clock times every edit: a peer types into its own block (with its caret
 * in awareness, as editors publish it), and every other peer in the room
 * records how long each keystroke took to arrive. Prints latency
 * percentiles, delivered keystrokes, and the bytes each peer received.
 */

import { WebsocketProvider } from "npm:y-websocket@3.0.0";
import * as Y from "npm:yjs@13.6.33";

const [
	base,
	roomsArg = "20",
	peersArg = "5",
	secondsArg = "30",
	rateArg = "5",
] = Deno.args;
const admin = Deno.env.get("CANVIST_ADMIN_TOKEN");

if (!base || !admin) {
	console.error(
		"usage: CANVIST_ADMIN_TOKEN=… load.ts <sync url> [rooms] [peers] [seconds] [keys/s]",
	);
	Deno.exit(2);
}

const rooms = Number(roomsArg);
const peersPerRoom = Number(peersArg);
const seconds = Number(secondsArg);
const rate = Number(rateArg);
const wsBase = base.replace(/^http/, "ws");

async function post(path: string, body: unknown, bearer: string) {
	const response = await fetch(`${base}${path}`, {
		method: "POST",
		headers: {
			authorization: `Bearer ${bearer}`,
			"content-type": "application/json",
		},
		body: JSON.stringify(body),
	});
	if (!response.ok) {
		throw new Error(`${path}: ${response.status} ${await response.text()}`);
	}
	return await response.json();
}

const project = await post("/v1/admin/projects", {
	name: `load-${Date.now()}`,
	environments: ["production"],
}, admin);
const key = project.keys.production as string;
await post(
	`/v1/admin/projects/${project.project}/terms`,
	{ plan: "scale" },
	admin,
).catch(() => {});
const { token } = await post("/v1/tokens", {
	subject: "load",
	document: "load/*",
	ttl_seconds: 3600,
}, key);

/** When each keystroke `peer:seq` was typed. */
const sentAt = new Map<string, number>();
const latencies: number[] = [];
let receivedBytes = 0;

type Peer = {
	id: string;
	doc: Y.Doc;
	text: Y.Text;
	provider: WebsocketProvider;
	seq: number;
};

function connect(room: number, index: number): Peer {
	const doc = new Y.Doc();
	const id = `r${room}p${index}`;
	const provider = new WebsocketProvider(
		`${wsBase}/v1/sync`,
		`load/room-${room}`,
		doc,
		{
			params: { token },
			// Same-process clients would otherwise sync over BroadcastChannel and
			// bypass the server.
			disableBc: true,
		},
	);
	provider.awareness.setLocalStateField("user", { name: id, color: "#f97316" });
	provider.on("status", () => {
		const socket = (provider as unknown as { ws: WebSocket | null }).ws;
		socket?.addEventListener("message", (event: MessageEvent) => {
			receivedBytes += (event.data as ArrayBuffer).byteLength ?? 0;
		});
	});
	const blocks = doc.getArray<Y.Map<unknown>>("blocks");
	const text = new Y.Text();

	// Only what each remote change inserted is scanned, so measuring stays
	// cheap however long the texts grow.
	blocks.observeDeep((events, transaction) => {
		if (transaction.local) return;
		const now = performance.now();

		for (const event of events) {
			if (!(event.target instanceof Y.Text)) continue;
			for (const change of event.delta) {
				if (typeof change.insert !== "string") continue;
				for (const mark of change.insert.matchAll(/(r\d+p\d+):(\d+);/g)) {
					const sent = sentAt.get(`${mark[1]}:${mark[2]}`);
					if (sent !== undefined) latencies.push(now - sent);
				}
			}
		}
	});

	return { id, doc, text, provider, seq: 0 };
}

const peers: Peer[] = [];

for (let room = 0; room < rooms; room++) {
	for (let index = 0; index < peersPerRoom; index++) {
		peers.push(connect(room, index));
	}
}

await new Promise((resolve) => setTimeout(resolve, 5000));
const synced = peers.filter((peer) => peer.provider.synced).length;
console.log(`${synced}/${peers.length} peers synced across ${rooms} rooms`);

// Each peer owns one block.
for (const peer of peers) {
	peer.doc.transact(() => {
		const block = new Y.Map<unknown>();
		block.set("id", `${(peer.doc.clientID % 1_000_000).toString(36)}.1`);
		block.set("type", "paragraph");
		block.set("attrs", new Y.Map());
		block.set("text", peer.text);
		peer.doc.getArray("blocks").push([block]);
	});
}

await new Promise((resolve) => setTimeout(resolve, 2000));
latencies.length = 0;
receivedBytes = 0;
const started = performance.now();
const deadline = started + seconds * 1000;

function type(peer: Peer) {
	if (performance.now() > deadline) return;
	peer.seq += 1;
	const mark = `${peer.id}:${peer.seq};`;
	sentAt.set(`${peer.id}:${peer.seq}`, performance.now());
	peer.text.insert(peer.text.length, mark);
	peer.provider.awareness.setLocalStateField("cursor", {
		anchor: peer.text.length,
		head: peer.text.length,
	});
	setTimeout(() => type(peer), 1000 / rate);
}

peers.forEach((peer, index) =>
	setTimeout(() => type(peer), (index * 1000) / rate / peers.length)
);
await new Promise((resolve) => setTimeout(resolve, seconds * 1000 + 3000));

latencies.sort((a, b) => a - b);
const percentile = (p: number) =>
	latencies[
		Math.min(latencies.length - 1, Math.floor((latencies.length * p) / 100))
	]?.toFixed(1);
const typed = peers.reduce((sum, peer) => sum + peer.seq, 0);
const expected = typed * (peersPerRoom - 1);

console.log(JSON.stringify(
	{
		rooms,
		peers: peers.length,
		seconds,
		keystrokes: typed,
		deliveries: `${latencies.length}/${expected}`,
		latency_ms: {
			p50: percentile(50),
			p90: percentile(90),
			p99: percentile(99),
			max: latencies.at(-1)?.toFixed(1),
		},
		received_kib_per_peer_per_minute:
			(receivedBytes / 1024 / peers.length / (seconds / 60)).toFixed(1),
	},
	null,
	2,
));

for (const peer of peers) peer.provider.destroy();
Deno.exit(0);
