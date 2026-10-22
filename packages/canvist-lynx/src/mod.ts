/**
 * The canvist rich-text editor for Lynx.
 *
 * `CanvistEditor` shows the native iOS or Android editor as a
 * `<canvist-editor>` element: canvist's Rust engine, painted natively and
 * edited through the platform's own text input. Commands, queries, and
 * documents are typed from the extensions you register, with the same API
 * as `@canvist/web` and `@canvist/react-native`: calling a command you did
 * not register is a type error. Calls are synchronous.
 *
 * ```tsx
 * import { CanvistEditor, bold, heading } from "@canvist/lynx";
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
export type {
	CanvistEditorElementProps,
	CanvistEditorsModule,
	EditorChangeDetail,
	EditorErrorDetail,
	EditorFocusDetail,
	EditorReadyDetail,
} from "./native.ts";
