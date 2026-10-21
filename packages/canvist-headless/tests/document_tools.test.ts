/**
 * The document tools through `@canvist/headless/full`: typed commands and
 * query results for find and replace, the table of contents, comments,
 * block moves, and clearing formatting; their argument errors; the
 * `comment` mark in typed TipTap JSON; and comments shared with a
 * JavaScript Yjs peer.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import {
	bold,
	clearFormatting,
	comments,
	createEngine,
	dragHandle,
	findAndReplace,
	heading,
	isCanvistError,
	starterKit,
	tableOfContents,
} from "../src/full.ts";

const tools = [
	findAndReplace,
	tableOfContents,
	comments,
	dragHandle,
	clearFormatting,
] as const;

test("the document tools' descriptors match what Rust reports", async () => {
	using engine = await createEngine(tools);
	const reported = (engine.metadata() as { name: string }[]).filter((entry) =>
		tools.some((tool) => tool.name === entry.name)
	);
	const byName = (list: readonly { name: string }[]) =>
		[...list].sort((a, b) => a.name.localeCompare(b.name));

	assert.deepEqual(byName(reported), byName(JSON.parse(JSON.stringify(tools))));
	assert.deepEqual(
		comments.nodes.map((node) => [node.kind, node.name]),
		[["mark", "comment"]],
		"the comment mark is declared, so typed JSON may use it",
	);
});

test("find and replace answers a typed record and replaces in one step", async () => {
	using engine = await createEngine([bold, findAndReplace]);
	using doc = engine.parse("markdown", "Ça **va**, ÇA va");

	assert.equal(doc.query.searchResults(), null);
	assert.equal(doc.can.replaceAll({ replacement: "x" }), false);
	assert.equal(doc.commands.setSearchTerm({ term: "ça" }), true);

	const results = doc.query.searchResults();
	assert.deepEqual(results, {
		term: "ça",
		count: 2,
		current: null,
		caseSensitive: false,
	});
	const count: number | undefined = results?.count;
	assert.equal(count, 2);

	doc.select({ path: [0], offset: 0 });
	assert.equal(doc.commands.nextSearchResult(), true);
	assert.equal(doc.query.searchResults()?.current, 0);
	assert.equal(doc.commands.previousSearchResult(), true);
	assert.deepEqual(doc.selection, {
		anchor: { path: [0], offset: 7 },
		head: { path: [0], offset: 9 },
	});

	doc.commands.setCaseSensitive({ caseSensitive: true });
	assert.equal(doc.query.searchResults()?.count, 0);

	doc.commands.setCaseSensitive({ caseSensitive: false });
	doc.commands.setSearchTerm({ term: "va" });
	assert.equal(doc.commands.replaceAll({ replacement: "🐟" }), true);
	assert.equal(doc.export("markdown"), "Ça **🐟**, ÇA 🐟");
	assert.equal(doc.undo(), true);
	assert.equal(doc.export("markdown"), "Ça **va**, ÇA va");
	assert.equal(doc.undo(), false, "replacing all is one step");
});

test("the table of contents is a typed list", async () => {
	using engine = await createEngine([heading, tableOfContents]);
	using doc = engine.parse("markdown", "# Intro\n\n### Détail\n\n## Part");
	doc.select({ path: [2], offset: 1 });

	const entries = doc.query.tableOfContents();
	assert.deepEqual(
		entries.map((entry) => `${entry.number} ${entry.text} ${entry.active}`),
		["1 Intro false", "1.1 Détail false", "1.2 Part true"],
	);
	const levels: number[] = entries.map((entry) => entry.level);
	assert.deepEqual(levels, [1, 3, 2]);

	assert.equal(doc.can.goToHeading({ index: 1 }), true);
	doc.commands.goToHeading({ index: 1 });
	assert.deepEqual(doc.selection?.head, { path: [1], offset: 0 });
	assert.equal(doc.commands.goToHeading({ index: 3 }), false);
	assert.throws(() => doc.commands.goToHeading({ index: -1 }), {
		name: "CanvistError",
		code: "invalidArgs",
	});
});

test("comments read and write typed TipTap JSON, HTML, and Markdown", async () => {
	using engine = await createEngine([...starterKit, comments]);
	using doc = engine.fromJson({
		type: "doc",
		content: [{
			type: "paragraph",
			content: [
				{ type: "text", text: "See " },
				{
					type: "text",
					text: "here",
					marks: [{ type: "bold" }, {
						type: "comment",
						attrs: { threads: ["a", "b"] },
					}],
				},
			],
		}],
	});

	assert.deepEqual(doc.query.comments(), [
		{ threadId: "a", text: "here" },
		{ threadId: "b", text: "here" },
	]);
	assert.equal(
		doc.export("html"),
		'<p>See <strong><span data-type="comment" data-threads="a b">here</span></strong></p>',
	);
	assert.equal(doc.export("markdown"), "See **here**");

	using fromHtml = engine.parse("html", doc.export("html"));
	assert.deepEqual(fromHtml.toJson(), doc.toJson());

	doc.select({ path: [0], offset: 6 });
	assert.deepEqual(doc.query.activeComments(), ["a", "b"]);
	doc.selectAll();
	assert.equal(doc.commands.setComment({ threadId: "a" }), true);
	assert.equal(doc.can.setComment({ threadId: "a" }), false);
	assert.equal(doc.commands.unsetComment({ threadId: "b" }), true);
	assert.deepEqual(doc.query.comments(), [{ threadId: "a", text: "See here" }]);
});

test("a JavaScript Yjs peer sees comments as formatting and adds its own", async () => {
	using engine = await createEngine([...starterKit, comments]);
	using server = engine.parse("text", "Shared words");
	server.share(1);
	server.select({ path: [0], offset: 0 }, { path: [0], offset: 6 });
	server.commands.setComment({ threadId: "a" });

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	server.takeUpdates();
	const text = peer.getArray<Y.Map<unknown>>("blocks").get(0)?.get(
		"text",
	) as Y.Text;
	const delta = text.toDelta() as {
		insert: string;
		attributes?: Record<string, unknown>;
	}[];
	assert.deepEqual(delta[0], {
		insert: "Shared",
		attributes: { comment: { threads: ["a"] } },
	});

	const updates: Uint8Array[] = [];
	peer.on("update", (update: Uint8Array) => updates.push(update));
	text.format(7, 5, { comment: { threads: ["b"] } });
	updates.forEach((update) => assert.equal(server.applyUpdate(update), true));

	assert.deepEqual(server.query.comments(), [
		{ threadId: "a", text: "Shared" },
		{ threadId: "b", text: "words" },
	]);
	assert.deepEqual(server.validate(), { rejectedBlocks: 0, waiting: false });
});

test("blocks move and formatting clears through typed commands", async () => {
	using engine = await createEngine([
		...starterKit,
		dragHandle,
		clearFormatting,
	]);
	using doc = engine.parse("markdown", "one\n\n## **two**\n\nthree");
	doc.select({ path: [1], offset: 1 });

	assert.equal(doc.commands.moveBlockUp(), true);
	assert.equal(doc.press("Alt-ArrowDown"), true);
	assert.equal(doc.press("Alt-ArrowDown"), true);
	assert.equal(doc.export("markdown"), "one\n\nthree\n\n## **two**");
	assert.equal(doc.commands.moveBlocksTo({ index: 0 }), true);
	assert.equal(doc.can.moveBlocksTo({ index: 1 }), false);
	assert.equal(doc.undo(), true);
	assert.equal(doc.export("markdown"), "one\n\nthree\n\n## **two**");

	doc.selectAll();
	assert.equal(doc.commands.clearFormatting(), true);
	assert.equal(doc.export("markdown"), "one\n\nthree\n\ntwo");
	assert.equal(doc.can.clearNodes(), false);
	assert.equal(doc.can.unsetAllMarks(), false);
	assert.equal(doc.undo(), true);
	doc.selectAll();
	assert.equal(doc.press("Mod-\\"), true);
	assert.equal(doc.export("markdown"), "one\n\nthree\n\ntwo");
});

test("document tools refuse wrong arguments with stable codes", async () => {
	using engine = await createEngine([findAndReplace, comments, dragHandle]);
	using doc = engine.parse("text", "text");
	const code = (run: () => unknown) => {
		try {
			run();
		} catch (error) {
			return isCanvistError(error) ? error.code : "not a CanvistError";
		}
		return "no error";
	};

	assert.equal(
		code(() => doc.run("setSearchTerm", {} as { term: string })),
		"invalidArgs",
	);
	assert.equal(
		code(() => doc.commands.setSearchTerm({ term: 1 as unknown as string })),
		"invalidArgs",
	);
	assert.equal(
		code(() => doc.commands.setComment({} as unknown as { threadId: string })),
		"invalidArgs",
	);
	assert.equal(
		code(() => doc.commands.moveBlocksTo({ index: -1 })),
		"invalidArgs",
	);
	assert.equal(doc.export("text"), "text");
});
