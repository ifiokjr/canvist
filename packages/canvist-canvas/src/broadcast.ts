/**
 * Multiplayer between tabs of one browser, over a `BroadcastChannel`.
 *
 * @module
 */
import type { SyncMessage, Transport } from "./transport.ts";

/** Whether `value` is a {@link SyncMessage}. */
function isSyncMessage(value: unknown): value is SyncMessage {
	if (typeof value !== "object" || value === null) return false;
	const message = value as Record<string, unknown>;

	if (typeof message.from !== "number") return false;

	switch (message.type) {
		case "hello":
		case "welcome":
			return message.stateVector instanceof Uint8Array;
		case "update":
			return message.update instanceof Uint8Array;
		default:
			return false;
	}
}

/** Syncs every tab of this origin that joins room `name`. */
export class BroadcastTransport implements Transport {
	readonly #channel: BroadcastChannel;

	constructor(name: string) {
		this.#channel = new BroadcastChannel(`canvist-canvas:${name}`);
	}

	connect(_client: number, receive: (message: SyncMessage) => void): void {
		this.#channel.onmessage = (event: MessageEvent<unknown>) => {
			if (isSyncMessage(event.data)) receive(event.data);
		};
	}

	send(message: SyncMessage): void {
		this.#channel.postMessage(message);
	}

	close(): void {
		this.#channel.close();
	}
}
