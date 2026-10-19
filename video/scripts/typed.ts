/**
 * Generates TypeScript declarations from an editor's `metadata_json()`, the
 * typed-API metadata every extension contributes (architecture Decision 8),
 * so the film's "headless and typed" scene shows the compiler's real
 * response to an extension that was never registered.
 */

/** The snippet the scene types, then type-checks. */
export const SNIPPET = `import { createEditor } from "./canvist";

const editor = createEditor();

editor.commands.setHeading({ level: 2 });
editor.commands.toggleBold();
editor.commands.insertTable({ rows: 3 });
`;

type ValueType =
	| { readonly type: "bool" | "float" | "string" }
	| { readonly type: "int"; readonly min?: number; readonly max?: number }
	| { readonly type: "nullable"; readonly of: ValueType };

interface Arg {
	readonly name: string;
	readonly doc: string;
	readonly type: ValueType;
}

interface CommandMeta {
	readonly name: string;
	readonly doc: string;
	readonly args: readonly Arg[];
}

interface ExtensionMeta {
	readonly name: string;
	readonly commands: readonly CommandMeta[];
}

/** The largest integer range written as a union of literals. */
const LITERAL_RANGE = 16;

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function valueType(value: unknown): ValueType {
	if (!isRecord(value)) {
		throw new Error(`invalid value type: ${JSON.stringify(value)}`);
	}

	switch (value.type) {
		case "bool":
		case "float":
		case "string":
			return { type: value.type };
		case "int": {
			const min = typeof value.min === "number" ? value.min : undefined;
			const max = typeof value.max === "number" ? value.max : undefined;
			return {
				type: "int",
				...(min === undefined ? {} : { min }),
				...(max === undefined ? {} : { max }),
			};
		}
		case "nullable":
			return { type: "nullable", of: valueType(value.of) };
		default:
			throw new Error(`unknown value type: ${JSON.stringify(value)}`);
	}
}

function text(value: unknown, what: string): string {
	if (typeof value !== "string") {
		throw new Error(`${what} is not a string`);
	}

	return value;
}

function list(value: unknown, what: string): unknown[] {
	if (!Array.isArray(value)) {
		throw new Error(`${what} is not an array`);
	}

	return value;
}

function extensions(metadata: unknown): ExtensionMeta[] {
	return list(metadata, "metadata").map((extension) => {
		if (!isRecord(extension)) {
			throw new Error("an extension entry is not an object");
		}

		return {
			name: text(extension.name, "extension name"),
			commands: list(extension.commands, "commands").map((command) => {
				if (!isRecord(command)) {
					throw new Error("a command entry is not an object");
				}

				return {
					name: text(command.name, "command name"),
					doc: text(command.doc, "command doc"),
					args: list(command.args, "args").map((arg) => {
						if (!isRecord(arg)) {
							throw new Error("an argument entry is not an object");
						}

						return {
							name: text(arg.name, "argument name"),
							doc: text(arg.doc, "argument doc"),
							type: valueType(arg.type),
						};
					}),
				};
			}),
		};
	});
}

function typeScript(type: ValueType): string {
	switch (type.type) {
		case "bool":
			return "boolean";
		case "float":
			return "number";
		case "string":
			return "string";
		case "int": {
			const { min, max } = type;

			if (min === undefined || max === undefined || max - min > LITERAL_RANGE) {
				return "number";
			}

			return Array.from(
				{ length: max - min + 1 },
				(_, index) => String(min + index),
			).join(" | ");
		}
		case "nullable":
			return `${typeScript(type.of)} | null`;
	}
}

function signature(command: CommandMeta): string {
	const args = command.args.length === 0
		? ""
		: `args: { ${
			command.args.map((arg) => `${arg.name}: ${typeScript(arg.type)}`).join(
				"; ",
			)
		} }`;

	return `\t/** ${command.doc} */\n\t${command.name}(${args}): boolean;`;
}

/** Declarations for every command the registered extensions provide. */
export function declarations(
	metadata: unknown,
): { source: string; commands: number; extensions: number } {
	const registered = extensions(metadata);
	const commands = registered.flatMap((extension) => extension.commands);
	const source = [
		"/** Commands of the extensions this editor registered (generated). */",
		"export interface StarterKitCommands {",
		...commands.map(signature),
		"}",
		"",
		"export interface Editor {",
		"\treadonly commands: StarterKitCommands;",
		"}",
		"",
		"export declare function createEditor(): Editor;",
		"",
	].join("\n");

	return { source, commands: commands.length, extensions: registered.length };
}
