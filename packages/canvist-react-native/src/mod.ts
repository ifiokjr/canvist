/**
 * The canvist rich-text editor for React Native.
 *
 * `CanvistEditor` shows the native iOS or Android editor: canvist's Rust
 * engine, painted natively and edited through the platform's own text
 * input, not a WebView. Commands, queries, and documents are typed from the
 * extensions you register, with the same API as `@canvist/web`: calling a
 * command you did not register is a type error. Calls are synchronous.
 *
 * ```tsx
 * import { CanvistEditor, bold, heading } from "@canvist/react-native";
 *
 * <CanvistEditor
 * 	extensions={[bold, heading]}
 * 	content={{ format: "markdown", input: "# Notes" }}
 * 	onReady={(editor) => editor.commands.toggleHeading({ level: 2 })}
 * 	style={{ flex: 1 }}
 * />
 * ```
 *
 * @module
 */
export type {
	CanvistError,
	CommandName,
	CommandParams,
	Commands,
	Content,
	DocJson,
	ErrorCode,
	ExtensionDescriptor,
	Extensions,
	Format,
	Position,
	Queries,
	QueryName,
	QueryParams,
	QueryResult,
	Range,
	Registered,
	StarterKit,
} from "@canvist/headless";
export * from "@canvist/headless/extensions";
export { isCanvistError } from "@canvist/headless/native";
export type {
	EditorChangeEvent,
	EditorErrorEvent,
	EditorFocusEvent,
} from "./CanvistEditorViewNativeComponent.ts";
export {
	CanvistEditor,
	type CanvistEditorHandle,
	type CanvistEditorProps,
	CanvistView,
	type CanvistViewProps,
	type Editor,
	type EditorOptions,
	type NativeExtensions,
	useCanvistEditor,
	useEditorState,
} from "./editor.tsx";
