/**
 * Collaborative rooms: one shared canvist document per room, relaying
 * y-sync messages between the WebSocket connections in it. Plain Yjs
 * clients (y-websocket's `WebsocketProvider`) connect without knowing about
 * canvist, and the server keeps a schema-checked document it can read,
 * export, or transform at any time.
 *
 * Nothing here depends on a runtime: give it anything with a `send` method.
 *
 * @module
 */
import type { Doc, Engine, StarterKit } from "../../src/mod.ts";

/** One client connection. */
export interface Connection {
	send(message: Uint8Array): void;
}

/** The yrs client id the server writes its own changes as. */
const SERVER_CLIENT = 1;

class Room {
	readonly doc: Doc<StarterKit>;
	readonly #connections = new Set<Connection>();

	constructor(doc: Doc<StarterKit>) {
		this.doc = doc;
	}

	join(connection: Connection): void {
		this.#connections.add(connection);
		connection.send(this.doc.syncStart());
	}

	leave(connection: Connection): void {
		this.#connections.delete(connection);
	}

	receive(from: Connection, message: Uint8Array): void {
		const sync = this.doc.receiveSync(message);
		sync.reply.forEach((reply) => from.send(reply));

		for (const connection of this.#connections) {
			if (connection !== from) {
				sync.broadcast.forEach((update) => connection.send(update));
			}
		}
	}

	/** Send the server's own changes, made through commands, to everyone. */
	flush(): void {
		for (const message of this.doc.flushSync()) {
			this.#connections.forEach((connection) => connection.send(message));
		}
	}
}

/** Rooms by name, each created on first use from `initial` Markdown. */
export class Rooms {
	readonly #engine: Engine<StarterKit>;
	readonly #initial: string;
	readonly #rooms = new Map<string, Room>();

	constructor(engine: Engine<StarterKit>, initial = "") {
		this.#engine = engine;
		this.#initial = initial;
	}

	/** The room named `name`, seeded and shared by the server first. */
	get(name: string): Room {
		let room = this.#rooms.get(name);

		if (room === undefined) {
			const doc = this.#engine.parse("markdown", this.#initial);
			doc.share(SERVER_CLIENT);
			room = new Room(doc);
			this.#rooms.set(name, room);
		}

		return room;
	}
}
