/**
 * Media blocks and the full module through the TypeScript API: typed
 * media commands, the StarterKit module refusing them, the emoji table
 * loaded as data on first use, error codes, and Yjs peers inserting media
 * together.
 *
 * @module
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

import * as Y from "yjs";

import {
	audio,
	createEngine as createFullEngine,
	emoji,
	extras,
	file,
	hashtag,
	image,
	isCanvistError,
	mention,
	starterKit,
	twitch,
	youtube,
} from "../src/full.ts";
import { gemoji } from "../src/gemoji.generated.ts";
import { createEngine } from "../src/mod.ts";

const media = [image, youtube, twitch, audio, file] as const;

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

/** Block types and attributes, without block ids. */
function blocks(json: unknown): unknown {
	const content = (json as { content: { attrs?: Record<string, unknown> }[] })
		.content;

	return content.map(({ attrs, ...block }) => {
		const { id: _id, ...rest } = attrs ?? {};
		return Object.keys(rest).length > 0 ? { ...block, attrs: rest } : block;
	});
}

test("typed media commands insert every block", async () => {
	using engine = await createFullEngine(media);
	using doc = engine.parse("text", "ab");
	doc.select({ path: [0], offset: 1 });

	assert.equal(doc.can.setImage({ src: "a.png" }), true);
	assert.equal(
		doc.commands.setImage({ src: "a.png", alt: "A", width: 320.5 }),
		true,
	);
	assert.deepEqual(doc.selection, {
		anchor: { path: [2], offset: 0 },
		head: { path: [2], offset: 0 },
	});
	assert.equal(
		doc.commands.setYoutubeVideo({ src: "https://youtu.be/x", start: 30 }),
		true,
	);
	assert.equal(
		doc.commands.setTwitchVideo({ src: "https://twitch.tv/a", muted: true }),
		true,
	);
	assert.equal(doc.commands.setAudio({ src: "a.mp3", controls: false }), true);
	assert.equal(
		doc.commands.insertFile({ url: "r.pdf", fileName: "Report", fileSize: 9 }),
		true,
	);

	assert.deepEqual(blocks(doc.toJson()), [
		{ type: "paragraph", content: [{ type: "text", text: "a" }] },
		{ type: "image", attrs: { alt: "A", src: "a.png", width: 320.5 } },
		{ type: "youtube", attrs: { src: "https://youtu.be/x", start: 30 } },
		{ type: "twitch", attrs: { muted: true, src: "https://twitch.tv/a" } },
		{ type: "audio", attrs: { controls: false, src: "a.mp3" } },
		{ type: "file", attrs: { fileName: "Report", fileSize: 9, url: "r.pdf" } },
		{ type: "paragraph", content: [{ type: "text", text: "b" }] },
	]);
	assert.match(
		doc.export("html"),
		/<iframe title="YouTube video" src="https:\/\/youtu.be\/x">/,
	);
	assert.equal(doc.undo(), true);
	assert.doesNotMatch(doc.export("html"), /data-type="file"/);
	assert.equal(doc.redo(), true);
	assert.match(doc.export("markdown"), /\[Report\]\(r\.pdf\)\n\nb$/);
});

test("video commands refuse URLs from other sites", async () => {
	using engine = await createFullEngine([youtube, twitch]);
	using doc = engine.create();

	for (
		const src of [
			"javascript:alert('youtube')",
			"https://youtube.com.example.com/x",
			"youtube.com/x",
		]
	) {
		assert.equal(doc.can.setYoutubeVideo({ src }), false, src);
		assert.equal(doc.commands.setYoutubeVideo({ src }), false, src);
	}

	assert.equal(
		doc.commands.setTwitchVideo({ src: "https://youtu.be/x" }),
		false,
	);
	assert.equal(doc.export("html"), "<p></p>");

	using pasted = engine.parse(
		"html",
		"<div data-youtube-video><iframe src=\"javascript:alert('youtube')\"></iframe></div><p>x</p>",
	);
	assert.equal(pasted.export("html"), "<p>x</p>");
});

test("invalid media arguments fail with stable codes", async () => {
	using engine = await createFullEngine(media);
	using doc = engine.create();
	const before = doc.export("json");

	assert.equal(
		errorCode(() => doc.commands.setImage({} as never)),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.commands.setImage({ src: null })),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() =>
			doc.commands.setImage({ src: "a.png", width: "wide" } as never)
		),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.commands.setAudio({ src: "a.mp3", loop: 1 } as never)),
		"invalidArgs",
	);
	assert.equal(
		errorCode(() => doc.commands.insertFile({ fileName: "r" } as never)),
		"invalidArgs",
	);
	assert.equal(doc.export("json"), before);
});

test("the StarterKit module refuses every extension beyond the StarterKit", async () => {
	for (const extension of extras) {
		await assert.rejects(
			createEngine([extension] as never),
			{ name: "CanvistError", code: "config" },
			extension.name,
		);
	}

	using engine = await createEngine();
	using doc = engine.create();
	doc.paste("html", '<p>a</p><img src="cat.png"><p>b</p>');
	assert.equal(
		doc.export("html"),
		"<p>a</p><p>b</p>",
		"media without the extension is dropped",
	);
	assert.equal(
		"setImage" in doc.commands,
		false,
		"the StarterKit module's documents have no media commands",
	);
});

test("the emoji table the full module loads is the Rust table", async () => {
	const rust = await readFile(
		new URL(
			"../../../crates/canvist_extensions/src/emoji/gemoji.txt",
			import.meta.url,
		),
		"utf8",
	);

	assert.equal(gemoji, rust);
});

test("full engines without emoji work, and the first emoji engine loads the table", async () => {
	using plain = await createFullEngine([...starterKit, image, mention]);
	using plainDoc = plain.create();
	plainDoc.typeText(":smile: ");
	assert.equal(
		plainDoc.export("text"),
		":smile: ",
		"no emoji extension, no conversion",
	);

	using first = await createFullEngine([emoji, hashtag]);
	using second = await createFullEngine([emoji]);

	for (const engine of [first, second]) {
		using doc = engine.create();
		doc.typeText("ship :rocket: ");
		assert.equal(doc.export("text"), "ship 🚀 ");
		assert.deepEqual(doc.query.searchEmoji({ query: "rocke", limit: 1 }), [
			{ name: "rocket", emoji: "🚀" },
		]);
		doc.typeText(":tad");
		assert.equal(doc.commands.setEmoji({ name: "tada" }), true);
	}
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

test("two peers inserting media concurrently converge", async () => {
	using firstEngine = await createFullEngine(media, { client: 1 });
	using secondEngine = await createFullEngine(media, { client: 2 });
	using first = firstEngine.parse("text", "one\ntwo");
	first.share(1);
	using second = secondEngine.fromYjs(first.encodeState(), 2);

	first.select({ path: [0], offset: 3 });
	first.commands.setImage({ src: "a.png", width: 320.5 });
	second.select({ path: [0], offset: 3 });
	second.commands.setAudio({ src: "a.mp3", loop: true });
	sync(first, second);

	assert.deepEqual(blocks(first.toJson()), blocks(second.toJson()));
	assert.equal(first.export("html"), second.export("html"));
	assert.match(first.export("html"), /<img src="a.png" width="320.5">/);
	assert.match(first.export("html"), /<audio src="a.mp3" controls loop>/);
	assert.deepEqual(second.validate(), { rejectedBlocks: 0, waiting: false });
});

test("a JavaScript Yjs peer reads and writes media blocks", async () => {
	using engine = await createFullEngine(media);
	using server = engine.create();
	server.share(1);
	server.commands.insertFile({
		url: "r.pdf",
		fileName: "Report",
		fileSize: 2048,
	});

	const peer = new Y.Doc();
	Y.applyUpdate(peer, server.encodeState());
	server.takeUpdates();
	const shared = peer.getArray<Y.Map<unknown>>("blocks");
	const card = shared.get(0);
	assert.equal(card?.get("type"), "file");
	assert.deepEqual((card?.get("attrs") as Y.Map<unknown>).toJSON(), {
		fileName: "Report",
		fileSize: 2048,
		url: "r.pdf",
	});

	const updates: Uint8Array[] = [];
	peer.on("update", (update: Uint8Array) => updates.push(update));
	const attrs = card?.get("attrs") as Y.Map<unknown>;
	attrs.set("fileName", "Renamed");
	updates.forEach((update) => assert.equal(server.applyUpdate(update), true));
	assert.match(server.export("html"), /data-file-name="Renamed"/);
});
