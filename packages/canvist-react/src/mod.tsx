/**
 * React bindings for the canvist canvas editor.
 *
 * `CanvistEditor` renders an editor; `useCanvistEditor` gives you the
 * editor itself, and `useEditorState` re-renders a component when
 * something it reads from the editor changes. Commands, queries, and
 * plugin APIs are typed from the extensions and plugins you pass, as in
 * `@canvist/web`.
 *
 * ```tsx
 * function Notes() {
 * 	const { ref, editor } = useCanvistEditor({
 * 		content: { format: "markdown", input: "Hello **world**" },
 * 	});
 * 	const bold = useEditorState(editor, (current) => current.can.toggleBold(), false);
 *
 * 	return (
 * 		<>
 * 			<button disabled={!bold} onClick={() => editor?.commands.toggleBold()}>Bold</button>
 * 			<div ref={ref} />
 * 		</>
 * 	);
 * }
 * ```
 *
 * @module
 */
import {
	createEditor,
	type Editor,
	type EditorOptions,
	type Extensions,
	type Plugins,
	type StarterKit,
} from "@canvist/web";
import {
	type CSSProperties,
	type ReactElement,
	type RefCallback,
	useCallback,
	useEffect,
	useRef,
	useState,
	useSyncExternalStore,
} from "react";

/** What {@link useCanvistEditor} returns. */
export interface CanvistEditorHandle<E extends Extensions, P extends Plugins> {
	/** Attach to the element the editor renders into. */
	readonly ref: RefCallback<HTMLElement>;
	/** The editor, once it has loaded; `null` before that and after unmounting. */
	readonly editor: Editor<E, P> | null;
}

/**
 * Create an editor in the element `ref` is attached to, and destroy it
 * when the element goes away. `options` are read when the editor is
 * created; later changes to them have no effect.
 */
export function useCanvistEditor<
	const E extends Extensions = StarterKit,
	const P extends Plugins = [],
>(options: EditorOptions<E, P> = {}): CanvistEditorHandle<E, P> {
	const [element, setElement] = useState<HTMLElement | null>(null);
	const [editor, setEditor] = useState<Editor<E, P> | null>(null);
	const initial = useRef(options);

	useEffect(() => {
		if (element === null) {
			return;
		}

		const mount: Mount<E, P> = { live: true, editor: null };
		void create(element, initial.current, mount, setEditor);

		return () => {
			mount.live = false;
			mount.editor?.destroy();
			setEditor(null);
		};
	}, [element]);

	return { ref: setElement, editor };
}

/**
 * Read `select(editor)` and re-render whenever it changes, after any edit
 * or selection change. `select` must return a primitive or a value that
 * stays the same object while nothing changed, as with
 * `useSyncExternalStore`. Until the editor exists, the value is
 * `fallback`.
 */
export function useEditorState<E extends Extensions, P extends Plugins, T>(
	editor: Editor<E, P> | null,
	select: (editor: Editor<E, P>) => T,
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

/** Props for {@link CanvistEditor}: the editor's options, read once, and the element's. */
export interface CanvistEditorProps<E extends Extensions, P extends Plugins>
	extends EditorOptions<E, P> {
	readonly className?: string;
	readonly style?: CSSProperties;
	/**
	 * Server-rendered HTML (`.cv-doc` markup from canvist's renderer) to
	 * show until the editor takes over. The editor reads the document from
	 * it when `content` is not given.
	 */
	readonly html?: string;
	/** Called once the editor has loaded. */
	readonly onReady?: (editor: Editor<E, P>) => void;
	/** Called after every change to the document or selection. */
	readonly onUpdate?: (editor: Editor<E, P>) => void;
}

/** A canvist editor. Give it a width; without a height it grows with its document. */
export function CanvistEditor<
	const E extends Extensions = StarterKit,
	const P extends Plugins = [],
>(props: CanvistEditorProps<E, P>): ReactElement {
	const { className, style, html, onReady, onUpdate, ...options } = props;
	const { ref, editor } = useCanvistEditor<E, P>(options);
	const callbacks = useRef({ onReady, onUpdate });

	useEffect(() => {
		callbacks.current = { onReady, onUpdate };
	});

	useEffect(() => {
		if (editor === null) {
			return;
		}

		callbacks.current.onReady?.(editor);

		return editor.subscribe(() => callbacks.current.onUpdate?.(editor));
	}, [editor]);

	return (
		<div
			ref={ref}
			className={className}
			style={style}
			// Set once: React leaves the editor's elements alone afterwards.
			dangerouslySetInnerHTML={html === undefined
				? undefined
				: { __html: html }}
		/>
	);
}

/** One effect's editor: whether the effect is still mounted, and what it created. */
interface Mount<E extends Extensions, P extends Plugins> {
	live: boolean;
	editor: Editor<E, P> | null;
}

/**
 * Create the editor for `mount`. An effect cleaned up while the module
 * loaded (as React's Strict Mode does once) destroys what arrives late.
 */
async function create<E extends Extensions, P extends Plugins>(
	element: HTMLElement,
	options: EditorOptions<E, P>,
	mount: Mount<E, P>,
	created: (editor: Editor<E, P>) => void,
): Promise<void> {
	try {
		const editor = await createEditor(element, options);

		if (!mount.live) {
			editor.destroy();
			return;
		}

		mount.editor = editor;
		created(editor);
	} catch (error) {
		if (mount.live) {
			reportError(error);
		}
	}
}

function noop(): void {}

/** DELIBERATE REGRESSION, REVERT ME: proves `bench / bundle size` fails. */
export const deliberateBloat = "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";
