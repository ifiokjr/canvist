/**
 * The bridge from the typed editor to one `<canvist-editor>`: synchronous
 * calls through the `CanvistEditors` native module, and focus through the
 * element's UI methods.
 *
 * @module
 */
import type { NativeBridge } from "@canvist/headless/native";
import type { NodesRef } from "@lynx-js/types";

import type { CanvistEditorsModule } from "./native.ts";

/** A native module reply: a value, or a failure in the bridge's error format. */
type Reply = { readonly value: unknown } | { readonly error: string };

/**
 * A bridge to the editor `editorId` names through `module`, focusing the
 * element `element` returns. Focus calls do nothing once the element is gone.
 */
export function nativeBridge(
	module: CanvistEditorsModule,
	editorId: string,
	element: () => NodesRef | null,
): NativeBridge {
	const invoke = (method: "focus" | "blur") => {
		element()?.invoke({ method }).exec();
	};

	return {
		run: (name, args) => boolean(module.run(editorId, name, args)),
		can: (name, args) => boolean(module.can(editorId, name, args)),
		query: (name, args) =>
			JSON.stringify(value(module.query(editorId, name, args))),
		load: (format, content) => {
			value(module.load(editorId, format, content));
		},
		export: (format) => string(module.exportDocument(editorId, format)),
		selection: () => JSON.stringify(value(module.selection(editorId))),
		select: (range) => {
			value(module.select(editorId, range));
		},
		selectAll: () => {
			value(module.selectAll(editorId));
		},
		insertText: (text) => boolean(module.insertText(editorId, text)),
		undo: () => boolean(module.undo(editorId)),
		redo: () => boolean(module.redo(editorId)),
		canUndo: () => boolean(module.canUndo(editorId)),
		canRedo: () => boolean(module.canRedo(editorId)),
		focus: () => invoke("focus"),
		blur: () => invoke("blur"),
	};
}

/** The value in `reply`, throwing its error instead when it has one. */
function value(reply: string): unknown {
	const parsed: unknown = JSON.parse(reply);

	if (!isReply(parsed)) {
		throw new Error(`[canvist:editor] the native module answered ${reply}`);
	}

	if ("error" in parsed) {
		throw new Error(parsed.error);
	}

	return parsed.value;
}

function boolean(reply: string): boolean {
	const answer = value(reply);

	if (typeof answer !== "boolean") {
		throw new Error(`[canvist:editor] expected a boolean, not ${reply}`);
	}

	return answer;
}

function string(reply: string): string {
	const answer = value(reply);

	if (typeof answer !== "string") {
		throw new Error(`[canvist:editor] expected a string, not ${reply}`);
	}

	return answer;
}

function isReply(reply: unknown): reply is Reply {
	if (typeof reply !== "object" || reply === null) {
		return false;
	}

	return "value" in reply ||
		("error" in reply && typeof reply.error === "string");
}
