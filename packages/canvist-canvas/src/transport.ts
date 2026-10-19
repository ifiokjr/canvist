/**
 * How peers exchange the shared scene.
 *
 * Every peer starts from the same demo state, so syncing is a state-vector
 * handshake: a new peer says `hello` with its state vector, each existing
 * peer answers with what the newcomer lacks and a `welcome` carrying its
 * own state vector, and the newcomer answers that in turn. After that,
 * every local change goes out as an `update`.
 *
 * @module
 */
/** A message between peers. `from` is the sender's client id. */
export type SyncMessage =
	| { type: "hello"; from: number; stateVector: Uint8Array }
	| { type: "welcome"; from: number; stateVector: Uint8Array }
	| { type: "update"; from: number; update: Uint8Array };

/** Carries {@link SyncMessage}s between peers. */
export interface Transport {
	/** Start delivering other peers' messages to `receive`. */
	connect(client: number, receive: (message: SyncMessage) => void): void;
	/** Send `message` to every other peer. */
	send(message: SyncMessage): void;
	/** Stop sending and receiving. */
	close(): void;
}
