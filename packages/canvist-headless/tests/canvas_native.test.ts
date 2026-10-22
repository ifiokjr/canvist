/**
 * Ink and kinetic text through `@canvist/headless/full`: typed commands,
 * query results, and JSON; argument errors; HTML and Markdown round trips;
 * and drawings and kinetic text shared with a JavaScript Yjs peer.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import {
	bold,
	createEngine,
	ink,
	isCanvistError,
	kinetic,
	paragraph,
} from "../src/full.ts";
import { comparable } from "./descriptors.ts";

test("ink and kinetic descriptors match what Rust reports", async () => {
	using engine = await createEngine([ink, kinetic]);
	const reported = comparable(engine.metadata()).filter((entry) =>
		entry.name === "ink" || entry.name === "kinetic"
	);
	const byName = (list: readonly { name: string }[]) =>
		[...list].sort((a, b) => a.name.localeCompare(b.name));

	assert.deepEqual(
		byName(reported),
		comparable([ink, kinetic]),
	);
	assert.deepEqual(ink.nodes.map((node) => [node.kind, node.name]), [[
		"block",
		"ink",
	]]);
	assert.deepEqual(kinetic.nodes.map((node) => [node.kind, node.name]), [[
		"mark",
		"kinetic",
	]]);
});

test("ink draws through typed commands and answers a typed list", async () => {
	using engine = await createEngine([ink]);
	using doc = engine.create();

	assert.deepEqual(doc.query.inks(), []);
	assert.equal(doc.commands.insertInk({ height: 300 }), true);
	assert.equal(
		doc.commands.addInkStroke({
			ink: 0,
			points: [1.04, 2.06, 1200, -5],
			color: "#ABCDEF",
			width: 3.3,
		}),
		true,
	);
	assert.equal(doc.commands.addInkStroke({ ink: 0, points: [5, 5] }), true);

	const drawings = doc.query.inks();
	const counts: number[] = drawings.map((drawing) => drawing.strokes);
	assert.deepEqual(counts, [2]);
	assert.equal(drawings[0]?.height, 300);

	const block = doc.toJson().content?.[0];
	assert.equal(block?.type, "ink");
	assert.deepEqual(
		block?.type === "ink" ? block.attrs?.strokes?.[0] : undefined,
		{ points: [1, 2.1, 1000, 0], color: "#abcdef", width: 3.3 },
	);

	assert.equal(doc.commands.undoInkStroke({ ink: 0 }), true);
	assert.equal(doc.undo(), true);
	assert.equal(doc.query.inks()[0]?.strokes, 2);
	assert.equal(doc.commands.clearInk({ ink: 0 }), true);
	assert.equal(doc.can.clearInk({ ink: 0 }), false);
	assert.equal(doc.commands.clearInk({ ink: 4 }), false);
	assert.equal(doc.undo(), true);
	assert.equal(doc.query.inks()[0]?.strokes, 2);
});

test("ink refuses malformed strokes with stable codes", async () => {
	using engine = await createEngine([ink]);
	using doc = engine.create();
	doc.commands.insertInk();
	const code = (run: () => unknown) => {
		try {
			run();
		} catch (error) {
			return isCanvistError(error) ? error.code : "not a CanvistError";
		}
		return "no error";
	};

	for (
		const run of [
			() => doc.commands.insertInk({ height: 0 }),
			() => doc.commands.insertInk({ height: 4001 }),
			() => doc.commands.addInkStroke({ ink: 0, points: [] }),
			() => doc.commands.addInkStroke({ ink: 0, points: [1, 2, 3] }),
			() => doc.commands.addInkStroke({ ink: 0, points: [1, 2], color: "red" }),
			() => doc.commands.addInkStroke({ ink: 0, points: [1, 2], width: 0 }),
			() => doc.commands.addInkStroke({ ink: -1, points: [1, 2] }),
			() =>
				doc.commands.addInkStroke({
					ink: 0,
					points: [1, "x" as unknown as number],
				}),
		]
	) {
		assert.equal(code(run), "invalidArgs");
	}
	assert.deepEqual(doc.query.inks(), [{ height: 400, strokes: 0 }]);
});

test("drawings round trip through typed TipTap JSON, HTML, and Markdown", async () => {
	using engine = await createEngine([ink]);
	using doc = engine.fromJson({
		type: "doc",
		content: [{
			type: "ink",
			attrs: {
				height: 120,
				strokes: [{ points: [10, 20, 30, 40], color: "#ff0000", width: 8 }],
			},
		}],
	});

	const html = doc.export("html");
	assert.match(html, /^<figure data-type="ink" data-height="120" /);
	assert.match(html, /role="img" aria-label="Drawing"/);
	using fromHtml = engine.parse("html", html);
	assert.deepEqual(fromHtml.toJson(), doc.toJson());
	using fromMarkdown = engine.parse("markdown", doc.export("markdown"));
	assert.deepEqual(fromMarkdown.toJson(), doc.toJson());
});

test("kinetic text has a typed effect and round trips through HTML", async () => {
	using engine = await createEngine([bold, kinetic]);
	using doc = engine.parse("text", "Say hello");
	doc.select({ path: [0], offset: 4 }, { path: [0], offset: 9 });

	assert.equal(doc.query.kineticEffect(), null);
	for (const effect of ["wave", "shake", "pulse", "rainbow"] as const) {
		assert.equal(doc.commands.setKinetic({ effect }), true);
		const current: "wave" | "shake" | "pulse" | "rainbow" | null = doc.query
			.kineticEffect();
		assert.equal(current, effect);
	}
	assert.equal(doc.commands.toggleKinetic({ effect: "wave" }), true);
	assert.equal(doc.query.kineticEffect(), "wave", "another effect switches");
	assert.equal(doc.commands.toggleKinetic({ effect: "wave" }), true);
	assert.equal(doc.query.kineticEffect(), null);
	assert.equal(doc.undo(), true);
	assert.equal(
		doc.export("html"),
		'<p>Say <span data-kinetic="wave">hello</span></p>',
	);
	assert.equal(doc.export("markdown"), "Say hello");
	assert.throws(
		() => doc.commands.setKinetic({ effect: "spin" as "wave" }),
		{ name: "CanvistError", code: "invalidArgs" },
	);

	using typed = engine.fromJson({
		type: "doc",
		content: [{
			type: "paragraph",
			content: [{
				type: "text",
				text: "Hey",
				marks: [{ type: "bold" }, {
					type: "kinetic",
					attrs: { effect: "shake" },
				}],
			}],
		}],
	});
	assert.equal(
		typed.export("html"),
		'<p><strong><span data-kinetic="shake">Hey</span></strong></p>',
	);
});

test("a JavaScript Yjs peer sees strokes and kinetic text and adds its own", async () => {
	using engine = await createEngine([paragraph, ink, kinetic]);
	using server = engine.fromJson({
		type: "doc",
		content: [
			{ type: "ink" },
			{ type: "paragraph", content: [{ type: "text", text: "Say hello" }] },
		],
	});
	server.share(1);
	server.commands.addInkStroke({ ink: 0, points: [1, 2, 3, 4] });

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	server.takeUpdates();
	const blocks = peer.getArray<Y.Map<unknown>>("blocks");
	assert.equal(blocks.get(0)?.get("type"), "ink");

	const updates: Uint8Array[] = [];
	peer.on("update", (update: Uint8Array) => updates.push(update));
	(blocks.get(1)?.get("text") as Y.Text).format(4, 5, {
		kinetic: { effect: "rainbow" },
	});
	updates.forEach((update) => assert.equal(server.applyUpdate(update), true));

	server.select({ path: [1], offset: 5 }, { path: [1], offset: 6 });
	assert.equal(server.query.kineticEffect(), "rainbow");
	assert.deepEqual(server.query.inks(), [{ height: 400, strokes: 1 }]);
	assert.deepEqual(server.validate(), { rejectedBlocks: 0, waiting: false });
});
