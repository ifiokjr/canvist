/**
 * The TypeScript API beyond the conformance fixtures: extension subsets,
 * errors, disposal, and wire compatibility with JavaScript Yjs.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import {
	bold,
	createEngine,
	heading,
	isCanvistError,
	starterKit,
} from "../src/mod.ts";
import { comparable } from "./descriptors.ts";

test("an engine offers exactly its extensions' commands and queries", async () => {
	using engine = await createEngine([bold, heading]);
	using doc = engine.create();

	assert.deepEqual(Object.keys(doc.commands).sort(), [
		"setBold",
		"setHeading",
		"setParagraph",
		"toggleBold",
		"toggleHeading",
		"unsetBold",
	]);
	assert.deepEqual(Object.keys(doc.query).sort(), [
		"headingLevel",
		"isBold",
		"isParagraph",
	]);

	const names = (engine.metadata() as { name: string }[]).map((extension) =>
		extension.name
	).sort();
	assert.deepEqual(names, [
		"baseKeymap",
		"bold",
		"doc",
		"heading",
		"paragraph",
		"undoInputRule",
	]);
});

test("the generated descriptors match what Rust reports", async () => {
	using engine = await createEngine();
	const metadata = engine.metadata();

	// The descriptions are in the metadata, and only there.
	assert.ok(
		metadata.some((extension) => (extension.commands[0]?.doc ?? "") !== ""),
	);
	assert.deepEqual(comparable(metadata), comparable(engine.extensions));
	assert.equal(engine.extensions.length, starterKit.length + 4);
});

test("failures are CanvistErrors with stable codes", async () => {
	const engine = await createEngine();

	const failure = (() => {
		try {
			engine.parse("json", "{ not json");
		} catch (error) {
			return error;
		}
	})();

	assert.ok(isCanvistError(failure));
	assert.equal(failure.code, "parse");
	assert.throws(() => engine.create().commands.setHeading({ level: 9 as 1 }), {
		name: "CanvistError",
		code: "invalidArgs",
	});
	assert.equal(isCanvistError(new Error("plain")), false);
});

test("a JavaScript Yjs peer reads and edits a shared document", async () => {
	const engine = await createEngine();
	const server = engine.parse("markdown", "# Title\n\nBody");
	server.share(1);

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	const blocks = peer.getArray<Y.Map<unknown>>("blocks");
	assert.equal(blocks.get(0)?.get("type"), "heading");
	assert.equal(String(blocks.get(1)?.get("text")), "Body");

	const updates: Uint8Array[] = [];
	peer.on("update", (update: Uint8Array) => updates.push(update));
	(blocks.get(1)?.get("text") as Y.Text).insert(4, " text", { bold: true });
	updates.forEach((update) => assert.equal(server.applyUpdate(update), true));
	assert.equal(server.export("markdown"), "# Title\n\nBody **text**");

	server.selectAll();
	server.commands.toggleItalic();
	server.takeUpdates().forEach((update) => Y.applyUpdate(peer, update));
	const formatted = (blocks.get(0)?.get("text") as Y.Text).toDelta() as {
		attributes?: Record<string, unknown>;
	}[];
	assert.deepEqual(formatted[0]?.attributes, { italic: true });
	assert.deepEqual(server.validate(), { rejectedBlocks: 0, waiting: false });
});

test("documents survive snapshots and selections use UTF-16 offsets", async () => {
	const engine = await createEngine();
	const doc = engine.parse("text", "a😀b");
	doc.select({ path: [0], offset: 3 }, { path: [0], offset: 4 });
	doc.insertText("c");

	const restored = engine.restore(doc.snapshot());
	assert.equal(restored.export("text"), "a😀c");
	assert.deepEqual(restored.selection, {
		anchor: { path: [0], offset: 4 },
		head: { path: [0], offset: 4 },
	});
});
