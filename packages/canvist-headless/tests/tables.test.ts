/**
 * Tables through `@canvist/headless/full`: the typed descriptor, typed
 * commands and the `tableCell` query, errors, TipTap JSON with merged
 * cells, and collaboration with a JavaScript Yjs peer. The shared cases in
 * `conformance/fixtures/tables.json` cover editing and the formats.
 *
 * @module
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import * as Y from "yjs";

import { createEngine, starterKit, table } from "../src/full.ts";

const withTable = [...starterKit, table] as const;

/**
 * What the commands below leave, as Markdown: HTML, since the merged and
 * split cell keeps both paragraphs, which a pipe table cannot hold.
 */
const MARKDOWN =
	"<table><tbody><tr><th><p>a</p><p>b</p></th><th><p></p></th><th><p></p></th></tr><tr><td><p></p></td><td><p></p></td><td><p></p></td></tr><tr><td><p></p></td><td><p></p></td><td><p></p></td></tr></tbody></table>";

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends
	<T>() => T extends B ? 1 : 2 ? true : false;

function assertType<Condition extends true>(_condition?: Condition): void {}

/** Every command that needs a table around the selection. */
const inTable = [
	"addColumnBefore",
	"addColumnAfter",
	"deleteColumn",
	"addRowBefore",
	"addRowAfter",
	"deleteRow",
	"deleteTable",
	"mergeCells",
	"splitCell",
	"mergeOrSplit",
	"toggleHeaderRow",
	"toggleHeaderColumn",
	"toggleHeaderCell",
	"goToNextCell",
	"goToPreviousCell",
	"fixTables",
] as const;

test("the table descriptor matches what Rust reports", async () => {
	using engine = await createEngine(withTable);
	const reported = (engine.metadata() as { name: string }[]).find((
		extension,
	) => extension.name === "table");

	assert.deepEqual(reported, JSON.parse(JSON.stringify(table)));
	assert.deepEqual(
		table.commands.map((command) => command.name).sort(),
		[...inTable, "insertTable", "setCellAttribute"].sort(),
	);
	assert.deepEqual(table.nodes.map((node) => node.name), [
		"table",
		"tableRow",
		"tableCell",
		"tableHeader",
	]);
});

test("table commands and the cell query are typed", async () => {
	using engine = await createEngine(withTable);
	using doc = engine.create();

	type Cell = ReturnType<typeof doc.query.tableCell>;
	assertType<
		Equal<
			Cell,
			{
				row: number;
				column: number;
				rowspan: number;
				colspan: number;
				header: boolean;
				rows: number;
				columns: number;
			} | null
		>
	>();

	for (const name of inTable) {
		assert.equal(doc.canRun(name), false, name);
	}

	assert.equal(doc.query.tableCell(), null);
	assert.equal(doc.can.insertTable({ rows: 2, cols: 3 }), true);
	assert.equal(
		doc.commands.insertTable({ rows: 2, cols: 3, withHeaderRow: false }),
		true,
	);
	assert.deepEqual(doc.query.tableCell(), {
		row: 0,
		column: 0,
		rowspan: 1,
		colspan: 1,
		header: false,
		rows: 2,
		columns: 3,
	});

	doc.typeText("a");
	assert.equal(doc.press("Tab"), true);
	doc.typeText("b");
	doc.select({ path: [0, 0, 0, 0], offset: 0 }, {
		path: [0, 0, 1, 0],
		offset: 0,
	});
	assert.equal(doc.can.mergeCells(), true);
	assert.equal(doc.commands.mergeCells(), true);
	assert.equal(doc.query.tableCell()?.colspan, 2);
	assert.equal(doc.commands.toggleHeaderRow(), true);
	assert.equal(doc.query.tableCell()?.header, true);
	assert.equal(
		doc.commands.setCellAttribute({ name: "align", value: "right" }),
		true,
	);
	assert.equal(
		doc.commands.setCellAttribute({ name: "align", value: null }),
		true,
	);
	assert.equal(doc.commands.splitCell(), true);
	assert.equal(doc.commands.addRowAfter(), true);
	assert.equal(doc.commands.addColumnBefore(), true);
	assert.equal(doc.query.tableCell()?.columns, 4);
	// Back into the new, empty column, and delete it again.
	assert.equal(doc.commands.goToPreviousCell(), true);
	assert.equal(doc.query.tableCell()?.column, 0);
	assert.equal(doc.commands.deleteColumn(), true);
	assert.equal(doc.commands.goToNextCell(), true);
	assert.equal(doc.query.tableCell()?.column, 1);
	assert.equal(doc.export("markdown"), MARKDOWN);

	assert.equal(doc.undo(), true);
	assert.equal(doc.redo(), true);
	assert.equal(doc.commands.deleteTable(), true);
	assert.equal(doc.query.tableCell(), null);
});

test("invalid table arguments fail with invalidArgs and change nothing", async () => {
	using engine = await createEngine(withTable);
	using doc = engine.create();
	doc.commands.insertTable({ rows: 1, cols: 1 });
	const before = doc.export("json");
	const invalid = { name: "CanvistError", code: "invalidArgs" };

	assert.throws(() => doc.commands.insertTable({ rows: 0 }), invalid);
	assert.throws(() => doc.commands.insertTable({ cols: 1001 }), invalid);
	assert.throws(
		// @ts-expect-error rows is a number
		() => doc.commands.insertTable({ rows: "2" }),
		invalid,
	);
	assert.throws(
		// @ts-expect-error withHeaderRow is a boolean
		() => doc.commands.insertTable({ withHeaderRow: 1 }),
		invalid,
	);
	assert.throws(
		// @ts-expect-error setCellAttribute needs a value, even null
		() => doc.commands.setCellAttribute({ name: "align" }),
		invalid,
	);
	assert.throws(
		// @ts-expect-error a value is a string or null
		() => doc.commands.setCellAttribute({ name: "align", value: 3 }),
		invalid,
	);
	assert.throws(
		// @ts-expect-error addRowAfter takes no arguments
		() => doc.commands.addRowAfter({ count: 2 }),
		invalid,
	);
	assert.equal(doc.export("json"), before);
});

test("an engine without the table extension has none of its names", async () => {
	using engine = await createEngine(starterKit);
	using doc = engine.create();

	assert.equal(Object.hasOwn(doc.commands, "insertTable"), false);
	assert.throws(
		// @ts-expect-error tables are not registered
		() => doc.run("insertTable"),
		{ name: "CanvistError", code: "unknownName" },
	);
	// @ts-expect-error tables are not registered
	doc.query.tableCell;
});

test("TipTap JSON with merged cells and widths reads and writes back unchanged", async () => {
	using engine = await createEngine(withTable);
	const cell = <T extends "tableCell" | "tableHeader">(
		type: T,
		text: string,
		attrs: {
			colspan: number;
			rowspan: number;
			colwidth: readonly number[] | null;
		},
	) => ({
		type,
		attrs,
		content: [{
			type: "paragraph",
			content: [{ type: "text", text }],
		}] as const,
	});
	const plain = { colspan: 1, rowspan: 1, colwidth: null };
	const json = {
		type: "doc",
		content: [{
			type: "table",
			content: [
				{
					type: "tableRow",
					content: [
						cell("tableHeader", "Wide", {
							colspan: 2,
							rowspan: 1,
							colwidth: [120, 0],
						}),
						cell("tableHeader", "C", plain),
					],
				},
				{
					type: "tableRow",
					content: [
						cell("tableCell", "Tall", {
							colspan: 1,
							rowspan: 2,
							colwidth: null,
						}),
						cell("tableCell", "b", plain),
						cell("tableCell", "c", plain),
					],
				},
				{
					type: "tableRow",
					content: [
						cell("tableCell", "e", plain),
						cell("tableCell", "f", plain),
					],
				},
			],
		}],
	} as const;
	using doc = engine.fromJson(json);

	assert.deepEqual(withoutIds(doc.toJson()), json);
	doc.select({ path: [0, 2, 1, 0], offset: 0 });
	assert.deepEqual(doc.query.tableCell(), {
		row: 2,
		column: 2,
		rowspan: 1,
		colspan: 1,
		header: false,
		rows: 3,
		columns: 3,
	});
	assert.equal(
		doc.export("html"),
		'<table><tbody><tr><th colspan="2" colwidth="120,0"><p>Wide</p></th><th><p>C</p></th></tr><tr><td rowspan="2"><p>Tall</p></td><td><p>b</p></td><td><p>c</p></td></tr><tr><td><p>e</p></td><td><p>f</p></td></tr></tbody></table><p></p>',
		"selecting adds the trailing paragraph",
	);
});

test("peers changing a table while others type converge, through a JavaScript Yjs peer", async () => {
	using engine = await createEngine(withTable);
	using first = engine.parse("markdown", "| a | b |\n| --- | --- |\n| c | d |");
	first.share(1);

	const relay = new Y.Doc();
	Y.applyUpdate(relay, first.encodeState());
	const blocks = relay.getArray<Y.Map<unknown>>("blocks");
	assert.equal(blocks.get(0)?.get("type"), "table");
	const rows = blocks.get(0)?.get("children") as Y.Array<Y.Map<unknown>>;
	assert.equal(rows.length, 2);
	assert.equal(
		(rows.get(0).get("children") as Y.Array<Y.Map<unknown>>).get(0).get("type"),
		"tableHeader",
	);
	using second = engine.fromYjs(Y.encodeStateAsUpdate(relay), 2);

	first.select({ path: [0, 0, 0, 0], offset: 0 });
	first.commands.addRowAfter();
	second.select({ path: [0, 1, 1, 0], offset: 1 });
	second.insertText("!");
	second.commands.setCellAttribute({ name: "align", value: "right" });

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
		"| a | b |\n| --- | --- |\n|  |  |\n| c | d! |",
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
