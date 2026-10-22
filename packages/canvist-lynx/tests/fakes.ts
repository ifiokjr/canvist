/**
 * A stand-in for the `CanvistEditors` native module: it records every call
 * and answers with the JSON envelopes the real module sends.
 */
import type { CanvistEditorsModule } from "../src/native.ts";

/** What the fake module was asked, in order. */
export const calls: string[] = [];

/** Answers for queries, by name, as JSON. */
export const answers = new Map<string, string>();

/** A failure the next `run` call answers with, if set. */
export const failures: { run?: string } = {};

function reply(call: string, value: string): string {
	calls.push(call);
	return `{"value":${value}}`;
}

export const fakeModule: CanvistEditorsModule = {
	run: (id, name, args) => {
		if (failures.run !== undefined) {
			const error = failures.run;
			delete failures.run;
			calls.push(`run ${id} ${name} ${args}`);
			return JSON.stringify({ error });
		}

		return reply(`run ${id} ${name} ${args}`, "true");
	},
	can: (id, name, args) => reply(`can ${id} ${name} ${args}`, "false"),
	query: (id, name, args) =>
		reply(`query ${id} ${name} ${args}`, answers.get(name) ?? "null"),
	load: (id, format, content) =>
		reply(`load ${id} ${format} ${content}`, "true"),
	exportDocument: (id, format) => reply(`export ${id} ${format}`, '"Hello"'),
	selection: (id) =>
		reply(
			`selection ${id}`,
			'{"anchor":{"path":[0],"offset":1},"head":{"path":[0],"offset":3}}',
		),
	select: (id, range) => reply(`select ${id} ${range}`, "true"),
	selectAll: (id) => reply(`selectAll ${id}`, "true"),
	insertText: (id, text) => reply(`insertText ${id} ${text}`, "true"),
	undo: (id) => reply(`undo ${id}`, "true"),
	redo: (id) => reply(`redo ${id}`, "false"),
	canUndo: (id) => reply(`canUndo ${id}`, "true"),
	canRedo: (id) => reply(`canRedo ${id}`, "false"),
};
