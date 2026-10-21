/**
 * Vue bindings for the canvist canvas editor.
 *
 * `CanvistEditor` renders an editor; `useCanvistEditor` gives you the
 * editor itself, and `useEditorState` keeps a ref in step with something
 * read from the editor. Commands, queries, and plugin APIs are typed from
 * the extensions and plugins you pass, as in `@canvist/web`. `BubbleMenu`
 * and `FloatingMenu` keep their slot next to the selection.
 *
 * ```ts
 * const { element, editor } = useCanvistEditor({
 * 	content: { format: "markdown", input: "Hello **world**" },
 * });
 * const bold = useEditorState(editor, (current) => current.can.toggleBold(), false);
 * // <button :disabled="!bold" @click="editor?.commands.toggleBold()">Bold</button>
 * // <div ref="element" />
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
	defineComponent,
	h,
	onBeforeUnmount,
	onMounted,
	type Ref,
	type SetupContext,
	type ShallowRef,
	shallowRef,
	watch,
} from "vue";

/** What {@link useCanvistEditor} returns. */
export interface CanvistEditorHandle<E extends Extensions, P extends Plugins> {
	/** Bind as the template ref of the element the editor renders into. */
	readonly element: ShallowRef<HTMLElement | null>;
	/** The editor, once it has loaded; `null` before that and after unmounting. */
	readonly editor: ShallowRef<Editor<E, P> | null>;
}

/**
 * Create an editor in `element` when the component mounts, and destroy it
 * when it unmounts. `options` are read when the editor is created.
 */
export function useCanvistEditor<
	const E extends Extensions = StarterKit,
	const P extends Plugins = [],
>(options: EditorOptions<E, P> = {}): CanvistEditorHandle<E, P> {
	const element = shallowRef<HTMLElement | null>(null);
	const editor = shallowRef<Editor<E, P> | null>(null);
	const mount: Mount<E, P> = { live: true, editor };

	onMounted(() => {
		if (element.value !== null) {
			void create(element.value, options, mount);
		}
	});

	onBeforeUnmount(() => {
		mount.live = false;
		editor.value?.destroy();
		editor.value = null;
	});

	return { element, editor };
}

/** One component's editor: whether it is still mounted, and where the editor goes. */
interface Mount<E extends Extensions, P extends Plugins> {
	live: boolean;
	readonly editor: ShallowRef<Editor<E, P> | null>;
}

/** Create the editor for `mount`, destroying it if the component unmounted while it loaded. */
async function create<E extends Extensions, P extends Plugins>(
	element: HTMLElement,
	options: EditorOptions<E, P>,
	mount: Mount<E, P>,
): Promise<void> {
	try {
		const editor = await createEditor(element, options);

		if (mount.live) {
			mount.editor.value = editor;
		} else {
			editor.destroy();
		}
	} catch (error) {
		if (mount.live) {
			reportError(error);
		}
	}
}

/**
 * A ref holding `select(editor)`, updated after every edit or selection
 * change. Until the editor exists, it holds `fallback`.
 */
export function useEditorState<E extends Extensions, P extends Plugins, T>(
	editor: Readonly<Ref<Editor<E, P> | null>>,
	select: (editor: Editor<E, P>) => T,
	fallback: T,
): Readonly<ShallowRef<T>> {
	const state = shallowRef(fallback);

	watch(editor, (current, _previous, onCleanup) => {
		if (current === null) {
			state.value = fallback;
			return;
		}

		state.value = select(current);
		onCleanup(current.subscribe(() => {
			state.value = select(current);
		}));
	}, { immediate: true });

	return state;
}

/**
 * Props for {@link CanvistEditor}: the editor's options, read once except
 * `editable` and `placeholder`, which follow the props.
 */
export interface CanvistEditorProps<E extends Extensions, P extends Plugins>
	extends EditorOptions<E, P> {
	/**
	 * Server-rendered HTML (`.cv-doc` markup from canvist's renderer) to
	 * show until the editor takes over. The editor reads the document from
	 * it when `content` is not given.
	 */
	readonly html?: string;
}

/** The option props {@link CanvistEditor} passes to the editor. */
const OPTION_PROPS = [
	"extensions",
	"plugins",
	"content",
	"theme",
	"breaks",
	"client",
	"overscan",
	"wasm",
	"input",
	"label",
	"platform",
	"editable",
	"placeholder",
] as const satisfies readonly (keyof EditorOptions<Extensions, Plugins>)[];

/**
 * A canvist editor. Give it a width; without a height it grows with its
 * document. It emits `ready` once the editor has loaded and `update`
 * after every change, each with the editor.
 */
export const CanvistEditor = defineComponent(
	<const E extends Extensions = StarterKit, const P extends Plugins = []>(
		props: CanvistEditorProps<E, P>,
		{ emit }: {
			emit: (event: "ready" | "update", editor: Editor<E, P>) => void;
		},
	) => {
		const { html, ...options } = props;
		const { element, editor } = useCanvistEditor<E, P>(options);

		watch(
			[editor, () => props.editable, () => props.placeholder],
			([current, editable, placeholder]) => {
				current?.setEditable(editable ?? true);
				current?.setPlaceholder(placeholder ?? "");
			},
		);

		watch(editor, (current, _previous, onCleanup) => {
			if (current === null) {
				return;
			}

			emit("ready", current);
			onCleanup(current.subscribe(() => emit("update", current)));
		});

		// `innerHTML` is set once: Vue leaves the editor's elements alone
		// while it is unchanged.
		return () => h("div", { ref: element, innerHTML: html });
	},
	{ props: ["html", ...OPTION_PROPS], emits: ["ready", "update"] },
);

/** Props for {@link BubbleMenu} and {@link FloatingMenu}. */
export interface MenuProps<E extends Extensions, P extends Plugins>
	extends MenuOptions<E, P> {
	/** The editor to follow; the menu stays hidden while it is `null`. */
	readonly editor: Editor<E, P> | null;
}

type Attach = <E extends Extensions, P extends Plugins>(
	editor: Editor<E, P>,
	element: HTMLElement,
	options: MenuOptions<E, P>,
) => Menu;

/** A component that keeps its default slot where `attach` places it. */
function followingMenu(attach: Attach) {
	return defineComponent(
		<E extends Extensions, P extends Plugins>(
			props: MenuProps<E, P>,
			{ slots }: SetupContext,
		) => {
			const element = shallowRef<HTMLElement | null>(null);

			watch(
				[
					() => props.editor,
					element,
					() => props.placement,
					() => props.offset,
					() => props.padding,
					() => props.shouldShow,
				],
				(
					[editor, current, placement, offset, padding, shouldShow],
					_,
					onCleanup,
				) => {
					if (editor === null || current === null) {
						return;
					}

					const menu = attach(editor, current, {
						...(placement === undefined ? {} : { placement }),
						...(offset === undefined ? {} : { offset }),
						...(padding === undefined ? {} : { padding }),
						...(shouldShow === undefined ? {} : { shouldShow }),
					});
					onCleanup(() => menu.destroy());
				},
				{ flush: "post" },
			);

			return () =>
				h("div", { ref: element, hidden: true }, slots["default"]?.());
		},
		{ props: ["editor", "placement", "offset", "padding", "shouldShow"] },
	);
}

/**
 * A menu over selected text, as `bubbleMenu` from `@canvist/web/menus`
 * places it. Pressing inside it keeps the editor's focus and selection.
 */
export const BubbleMenu = followingMenu(bubbleMenu);

/**
 * A menu beside the caret on an empty line, as `floatingMenu` from
 * `@canvist/web/menus` places it.
 */
export const FloatingMenu = followingMenu(floatingMenu);
