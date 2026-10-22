/**
 * The typed engine and document, over either WebAssembly module: the
 * StarterKit one `@canvist/headless` loads, or the one with every extension
 * `@canvist/headless/full` loads. Both export the same classes.
 *
 * @module
 */
import type { WasmDoc, WasmEngine } from "../wasm/canvist_headless_wasm.js";
import { calls, encode } from "./calls.ts";
export { type CanvistError, isCanvistError } from "./errors.ts";
import { core } from "./extensions.generated.ts";
import type { WasmSource } from "./load.ts";
import type {
	CommandName,
	CommandParams,
	Commands,
	DocJson,
	ExtensionDescriptor,
	Extensions,
	Format,
	Position,
	Queries,
	QueryName,
	QueryParams,
	QueryResult,
	Range,
	Registered,
	StarterKit,
} from "./types.ts";

/** Options for `createEngine`. */
export interface EngineOptions {
	/** The client block ids are allocated under; collaborating peers need distinct ones. Default 1. */
	readonly client?: number;
	/** Where to load the WebAssembly module from, for runtimes without a file system. */
	readonly wasm?: WasmSource;
}

/** What {@link Doc.receiveSync} says to send. */
export interface SyncReply {
	/** Messages for the connection the message came from. */
	readonly reply: readonly Uint8Array[];
	/** Messages for every other connection to the document. */
	readonly broadcast: readonly Uint8Array[];
	/** Whether the document changed. */
	readonly changed: boolean;
}

/** How well a shared document's state fits the schema. */
export interface Validation {
	/** Shared items the schema rejects; they are hidden from the document but kept. */
	readonly rejectedBlocks: number;
	/** Whether updates wait for earlier updates that have not arrived. */
	readonly waiting: boolean;
}

/** A set of extensions that creates documents. Create one with `createEngine`. */
export class Engine<E extends Extensions> {
	readonly #wasm: WasmEngine;
	readonly #extensions: Registered<E>;

	/** @internal Use `createEngine`, which loads the WebAssembly module first. */
	constructor(wasm: WasmEngine, extensions: E) {
		this.#extensions = [...core, ...extensions];
		this.#wasm = wasm;
	}

	/** The registered extensions, including the ones every engine has. */
	get extensions(): Registered<E> {
		return this.#extensions;
	}

	/**
	 * The registered extensions' metadata as Rust reports it: their
	 * descriptors, with every command, query, and field's `doc`.
	 */
	metadata(): ExtensionDescriptor[] {
		return JSON.parse(this.#wasm.metadata());
	}

	/** A document with one empty paragraph. */
	create(): Doc<E> {
		return new Doc(this.#wasm.create(), this.#extensions);
	}

	/** Read `input` as `format`. */
	parse(format: Format, input: string): Doc<E> {
		return new Doc(this.#wasm.parse(format, input), this.#extensions);
	}

	/** Read a ProseMirror or TipTap document that uses only the registered extensions. */
	fromJson(json: DocJson<Registered<E>>): Doc<E> {
		return this.parse("json", JSON.stringify(json));
	}

	/** Join a shared document from v1 Yjs state as yrs client `clientId`. */
	fromYjs(state: Uint8Array, clientId: number): Doc<E> {
		return new Doc(this.#wasm.fromYjs(state, clientId), this.#extensions);
	}

	/** Recreate a document from {@link Doc.snapshot} text. */
	restore(snapshot: string): Doc<E> {
		return new Doc(this.#wasm.restore(snapshot), this.#extensions);
	}

	/** Release the engine's memory now rather than when it is collected. */
	[Symbol.dispose](): void {
		this.#wasm.free();
	}
}

/** One document and the editor that changes it. Every call is one undo step. */
export class Doc<E extends Extensions> {
	readonly #wasm: WasmDoc;

	/** Run a registered command against the selection; returns whether it applied. */
	readonly commands: Commands<Registered<E>>;
	/** Whether a registered command would apply. Nothing changes. */
	readonly can: Commands<Registered<E>>;
	/** Answer a registered query. */
	readonly query: Queries<Registered<E>>;

	/** @internal Documents come from an {@link Engine}. */
	constructor(wasm: WasmDoc, extensions: Extensions) {
		this.#wasm = wasm;
		const commands = extensions.flatMap((extension) =>
			extension.commands.map((command) => command.name)
		);
		const queries = extensions.flatMap((extension) =>
			extension.queries.map((query) => query.name)
		);
		this.commands = calls(
			commands,
			(name, args) => this.#wasm.run(name, encode(args)),
		);
		this.can = calls(
			commands,
			(name, args) => this.#wasm.can(name, encode(args)),
		);
		this.query = calls(
			queries,
			(name, args) => JSON.parse(this.#wasm.query(name, encode(args))),
		);
	}

	/** Run the command named `name`, for callers that pick commands at runtime. */
	run<N extends CommandName<Registered<E>>>(
		name: N,
		...args: CommandParams<Registered<E>, N>
	): boolean {
		return this.#wasm.run(name, encode(args[0]));
	}

	/** Whether the command named `name` would apply. Nothing changes. */
	canRun<N extends CommandName<Registered<E>>>(
		name: N,
		...args: CommandParams<Registered<E>, N>
	): boolean {
		return this.#wasm.can(name, encode(args[0]));
	}

	/** Answer the query named `name`, for callers that pick queries at runtime. */
	ask<N extends QueryName<Registered<E>>>(
		name: N,
		...args: QueryParams<Registered<E>, N>
	): QueryResult<Registered<E>, N> {
		return JSON.parse(this.#wasm.query(name, encode(args[0])));
	}

	/** The document as `format`. */
	export(format: Format): string {
		return this.#wasm.export(format);
	}

	/** The document as ProseMirror JSON, with block ids in `attrs.id`. */
	toJson(): DocJson<Registered<E>> {
		return JSON.parse(this.#wasm.export("json"));
	}

	/** The text selection, or `null` for a block selection. */
	get selection(): Range | null {
		return JSON.parse(this.#wasm.selection());
	}

	/** Select from `anchor` to `head`, or place a caret at `anchor`. Offsets count UTF-16 code units. */
	select(anchor: Position, head: Position = anchor): void {
		this.#wasm.select(
			Uint32Array.from(anchor.path),
			anchor.offset,
			Uint32Array.from(head.path),
			head.offset,
		);
	}

	/** Select the whole document. */
	selectAll(): void {
		this.#wasm.selectAll();
	}

	/** Replace the selection with `text`, without input rules. */
	insertText(text: string): boolean {
		return this.#wasm.insertText(text);
	}

	/** Type `text` one character at a time, so input rules such as `## ` apply. */
	typeText(text: string): boolean {
		return this.#wasm.typeText(text);
	}

	/** Delete the selection. */
	deleteSelection(): boolean {
		return this.#wasm.deleteSelection();
	}

	/** Press a key binding such as `Enter`, `Tab`, or `Mod-b` (`Mod` is Ctrl). */
	press(key: string): boolean {
		return this.#wasm.press(key);
	}

	/** Paste `content` read as `format` over the selection. */
	paste(format: Format, content: string): boolean {
		return this.#wasm.paste(format, content);
	}

	/** Undo the last call that changed the document. */
	undo(): boolean {
		return this.#wasm.undo();
	}

	/** Redo the last undone call. */
	redo(): boolean {
		return this.#wasm.redo();
	}

	/** The content and selection as snapshot JSON text, for {@link Engine.restore}. */
	snapshot(): string {
		return this.#wasm.snapshot();
	}

	/** Start sharing the document as yrs client `clientId`. */
	share(clientId: number): void {
		this.#wasm.share(clientId);
	}

	/** Apply a v1 Yjs update from another peer; returns whether the document changed. */
	applyUpdate(update: Uint8Array): boolean {
		return this.#wasm.applyUpdate(update);
	}

	/** The whole shared state as one v1 update. */
	encodeState(): Uint8Array {
		return this.#wasm.encodeState();
	}

	/** The v1 state vector. */
	stateVector(): Uint8Array {
		return this.#wasm.stateVector();
	}

	/** Everything a peer at `stateVector` is missing, as one v1 update. */
	encodeDiff(stateVector: Uint8Array): Uint8Array {
		return this.#wasm.encodeDiff(stateVector);
	}

	/** Drain this peer's own v1 updates for broadcasting. */
	takeUpdates(): Uint8Array[] {
		return this.#wasm.takeUpdates();
	}

	/** Check the shared state against the schema. */
	validate(): Validation {
		return JSON.parse(this.#wasm.validate());
	}

	/** The y-sync message to greet a connecting client with. */
	syncStart(): Uint8Array {
		return this.#wasm.syncStart();
	}

	/** Handle one y-sync message from a client connection. */
	receiveSync(message: Uint8Array): SyncReply {
		const [reply, broadcast, changed] = this.#wasm.receiveSync(message);
		return { reply, broadcast, changed };
	}

	/** This peer's changes since the last call, as y-sync messages for every connection. */
	flushSync(): Uint8Array[] {
		return this.#wasm.flushSync();
	}

	/** Release the document's memory now rather than when it is collected. */
	[Symbol.dispose](): void {
		this.#wasm.free();
	}
}
