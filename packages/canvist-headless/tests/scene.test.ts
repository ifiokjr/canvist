/**
 * Canvas scenes from TypeScript: load, transform, export, and sync.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import {
	createSceneEngine,
	isCanvistError,
	type SceneJson,
} from "../src/scene.ts";

/** The PNG file signature. */
const PNG_SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
/** Bytes before a PNG's width in its header. */
const PNG_WIDTH_AT = 16;

const poster = {
	type: "canvist-scene",
	version: 1,
	nodes: [{
		type: "page",
		props: { fills: [{ type: "solid", color: "#f6f3ec" }] },
		children: [{
			type: "frame",
			props: {
				name: "Card",
				size: [300, 200],
				transform: [1, 0, 0, 1, 40, 30],
				fills: [{ type: "solid", color: "#ffffff" }],
			},
			children: [
				{
					type: "ellipse",
					props: {
						name: "Sun",
						size: [80, 80],
						transform: [1, 0, 0, 1, 20, 20],
					},
				},
				{
					type: "text",
					props: {
						name: "Title",
						size: [200, 0],
						transform: [1, 0, 0, 1, 20, 120],
					},
					text: {
						type: "doc",
						content: [{
							type: "paragraph",
							content: [{ type: "text", text: "Summer sale" }],
						}],
					},
				},
			],
		}],
	}],
} as const satisfies SceneJson;

/** The id of the node named `name` in a flat scene. */
function idOf(json: SceneJson, name: string): string {
	const node = json.nodes.find((node) => node.props?.name === name);
	assert.ok(
		node && "id" in node && node.id,
		`the scene has a node named ${name}`,
	);
	return node.id;
}

test("a scene round-trips through JSON", async () => {
	using engine = await createSceneEngine();
	using doc = engine.fromJson(poster);
	using again = engine.fromJson(doc.toJson());

	assert.deepEqual(again.toJson(), doc.toJson());
	assert.equal(doc.toJson().nodes.length, 4);
});

test("commands and inspector fields transform a scene", async () => {
	using engine = await createSceneEngine();
	using doc = engine.fromJson(poster);
	const sun = idOf(doc.toJson(), "Sun");

	doc.select([sun]);
	assert.deepEqual(doc.selection(), [sun]);
	doc.setField("name", "Moon");
	doc.run("insert", {
		type: "rectangle",
		props: { name: "Badge", size: [40, 20] },
	});

	const names = doc.toJson().nodes.map((node) => node.props?.name);
	assert.ok(names.includes("Moon") && names.includes("Badge"));
	assert.equal(doc.undo(), true);
	assert.ok(!doc.toJson().nodes.some((node) => node.props?.name === "Badge"));
});

test("scenes export as SVG and PNG", async () => {
	using engine = await createSceneEngine();
	using doc = engine.fromJson(poster);

	const svg = doc.exportSvg();
	assert.ok(svg.startsWith("<svg") && svg.includes("Summer"));

	const png = doc.exportPng({
		nodes: [idOf(doc.toJson(), "Sun")],
		scale: 2,
		padding: 5,
	});
	assert.deepEqual([...png.subarray(0, PNG_SIGNATURE.length)], PNG_SIGNATURE);
	const header = new DataView(png.buffer, png.byteOffset + PNG_WIDTH_AT, 8);
	// An 80-unit sun, padded by 5 on each side, at twice the size.
	assert.deepEqual([header.getUint32(0), header.getUint32(4)], [180, 180]);
});

test("failures are CanvistErrors with stable codes", async () => {
	using engine = await createSceneEngine();
	using doc = engine.create();

	const code = (run: () => unknown) => {
		try {
			run();
		} catch (error) {
			assert.ok(isCanvistError(error));
			return error.code;
		}
		return undefined;
	};

	assert.equal(code(() => doc.run("teleport")), "unknownName");
	assert.equal(code(() => doc.exportSvg()), "invalidArgs");
	assert.equal(code(() => doc.exportPng({ scale: -1 })), "invalidArgs");
	assert.equal(code(() => doc.select(["not an id"])), "invalidArgs");
});

test("a shared scene syncs with a peer", async () => {
	using engine = await createSceneEngine({ client: 3 });
	using ada = engine.fromJson(poster);
	ada.share();
	using grace = engine.fromYjs(ada.encodeState());
	assert.deepEqual(grace.toJson(), ada.toJson());

	ada.select([idOf(ada.toJson(), "Sun")]);
	ada.setField("name", "Moon");
	const changed = ada.takeUpdates().map((update) => grace.applyUpdate(update));

	assert.ok(changed.includes(true));
	assert.deepEqual(grace.toJson(), ada.toJson());
});

// Type-level checks: `tsc` fails if any of these compile.
type Node = SceneJson["nodes"][number];
type Props = NonNullable<Node["props"]>;

// @ts-expect-error scenes have a fixed type tag
export const wrongTag: SceneJson["type"] = "scene";
// @ts-expect-error node types are a closed set
export const wrongNode: Node["type"] = "hexagon";
// @ts-expect-error colours are hex strings
export const wrongColor: Props["fills"] = [{ type: "solid", color: "red" }];
