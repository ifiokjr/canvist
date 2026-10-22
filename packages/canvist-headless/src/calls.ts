/**
 * How typed commands and queries reach an engine: one method per
 * registered name, with arguments sent as JSON. The WebAssembly documents
 * and the native editors share it.
 *
 * @module
 */

type Call = (args?: unknown) => unknown;

/**
 * An object with one method per name, each calling `invoke`. The names come
 * from the same metadata as the type `Methods`, which is what makes the
 * object that type.
 */
export function calls<Methods>(
	names: readonly string[],
	invoke: (name: string, args: unknown) => unknown,
): Methods {
	const methods: Record<string, Call> = Object.fromEntries(
		names.map((name) => [name, (args?: unknown) => invoke(name, args)]),
	);
	return methods as Methods;
}

/** A command's or query's arguments as the engine reads them: JSON, or `null` for none. */
export function encode(args: unknown): string {
	return args === undefined ? "null" : JSON.stringify(args);
}
