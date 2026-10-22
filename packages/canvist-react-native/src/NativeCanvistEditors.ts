/**
 * The TurboModule spec for calls into a view's editor. Every method is
 * synchronous on the JavaScript thread, so commands and queries answer at
 * once, as they do on the web. Methods return a value even when there is
 * nothing to say, because React Native runs `void` methods asynchronously.
 *
 * Failures throw with `[canvist:<code>] <reason>` in the message.
 *
 * @module
 */
import type { TurboModule } from "react-native";
import { TurboModuleRegistry } from "react-native";

export interface Spec extends TurboModule {
	run(editorId: string, name: string, args: string): boolean;
	can(editorId: string, name: string, args: string): boolean;
	query(editorId: string, name: string, args: string): string;
	/** Returns `true`. */
	load(editorId: string, format: string, content: string): boolean;
	exportDocument(editorId: string, format: string): string;
	selection(editorId: string): string;
	/** Returns whether the selection moved. */
	select(editorId: string, range: string): boolean;
	/** Returns whether the selection moved. */
	selectAll(editorId: string): boolean;
	insertText(editorId: string, text: string): boolean;
	undo(editorId: string): boolean;
	redo(editorId: string): boolean;
	canUndo(editorId: string): boolean;
	canRedo(editorId: string): boolean;
}

export default TurboModuleRegistry.getEnforcing<Spec>("CanvistEditors");
