/**
 * The editor component and hooks.
 *
 * @module
 */
import type { CanvistError, Content, StarterKit } from "@canvist/headless";
import { starterKit } from "@canvist/headless/extensions";
import { nativeCanvistError, NativeEditor } from "@canvist/headless/native";
import {
	type ReactElement,
	type Ref,
	type RefObject,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore,
} from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

import { type EditorViewInstance, nativeBridge } from "./bridge.ts";
import CanvistEditorView, {
	type EditorChangeEvent,
	type EditorErrorEvent,
	type EditorFocusEvent,
	type NativeProps,
} from "./CanvistEditorViewNativeComponent.ts";
import CanvistEditors from "./NativeCanvistEditors.ts";

/** The extensions the native editors compile in: TipTap's StarterKit, or some of it. */
export type NativeExtensions = readonly StarterKit[number][];

/**
 * A native editor whose commands, queries, and documents are typed from
 * the extensions `E`, with the same API as `@canvist/web`'s editor.
 */
export type Editor<E extends NativeExtensions> = NativeEditor<E>;

/** Options for {@link useCanvistEditor} and {@link CanvistEditor}. */
export interface EditorOptions<E extends NativeExtensions> {
	/** The extensions to register: every StarterKit extension by default. Read once. */
	readonly extensions?: E;
	/** The document to start with: one empty paragraph by default. Read once. */
	readonly content?: Content<E>;
	/** The body text size before the user's text size setting; the platform's body size by default. */
	readonly fontSize?: number;
	/** Called when the view could not create its editor or load `content`. */
	readonly onError?: (error: CanvistError) => void;
}

/** Props for {@link CanvistView}: what {@link useCanvistEditor} returns, plus the view's own. */
export interface CanvistViewProps extends NativeProps {
	readonly ref?: Ref<EditorViewInstance>;
}

/** What {@link useCanvistEditor} returns. */
export interface CanvistEditorHandle<E extends NativeExtensions> {
	/** The editor, once the view has created it; `null` before that. */
	readonly editor: Editor<E> | null;
	/** Spread onto a {@link CanvistView}, alongside its style and accessibility props. */
	readonly props: CanvistViewProps;
}

/**
 * The native editor view. Render it with the props {@link useCanvistEditor}
 * returns; give it a size, as it scrolls its own document.
 */
export const CanvistView = CanvistEditorView;

let editors = 0;

/** A key for one view's editor, unique in this JavaScript runtime. */
function nextEditorId(): string {
	editors += 1;
	return `canvist-${editors}`;
}

/**
 * Create an editor in the {@link CanvistView} you spread `props` onto.
 * `extensions` and `content` are read when the view is created; later
 * changes to them have no effect.
 *
 * ```tsx
 * function Notes() {
 * 	const { editor, props } = useCanvistEditor({ extensions: [bold, heading] });
 *
 * 	return (
 * 		<>
 * 			<Button title="Bold" onPress={() => editor?.commands.toggleBold()} />
 * 			<CanvistView {...props} style={{ flex: 1 }} />
 * 		</>
 * 	);
 * }
 * ```
 */
export function useCanvistEditor<
	const E extends NativeExtensions = StarterKit,
>(options: EditorOptions<E> = {}): CanvistEditorHandle<E> {
	const [editorId] = useState(nextEditorId);
	const [editor, setEditor] = useState<Editor<E> | null>(null);
	const initial = useRef(options);
	const view = useRef<EditorViewInstance>(null);
	// Events arrive through callbacks made once, so they read the editor here.
	const current = useRef<Editor<E> | null>(null);
	const onError = useLatest(options.onError);

	const onEditorReady = useCallback(() => {
		// Without extensions, `E` is the default: the whole StarterKit.
		const extensions = (initial.current.extensions ?? starterKit) as E;
		const created = new NativeEditor(
			nativeBridge(CanvistEditors, editorId, () => view.current),
			extensions,
		);
		current.current = created;
		setEditor(created);
	}, [editorId]);

	const onEditorChange = useCallback(
		(_event: NativeSyntheticEvent<EditorChangeEvent>) => {
			current.current?.notifyChange();
		},
		[],
	);

	const onEditorFocus = useCallback(
		(event: NativeSyntheticEvent<EditorFocusEvent>) => {
			current.current?.notifyFocus(event.nativeEvent.focused);
		},
		[],
	);

	const onEditorError = useCallback(
		(event: NativeSyntheticEvent<EditorErrorEvent>) => {
			const { code, message } = event.nativeEvent;
			const error = nativeCanvistError(code, message);

			if (onError.current === undefined) {
				console.error(error);
			} else {
				onError.current(error);
			}
		},
		[onError],
	);

	const { fontSize } = options;
	const props = useMemo((): CanvistViewProps => {
		const { extensions = starterKit, content } = initial.current;

		return {
			ref: view,
			editorId,
			extensions: extensions.map((extension) => extension.name),
			...documentProps(content),
			...(fontSize === undefined ? {} : { fontSize }),
			onEditorReady,
			onEditorChange,
			onEditorFocus,
			onEditorError,
		};
	}, [
		editorId,
		fontSize,
		onEditorReady,
		onEditorChange,
		onEditorFocus,
		onEditorError,
	]);

	return { editor, props };
}

/**
 * Read `select(editor)` and re-render whenever it changes: after any edit,
 * selection change, or change of focus. `select` must return a primitive
 * or a value that stays the same object while nothing changed, as with
 * `useSyncExternalStore`. Until the editor exists, the value is `fallback`.
 */
export function useEditorState<E extends NativeExtensions, T>(
	editor: Editor<E> | null,
	select: (editor: Editor<E>) => T,
	fallback: T,
): T {
	const subscribe = useCallback(
		(listener: () => void) => editor?.subscribe(listener) ?? noop,
		[editor],
	);

	return useSyncExternalStore(
		subscribe,
		() => editor === null ? fallback : select(editor),
		() => fallback,
	);
}

/** Props for {@link CanvistEditor}: the editor's options and the view's. */
export interface CanvistEditorProps<E extends NativeExtensions>
	extends EditorOptions<E>, ViewProps {
	/** Called once the editor is ready. */
	readonly onReady?: (editor: Editor<E>) => void;
	/** Called after every change to the document, the selection, or focus. */
	readonly onUpdate?: (editor: Editor<E>) => void;
}

/**
 * A native canvist editor: the iOS or Android `CanvistView`. Give it a
 * size, as it scrolls its own document.
 *
 * ```tsx
 * <CanvistEditor
 * 	content={{ format: "markdown", input: "# Notes" }}
 * 	onUpdate={(editor) => save(editor.export("markdown"))}
 * 	style={{ flex: 1 }}
 * />
 * ```
 */
export function CanvistEditor<const E extends NativeExtensions = StarterKit>(
	props: CanvistEditorProps<E>,
): ReactElement {
	const { extensions, content, fontSize, onError, onReady, onUpdate, ...view } =
		props;
	const { editor, props: editorProps } = useCanvistEditor<E>({
		...(extensions === undefined ? {} : { extensions }),
		...(content === undefined ? {} : { content }),
		...(fontSize === undefined ? {} : { fontSize }),
		...(onError === undefined ? {} : { onError }),
	});
	const callbacks = useLatest({ onReady, onUpdate });

	useEffect(() => {
		if (editor === null) {
			return;
		}

		callbacks.current.onReady?.(editor);

		return editor.subscribe(() => callbacks.current.onUpdate?.(editor));
	}, [editor, callbacks]);

	return <CanvistView {...view} {...editorProps} />;
}

/** The view props for the starting document. */
function documentProps<E extends NativeExtensions>(
	content: Content<E> | undefined,
): Pick<NativeProps, "content" | "contentFormat"> {
	if (content === undefined) {
		return {};
	}

	return "format" in content
		? { contentFormat: content.format, content: content.input }
		: { contentFormat: "json", content: JSON.stringify(content) };
}

/** A ref that always holds the latest `value`. */
function useLatest<T>(value: T): RefObject<T> {
	const ref = useRef(value);

	useEffect(() => {
		ref.current = value;
	});

	return ref;
}

function noop(): void {}
