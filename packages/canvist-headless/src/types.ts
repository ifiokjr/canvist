/**
 * Types derived from extension metadata.
 *
 * Every extension descriptor is the metadata `canvist_editor` produces,
 * kept as a literal type. The types here read it, so a document's commands,
 * queries, argument types, and JSON node names follow from the extensions
 * an engine registers, with nothing written by hand per extension.
 * Descriptions are comments in `extensions.generated.ts`, so they never
 * ship in a browser bundle; `engine.metadata()` returns them as data.
 *
 * @module
 */
import type { core, starterKit } from "./extensions.generated.ts";

/** The type of an argument, attribute, or query result. */
export type ValueSpec =
	| { readonly type: "bool" }
	| { readonly type: "int"; readonly min?: number; readonly max?: number }
	| { readonly type: "float" }
	| { readonly type: "string"; readonly enum?: readonly string[] }
	| { readonly type: "nullable"; readonly of: ValueSpec }
	| { readonly type: "list"; readonly of: ValueSpec }
	| { readonly type: "record"; readonly fields: readonly FieldSpec[] };

/** A named, typed value. A `default` makes an argument optional. */
export interface FieldSpec {
	readonly name: string;
	/** What it means: in `engine.metadata()`, not in the generated descriptors. */
	readonly doc?: string;
	readonly type: ValueSpec;
	readonly default?: unknown;
}

/** A block, mark, or inline embed an extension adds to the schema. */
export interface NodeSpec {
	readonly kind: "block" | "mark" | "embed";
	readonly name: string;
	readonly attrs: readonly FieldSpec[];
}

/** A command an extension registers. */
export interface CommandSpec {
	readonly name: string;
	/** What it does: in `engine.metadata()`, not in the generated descriptors. */
	readonly doc?: string;
	readonly args: readonly FieldSpec[];
}

/** A query an extension registers. */
export interface QuerySpec {
	readonly name: string;
	/** What it answers: in `engine.metadata()`, not in the generated descriptors. */
	readonly doc?: string;
	readonly args: readonly FieldSpec[];
	readonly returns: ValueSpec;
}

/** What one extension adds: its metadata. */
export interface ExtensionDescriptor {
	readonly name: string;
	readonly nodes: readonly NodeSpec[];
	readonly commands: readonly CommandSpec[];
	readonly queries: readonly QuerySpec[];
}

/** A list of extensions. */
export type Extensions = readonly ExtensionDescriptor[];

/** Integer ranges this short become unions of their members. */
type MaxRangeLength = 64;

/** `Min | Min + 1 | ... | Max`, or `number` for long or open ranges. */
type IntRange<
	Min extends number,
	Max extends number,
	Counter extends readonly unknown[] = [],
	Started extends boolean = false,
> = Counter["length"] extends MaxRangeLength ? number
	: Counter["length"] extends Max ? Counter["length"]
	: Started extends true
		? Counter["length"] | IntRange<Min, Max, [...Counter, unknown], true>
	: Counter["length"] extends Min
		? Counter["length"] | IntRange<Min, Max, [...Counter, unknown], true>
	: IntRange<Min, Max, [...Counter, unknown], false>;

/** The TypeScript type of a value with spec `S`. */
export type ValueOf<S> = S extends { readonly type: "bool" } ? boolean
	: S extends {
		readonly type: "int";
		readonly min: infer Min extends number;
		readonly max: infer Max extends number;
	} ? IntRange<Min, Max>
	: S extends { readonly type: "int" } | { readonly type: "float" } ? number
	: S extends
		{ readonly type: "string"; readonly enum: readonly (infer Option)[] }
		? Option
	: S extends { readonly type: "string" } ? string
	: S extends { readonly type: "nullable"; readonly of: infer Inner }
		? ValueOf<Inner> | null
	: S extends { readonly type: "list"; readonly of: infer Inner }
		? readonly ValueOf<Inner>[]
	: S extends {
		readonly type: "record";
		readonly fields: infer Fields extends readonly FieldSpec[];
	} ? ArgsOf<Fields>
	: never;

type Simplify<T> = { [Key in keyof T]: T[Key] } & {};

/** The object of named arguments `Fields` describe. */
export type ArgsOf<Fields extends readonly FieldSpec[]> = Simplify<
	& {
		[
			Field in Fields[number] as Field extends { readonly default: unknown }
				? never
				: Field["name"]
		]: ValueOf<Field["type"]>;
	}
	& {
		[
			Field in Fields[number] as Field extends { readonly default: unknown }
				? Field["name"]
				: never
		]?: ValueOf<Field["type"]>;
	}
>;

/** The parameters of a command or query: none, optional, or required. */
export type ParamsOf<Fields extends readonly FieldSpec[]> = Fields extends
	readonly [] ? []
	: Exclude<Fields[number], { readonly default: unknown }> extends never
		? [args?: ArgsOf<Fields>]
	: [args: ArgsOf<Fields>];

type CommandsIn<E extends Extensions> = E[number]["commands"][number];
type QueriesIn<E extends Extensions> = E[number]["queries"][number];
type NodesIn<E extends Extensions> = E[number]["nodes"][number];

/** Every command the extensions `E` register, by name. Each returns whether it applied. */
export type Commands<E extends Extensions> = {
	readonly [Command in CommandsIn<E> as Command["name"]]: (
		...args: ParamsOf<Command["args"]>
	) => boolean;
};

/** Every query the extensions `E` register, by name. */
export type Queries<E extends Extensions> = {
	readonly [Query in QueriesIn<E> as Query["name"]]: (
		...args: ParamsOf<Query["args"]>
	) => ValueOf<Query["returns"]>;
};

/** The name of a command the extensions `E` register. */
export type CommandName<E extends Extensions> = CommandsIn<E>["name"];

/** The name of a query the extensions `E` register. */
export type QueryName<E extends Extensions> = QueriesIn<E>["name"];

/** The parameters of the command named `N`. */
export type CommandParams<E extends Extensions, N extends CommandName<E>> =
	N extends CommandName<E>
		? Commands<E>[N] extends (...args: infer Params) => boolean ? Params : never
		: never;

/** The parameters of the query named `N`. */
export type QueryParams<E extends Extensions, N extends QueryName<E>> =
	N extends QueryName<E>
		? Queries<E>[N] extends (...args: infer Params) => unknown ? Params : never
		: never;

/** The answer of the query named `N`. */
export type QueryResult<E extends Extensions, N extends QueryName<E>> =
	Queries<E>[N] extends (...args: never) => infer Result ? Result : never;

/** The names of the blocks, marks, or embeds `E` add. */
export type NodeName<E extends Extensions, Kind extends NodeSpec["kind"]> =
	Extract<NodesIn<E>, { readonly kind: Kind }>["name"];

type AttrsJson<Node extends NodeSpec> = Simplify<
	& {
		readonly [Field in Node["attrs"][number] as Field["name"]]?: ValueOf<
			Field["type"]
		>;
	}
	& { readonly id?: string }
>;

/** A mark in ProseMirror JSON, for the marks `E` add. */
export type MarkJson<E extends Extensions> = {
	[Node in Extract<NodesIn<E>, { readonly kind: "mark" }> as Node["name"]]: {
		readonly type: Node["name"];
		readonly attrs?: AttrsJson<Node>;
	};
}[NodeName<E, "mark">];

/** Inline content in ProseMirror JSON: text, or the embeds `E` add. */
export type InlineJson<E extends Extensions> =
	| {
		readonly type: "text";
		readonly text: string;
		readonly marks?: readonly MarkJson<E>[];
	}
	| {
		[Node in Extract<NodesIn<E>, { readonly kind: "embed" }> as Node["name"]]: {
			readonly type: Node["name"];
			readonly attrs?: AttrsJson<Node>;
			readonly marks?: readonly MarkJson<E>[];
		};
	}[NodeName<E, "embed">];

/** A block in ProseMirror JSON, for the blocks `E` add. */
export type BlockJson<E extends Extensions> = {
	[Node in Extract<NodesIn<E>, { readonly kind: "block" }> as Node["name"]]: {
		readonly type: Node["name"];
		readonly attrs?: AttrsJson<Node>;
		readonly content?: readonly (BlockJson<E> | InlineJson<E>)[];
	};
}[NodeName<E, "block">];

/** A ProseMirror or TipTap document using only what `E` adds. */
export interface DocJson<E extends Extensions> {
	readonly type: "doc";
	readonly content?: readonly BlockJson<E>[];
}

/** A format documents are read from and written to. */
export type Format = "json" | "html" | "markdown" | "text";

/** The extensions every engine has, plus the ones it was created with. */
export type Registered<E extends Extensions> = readonly [...typeof core, ...E];

/** The extensions of TipTap's StarterKit. */
export type StarterKit = typeof starterKit;

/** A document to start from: text in a format, or ProseMirror JSON. */
export type Content<E extends Extensions> =
	| { readonly format: Format; readonly input: string }
	| DocJson<Registered<E>>;

/**
 * The StarterKit theme's options. The browser editor (`@canvist/web`) and
 * the server renderer (`@canvist/headless/ssr`) take the same ones, and
 * must be given the same values for server-rendered HTML to hydrate
 * without moving.
 */
export interface ThemeOptions {
	/** Light or dark colours. Default `"light"`. */
	readonly scheme?: "light" | "dark";
	/** The CSS `font-family` list for body text and headings. */
	readonly bodyFamily?: string;
	/** The CSS `font-family` list for code. */
	readonly codeFamily?: string;
	/** The body font size in pixels. Default 16. */
	readonly size?: number;
	/** The body line height as a multiple of the font size. Default 1.5. */
	readonly lineHeight?: number;
	/** CSS `letter-spacing` of body text, in pixels. Default 0. */
	readonly letterSpacing?: number;
}

/** A point in a text block: a path from the root and a UTF-16 offset. */
export interface Position {
	readonly path: readonly number[];
	readonly offset: number;
}

/** A text selection, or a caret when the anchor and head are equal. */
export interface Range {
	readonly anchor: Position;
	readonly head: Position;
}

/** The stable name of each kind of failure, shared with Rust and Dart. */
export type ErrorCode =
	| "config"
	| "parse"
	| "schema"
	| "unknownName"
	| "invalidArgs"
	| "invalidPosition"
	| "invalidKey"
	| "editor"
	| "snapshot"
	| "notShared"
	| "alreadyShared"
	| "collab"
	| "render";
