/**
 * A plain JavaScript Yjs peer for the compatibility tests.
 *
 * It knows the canvist Yjs schema from the crate docs, nothing more: it
 * reads and writes `Y.Map` blocks, `Y.Text` with marks as formatting
 * attributes, and embeds as `{ "<type>": attrs }` values. The Rust test
 * drives it over stdin and stdout, one JSON message per line:
 *
 * - `{ "update": "<hex>" }` applies a Yjs update (v1).
 * - `{ "edit": "<name>" }` runs one of the scripted edits below.
 * - `{ "dump": true }` replies with `{ "json": <ProseMirror JSON> }`.
 *
 * Every local change is sent back as `{ "update": "<hex>" }`, and every
 * message is acknowledged with `{ "done": true }`.
 */
import * as Y from "npm:yjs@13.6.33";

/** Mark names in schema order, which the Rust model's JSON uses. */
const MARK_ORDER = ["bold", "italic", "code", "link"];
const EMBEDS = new Set(["mention"]);

const doc = new Y.Doc();
doc.clientID = 4242;
const blocks = doc.getArray<Y.Map<unknown>>("blocks");
const encoder = new TextEncoder();

function toHex(bytes: Uint8Array): string {
	return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join(
		"",
	);
}

function fromHex(hex: string): Uint8Array {
	return Uint8Array.from(hex.match(/../g) ?? [], (pair) => parseInt(pair, 16));
}

function send(message: unknown) {
	Deno.stdout.writeSync(encoder.encode(`${JSON.stringify(message)}\n`));
}

doc.on("update", (update: Uint8Array, origin: unknown) => {
	if (origin !== "remote") {
		send({ update: toHex(update) });
	}
});

function block(path: number[]): Y.Map<unknown> {
	let array = blocks;
	let map: Y.Map<unknown> | undefined;

	for (const index of path) {
		map = array.get(index);
		array = map.get("children") as Y.Array<Y.Map<unknown>>;
	}

	if (!map) throw new Error(`no block at ${path}`);

	return map;
}

function text(path: number[]): Y.Text {
	return block(path).get("text") as Y.Text;
}

/** A new block map, as any JavaScript client would build one. */
function newBlock(
	id: string,
	type: string,
	content: Y.Text | Y.Array<Y.Map<unknown>>,
) {
	const map = new Y.Map<unknown>();
	map.set("id", id);
	map.set("type", type);
	map.set("attrs", new Y.Map());
	map.set(content instanceof Y.Text ? "text" : "children", content);

	return map;
}

const edits: Record<string, () => void> = {
	/** Edits made concurrently with the Rust peer's. */
	concurrent() {
		doc.transact(() => {
			// "bold é😀": insert after the emoji (UTF-16 index 8), then delete
			// the emoji itself (two UTF-16 code units).
			const paragraph = text([1]);
			paragraph.insert(8, " 🎉");
			paragraph.delete(6, 2);
			// Italicise "Ti" in the heading and raise it to level 2.
			text([0]).format(0, 2, { italic: true });
			(block([0]).get("attrs") as Y.Map<unknown>).set("level", 2);
			// Link "quoted" inside the blockquote.
			text([3, 0]).format(0, 6, { link: { href: "https://yjs.dev" } });
			// Mention someone in bold at the end of the last paragraph.
			const end = text([5]);
			end.insertEmbed(end.length, { mention: { id: "js" } }, { bold: true });
		});
	},
	/** A nested block added from JavaScript. */
	nested() {
		const paragraph = new Y.Text();
		paragraph.insert(0, "from js", { code: true });
		const children = new Y.Array<Y.Map<unknown>>();
		children.push([newBlock("3a.1", "paragraph", paragraph)]);
		const list = block([2]).get("children") as Y.Array<Y.Map<unknown>>;
		list.push([newBlock("3a.0", "list_item", children)]);
	},
};

/** The document as ProseMirror JSON, the way the Rust model writes it. */
function toJson() {
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

type Op = {
	insert: string | Record<string, unknown>;
	attributes?: Record<string, unknown>;
};

/**
 * One delta operation as ProseMirror inline JSON. Text and embeds map to
 * nodes; formatting attributes become marks, skipping cleared ones, in the
 * schema's mark order, with mark attributes only when there are any.
 */
function inlineJson(op: Op) {
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
		.map(([type, value]) =>
			typeof value === "object" && value !== null &&
				Object.keys(value).length > 0
				? { type, attrs: value }
				: { type }
		);

	if (marks.length > 0) node.marks = marks;

	return node;
}

const decoder = new TextDecoder();
let buffered = "";

for await (const chunk of Deno.stdin.readable) {
	buffered += decoder.decode(chunk, { stream: true });
	let newline;

	while ((newline = buffered.indexOf("\n")) >= 0) {
		const line = buffered.slice(0, newline);
		buffered = buffered.slice(newline + 1);
		const message = JSON.parse(line);

		if (message.update) {
			Y.applyUpdate(doc, fromHex(message.update), "remote");
		} else if (message.edit) {
			edits[message.edit]();
		} else if (message.dump) {
			send({ json: toJson() });
		}

		send({ done: true });
	}
}
