/**
 * The formatting extensions beyond the StarterKit through the TypeScript
 * API: their generated descriptors, typed commands and queries, and Yjs
 * marks a JavaScript peer reads and writes.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import {
	allExtensions,
	backgroundColor,
	color,
	createEngine,
	fontFamily,
	fontSize,
	highlight,
	isCanvistError,
	lineHeight,
	starterKit,
	subscript,
	superscript,
	textStyle,
} from "../src/mod.ts";

const formatting = [
	subscript,
	superscript,
	highlight,
	textStyle,
	color,
	backgroundColor,
	fontFamily,
	fontSize,
	lineHeight,
] as const;

/** The code a failing call throws, or `undefined` when it succeeds. */
function errorCode(call: () => unknown): string | undefined {
	try {
		call();
	} catch (error) {
		assert.ok(isCanvistError(error), `${String(error)} is a CanvistError`);
		return error.code;
	}

	return undefined;
}

test("every formatting extension has a descriptor beyond the StarterKit", () => {
	const kit = new Set<string>(starterKit.map((extension) => extension.name));
	const all = new Set<string>(allExtensions.map((extension) => extension.name));

	for (const extension of formatting) {
		assert.equal(kit.has(extension.name), false, extension.name);
		assert.equal(all.has(extension.name), true, extension.name);
	}

	assert.deepEqual(
		color.nodes.map((node) => [node.kind, node.name]),
		[["mark", "textStyle"]],
		"style attributes live on textStyle",
	);
	assert.deepEqual(subscript.commands.map((command) => command.name), [
		"toggleSubscript",
		"setSubscript",
		"unsetSubscript",
	]);
});

test("the formatting descriptors match what Rust reports, less descriptions", async () => {
	using engine = await createEngine([...starterKit, ...formatting]);
	const byName = (list: readonly { name: string }[]) =>
		[...list].sort((a, b) => a.name.localeCompare(b.name));
	// Descriptions are comments in the generated file, not data.
	const withoutDocs = (value: unknown): { name: string }[] =>
		JSON.parse(
			JSON.stringify(value, (key, item) => key === "doc" ? undefined : item),
		);

	assert.deepEqual(
		byName(withoutDocs(engine.metadata())),
		byName(withoutDocs(engine.extensions)),
	);
});

test("an engine offers exactly the formatting commands it registers", async () => {
	using engine = await createEngine([highlight, textStyle, fontSize]);
	using doc = engine.create();

	assert.deepEqual(Object.keys(doc.commands).sort(), [
		"removeEmptyTextStyle",
		"setFontSize",
		"setHighlight",
		"setParagraph",
		"toggleHighlight",
		"unsetFontSize",
		"unsetHighlight",
	]);
	assert.deepEqual(Object.keys(doc.query).sort(), [
		"fontSize",
		"highlightColor",
		"isHighlight",
		"isParagraph",
	]);
});

test("typed commands and queries set, read, and clear every style", async () => {
	using engine = await createEngine([...starterKit, ...formatting]);
	using doc = engine.parse("text", "H2O ink");

	doc.select({ path: [0], offset: 1 }, { path: [0], offset: 2 });
	assert.equal(doc.can.toggleSubscript(), true);
	assert.equal(doc.commands.toggleSubscript(), true);
	assert.equal(doc.query.isSubscript(), true);
	assert.equal(doc.commands.setSuperscript(), true);
	assert.equal(
		doc.query.isSubscript(),
		false,
		"the scripts exclude each other",
	);
	assert.equal(doc.commands.unsetSuperscript(), true);

	doc.select({ path: [0], offset: 4 }, { path: [0], offset: 7 });
	doc.commands.setHighlight({ color: "#fef08a" });
	doc.commands.setColor({ color: "#958df1" });
	doc.commands.setBackgroundColor({ backgroundColor: "#fafafa" });
	doc.commands.setFontFamily({ fontFamily: "Georgia" });
	doc.commands.setFontSize({ fontSize: "18px" });
	doc.commands.setLineHeight({ lineHeight: "1.5" });

	assert.equal(doc.query.highlightColor(), "#fef08a");
	assert.equal(doc.query.color(), "#958df1");
	assert.equal(doc.query.backgroundColor(), "#fafafa");
	assert.equal(doc.query.fontFamily(), "Georgia");
	assert.equal(doc.query.fontSize(), "18px");
	assert.equal(doc.query.lineHeight(), "1.5");
	assert.equal(
		doc.export("html"),
		'<p>H2O <span style="color: #958df1; background-color: #fafafa; font-family: Georgia; font-size: 18px; line-height: 1.5"><mark data-color="#fef08a" style="background-color: #fef08a; color: inherit">ink</mark></span></p>',
	);

	const json = doc.toJson();
	using back = engine.fromJson(json);
	assert.deepEqual(back.toJson(), json, "JSON reads back equal");

	for (
		const unset of [
			doc.commands.unsetColor,
			doc.commands.unsetBackgroundColor,
			doc.commands.unsetFontFamily,
			doc.commands.unsetFontSize,
			doc.commands.unsetLineHeight,
			doc.commands.unsetHighlight,
		]
	) {
		assert.equal(unset(), true);
	}

	assert.equal(doc.query.fontSize(), null);
	assert.equal(doc.export("html"), "<p>H2O ink</p>");
	assert.equal(doc.undo(), true);
	assert.equal(doc.query.isHighlight(), true, "undo restores the highlight");
});

test("formatting errors carry stable codes", async () => {
	using engine = await createEngine([...starterKit, ...formatting]);
	using doc = engine.parse("text", "x");
	doc.selectAll();

	assert.equal(
		errorCode(() => doc.run("setFontSize", { fontSize: 18 } as never)),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.run("setHighlight", { color: true } as never)),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.run("setLineHeight", {} as never)),
		"invalidArgs",
	);
	// The types reject unknown names; the cast reaches Rust's own check.
	assert.equal(
		errorCode(() => doc.run("toggleSpoiler" as "toggleBold")),
		"unknownName",
	);
	assert.equal(
		doc.export("html"),
		"<p>x</p>",
		"failed commands change nothing",
	);

	using withoutStyle = await createEngine([color]);
	using bare = withoutStyle.parse("text", "x");
	bare.selectAll();
	assert.equal(
		errorCode(() => bare.commands.setColor({ color: "red" })),
		"editor",
	);

	using plain = await createEngine();
	assert.equal(
		errorCode(() =>
			plain.fromJson({
				type: "doc",
				content: [{
					type: "paragraph",
					content: [{
						type: "text",
						text: "x",
						marks: [{ type: "highlight" }],
					}],
				}],
			} as never)
		),
		"schema",
	);
});

test("a JavaScript Yjs peer reads and writes formatting marks", async () => {
	using engine = await createEngine([...starterKit, ...formatting]);
	using server = engine.parse("text", "Hello world");
	server.share(1);

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	const blocks = peer.getArray<Y.Map<unknown>>("blocks");
	const text = blocks.get(0)?.get("text") as Y.Text;

	server.select({ path: [0], offset: 0 }, { path: [0], offset: 5 });
	server.commands.setHighlight({ color: "#fef08a" });
	server.commands.setFontSize({ fontSize: "20px" });
	server.takeUpdates().forEach((update) => Y.applyUpdate(peer, update));

	const delta = text.toDelta() as {
		insert: string;
		attributes?: Record<string, unknown>;
	}[];
	assert.deepEqual(delta[0], {
		insert: "Hello",
		attributes: {
			highlight: { color: "#fef08a" },
			textStyle: { fontSize: "20px" },
		},
	});

	const updates: Uint8Array[] = [];
	peer.on("update", (update: Uint8Array) => updates.push(update));
	text.format(6, 5, { superscript: true, textStyle: { color: "#e8590c" } });
	updates.forEach((update) => assert.equal(server.applyUpdate(update), true));

	assert.equal(
		server.export("html"),
		'<p><span style="font-size: 20px"><mark data-color="#fef08a" style="background-color: #fef08a; color: inherit">Hello</mark></span> <span style="color: #e8590c"><sup>world</sup></span></p>',
	);
	assert.deepEqual(server.validate(), { rejectedBlocks: 0, waiting: false });
});

test("two TypeScript peers formatting concurrently converge", async () => {
	using engine = await createEngine([...starterKit, ...formatting]);
	using first = engine.parse("text", "Hello world");
	first.share(1);
	using second = engine.fromYjs(first.encodeState(), 2);

	first.select({ path: [0], offset: 0 }, { path: [0], offset: 5 });
	first.commands.setColor({ color: "#e8590c" });
	second.select({ path: [0], offset: 3 }, { path: [0], offset: 11 });
	second.commands.toggleHighlight({ color: "#fef08a" });

	const fromFirst = first.takeUpdates();
	const fromSecond = second.takeUpdates();
	fromFirst.forEach((update) => second.applyUpdate(update));
	fromSecond.forEach((update) => first.applyUpdate(update));

	assert.deepEqual(first.toJson(), second.toJson());
	assert.equal(
		first.export("html"),
		'<p><span style="color: #e8590c">Hel</span><span style="color: #e8590c"><mark data-color="#fef08a" style="background-color: #fef08a; color: inherit">lo</mark></span><mark data-color="#fef08a" style="background-color: #fef08a; color: inherit"> world</mark></p>',
	);
});
