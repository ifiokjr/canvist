/**
 * Runs the language-neutral fixtures in `conformance/fixtures` against the
 * TypeScript API, under `node --test` and `deno test` alike. See
 * `conformance/readme.md` for the format.
 *
 * @module
 */
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";

import {
	allExtensions,
	createEngine,
	type Doc,
	type Engine,
	isCanvistError,
	starterKit,
} from "../src/mod.ts";
import type {
	CommandName,
	Format,
	Position,
	QueryName,
	Registered,
} from "../src/mod.ts";

type Json = null | boolean | number | string | readonly Json[] | {
	readonly [key: string]: Json;
};
type Fields = { readonly [key: string]: Json };
/**
 * Cases are typed as if every extension were registered: a fixture only
 * names commands its case registers, and Rust rejects any other name.
 */
type Every = typeof allExtensions;
type Kit = Registered<Every>;
type KitDoc = Doc<Every>;

const FIXTURES = new URL("../../../conformance/fixtures/", import.meta.url);
/** The optional features this binding has. */
const FEATURES = ["collab", "extensions"];

/** The StarterKit plus the `extensions` a case lists, by name. */
async function caseEngine(names: readonly string[]): Promise<Engine<Every>> {
	const extra = allExtensions.filter((extension) =>
		names.includes(extension.name)
	);
	const engine = await createEngine([...starterKit, ...extra]);

	return engine as unknown as Engine<Every>;
}

const files = readdirSync(FIXTURES).filter((name) => name.endsWith(".json"))
	.sort();
const fixtures = files.map((file) => {
	const fixture = JSON.parse(readFileSync(new URL(file, FIXTURES), "utf8")) as {
		cases: Fields[];
	};

	return { file, cases: fixture.cases };
});
const extensionLists = (cases: readonly Fields[]) =>
	cases.map((testCase) => (testCase.extensions ?? []) as string[]);

/** Engines by the extension list they were created with, all created before
 * any test runs, so registering tests never awaits. */
const engines = new Map<string, Engine<Every>>();

for (const names of fixtures.flatMap(({ cases }) => extensionLists(cases))) {
	if (!engines.has(names.join(","))) {
		engines.set(names.join(","), await caseEngine(names));
	}
}

for (const { file, cases } of fixtures) {
	for (const testCase of cases) {
		const requires = (testCase.requires ?? []) as string[];
		const skip = !requires.every((feature) => FEATURES.includes(feature));
		const names = (testCase.extensions ?? []) as string[];
		const engine = engines.get(names.join(",")) as Engine<Every>;
		test(
			`${file}: ${String(testCase.name)}`,
			{ skip },
			() => runCase(engine, testCase),
		);
	}
}

function runCase(engine: Engine<Every>, testCase: Fields): void {
	const input = (testCase.input ?? {}) as Fields;

	if (typeof input.error === "string") {
		expectError(() => load(engine, input), input.error);

		return;
	}

	let doc = load(engine, input);

	for (const step of (testCase.steps ?? []) as Fields[]) {
		doc = runStep(engine, doc, step);
	}

	check(doc, (testCase.expect ?? {}) as Fields);
}

function load(engine: Engine<Every>, input: Fields): KitDoc {
	switch (input.format) {
		case undefined:
			return engine.create();
		case "json":
			return engine.parse("json", JSON.stringify(input.content));
		case "yjs":
			return engine.fromYjs(base64(input.content), Number(input.clientId));
		default:
			return engine.parse(input.format as Format, String(input.content));
	}
}

/** Run a step, returning the document later steps use. */
function runStep(engine: Engine<Every>, doc: KitDoc, step: Fields): KitDoc {
	if (typeof step.error === "string") {
		const before = doc.export("json");
		expectError(() => apply(doc, step), step.error);
		assert.equal(doc.export("json"), before, "a failed step changes nothing");

		return doc;
	}

	switch (step.op) {
		case "expect":
			check(doc, step);

			return doc;
		case "snapshotRoundTrip":
			return engine.restore(JSON.stringify(JSON.parse(doc.snapshot())));
		case "rejoin":
			return engine.fromYjs(doc.encodeState(), Number(step.clientId));
	}

	const outcome = apply(doc, step);
	const expected = expectedOutcome(step);

	if (expected !== undefined) {
		assert.deepEqual(outcome, expected, `${String(step.op)} outcome`);
	}

	return doc;
}

/** Where a step declares the outcome it expects. */
const OUTCOMES: Record<string, (step: Fields) => Json | undefined> = {
	receiveSync: (
		step,
	) => [step.reply ?? null, step.broadcast ?? null, step.changed ?? null],
	validate: (step) => [step.rejectedBlocks ?? null, step.waiting ?? null],
	takeUpdates: (step) => step.count,
	can: (step) => step.expect,
	query: (step) => step.expect,
	applyUpdate: (step) => step.changed,
};

function expectedOutcome(step: Fields): Json | undefined {
	const outcome = OUTCOMES[String(step.op)];

	return outcome === undefined ? step.applied : outcome(step);
}

type Operation = (doc: KitDoc, step: Fields) => Json;

const command = (step: Fields) => String(step.command) as CommandName<Kit>;
const args = (step: Fields) => step.args as Record<string, unknown> | undefined;
const text = (step: Fields) => String(step.text ?? "");

/** Every operation, each returning what it reports. */
const OPERATIONS: Record<string, Operation> = {
	select: (doc, step) => {
		const anchor = position(step.anchor);
		doc.select(anchor, step.head === undefined ? anchor : position(step.head));

		return true;
	},
	selectAll: (doc) => {
		doc.selectAll();

		return true;
	},
	insertText: (doc, step) => doc.insertText(text(step)),
	typeText: (doc, step) => doc.typeText(text(step)),
	deleteSelection: (doc) => doc.deleteSelection(),
	press: (doc, step) => doc.press(String(step.key)),
	paste: (doc, step) => doc.paste(step.format as Format, String(step.content)),
	run: (doc, step) => doc.run(command(step), args(step) as never),
	can: (doc, step) => doc.canRun(command(step), args(step) as never),
	query: (doc, step) => {
		assert.equal(args(step), undefined, "fixture queries take no arguments");

		return doc.ask(String(step.query) as QueryName<Kit>) as Json;
	},
	undo: (doc) => doc.undo(),
	redo: (doc) => doc.redo(),
	share: (doc, step) => {
		doc.share(Number(step.clientId));

		return true;
	},
	applyUpdate: (doc, step) => doc.applyUpdate(base64(step.update)),
	takeUpdates: (doc) => doc.takeUpdates().length,
	validate: (doc) => {
		const validation = doc.validate();

		return [validation.rejectedBlocks, validation.waiting];
	},
	receiveSync: (doc, step) => {
		const sync = doc.receiveSync(base64(step.message));

		return [sync.reply.length, sync.broadcast.length, sync.changed];
	},
};

function apply(doc: KitDoc, step: Fields): Json {
	const operation = OPERATIONS[String(step.op)];
	assert.ok(operation, `unknown op ${String(step.op)}`);

	return operation(doc, step);
}

function expectError(run: () => unknown, code: Json): void {
	assert.throws(run, (error: unknown) => {
		assert.ok(
			isCanvistError(error),
			`expected a CanvistError, got ${String(error)}`,
		);
		assert.equal(error.code, code);
		return true;
	});
}

function check(doc: KitDoc, expect: Fields): void {
	for (const format of ["html", "markdown", "text"] as const) {
		if (format in expect) {
			assert.equal(doc.export(format), expect[format], format);
		}
	}

	if ("json" in expect) {
		assert.deepEqual(
			stripIds(JSON.parse(doc.export("json")) as Json),
			stripIds(expect.json ?? null),
			"json",
		);
	}

	if ("selection" in expect) {
		assert.deepEqual(doc.selection, expect.selection, "selection");
	}
}

/** Remove `id` from every `attrs` object, then every `attrs` left empty. */
function stripIds(json: Json): Json {
	if (Array.isArray(json)) {
		return json.map(stripIds);
	}

	if (json === null || typeof json !== "object") {
		return json;
	}

	const fields: Record<string, Json> = {};

	for (const [key, value] of Object.entries(json as Fields)) {
		if (key !== "attrs") {
			fields[key] = stripIds(value);
			continue;
		}

		const { id: _id, ...attrs } = value as Fields;

		if (Object.keys(attrs).length > 0) {
			fields.attrs = attrs;
		}
	}

	return fields;
}

function position(json: Json | undefined): Position {
	const fields = json as Fields;
	return { path: fields.path as number[], offset: fields.offset as number };
}

function base64(json: Json | undefined): Uint8Array {
	return Uint8Array.from(
		atob(String(json)),
		(character) => character.charCodeAt(0),
	);
}
