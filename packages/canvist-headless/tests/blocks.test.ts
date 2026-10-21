/**
 * Details, callouts, and math through `@canvist/headless/full`: their typed
 * descriptors, typed commands and queries, errors, TipTap JSON, and
 * collaboration with JavaScript Yjs peers. The shared cases in
 * `conformance/fixtures/blocks.json` cover editing and the formats.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import {
	callout,
	createEngine,
	details,
	mathematics,
	starterKit,
} from "../src/full.ts";

const blocks = [...starterKit, details, callout, mathematics] as const;

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends
	<T>() => T extends B ? 1 : 2 ? true : false;

function assertType<Condition extends true>(_condition?: Condition): void {}

test("the block descriptors match what Rust reports", async () => {
	using engine = await createEngine(blocks);
	const reported = engine.metadata() as { name: string }[];

	for (const descriptor of [details, callout, mathematics]) {
		assert.deepEqual(
			reported.find((extension) => extension.name === descriptor.name),
			JSON.parse(JSON.stringify(descriptor)),
		);
	}

	assert.deepEqual(
		callout.commands[0].args[0].type.enum,
		["info", "warning", "error", "success", "blank"],
	);
	assert.deepEqual(details.nodes.map((node) => node.name), [
		"details",
		"detailsSummary",
		"detailsContent",
	]);
	assert.deepEqual(mathematics.nodes.map((node) => node.kind), [
		"embed",
		"block",
	]);
});

test("block commands and queries are typed by their descriptors", async () => {
	using engine = await createEngine(blocks);
	using doc = engine.parse("markdown", "Careful");
	doc.select({ path: [0], offset: 7 });

	assertType<
		Equal<
			ReturnType<typeof doc.query.calloutType>,
			"info" | "warning" | "error" | "success" | "blank" | null
		>
	>();
	assertType<Equal<ReturnType<typeof doc.query.detailsOpen>, boolean | null>>();

	assert.equal(doc.query.calloutType(), null);
	assert.equal(doc.can.setCalloutType({ type: "error" }), false);
	assert.equal(doc.commands.toggleCallout({ type: "warning" }), true);
	assert.equal(doc.query.calloutType(), "warning");
	assert.equal(doc.can.setCalloutType({ type: "error" }), true);
	assert.equal(doc.run("setCalloutType", { type: "error" }), true);
	assert.equal(doc.ask("calloutType"), "error");
	assert.equal(doc.commands.toggleCallout(), true);
	assert.equal(doc.query.calloutType(), null);

	assert.equal(doc.commands.setDetails(), true);
	assert.equal(doc.query.detailsOpen(), true);
	assert.equal(doc.commands.toggleDetailsOpen(), true);
	assert.equal(doc.query.detailsOpen(), false);
	assert.equal(doc.commands.unsetDetails(), true);
	assert.equal(doc.canRun("unsetDetails"), false);

	assert.equal(doc.commands.insertInlineMath({ latex: "x^2" }), true);
	// The block formula splits the paragraph at the caret, after the inline one.
	assert.equal(doc.commands.insertBlockMath({ latex: "E = mc^2" }), true);
	assert.equal(doc.export("markdown"), "$x^2$\n\n$$\nE = mc^2\n$$\n\nCareful");

	assert.equal(doc.undo(), true);
	assert.equal(doc.export("markdown"), "$x^2$Careful");
	assert.equal(doc.redo(), true);
	assert.equal(doc.export("markdown"), "$x^2$\n\n$$\nE = mc^2\n$$\n\nCareful");
});

test("invalid block arguments fail with invalidArgs and change nothing", async () => {
	using engine = await createEngine(blocks);
	using doc = engine.parse("markdown", "x");
	const before = doc.export("json");
	const invalid = { name: "CanvistError", code: "invalidArgs" };

	assert.throws(
		// @ts-expect-error purple is not a callout type
		() => doc.commands.toggleCallout({ type: "purple" }),
		invalid,
	);
	assert.throws(
		// @ts-expect-error setCalloutType needs a type
		() => doc.commands.setCalloutType(),
		invalid,
	);
	assert.throws(
		// @ts-expect-error latex is required
		() => doc.commands.insertInlineMath({}),
		invalid,
	);
	assert.throws(
		// @ts-expect-error latex is a string
		() => doc.commands.insertBlockMath({ latex: 2 }),
		invalid,
	);
	assert.equal(doc.export("json"), before);
});

test("an engine without the block extensions has none of their names", async () => {
	using engine = await createEngine([...starterKit, details]);
	using doc = engine.create();

	assert.equal(Object.hasOwn(doc.commands, "toggleCallout"), false);
	assert.equal(Object.hasOwn(doc.query, "calloutType"), false);
	assert.throws(
		// @ts-expect-error callout is not registered
		() => doc.run("toggleCallout"),
		{ name: "CanvistError", code: "unknownName" },
	);
	// @ts-expect-error mathematics is not registered
	doc.commands.insertInlineMath;
});

test("TipTap JSON blocks read and write back unchanged", async () => {
	using engine = await createEngine(blocks);
	const json = {
		type: "doc",
		content: [
			{
				type: "details",
				attrs: { open: true },
				content: [
					{
						type: "detailsSummary",
						content: [{ type: "text", text: "Title" }],
					},
					{
						type: "detailsContent",
						content: [{
							type: "callout",
							attrs: { type: "success" },
							content: [{
								type: "paragraph",
								content: [
									{ type: "text", text: "Area " },
									{ type: "inlineMath", attrs: { latex: "\\pi r^2" } },
								],
							}],
						}],
					},
				],
			},
			{ type: "blockMath", attrs: { latex: "E = mc^2" } },
		],
	} as const;
	using doc = engine.fromJson(json);

	assert.deepEqual(withoutIds(doc.toJson()), json);
	assert.equal(
		doc.export("markdown"),
		"<details open>\n<summary>Title</summary>\n\n> [!TIP]\n> Area $\\pi r^2$\n\n</details>\n\n$$\nE = mc^2\n$$",
	);
});

test("peers editing blocks concurrently converge, through a JavaScript Yjs peer", async () => {
	using engine = await createEngine(blocks);
	using first = engine.parse(
		"markdown",
		"> [!NOTE]\n> Careful\n\n<details open>\n<summary>Sum</summary>\n\nBody\n\n</details>",
	);
	first.share(1);

	// The second peer joins through a plain JavaScript Yjs document, which
	// carries the block types and attributes it does not understand.
	const relay = new Y.Doc();
	Y.applyUpdate(relay, first.encodeState());
	const roots = relay.getArray<Y.Map<unknown>>("blocks");
	assert.equal(roots.get(0)?.get("type"), "callout");
	assert.equal(
		(roots.get(0)?.get("attrs") as Y.Map<unknown>).get("type"),
		"info",
	);
	assert.equal(
		(roots.get(1)?.get("attrs") as Y.Map<unknown>).get("open"),
		true,
	);
	using second = engine.fromYjs(Y.encodeStateAsUpdate(relay), 2);
	assert.deepEqual(second.toJson(), first.toJson());

	first.select({ path: [0, 0], offset: 0 });
	first.commands.setCalloutType({ type: "warning" });
	first.select({ path: [1, 0], offset: 0 });
	first.commands.toggleDetailsOpen();
	second.select({ path: [0, 0], offset: 7 });
	second.insertText(" now");
	second.select({ path: [1, 1, 0], offset: 4 });
	second.commands.insertInlineMath({ latex: "x" });

	const fromFirst = first.takeUpdates();
	const fromSecond = second.takeUpdates();
	fromSecond.forEach((update) => first.applyUpdate(update));
	fromFirst.forEach((update) => second.applyUpdate(update));
	fromFirst.concat(fromSecond).forEach((update) =>
		Y.applyUpdate(relay, update)
	);

	assert.deepEqual(first.toJson(), second.toJson());
	assert.equal(
		first.export("markdown"),
		"> [!WARNING]\n> Careful now\n\n<details>\n<summary>Sum</summary>\n\nBody$x$\n\n</details>",
	);
	using third = engine.fromYjs(Y.encodeStateAsUpdate(relay), 3);
	assert.deepEqual(third.toJson(), first.toJson());
	assert.deepEqual(first.validate(), { rejectedBlocks: 0, waiting: false });
});

/** Remove block ids, and the `attrs` they leave empty. */
function withoutIds(node: unknown): unknown {
	if (Array.isArray(node)) {
		return node.map(withoutIds);
	}

	if (typeof node !== "object" || node === null) {
		return node;
	}

	const entries = Object.entries(node).flatMap(([key, value]) => {
		if (key !== "attrs" || typeof value !== "object" || value === null) {
			return [[key, withoutIds(value)]];
		}

		const attrs = Object.fromEntries(
			Object.entries(value).filter(([name]) => name !== "id"),
		);
		return Object.keys(attrs).length === 0 ? [] : [[key, attrs]];
	});

	return Object.fromEntries(entries);
}
