/**
 * Mentions, hashtags, and emoji through the TypeScript API: typed commands
 * and query results, engines without them, error codes, and Yjs peers (two
 * canvist documents, and a JavaScript Yjs peer) editing them together.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import {
	bold,
	createEngine,
	emoji,
	hashtag,
	isCanvistError,
	mention,
} from "../src/mod.ts";

const atoms = [mention, hashtag, emoji] as const;

/** The code of the CanvistError `run` throws. */
function errorCode(run: () => unknown): string {
	try {
		run();
	} catch (error) {
		assert.ok(isCanvistError(error), `not a CanvistError: ${error}`);
		return error.code;
	}

	assert.fail("expected a CanvistError");
}

test("typed commands insert atoms and typed queries describe them", async () => {
	using engine = await createEngine(atoms);
	using doc = engine.create();

	doc.typeText("Hi @ad");
	const suggestion: { readonly char: string; readonly query: string } | null =
		doc.query.mentionSuggestion();
	assert.deepEqual(suggestion, { char: "@", query: "ad" });
	assert.equal(doc.can.insertMention({ id: "7" }), true);
	assert.equal(doc.commands.insertMention({ id: "7", label: "Ada" }), true);
	assert.equal(doc.query.mentionSuggestion(), null);

	doc.typeText(":tad");
	const found: readonly { readonly name: string; readonly emoji: string }[] =
		doc.query.searchEmoji({ query: "tad", limit: 1 });
	assert.deepEqual(found, [{ name: "tada", emoji: "🎉" }]);
	assert.equal(doc.commands.setEmoji({ name: found[0]?.name ?? "" }), true);
	assert.equal(doc.commands.setEmoji({ name: "not_an_emoji" }), false);

	doc.typeText("#ca");
	assert.deepEqual(doc.query.hashtagSuggestion(), { char: "#", query: "ca" });
	assert.equal(doc.commands.dismissHashtagSuggestion(), true);
	assert.equal(doc.query.hashtagSuggestion(), null);
	assert.equal(doc.commands.dismissHashtagSuggestion(), false);
	doc.typeText(" ");
	doc.commands.insertHashtag({ tag: "#canvas" });
	const tags: readonly string[] = doc.query.hashtags();
	assert.deepEqual(
		tags,
		["ca", "canvas"],
		"a dismissed suggestion is still a tag once its word ends",
	);

	assert.equal(doc.export("text"), "Hi @Ada 🎉 #ca #canvas ");
	assert.deepEqual(doc.selection, {
		anchor: { path: [0], offset: 19 },
		head: { path: [0], offset: 19 },
	}, "an atom is one UTF-16 unit");
});

test("the default search limit is ten and Escape dismisses through press", async () => {
	using engine = await createEngine([emoji]);
	using doc = engine.create();

	assert.equal(doc.query.searchEmoji({ query: "face" }).length, 10);
	doc.typeText(":sm");
	assert.equal(doc.press("Escape"), true);
	assert.equal(doc.query.emojiSuggestion(), null);
	doc.typeText("ile:");
	assert.equal(doc.export("text"), "😄", "the shortcode still converts");
	assert.equal(doc.press("Backspace"), true);
	assert.equal(
		doc.export("text"),
		":smile:",
		"Backspace undoes the conversion",
	);
	assert.equal(doc.undo(), true);
	assert.equal(doc.export("text"), "😄");
});

test("an engine offers only the atoms it registered", async () => {
	using engine = await createEngine([bold, mention]);
	using doc = engine.create();

	assert.deepEqual(
		Object.keys(doc.commands).filter((name) => !name.endsWith("Bold")).sort(),
		["dismissMentionSuggestion", "insertMention", "setParagraph"],
	);
	assert.deepEqual(
		Object.keys(doc.query).filter((name) => name !== "isBold").sort(),
		["isParagraph", "mentionSuggestion"],
	);
	// Names from outside the type system, as a host picking them at runtime.
	const untyped = doc as unknown as {
		run(name: string, args?: unknown): boolean;
	};
	assert.equal(
		errorCode(() => untyped.run("setEmoji", { name: "smile" })),
		"unknownName",
	);
	doc.typeText(":smile: #tag ");
	assert.equal(doc.export("html"), "<p>:smile: #tag </p>");

	const metadata = engine.metadata() as {
		name: string;
		nodes: { name: string; attrs: { name: string }[] }[];
	}[];
	const node = metadata.find((extension) => extension.name === "mention")
		?.nodes[0];
	assert.equal(node?.name, "mention");
	assert.deepEqual(node?.attrs.map((attr) => attr.name), [
		"id",
		"label",
		"mentionSuggestionChar",
	]);
});

test("invalid arguments fail with stable codes and change nothing", async () => {
	using engine = await createEngine(atoms);
	using doc = engine.create();
	doc.typeText("@a");
	const before = doc.export("json");

	assert.equal(
		errorCode(() => doc.commands.insertMention({} as never)),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.commands.insertMention({ id: 7 } as never)),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.commands.insertHashtag({ tag: ["a"] } as never)),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.commands.setEmoji({} as never)),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() =>
			doc.query.searchEmoji({ query: "a", limit: "ten" } as never)
		),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.query.searchEmoji({ query: "a", limit: 1.5 })),
		"invalidArgs",
	);
	assert.equal(doc.export("json"), before);
	const unknown = { name: "emojis", commands: [], queries: [] };
	await assert.rejects(
		createEngine([unknown] as never),
		(error) => isCanvistError(error) && error.code === "config",
	);
});

test("TipTap JSON atoms load through fromJson and export as HTML", async () => {
	using engine = await createEngine(atoms);
	using doc = engine.fromJson({
		type: "doc",
		content: [{
			type: "paragraph",
			content: [
				{ type: "mention", attrs: { id: "u1" } },
				{ type: "text", text: " " },
				{ type: "emoji", attrs: { name: "smile" } },
				{
					type: "text",
					text: " #x",
					marks: [{ type: "hashtag", attrs: { tag: "x" } }],
				},
			],
		}],
	});

	assert.equal(doc.export("text"), "@u1 :smile: #x");
	assert.equal(
		doc.export("html"),
		'<p><span data-type="mention" data-id="u1">@u1</span> <span data-type="emoji" data-name="smile">:smile:</span><span data-type="hashtag" data-tag="x"> #x</span></p>',
	);
	assert.deepEqual(doc.query.hashtags(), ["x"]);
});

/** Deliver each peer's pending updates to the other. */
function sync(
	first: {
		takeUpdates(): Uint8Array[];
		applyUpdate(update: Uint8Array): boolean;
	},
	second: {
		takeUpdates(): Uint8Array[];
		applyUpdate(update: Uint8Array): boolean;
	},
): void {
	const fromFirst = first.takeUpdates();
	const fromSecond = second.takeUpdates();
	fromFirst.forEach((update) => second.applyUpdate(update));
	fromSecond.forEach((update) => first.applyUpdate(update));
}

test("two peers adding atoms concurrently converge", async () => {
	using firstEngine = await createEngine(atoms, { client: 1 });
	using secondEngine = await createEngine(atoms, { client: 2 });
	using first = firstEngine.parse("text", "Hello world");
	first.share(1);
	using second = secondEngine.fromYjs(first.encodeState(), 2);

	first.select({ path: [0], offset: 5 });
	first.typeText(" @gr");
	first.commands.insertMention({ id: "3", label: "Grace" });
	second.select({ path: [0], offset: 5 });
	second.typeText(" :rocket: #wasm ");
	sync(first, second);

	const strip = (json: unknown) =>
		JSON.stringify((json as { content: unknown }).content);
	assert.equal(strip(first.toJson()), strip(second.toJson()));
	assert.equal(first.export("html"), second.export("html"));
	assert.match(first.export("html"), /data-label="Grace"/);
	assert.match(first.export("html"), /data-name="rocket"/);
	assert.deepEqual(first.query.hashtags(), ["wasm"]);
	assert.deepEqual(second.validate(), { rejectedBlocks: 0, waiting: false });

	assert.equal(first.undo(), true);
	sync(first, second);
	assert.doesNotMatch(second.export("html"), /data-type="mention"/);
	assert.equal(first.export("html"), second.export("html"));
});

test("a JavaScript Yjs peer reads and writes atoms and hashtags", async () => {
	using engine = await createEngine(atoms);
	using server = engine.create();
	server.share(1);
	server.typeText("@a");
	server.commands.insertMention({ id: "7", label: "Ada" });
	server.typeText(":wave: #team ");

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	server.takeUpdates();
	const text = peer.getArray<Y.Map<unknown>>("blocks").get(0)?.get(
		"text",
	) as Y.Text;
	const delta = text.toDelta() as {
		insert: unknown;
		attributes?: Record<string, unknown>;
	}[];
	assert.deepEqual(delta[0]?.insert, {
		mention: { id: "7", label: "Ada", mentionSuggestionChar: "@" },
	});
	assert.deepEqual(delta[2]?.insert, { emoji: { name: "wave", emoji: "👋" } });
	assert.deepEqual(delta[4], {
		insert: "#team",
		attributes: { hashtag: { tag: "team" } },
	});

	const updates: Uint8Array[] = [];
	peer.on("update", (update: Uint8Array) => updates.push(update));
	const end = text.length;
	text.insertEmbed(end, {
		mention: { id: "9", label: "Zed", mentionSuggestionChar: "@" },
	});
	text.insert(end + 1, " #js", {});
	text.format(end + 2, 3, { hashtag: { tag: "js" } });
	updates.forEach((update) => assert.equal(server.applyUpdate(update), true));

	assert.equal(server.export("text"), "@Ada 👋 #team @Zed #js");
	assert.deepEqual(server.query.hashtags(), ["js", "team"]);
	assert.deepEqual(server.validate(), { rejectedBlocks: 0, waiting: false });
});
