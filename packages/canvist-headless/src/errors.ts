/**
 * The error every canvist call throws, from WebAssembly or a native editor.
 *
 * @module
 */
import type { ErrorCode } from "./types.ts";

/** A failure inside canvist, with its stable code. */
export interface CanvistError extends Error {
	readonly name: "CanvistError";
	readonly code: ErrorCode;
}

/** Whether `error` came from canvist, narrowing it to read its `code`. */
export function isCanvistError(error: unknown): error is CanvistError {
	return error instanceof Error && error.name === "CanvistError" &&
		"code" in error;
}
