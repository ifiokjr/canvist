/**
 * The native editor's typed API, over a bridge that answers from a real
 * document the way the iOS and Android editors do: names and JSON in, JSON
 * out.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { bold, heading, isCanvistError, load, starterKit } from "../src/mod.ts";
import { type NativeBridge, NativeEditor } from "../src/native.ts";
import type { Extensions, Range } from "../src/types.ts";
import { type WasmDoc, WasmEngine } from "../wasm/canvist_headless_wasm.js";

/**
 * A bridge over a raw WebAssembly document of `extensions`, which takes
 * names and JSON as the native editors do, recording every call. Loading
 * replaces the document, as it does natively.
 */
async function bridge(
	extensions: Extensions,
	calls: string[] = [],
): Promise<NativeBridge> {
	await load();
	const engine = new WasmEngine(extensions.map(({ name }) => name), 1);
	let doc: WasmDoc = engine.create();
	const record = <T>(name: string, value: T): T => {
		calls.push(name);
		return value;
	};
	// Fail as the native editors do: a plain error with the code in its text.
	const native = <T>(call: () => T): T => {
		try {
			return call();
		} catch (error) {
			if (!isCanvistError(error)) throw error;
			throw new Error(
				`Exception in HostFunction: [canvist:${error.code}] ${error.message}`,
			);
		}
	};

	return {
		run: (name, args) =>
			record(`run ${name} ${args}`, native(() => doc.run(name, args))),
		can: (name, args) => record(`can ${name} ${args}`, doc.can(name, args)),
		query: (name, args) =>
			record(`query ${name} ${args}`, doc.query(name, args)),
		load: (format, content) => {
			record(`load ${format}`, undefined);
			doc.free();
			doc = engine.parse(format, content);
		},
		export: (format) => record(`export ${format}`, doc.export(format)),
		selection: () => record("selection", doc.selection()),
		select: (range) => {
			record(`select ${range}`, undefined);
			const { anchor, head }: Range = JSON.parse(range);
			doc.select(
				Uint32Array.from(anchor.path),
				anchor.offset,
				Uint32Array.from(head.path),
				head.offset,
			);
		},
		selectAll: () => record("selectAll", doc.selectAll()),
		insertText: (text) => record(`insertText ${text}`, doc.typeText(text)),
		undo: () => record("undo", doc.undo()),
		redo: () => record("redo", doc.redo()),
		canUndo: () => record("canUndo", true),
		canRedo: () => record("canRedo", true),
		focus: () => record("focus", undefined),
		blur: () => record("blur", undefined),
	};
}

test("commands, queries, and the selection go over the bridge as JSON", async () => {
	const sent: string[] = [];
	const editor = new NativeEditor(await bridge(starterKit, sent), starterKit);
	editor.setContent({ format: "markdown", input: "Hello world" });

	editor.select({ path: [0], offset: 6 }, { path: [0], offset: 11 });
	assert.equal(editor.commands.toggleBold(), true);
	assert.equal(editor.query.isBold(), true);
	assert.equal(editor.commands.toggleHeading({ level: 2 }), true);
	assert.equal(editor.query.headingLevel(), 2);
	assert.equal(editor.export("markdown"), "## Hello **world**");
	assert.deepEqual(editor.selection, {
		anchor: { path: [0], offset: 6 },
		head: { path: [0], offset: 11 },
	});

	assert.deepEqual(sent.slice(0, 5), [
		"load markdown",
		'select {"anchor":{"path":[0],"offset":6},"head":{"path":[0],"offset":11}}',
		"run toggleBold null",
		"query isBold null",
		'run toggleHeading {"level":2}',
	]);
});

test("runtime names, can, and undo reach the same calls", async () => {
	const editor = new NativeEditor(await bridge(starterKit), starterKit);
	editor.setContent({ format: "markdown", input: "Hello" });

	editor.selectAll();
	assert.equal(editor.canRun("toggleItalic"), true);
	assert.equal(editor.run("toggleItalic"), true);
	assert.equal(editor.ask("isItalic"), true);
	assert.equal(editor.can.toggleCode(), true);
	assert.equal(editor.export("markdown"), "*Hello*");
	assert.equal(editor.undo(), true);
	assert.equal(editor.export("markdown"), "Hello");
	assert.equal(editor.redo(), true);
	assert.equal(editor.export("markdown"), "*Hello*");
});

test("content loads from text or ProseMirror JSON", async () => {
	const sent: string[] = [];
	const editor = new NativeEditor(await bridge([bold, heading], sent), [
		bold,
		heading,
	]);

	editor.setContent({
		type: "doc",
		content: [{
			type: "heading",
			attrs: { level: 1 },
			content: [{ type: "text", text: "Title" }],
		}],
	});
	assert.equal(editor.export("markdown"), "# Title");
	assert.equal(sent[0], "load json");
	assert.deepEqual(editor.toJson().content?.[0]?.type, "heading");

	editor.setContent({ format: "markdown", input: "**Bold**" });
	assert.equal(editor.export("html"), "<p><strong>Bold</strong></p>");
});

test("an editor offers exactly its extensions' commands and queries", async () => {
	const editor = new NativeEditor(await bridge([bold, heading]), [
		bold,
		heading,
	]);

	assert.deepEqual(Object.keys(editor.commands).sort(), [
		"setBold",
		"setHeading",
		"setParagraph",
		"toggleBold",
		"toggleHeading",
		"unsetBold",
	]);
	assert.deepEqual(Object.keys(editor.query).sort(), [
		"headingLevel",
		"isBold",
		"isParagraph",
	]);

	// Never called: the type checker reads these.
	const unregistered = () => {
		// @ts-expect-error italic is not registered
		editor.commands.toggleItalic();
		// @ts-expect-error levels are 1 to 6
		editor.commands.setHeading({ level: 7 });
		// @ts-expect-error toggleHeading needs a level
		editor.run("toggleHeading");
		const level: number | null = editor.query.headingLevel();
		return level;
	};
	assert.equal(typeof unregistered, "function");
});

test("typing goes through input rules", async () => {
	const editor = new NativeEditor(await bridge(starterKit), starterKit);

	editor.insertText("## Notes");
	assert.equal(editor.export("markdown"), "## Notes");
	assert.equal(editor.query.headingLevel(), 2);
});

test("subscribers hear changes and focus, and can stop listening", async () => {
	const sent: string[] = [];
	const editor = new NativeEditor(await bridge(starterKit, sent), starterKit);
	let heard = 0;
	const stop = editor.subscribe(() => heard++);

	editor.notifyChange();
	assert.equal(heard, 1);
	editor.notifyFocus(true);
	assert.equal(editor.focused, true);
	assert.equal(heard, 2);
	// The same focus twice is not a change.
	editor.notifyFocus(true);
	assert.equal(heard, 2);

	editor.focus();
	editor.blur();
	assert.deepEqual(sent, ["focus", "blur"]);

	// Edits from JavaScript notify once the task's calls are done, once for
	// all of them; queries do not notify.
	editor.commands.toggleBold();
	editor.insertText("a");
	editor.undo();
	assert.equal(heard, 2);
	await Promise.resolve();
	assert.equal(heard, 3);
	editor.query.isBold();
	editor.export("text");
	await Promise.resolve();
	assert.equal(heard, 3);

	stop();
	editor.notifyChange();
	assert.equal(heard, 3);
});

test("bridge failures reach the caller as canvist errors", async () => {
	const editor = new NativeEditor(await bridge(starterKit), starterKit);

	assert.throws(
		() => editor.run("toggleHeading", { level: 9 as 1 }),
		(error) =>
			isCanvistError(error) && error.code === "invalidArgs" &&
			!error.message.includes("[canvist:"),
	);
});

test("other bridge failures pass through unchanged", async () => {
	const failing: NativeBridge = {
		...await bridge(starterKit),
		export: () => {
			throw new TypeError("no editor with id 7");
		},
		undo: () => {
			throw new Error("[canvist:notACode] what");
		},
	};
	const editor = new NativeEditor(failing, starterKit);

	assert.throws(() => editor.export("text"), TypeError);
	assert.throws(() => editor.undo(), (error) => !isCanvistError(error));
});
