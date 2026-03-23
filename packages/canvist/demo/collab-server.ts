#!/usr/bin/env -S deno run --allow-net --allow-env
/**
 * canvist collaboration relay server.
 *
 * A minimal WebSocket server that relays Yrs CRDT updates between peers.
 * Each "room" is an independent document. Peers connect to a room via
 * `ws://host:port/room/<room-id>`.
 *
 * Usage:
 *   deno run --allow-net demo/collab-server.ts
 *   # or with a custom port:
 *   PORT=9090 deno run --allow-net demo/collab-server.ts
 *
 * Protocol:
 *   - Client sends: JSON `{ type: "update", data: number[] }` (Yrs binary update as array)
 *   - Client sends: JSON `{ type: "cursor", offset: number, name: string, color: [r,g,b] }`
 *   - Server broadcasts to all other peers in the same room.
 *   - On connect, server sends `{ type: "peer_count", count: number }`.
 *   - On join/leave, server broadcasts updated peer count.
 */

interface Peer {
	socket: WebSocket;
	name: string;
	room: string;
}

const rooms = new Map<string, Set<Peer>>();
const port = parseInt(Deno.env.get("PORT") || "9001", 10);

function broadcast(room: string, message: string, exclude?: WebSocket) {
	const peers = rooms.get(room);
	if (!peers) return;
	for (const peer of peers) {
		if (peer.socket !== exclude && peer.socket.readyState === WebSocket.OPEN) {
			try {
				peer.socket.send(message);
			} catch {
				// Peer disconnected — will be cleaned up on close.
			}
		}
	}
}

function broadcastPeerCount(room: string) {
	const count = rooms.get(room)?.size ?? 0;
	broadcast(room, JSON.stringify({ type: "peer_count", count }));
}

function handleWebSocket(socket: WebSocket, room: string) {
	const peerId = `peer-${Math.random().toString(36).slice(2, 8)}`;
	const peer: Peer = { socket, name: peerId, room };

	// Add to room.
	if (!rooms.has(room)) {
		rooms.set(room, new Set());
	}
	rooms.get(room)!.add(peer);

	console.log(`[${room}] ${peerId} joined (${rooms.get(room)!.size} peers)`);

	// Send initial peer count.
	socket.send(JSON.stringify({ type: "peer_count", count: rooms.get(room)!.size }));
	broadcastPeerCount(room);

	socket.addEventListener("message", (event) => {
		const data = typeof event.data === "string" ? event.data : "";
		// Relay to all other peers in the same room.
		broadcast(room, data, socket);
	});

	socket.addEventListener("close", () => {
		rooms.get(room)?.delete(peer);
		console.log(`[${room}] ${peerId} left (${rooms.get(room)?.size ?? 0} peers)`);
		if (rooms.get(room)?.size === 0) {
			rooms.delete(room);
		} else {
			broadcastPeerCount(room);
		}
	});

	socket.addEventListener("error", (e) => {
		console.error(`[${room}] ${peerId} error:`, e);
	});
}

Deno.serve({ port }, (req) => {
	const url = new URL(req.url);

	// Health check.
	if (url.pathname === "/" || url.pathname === "/health") {
		const totalPeers = [...rooms.values()].reduce((sum, s) => sum + s.size, 0);
		return new Response(
			JSON.stringify({
				status: "ok",
				rooms: rooms.size,
				peers: totalPeers,
			}),
			{ headers: { "content-type": "application/json" } },
		);
	}

	// WebSocket upgrade for room connections.
	const roomMatch = url.pathname.match(/^\/room\/([a-zA-Z0-9_-]+)$/);
	if (roomMatch) {
		const room = roomMatch[1];
		const upgrade = req.headers.get("upgrade") || "";

		if (upgrade.toLowerCase() !== "websocket") {
			return new Response(`Room "${room}": connect via WebSocket`, { status: 400 });
		}

		const { socket, response } = Deno.upgradeWebSocket(req);
		handleWebSocket(socket, room);
		return response;
	}

	return new Response("Not Found", { status: 404 });
});

console.log(`canvist collab server listening on ws://localhost:${port}`);
console.log(`Connect to a room: ws://localhost:${port}/room/<room-id>`);
