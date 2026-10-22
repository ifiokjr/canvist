/**
 * The testkit page: a real `@canvist/web` editor that the Rust testkit
 * drives through Playwright, with real keyboard events.
 *
 * `?extensions=bold,heading` picks the extensions by name (the StarterKit
 * when absent), and `?client=2` the block-id client. `window.testkit`
 * answers the ops the Rust side sends:
 *
 * ```ts
 * window.testkit.op("insertText", "hi"); // { ok: true, value: true }
 * ```
 *
 * Typing and key presses do NOT go through `op`: the Rust side sends them
 * as real browser keyboard events, which the editor's own input layer
 * handles — that is the point of the dom target.
 */
import * as generated from "@canvist/headless/extensions";
import { starterKit } from "@canvist/headless/extensions";
import { createEditor, detectPlatform } from "../../src/mod.ts";

/** A named extension, in the StarterKit's own shape. */
type NamedExtension = (typeof starterKit)[number];

/** Every named extension the bindings ship, keyed by its registry name. */
const byName = new Map<string, NamedExtension>();
for (const extension of Object.values(generated)) {
	if (
		typeof extension === "object" && extension !== null && "name" in extension
	) {
		const record = extension as Record<string, unknown>;
		if (typeof record.name === "string") {
			byName.set(record.name, extension as unknown as NamedExtension);
		}
	}
}

const params = new URLSearchParams(location.search);
const names = (params.get("extensions") ?? "")
	.split(",")
	.map((name) => name.trim())
	.filter(Boolean);

const extensions: NamedExtension[] = names.length > 0 &&
		names.every((name) => byName.has(name))
	? names.map((name) => byName.get(name)!)
	: [...starterKit];

const editor = await createEditor(
	document.querySelector<HTMLElement>("#host")!,
	{
		extensions,
		client: Number(params.get("client") ?? 1),
		content: { format: "text", input: "" },
		label: "Test editor",
	},
);
await editor.ready();

/**
 * The editor as the bridge needs it. The typed API derives command names
 * from the extension set at compile time; the bridge dispatches runtime
 * strings from the wire, so it takes the same methods with `string` names.
 */
interface Bridged {
	setContent(
		content: { format: "json" | "html" | "markdown" | "text"; input: string },
	): void;
	insertText(text: string): boolean;
	select(
		anchor: { path: number[]; offset: number },
		head?: { path: number[]; offset: number },
	): void;
	run(name: string, args?: unknown): boolean;
	ask(name: string, args?: unknown): unknown;
	undo(): boolean;
	redo(): boolean;
	export(format: "json" | "html" | "markdown" | "text"): string;
	toJson(): unknown;
	readonly selection: {
		anchor: { path: number[]; offset: number };
		head: { path: number[]; offset: number };
	} | null;
	readonly focused: boolean;
	focus(): void;
}

const bridge = editor as unknown as Bridged;

/** One op's answer. */
interface Answer {
	ok: boolean;
	value?: unknown;
	error?: string;
}

/** Run one op against the editor. Keyboard input bypasses this on purpose. */
function op(name: string, arg: unknown): Answer {
	try {
		const value = run(name, arg);
		return { ok: true, value };
	} catch (error) {
		return {
			ok: false,
			error: error instanceof Error ? error.message : String(error),
		};
	}
}

function run(name: string, arg: unknown): unknown {
	switch (name) {
		case "load": {
			const load = arg as {
				format: "json" | "html" | "markdown" | "text";
				content: string;
			};
			bridge.setContent({ format: load.format, input: load.content });
			return null;
		}
		case "insertText":
			return bridge.insertText(arg as string);
		case "select": {
			const range = arg as {
				anchor: { path: number[]; offset: number };
				head?: { path: number[]; offset: number };
			};
			bridge.select(range.anchor, range.head ?? range.anchor);
			return null;
		}
		case "run": {
			const call = arg as { command: string; args?: unknown };
			return bridge.run(call.command, call.args);
		}
		case "can": {
			// The bridge answers `can` as "would not throw"; the harness
			// reads capability through `run` where it matters.
			const call = arg as { command: string; args?: unknown };
			try {
				return bridge.run(call.command, call.args) !== undefined;
			} catch {
				return false;
			}
		}
		case "query": {
			const call = arg as { query: string; args?: unknown };
			return bridge.ask(call.query, call.args);
		}
		case "undo":
			return bridge.undo();
		case "redo":
			return bridge.redo();
		case "export":
			return bridge.export(arg as "json" | "html" | "markdown" | "text");
		case "json":
			return JSON.stringify(bridge.toJson());
		case "selection":
			return bridge.selection;
		case "focus":
			// Only a first focus: refocusing moves the caret, which would
			// drop a selection a test just made with the keyboard.
			if (!bridge.focused) {
				bridge.focus();
			}
			return null;
		default:
			throw new Error(`unknown op ${name}`);
	}
}

Object.assign(globalThis, {
	testkit: {
		ready: true,
		/** What `Mod` means on this machine: "Meta" or "Control". */
		modifier: detectPlatform(navigator) === "apple" ? "Meta" : "Control",
		op,
	},
	editor,
});
