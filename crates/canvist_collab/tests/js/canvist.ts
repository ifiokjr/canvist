/**
 * What a JavaScript client needs to know about canvist documents, written
 * only from the Yjs schema in the `canvist_collab` crate docs, plus the
 * line protocol the Rust tests use to drive these scripts.
 */

import * as Y from "npm:yjs@13.6.33";

export { Y };

/** Mark names in schema order, which the Rust model's JSON uses. */
const MARK_ORDER = ["bold", "italic", "code", "link"];
const EMBEDS = new Set(["mention"]);

const encoder = new TextEncoder();

export type Blocks = Y.Array<Y.Map<unknown>>;

export function toHex(bytes: Uint8Array): string {
	return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join(
		"",
	);
}

export function fromHex(hex: string): Uint8Array {
	return Uint8Array.from(hex.match(/../g) ?? [], (pair) => parseInt(pair, 16));
}

/** Write one JSON message line to the Rust test. */
export function send(message: unknown) {
	Deno.stdout.writeSync(encoder.encode(`${JSON.stringify(message)}\n`));
}

/** Every JSON message line the Rust test writes to stdin. */
export async function* messages(): AsyncGenerator<Record<string, unknown>> {
	const decoder = new TextDecoder();
	let buffered = "";

	for await (const chunk of Deno.stdin.readable) {
		buffered += decoder.decode(chunk, { stream: true });
		let newline;

		while ((newline = buffered.indexOf("\n")) >= 0) {
			const line = buffered.slice(0, newline);
			buffered = buffered.slice(newline + 1);
			yield JSON.parse(line);
		}
	}
}

/** The block map at `path`, a list of child indices from the root. */
export function block(blocks: Blocks, path: number[]): Y.Map<unknown> {
	let array = blocks;
	let map: Y.Map<unknown> | undefined;

	for (const index of path) {
		map = array.get(index);
		array = map.get("children") as Blocks;
	}

	if (!map) throw new Error(`no block at ${path}`);

	return map;
}

export function text(blocks: Blocks, path: number[]): Y.Text {
	return block(blocks, path).get("text") as Y.Text;
}

/** A new block map, as any JavaScript client would build one. */
function newBlock(id: string, type: string, content: Y.Text | Blocks) {
	const map = new Y.Map<unknown>();
	map.set("id", id);
	map.set("type", type);
	map.set("attrs", new Y.Map());
	map.set(content instanceof Y.Text ? "text" : "children", content);

	return map;
}

/** Scripted edits to the starter document of the Rust tests. */
export function edit(doc: Y.Doc, name: string) {
	const blocks = doc.getArray<Y.Map<unknown>>("blocks");

	if (name === "concurrent") {
		doc.transact(() => {
			// "bold é😀": insert after the emoji (UTF-16 index 8), then delete
			// the emoji itself (two UTF-16 code units).
			const paragraph = text(blocks, [1]);
			paragraph.insert(8, " 🎉");
			paragraph.delete(6, 2);
			// Italicise "Ti" in the heading and raise it to level 2.
			text(blocks, [0]).format(0, 2, { italic: true });
			(block(blocks, [0]).get("attrs") as Y.Map<unknown>).set("level", 2);
			// Link "quoted" inside the blockquote.
			text(blocks, [3, 0]).format(0, 6, { link: { href: "https://yjs.dev" } });
			// Mention someone in bold at the end of the last paragraph.
			const end = text(blocks, [5]);
			end.insertEmbed(end.length, { mention: { id: "js" } }, { bold: true });
		});
	} else if (name === "nested") {
		// A list item holding a code-formatted paragraph.
		const paragraph = new Y.Text();
		paragraph.insert(0, "from js", { code: true });
		const children: Blocks = new Y.Array();
		children.push([newBlock("3a.1", "paragraph", paragraph)]);
		const list = block(blocks, [2]).get("children") as Blocks;
		list.push([newBlock("3a.0", "list_item", children)]);
	} else {
		throw new Error(`unknown edit ${name}`);
	}
}

/** The document as ProseMirror JSON, the way the Rust model writes it. */
export function toJson(doc: Y.Doc) {
	const blocks = doc.getArray<Y.Map<unknown>>("blocks");

	return { type: "doc", content: blocks.toArray().map(blockJson) };
}

function blockJson(map: Y.Map<unknown>): Record<string, unknown> {
	const attrs = {
		...(map.get("attrs") as Y.Map<unknown>).toJSON(),
		id: map.get("id"),
	};
	const node: Record<string, unknown> = { type: map.get("type"), attrs };
	const value = map.get("text") ?? map.get("children");
	let content: unknown[] = [];

	if (value instanceof Y.Text) {
		content = value.toDelta().map(inlineJson);
	} else if (value instanceof Y.Array) {
		content = value.toArray().map(blockJson);
	}

	if (content.length > 0) node.content = content;

	return node;
}

type DeltaOp = {
	insert: string | Record<string, unknown>;
	attributes?: Record<string, unknown>;
};

/**
 * One delta operation as ProseMirror inline JSON. Text and embeds map to
 * nodes; formatting attributes become marks, skipping cleared ones, in the
 * schema's mark order, with mark attributes only when there are any.
 */
function inlineJson(op: DeltaOp) {
	let node: Record<string, unknown>;

	if (typeof op.insert === "string") {
		node = { type: "text", text: op.insert };
	} else {
		const [type, attrs] = Object.entries(op.insert)[0];

		if (!EMBEDS.has(type)) throw new Error(`unknown embed ${type}`);
		node = { type, attrs };
	}

	const marks = Object.entries(op.attributes ?? {})
		.filter(([, value]) => value !== null && value !== false)
		.sort(([a], [b]) => MARK_ORDER.indexOf(a) - MARK_ORDER.indexOf(b))
		.map(([type, value]) => markJson(type, value));

	if (marks.length > 0) node.marks = marks;

	return node;
}

function markJson(type: string, value: unknown) {
	const hasAttrs = typeof value === "object" && value !== null &&
		Object.keys(value).length > 0;

	return hasAttrs ? { type, attrs: value } : { type };
}
