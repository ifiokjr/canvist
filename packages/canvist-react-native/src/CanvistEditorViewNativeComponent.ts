/**
 * The Fabric component spec for the native editor view: React Native's
 * codegen reads this file to generate the iOS and Android glue, so it keeps
 * to the shapes codegen understands.
 *
 * @module
 */
import type * as React from "react";
import type { CodegenTypes, HostComponent, ViewProps } from "react-native";
import { codegenNativeCommands, codegenNativeComponent } from "react-native";

/** What changed, after every change to the document or selection. */
export type EditorChangeEvent = Readonly<{
	document: boolean;
	selection: boolean;
	composition: boolean;
}>;

/** The view gained or lost focus. */
export type EditorFocusEvent = Readonly<{
	focused: boolean;
}>;

/** The view could not create or load its editor. */
export type EditorErrorEvent = Readonly<{
	code: string;
	message: string;
}>;

/** The view is ready for calls through the `CanvistEditors` module. */
export type EditorReadyEvent = Readonly<{
	editorId: string;
}>;

export interface NativeProps extends ViewProps {
	/** The key the `CanvistEditors` module finds this view's editor by. */
	editorId: string;
	/** The extensions to register, by name. Read when the view is created. */
	extensions: ReadonlyArray<string>;
	/** The format of `content`. Read when the view is created. */
	contentFormat?: string;
	/** The document to start with. Read when the view is created. */
	content?: string;
	/** The body text size before the user's text size setting; 0 is the platform's default. */
	fontSize?: CodegenTypes.WithDefault<CodegenTypes.Float, 0>;
	onEditorReady?: CodegenTypes.DirectEventHandler<EditorReadyEvent>;
	onEditorChange?: CodegenTypes.DirectEventHandler<EditorChangeEvent>;
	onEditorFocus?: CodegenTypes.DirectEventHandler<EditorFocusEvent>;
	onEditorError?: CodegenTypes.DirectEventHandler<EditorErrorEvent>;
}

export type CanvistEditorViewType = HostComponent<NativeProps>;

interface NativeCommands {
	focus: (viewRef: React.ElementRef<CanvistEditorViewType>) => void;
	blur: (viewRef: React.ElementRef<CanvistEditorViewType>) => void;
}

export const Commands: NativeCommands = codegenNativeCommands<NativeCommands>({
	supportedCommands: ["focus", "blur"],
});

export default codegenNativeComponent<NativeProps>("CanvistEditorView");
