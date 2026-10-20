/**
 * React bindings for the canvist canvas editor.
 *
 * `CanvistEditor` renders an editor; `useCanvistEditor` gives you the
 * editor itself, and `useEditorState` re-renders a component when
 * something it reads from the editor changes. `BubbleMenu` and
 * `FloatingMenu` keep their children next to the selection. Commands, queries, and
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
	bubbleMenu,
	floatingMenu,
	type Menu,
	type MenuOptions,
} from "@canvist/web/menus";
import {
	type CSSProperties,
	type ReactElement,
	type ReactNode,
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

/**
 * Props for {@link CanvistEditor}: the editor's options, read once except
 * `editable` and `placeholder`, which follow the props, and the element's.
 */
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
	const { editable = true, placeholder = "" } = options;

	useEffect(() => {
		editor?.setEditable(editable);
	}, [editor, editable]);

	useEffect(() => {
		editor?.setPlaceholder(placeholder);
	}, [editor, placeholder]);

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

/** Props for {@link BubbleMenu} and {@link FloatingMenu}. */
export interface MenuProps<E extends Extensions, P extends Plugins>
	extends MenuOptions<E, P> {
	/** The editor to follow; the menu stays hidden while it is `null`. */
	readonly editor: Editor<E, P> | null;
	readonly className?: string;
	readonly style?: CSSProperties;
	readonly children?: ReactNode;
}

/**
 * A menu over selected text, as `bubbleMenu` from `@canvist/web/menus`
 * places it. Pressing inside it keeps the editor's focus and selection.
 */
export function BubbleMenu<E extends Extensions, P extends Plugins>(
	props: MenuProps<E, P>,
): ReactElement {
	return <FollowingMenu {...props} attach={bubbleMenu} />;
}

/**
 * A menu beside the caret on an empty line, as `floatingMenu` from
 * `@canvist/web/menus` places it.
 */
export function FloatingMenu<E extends Extensions, P extends Plugins>(
	props: MenuProps<E, P>,
): ReactElement {
	return <FollowingMenu {...props} attach={floatingMenu} />;
}

type Attach = <E extends Extensions, P extends Plugins>(
	editor: Editor<E, P>,
	element: HTMLElement,
	options: MenuOptions<E, P>,
) => Menu;

function FollowingMenu<E extends Extensions, P extends Plugins>(
	props: MenuProps<E, P> & { readonly attach: Attach },
): ReactElement {
	const { editor, attach, className, style, children, ...options } = props;
	const element = useRef<HTMLDivElement>(null);
	const menu = useRef<Menu | null>(null);
	// The latest rule, without re-attaching each time a new closure renders.
	const shouldShow = useRef(options.shouldShow);
	shouldShow.current = options.shouldShow;
	const custom = options.shouldShow !== undefined;
	const { placement, offset, padding } = options;

	useEffect(() => {
		if (editor === null || element.current === null) {
			return;
		}

		const attached = attach(editor, element.current, {
			...(placement === undefined ? {} : { placement }),
			...(offset === undefined ? {} : { offset }),
			...(padding === undefined ? {} : { padding }),
			...(custom
				? { shouldShow: (current) => shouldShow.current?.(current) ?? false }
				: {}),
		});
		menu.current = attached;

		return () => {
			attached.destroy();
			menu.current = null;
		};
	}, [editor, attach, placement, offset, padding, custom]);

	// A re-render can change what `shouldShow` or the children decide.
	useEffect(() => {
		menu.current?.update();
	});

	return (
		<div ref={element} className={className} style={style} hidden>
			{children}
		</div>
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
