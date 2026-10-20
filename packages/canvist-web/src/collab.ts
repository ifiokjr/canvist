/**
 * Collaborative editing over y-websocket, out of the box.
 *
 * `createEditor` here makes an editor from a build of canvist with Yjs
 * inside (a second WebAssembly module, so editors that don't collaborate
 * never download it), and `connect` joins it to a room on any server that
 * speaks y-websocket's protocol: y-websocket's own server, canvist's
 * `receiveSync` servers, or canvist Cloud Sync. The editor is the Yjs peer
 * itself, so there is no `Y.Doc` to keep in step, and JavaScript Yjs peers
 * in the same room see the same document.
 *
 * ```ts
 * import { connect, createEditor } from "@canvist/web/collab";
 *
 * const editor = await createEditor(document.querySelector("#editor")!);
 * const connection = connect(editor, {
 * 	url: "wss://sync.example.com",
 * 	room: "team-notes",
 * });
 * await connection.whenSynced();
 * ```
 *
 * @module
 */
import type { Extensions, StarterKit } from "@canvist/headless";
import init, { WebEditor } from "../wasm-collab/canvist_web.js";
import { Editor, type EditorOptions } from "./editor.ts";
import type { WasmSource } from "./load.ts";
import type { Plugins } from "./plugin.ts";

/** The longest wait between reconnection attempts, in milliseconds. */
const MAX_BACKOFF_MS = 10_000;
/** The first wait before reconnecting, in milliseconds; it doubles each time. */
const FIRST_BACKOFF_MS = 250;

/** The collaborative build of each editor `createEditor` here made. */
const builds = new WeakMap<object, WebEditor>();

let loading: Promise<void> | undefined;

/**
 * Load the collaborative WebAssembly module once. Without a source it is
 * fetched from next to this file, which bundlers such as Vite follow.
 */
export function loadCollab(source?: WasmSource): Promise<void> {
	loading ??= init({
		module_or_path: source ??
			new URL("../wasm-collab/canvist_web_bg.wasm", import.meta.url),
	}).then(() => undefined);

	return loading;
}

/**
 * Create an editor that can collaborate: `@canvist/web`'s `createEditor`,
 * with Yjs inside. `options.wasm` is where the collaborative module comes
 * from.
 */
export async function createEditor<
	const E extends Extensions = StarterKit,
	const P extends Plugins = [],
>(
	element: HTMLElement,
	options: EditorOptions<E, P> = {},
): Promise<Editor<E, P>> {
	await loadCollab(options.wasm);
	let build: WebEditor | undefined;
	const editor = new Editor(element, options, (...args) => {
		build = new WebEditor(...args);
		return build;
	});

	if (build === undefined) {
		throw new Error("the editor did not make its WebAssembly half");
	}

	builds.set(editor, build);
	return editor;
}

/** What a connection needs from its editor. */
type Peer = Pick<Editor<StarterKit>, "documentChanged" | "subscribe">;

/** Where and how to connect. */
export interface ConnectOptions {
	/**
	 * The server's WebSocket URL, such as `wss://sync.example.com`. The room
	 * is added as a path segment, as y-websocket's `WebsocketProvider` does.
	 */
	readonly url: string;
	/** The room: every peer in it edits one document. */
	readonly room: string;
	/** Query parameters for the URL, such as a short-lived token. */
	readonly params?: Readonly<Record<string, string>>;
	/** This peer's Yjs client id, unique in the room. Random by default. */
	readonly clientId?: number;
	/** Reconnect, with backoff, when the connection drops. Default `true`. */
	readonly reconnect?: boolean;
	/** The `WebSocket` class to use. Default the global one. */
	readonly WebSocket?: typeof WebSocket;
}

/** Whether a {@link Connection} is open. */
export type ConnectionStatus = "connecting" | "connected" | "disconnected";

/**
 * Join `editor` to a room. Until the room's document arrives, the editor
 * keeps showing what it had; then the room's document replaces it (an
 * empty room gets one empty paragraph). Edits travel both ways from then
 * on, and undo reverts only this peer's changes.
 *
 * `editor` must come from this module's {@link createEditor}.
 */
export function connect<E extends Extensions, P extends Plugins>(
	editor: Editor<E, P>,
	options: ConnectOptions,
): Connection {
	const build = builds.get(editor);

	if (build === undefined) {
		throw new TypeError(
			"connect() needs an editor made by createEditor from @canvist/web/collab",
		);
	}

	return new Connection(editor, build, options);
}

/** A live link between an editor and a room. Create one with {@link connect}. */
export class Connection {
	readonly #editor: Peer;
	readonly #build: WebEditor;
	readonly #options: ConnectOptions;
	readonly #clientId: number;
	readonly #listeners = new Set<() => void>();
	readonly #unsubscribe: () => void;
	#socket: WebSocket | undefined;
	#status: ConnectionStatus = "connecting";
	#synced = false;
	#closed = false;
	#backoff = FIRST_BACKOFF_MS;
	#retry = 0;

	/** @internal Use {@link connect}. */
	constructor(
		editor: Peer,
		build: WebEditor,
		options: ConnectOptions,
	) {
		this.#editor = editor;
		this.#build = build;
		this.#options = options;
		this.#clientId = options.clientId ?? randomClientId();
		// Every local change goes out as soon as it is made.
		this.#unsubscribe = editor.subscribe(() => this.#flush());
		this.#open();
	}

	/** Whether the connection is open. */
	get status(): ConnectionStatus {
		return this.#status;
	}

	/** Whether the room's document has arrived. */
	get synced(): boolean {
		return this.#synced;
	}

	/** This peer's Yjs client id. */
	get clientId(): number {
		return this.#clientId;
	}

	/**
	 * Call `listener` whenever {@link Connection.status} or
	 * {@link Connection.synced} changes. Returns a function that
	 * unsubscribes.
	 */
	subscribe(listener: () => void): () => void {
		this.#listeners.add(listener);

		return () => this.#listeners.delete(listener);
	}

	/** Resolve once the room's document has arrived. */
	whenSynced(): Promise<void> {
		if (this.#synced) {
			return Promise.resolve();
		}

		return new Promise((resolve) => {
			const unsubscribe = this.subscribe(() => {
				if (this.#synced) {
					unsubscribe();
					resolve();
				}
			});
		});
	}

	/**
	 * Close the connection for good. The editor keeps the shared document;
	 * {@link connect} it again to carry on, and it catches up with what it
	 * missed.
	 */
	disconnect(): void {
		this.#closed = true;
		clearTimeout(this.#retry);
		this.#unsubscribe();
		this.#socket?.close();
		this.#setStatus("disconnected");
	}

	[Symbol.dispose](): void {
		this.disconnect();
	}

	#open(): void {
		const Socket = this.#options.WebSocket ?? WebSocket;
		const socket = new Socket(roomUrl(this.#options));
		socket.binaryType = "arraybuffer";
		this.#socket = socket;
		this.#setStatus("connecting");

		socket.addEventListener("open", () => {
			this.#backoff = FIRST_BACKOFF_MS;
			this.#setStatus("connected");
			// A shared document asks for what it missed and sends what it
			// changed offline; a new one asks for the whole document.
			this.#send(
				this.#build.isShared()
					? this.#build.syncStart()
					: this.#build.join(this.#clientId),
			);
			this.#flush();
		});
		socket.addEventListener("message", (event: MessageEvent<unknown>) => {
			if (event.data instanceof ArrayBuffer) {
				this.#receive(new Uint8Array(event.data));
			}
		});
		socket.addEventListener("close", () => this.#closedBy(socket));
	}

	#receive(message: Uint8Array): void {
		const [replies, changed] = this.#build.receiveSync(message);

		for (const reply of replies) {
			this.#send(reply);
		}

		if (changed) {
			this.#editor.documentChanged();
		}

		if (!this.#synced && this.#build.isShared()) {
			this.#synced = true;
			// Joining an empty room wrote its first paragraph.
			this.#flush();
			this.#notify();
		}
	}

	#flush(): void {
		if (this.#status !== "connected") {
			return;
		}

		for (const message of this.#build.flushSync()) {
			this.#send(message);
		}
	}

	#send(message: Uint8Array): void {
		if (this.#socket?.readyState === WebSocket.OPEN) {
			// A copy the socket owns: the view's buffer type is too wide to send.
			this.#socket.send(new Uint8Array(message));
		}
	}

	#closedBy(socket: WebSocket): void {
		if (socket !== this.#socket || this.#closed) {
			return;
		}

		this.#setStatus("disconnected");

		if (this.#options.reconnect ?? true) {
			this.#retry = window.setTimeout(() => this.#open(), this.#backoff);
			this.#backoff = Math.min(this.#backoff * 2, MAX_BACKOFF_MS);
		}
	}

	#setStatus(status: ConnectionStatus): void {
		if (status !== this.#status) {
			this.#status = status;
			this.#notify();
		}
	}

	#notify(): void {
		for (const listener of this.#listeners) {
			listener();
		}
	}
}

/** The room's URL, as y-websocket's `WebsocketProvider` builds it. */
function roomUrl(options: ConnectOptions): string {
	const base = options.url.replace(/\/+$/, "");
	const query = new URLSearchParams(options.params ?? {}).toString();

	return `${base}/${options.room}${query === "" ? "" : `?${query}`}`;
}

/** A random 32-bit Yjs client id, as Yjs picks its own. */
function randomClientId(): number {
	return crypto.getRandomValues(new Uint32Array(1))[0] ?? 0;
}
