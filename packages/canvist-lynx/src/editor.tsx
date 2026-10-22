/**
 * The editor component and hooks.
 *
 * @module
 */
import type { CanvistError, Content, StarterKit } from "@canvist/headless";
import { starterKit } from "@canvist/headless/extensions";
import { nativeCanvistError, NativeEditor } from "@canvist/headless/native";
import {
	type ReactNode,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore,
} from "@lynx-js/react";
import type { NodesRef, StandardProps } from "@lynx-js/types";

import { nativeBridge } from "./bridge.ts";
import type {
	CanvistEditorElementProps,
	EditorErrorDetail,
	EditorFocusDetail,
} from "./native.ts";

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
	readonly textSize?: number;
	/** Called when the element could not create its editor or load `content`. */
	readonly onError?: (error: CanvistError) => void;
}

/**
 * Props for {@link CanvistView}: what {@link useCanvistEditor} returns, plus
 * the element's own, such as `style` and `accessibility-label`.
 */
export type CanvistViewProps = CanvistEditorElementProps;

/** What {@link useCanvistEditor} returns. */
export interface CanvistEditorHandle<E extends NativeExtensions> {
	/** The editor, once the element has created it; `null` before that. */
	readonly editor: Editor<E> | null;
	/** Spread onto a {@link CanvistView}, alongside its style and accessibility props. */
	readonly props: CanvistViewProps;
}

/** The `<canvist-editor>` element. Give it a size, as it scrolls its own document. */
export function CanvistView(props: CanvistViewProps): ReactNode {
	return <canvist-editor {...props} />;
}

let editors = 0;

/** A key for one element's editor, unique in this JavaScript runtime. */
function nextEditorId(): string {
	editors += 1;
	return `canvist-${editors}`;
}

/**
 * Create an editor in the {@link CanvistView} you spread `props` onto.
 * `extensions` and `content` are read when the element is created; later
 * changes to them have no effect.
 *
 * ```tsx
 * function Notes() {
 * 	const { editor, props } = useCanvistEditor({ extensions: [bold, heading] });
 *
 * 	return (
 * 		<view>
 * 			<text bindtap={() => editor?.commands.toggleBold()}>Bold</text>
 * 			<CanvistView {...props} style={{ flex: 1 }} />
 * 		</view>
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
	const element = useRef<NodesRef>(null);
	// Events arrive through callbacks made once, so they read the editor here.
	const current = useRef<Editor<E> | null>(null);
	const onError = useLatest(options.onError);

	const bindready = useCallback(() => {
		const module = NativeModules.CanvistEditors;

		if (module === undefined) {
			reportError(
				nativeCanvistError(
					"config",
					"the CanvistEditors native module is not registered; see @canvist/lynx's setup",
				),
				onError.current,
			);
			return;
		}

		// Without extensions, `E` is the default: the whole StarterKit.
		const extensions = (initial.current.extensions ?? starterKit) as E;
		const created = new NativeEditor(
			nativeBridge(module, editorId, () => element.current),
			extensions,
		);
		current.current = created;
		setEditor(created);
	}, [editorId, onError]);

	const bindchange = useCallback(() => {
		current.current?.notifyChange();
	}, []);

	const bindfocuschange = useCallback(
		(event: { readonly detail: EditorFocusDetail }) => {
			current.current?.notifyFocus(event.detail.focused);
		},
		[],
	);

	const binderror = useCallback(
		(event: { readonly detail: EditorErrorDetail }) => {
			const { code, message } = event.detail;
			reportError(nativeCanvistError(code, message), onError.current);
		},
		[onError],
	);

	const { textSize } = options;
	const props = useMemo((): CanvistViewProps => {
		const { extensions = starterKit, content } = initial.current;

		return {
			ref: element,
			"editor-id": editorId,
			extensions: extensions.map((extension) => extension.name),
			...documentProps(content),
			...(textSize === undefined ? {} : { "text-size": textSize }),
			bindready,
			bindchange,
			bindfocuschange,
			binderror,
		};
	}, [editorId, textSize, bindready, bindchange, bindfocuschange, binderror]);

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

/** Props for {@link CanvistEditor}: the editor's options and the element's. */
export interface CanvistEditorProps<E extends NativeExtensions>
	extends EditorOptions<E>, StandardProps {
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
): ReactNode {
	const {
		extensions,
		content,
		textSize,
		onError,
		onReady,
		onUpdate,
		...element
	} = props;
	const { editor, props: editorProps } = useCanvistEditor<E>({
		...(extensions === undefined ? {} : { extensions }),
		...(content === undefined ? {} : { content }),
		...(textSize === undefined ? {} : { textSize }),
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

	return <CanvistView {...element} {...editorProps} />;
}

/** The element attributes for the starting document. */
function documentProps<E extends NativeExtensions>(
	content: Content<E> | undefined,
): Pick<CanvistEditorElementProps, "content" | "content-format"> {
	if (content === undefined) {
		return {};
	}

	return "format" in content
		? { "content-format": content.format, content: content.input }
		: { "content-format": "json", content: JSON.stringify(content) };
}

/** A ref that always holds the latest `value`. */
function useLatest<T>(value: T): { readonly current: T } {
	const ref = useRef(value);

	useEffect(() => {
		ref.current = value;
	});

	return ref;
}

function reportError(
	error: CanvistError,
	onError: ((error: CanvistError) => void) | undefined,
): void {
	if (onError === undefined) {
		console.error(error);
	} else {
		onError(error);
	}
}

function noop(): void {}
