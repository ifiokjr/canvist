/**
 * The typed editor for JavaScript hosts that run canvist natively.
 *
 * `@canvist/react-native` and `@canvist/lynx` show the iOS and Android
 * `CanvistView`s, whose editors live in Rust, and reach each one through a
 * {@link NativeBridge}: a handful of synchronous calls that pass names and
 * JSON. {@link NativeEditor} puts the same typed API as `@canvist/web` and
 * `@canvist/headless` over it, so commands, queries, and documents are
 * typed from the extensions you register and calling one you did not
 * register is a type error.
 *
 * This module loads no WebAssembly: it is types, metadata, and a little
 * glue.
 *
 * @module
 */
import { calls, encode } from "./calls.ts";
import { core } from "./extensions.generated.ts";

export { type CanvistError, isCanvistError } from "./errors.ts";
import type { CanvistError } from "./errors.ts";
import type {
	CommandName,
	CommandParams,
	Commands,
	Content,
	DocJson,
	ErrorCode,
	Extensions,
	Format,
	Position,
	Queries,
	QueryName,
	QueryParams,
	QueryResult,
	Range,
	Registered,
} from "./types.ts";

/**
 * The calls a native editor answers, synchronously, on the JavaScript
 * thread. Arguments, answers, and selections are JSON text, as the Rust
 * engine reads and writes them.
 *
 * A call that fails throws an `Error` whose message contains
 * `[canvist:<code>] <reason>`, where `<code>` is an {@link ErrorCode};
 * {@link NativeEditor} rethrows it as a `CanvistError` with that code.
 */
export interface NativeBridge {
	/** Run the command `name` with `args` (a JSON object or `null`); whether it applied. */
	run(name: string, args: string): boolean;
	/** Whether the command `name` would apply. Nothing changes. */
	can(name: string, args: string): boolean;
	/** Answer the query `name`, as JSON. */
	query(name: string, args: string): string;
	/** Replace the document with `content` read as `format`. */
	load(format: Format, content: string): void;
	/** The document as `format`. */
	export(format: Format): string;
	/** The selection as a JSON {@link Range}, or `null` for a block selection. */
	selection(): string;
	/** Select the JSON {@link Range} `range`. */
	select(range: string): void;
	/** Select the whole document. */
	selectAll(): void;
	/** Type `text` over the selection, as the keyboard does; whether it changed anything. */
	insertText(text: string): boolean;
	/** Undo the last change; whether there was one. */
	undo(): boolean;
	/** Redo the last undone change; whether there was one. */
	redo(): boolean;
	/** Whether there is a change to undo. */
	canUndo(): boolean;
	/** Whether there is an undone change to redo. */
	canRedo(): boolean;
	/** Focus the view and show the keyboard. */
	focus(): void;
	/** Take focus away from the view and hide the keyboard. */
	blur(): void;
}

/**
 * A native editor with commands, queries, and document types that follow
 * its extensions. Hosts create it once their view is ready and report
 * changes with {@link NativeEditor.notifyChange} and
 * {@link NativeEditor.notifyFocus}; subscribers then hear of every edit,
 * whether it came from the keyboard or from JavaScript. Edits made from
 * JavaScript notify subscribers once the calls in the current task are
 * done, and again when the view reports them.
 */
export class NativeEditor<E extends Extensions> {
	/** Run a registered command against the selection; returns whether it applied. */
	readonly commands: Commands<Registered<E>>;
	/** Whether a registered command would apply. Nothing changes. */
	readonly can: Commands<Registered<E>>;
	/** Answer a registered query. */
	readonly query: Queries<Registered<E>>;

	readonly #bridge: NativeBridge;
	readonly #extensions: Registered<E>;
	readonly #subscribers = new Set<() => void>();
	#focused = false;
	/** Whether subscribers will hear of edits made from JavaScript. */
	#pending = false;

	/** An editor over `bridge`, whose view registered `extensions`. */
	constructor(bridge: NativeBridge, extensions: E) {
		this.#bridge = bridge;
		this.#extensions = [...core, ...extensions];
		const commands = this.#extensions.flatMap((extension) =>
			extension.commands.map((command) => command.name)
		);
		const queries = this.#extensions.flatMap((extension) =>
			extension.queries.map((query) => query.name)
		);
		this.commands = calls(
			commands,
			(name, args) => this.#edit(() => bridge.run(name, encode(args))),
		);
		this.can = calls(
			commands,
			(name, args) => this.#call(() => bridge.can(name, encode(args))),
		);
		this.query = calls(
			queries,
			(name, args) =>
				JSON.parse(this.#call(() => bridge.query(name, encode(args)))),
		);
	}

	/** The registered extensions, including the ones every editor has. */
	get extensions(): Registered<E> {
		return this.#extensions;
	}

	/** Run the command named `name`, for callers that pick commands at runtime. */
	run<N extends CommandName<Registered<E>>>(
		name: N,
		...args: CommandParams<Registered<E>, N>
	): boolean {
		return this.#edit(() => this.#bridge.run(name, encode(args[0])));
	}

	/** Whether the command named `name` would apply. Nothing changes. */
	canRun<N extends CommandName<Registered<E>>>(
		name: N,
		...args: CommandParams<Registered<E>, N>
	): boolean {
		return this.#call(() => this.#bridge.can(name, encode(args[0])));
	}

	/** Answer the query named `name`, for callers that pick queries at runtime. */
	ask<N extends QueryName<Registered<E>>>(
		name: N,
		...args: QueryParams<Registered<E>, N>
	): QueryResult<Registered<E>, N> {
		return JSON.parse(
			this.#call(() => this.#bridge.query(name, encode(args[0]))),
		);
	}

	/** Replace the document. */
	setContent(content: Content<E>): void {
		const [format, input] = "format" in content
			? [content.format, content.input]
			: ["json" as const, JSON.stringify(content)];
		this.#edit(() => this.#bridge.load(format, input));
	}

	/** The document as `format`. */
	export(format: Format): string {
		return this.#call(() => this.#bridge.export(format));
	}

	/** The document as ProseMirror JSON, with block ids in `attrs.id`. */
	toJson(): DocJson<Registered<E>> {
		return JSON.parse(this.#call(() => this.#bridge.export("json")));
	}

	/** The text selection, or `null` for a block selection. */
	get selection(): Range | null {
		return JSON.parse(this.#call(() => this.#bridge.selection()));
	}

	/** Select from `anchor` to `head`, or place a caret at `anchor`. Offsets count UTF-16 code units. */
	select(anchor: Position, head: Position = anchor): void {
		const range: Range = { anchor, head };
		this.#edit(() => this.#bridge.select(JSON.stringify(range)));
	}

	/** Select the whole document. */
	selectAll(): void {
		this.#edit(() => this.#bridge.selectAll());
	}

	/** Replace the selection with `text`, as typing does (input rules apply). */
	insertText(text: string): boolean {
		return this.#edit(() => this.#bridge.insertText(text));
	}

	/** Undo the last change. */
	undo(): boolean {
		return this.#edit(() => this.#bridge.undo());
	}

	/** Redo the last undone change. */
	redo(): boolean {
		return this.#edit(() => this.#bridge.redo());
	}

	/** Whether {@link NativeEditor.undo} has anything to undo. */
	canUndo(): boolean {
		return this.#call(() => this.#bridge.canUndo());
	}

	/** Whether {@link NativeEditor.redo} has anything to redo. */
	canRedo(): boolean {
		return this.#call(() => this.#bridge.canRedo());
	}

	/** Focus the editor and show the keyboard. */
	focus(): void {
		this.#call(() => this.#bridge.focus());
	}

	/** Take focus away from the editor and hide the keyboard. */
	blur(): void {
		this.#call(() => this.#bridge.blur());
	}

	/** Whether the editor has focus, as the view last reported. */
	get focused(): boolean {
		return this.#focused;
	}

	/**
	 * Call `listener` after every change to the document, the selection, or
	 * focus. Returns a function that stops it.
	 */
	subscribe(listener: () => void): () => void {
		this.#subscribers.add(listener);
		return () => {
			this.#subscribers.delete(listener);
		};
	}

	/** For hosts: the view reported that the document or selection changed. */
	notifyChange(): void {
		this.#notify();
	}

	/** For hosts: the view gained or lost focus. */
	notifyFocus(focused: boolean): void {
		if (focused === this.#focused) {
			return;
		}

		this.#focused = focused;
		this.#notify();
	}

	/**
	 * Make `call`, which may change the document or selection, and tell
	 * subscribers once the current task's calls are done: a handler that
	 * selects and then formats re-renders once. The view reports the change
	 * too, a moment later, but a command that only changes what typing will
	 * insert (a toggled mark at the caret) has nothing else to report.
	 */
	#edit<T>(call: () => T): T {
		const result = this.#call(call);

		if (!this.#pending) {
			this.#pending = true;
			// A microtask; Lynx's JavaScript engine has no `queueMicrotask`.
			void Promise.resolve().then(() => {
				this.#pending = false;
				this.#notify();
			});
		}

		return result;
	}

	/** Make `call` on the bridge, rethrowing its failures as `CanvistError`s. */
	#call<T>(call: () => T): T {
		try {
			return call();
		} catch (error) {
			throw nativeError(error);
		}
	}

	#notify(): void {
		for (const listener of this.#subscribers) {
			listener();
		}
	}
}

const NATIVE_ERROR = /\[canvist:(\w+)\] ([\s\S]*)$/;

/** Every error code, so a code from native text can be checked. */
const ERROR_CODES: Readonly<Record<ErrorCode, true>> = {
	config: true,
	parse: true,
	schema: true,
	unknownName: true,
	invalidArgs: true,
	invalidPosition: true,
	invalidKey: true,
	editor: true,
	snapshot: true,
	notShared: true,
	alreadyShared: true,
	collab: true,
	render: true,
};

function isErrorCode(code: string): code is ErrorCode {
	return Object.hasOwn(ERROR_CODES, code);
}

/** A failure on the native side, with the code its message carries. */
class NativeCanvistError extends Error implements CanvistError {
	override readonly name = "CanvistError";
	readonly code: ErrorCode;

	constructor(code: ErrorCode, reason: string, cause?: Error) {
		super(reason, cause === undefined ? {} : { cause });
		this.code = code;
	}
}

/**
 * `error` as a `CanvistError` when its message follows the bridge's
 * `[canvist:<code>] <reason>` contract, and unchanged otherwise.
 */
function nativeError(error: unknown): unknown {
	if (!(error instanceof Error)) {
		return error;
	}

	const [, code = "", reason = ""] = NATIVE_ERROR.exec(error.message) ?? [];
	return isErrorCode(code)
		? new NativeCanvistError(code, reason, error)
		: error;
}

/**
 * A `CanvistError` for a failure a native view reports outside a call,
 * such as an editor it could not create. A code canvist does not define
 * is reported as `editor`, with the code kept in the message.
 */
export function nativeCanvistError(code: string, reason: string): CanvistError {
	return isErrorCode(code)
		? new NativeCanvistError(code, reason)
		: new NativeCanvistError("editor", `${code}: ${reason}`);
}
