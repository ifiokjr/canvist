/**
 * What the native side of `@canvist/lynx` provides: the `<canvist-editor>`
 * element and the `CanvistEditors` native module, typed for ReactLynx.
 *
 * @module
 */
import type { BaseEvent, StandardProps } from "@lynx-js/types";

/** What changed, after every change to the document or selection. */
export interface EditorChangeDetail {
	readonly document: boolean;
	readonly selection: boolean;
	readonly composition: boolean;
}

/** The element gained or lost focus. */
export interface EditorFocusDetail {
	readonly focused: boolean;
}

/** The element could not create its editor or load its content. */
export interface EditorErrorDetail {
	readonly code: string;
	readonly message: string;
}

/** The element's editor is ready for calls through `CanvistEditors`. */
export interface EditorReadyDetail {
	readonly editorId: string;
}

/** The attributes and events of `<canvist-editor>`. */
export interface CanvistEditorElementProps extends StandardProps {
	/** The key the `CanvistEditors` module finds this element's editor by. */
	"editor-id": string;
	/** The extensions to register, by name. Read when the element is created. */
	extensions: readonly string[];
	/** The format of `content`. Read when the element is created. */
	"content-format"?: string;
	/** The document to start with. Read when the element is created. */
	content?: string;
	/** The body text size before the user's text size setting; 0 is the platform's default. */
	"text-size"?: number;
	bindready?: (event: BaseEvent<"ready", EditorReadyDetail>) => void;
	bindchange?: (event: BaseEvent<"change", EditorChangeDetail>) => void;
	bindfocuschange?: (
		event: BaseEvent<"focuschange", EditorFocusDetail>,
	) => void;
	binderror?: (event: BaseEvent<"error", EditorErrorDetail>) => void;
}

/**
 * The `CanvistEditors` native module. Every method is synchronous and
 * answers with a JSON envelope: `{"value": ...}`, or `{"error": message}`
 * with `[canvist:<code>] <reason>` in the message, because Lynx reports
 * native exceptions to the host rather than to JavaScript.
 */
export interface CanvistEditorsModule {
	run(editorId: string, name: string, args: string): string;
	can(editorId: string, name: string, args: string): string;
	query(editorId: string, name: string, args: string): string;
	load(editorId: string, format: string, content: string): string;
	exportDocument(editorId: string, format: string): string;
	selection(editorId: string): string;
	select(editorId: string, range: string): string;
	selectAll(editorId: string): string;
	insertText(editorId: string, text: string): string;
	undo(editorId: string): string;
	redo(editorId: string): string;
	canUndo(editorId: string): string;
	canRedo(editorId: string): string;
}

declare module "@lynx-js/types" {
	interface IntrinsicElements {
		"canvist-editor": CanvistEditorElementProps;
	}

	interface NativeModules {
		/** Registered by the app with `@canvist/lynx`'s native setup. */
		CanvistEditors?: CanvistEditorsModule;
	}
}

declare module "react" {
	// ReactLynx reads intrinsic elements from React's JSX namespace.
	namespace JSX {
		interface IntrinsicElements {
			"canvist-editor": CanvistEditorElementProps;
		}
	}
}
