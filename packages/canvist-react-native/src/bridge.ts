/**
 * The bridge from the typed editor to one native view: synchronous calls
 * through the `CanvistEditors` TurboModule, and focus through the view's
 * commands.
 *
 * @module
 */
import type { NativeBridge } from "@canvist/headless/native";
import type * as React from "react";

import {
	type CanvistEditorViewType,
	Commands,
} from "./CanvistEditorViewNativeComponent.ts";
import type { Spec } from "./NativeCanvistEditors.ts";

/** The mounted native view, as React refers to it. */
export type EditorViewInstance = React.ElementRef<CanvistEditorViewType>;

/**
 * A bridge to the editor `editorId` names through `module`, focusing the
 * view `view` returns. Focus calls do nothing once the view is gone.
 */
export function nativeBridge(
	module: Spec,
	editorId: string,
	view: () => EditorViewInstance | null,
): NativeBridge {
	return {
		run: (name, args) => module.run(editorId, name, args),
		can: (name, args) => module.can(editorId, name, args),
		query: (name, args) => module.query(editorId, name, args),
		load: (format, content) => {
			module.load(editorId, format, content);
		},
		export: (format) => module.exportDocument(editorId, format),
		selection: () => module.selection(editorId),
		select: (range) => {
			module.select(editorId, range);
		},
		selectAll: () => {
			module.selectAll(editorId);
		},
		insertText: (text) => module.insertText(editorId, text),
		undo: () => module.undo(editorId),
		redo: () => module.redo(editorId),
		canUndo: () => module.canUndo(editorId),
		canRedo: () => module.canRedo(editorId),
		focus: () => {
			const instance = view();

			if (instance !== null) {
				Commands.focus(instance);
			}
		},
		blur: () => {
			const instance = view();

			if (instance !== null) {
				Commands.blur(instance);
			}
		},
	};
}
