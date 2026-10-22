/**
 * Stand-ins for the native side: a `CanvistEditors` module that records
 * every call and answers from a table, and the view's commands.
 */
import { jest } from "@jest/globals";

import type { Spec } from "../src/NativeCanvistEditors.ts";

/** What the fake module was asked, in order. */
export const calls: string[] = [];

/** Answers for the fake module's queries, by name. */
export const answers = new Map<string, string>();

function record<T>(call: string, value: T): T {
	calls.push(call);
	return value;
}

export const fakeModule: Spec = {
	run: (id, name, args) => record(`run ${id} ${name} ${args}`, true),
	can: (id, name, args) => record(`can ${id} ${name} ${args}`, false),
	query: (id, name, args) =>
		record(`query ${id} ${name} ${args}`, answers.get(name) ?? "null"),
	load: (id, format, content) =>
		record(`load ${id} ${format} ${content}`, true),
	exportDocument: (id, format) => record(`export ${id} ${format}`, "Hello"),
	selection: (id) =>
		record(
			`selection ${id}`,
			'{"anchor":{"path":[0],"offset":1},"head":{"path":[0],"offset":3}}',
		),
	select: (id, range) => record(`select ${id} ${range}`, true),
	selectAll: (id) => record(`selectAll ${id}`, true),
	insertText: (id, text) => record(`insertText ${id} ${text}`, true),
	undo: (id) => record(`undo ${id}`, true),
	redo: (id) => record(`redo ${id}`, false),
	canUndo: (id) => record(`canUndo ${id}`, true),
	canRedo: (id) => record(`canRedo ${id}`, false),
};

export const fakeCommands = { focus: jest.fn(), blur: jest.fn() };
