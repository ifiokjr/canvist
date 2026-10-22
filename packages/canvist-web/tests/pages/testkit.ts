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
import { core, starterKit } from "@canvist/headless/extensions";
import type { Editor } from "../../src/editor.ts";
import { createEditor, detectPlatform } from "../../src/mod.ts";

/** Every named extension the bindings ship, keyed by its registry name. */
const byName = new Map<string, (typeof generated)[keyof typeof generated]>();
for (const extension of Object.values(generated)) {
	if (
		typeof extension === "object" && extension !== null && "name" in extension
	) {
		const record = extension as Record<string, unknown>;
		if (typeof record.name === "string") {
			byName.set(
				record.name,
				extension as (typeof generated)[keyof typeof generated],
			);
		}
	}
}

const params = new URLSearchParams(location.search);
const names = (params.get("extensions") ?? "")
	.split(",")
	.map((name) => name.trim())
	.filter(Boolean);

const missing = names.filter((name) => !byName.has(name));
const extensions = missing.length === 0 && names.length > 0
	? names.map((name) => byName.get(name)!)
	: (starterKit as unknown as typeof extensions);

const editor: Editor<typeof extensions, []> = await createEditor(
	document.querySelector<HTMLElement>("#host")!,
	{
		extensions,
		client: Number(params.get("client") ?? 1),
		content: { format: "text", input: "" },
		label: "Test editor",
	},
);
await editor.ready();

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
			editor.setContent({ format: load.format, input: load.content });
			return null;
		}
		case "insertText":
			return editor.insertText(arg as string);
		case "select": {
			const range = arg as {
				anchor: { path: number[]; offset: number };
				head?: { path: number[]; offset: number };
			};
			editor.select(range.anchor, range.head ?? range.anchor);
			return null;
		}
		case "run": {
			const call = arg as { command: string; args?: unknown };
			return editor.run(call.command as never, call.args as never);
		}
		case "query": {
			const call = arg as { query: string; args?: unknown };
			return editor.ask(call.query as never, call.args as never);
		}
		case "can": {
			// `can` reaches the wasm the typed commands reach; unknown names
			// report false rather than throw.
			const call = arg as { command: string; args?: unknown };
			try {
				return editor.run(call.command as never, call.args as never) !==
					undefined;
			} catch {
				return false;
			}
		}
		case "undo":
			return editor.undo();
		case "redo":
			return editor.redo();
		case "export":
			return editor.export(arg as "json" | "html" | "markdown" | "text");
		case "json":
			return JSON.stringify(editor.toJson());
		case "selection":
			return editor.selection;
		case "focus":
			// Only a first focus: refocusing moves the caret, which would
			// drop a selection a test just made with the keyboard.
			if (!editor.focused) {
				editor.focus();
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
