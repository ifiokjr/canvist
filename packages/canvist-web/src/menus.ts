/**
 * Menus that follow the selection: a bubble menu over selected text, and
 * a floating menu beside an empty line, as in TipTap.
 *
 * Each takes an element you build (buttons, a link field, anything), keeps
 * it next to the selection as it moves, scrolls, and resizes, and hides it
 * when it doesn't apply (with `visibility: hidden`, so the page's own
 * `display` stays). Pressing inside a menu keeps the editor's focus, so
 * its buttons act on the selection.
 *
 * ```ts
 * import { createEditor } from "@canvist/web";
 * import { bubbleMenu } from "@canvist/web/menus";
 *
 * const editor = await createEditor(document.querySelector("#editor")!);
 * const menu = bubbleMenu(editor, document.querySelector("#bubble")!);
 * // menu.destroy() when you are done.
 * ```
 *
 * @module
 */
import type { Extensions } from "@canvist/headless";
import type { Editor } from "./editor.ts";
import type { Plugins } from "./plugin.ts";

/** The side of its anchor a menu prefers. It flips to the other side when it doesn't fit. */
export type Placement = "top" | "bottom" | "left" | "right";

/** Where to put a menu, from {@link placeMenu}. */
export interface PlacementOptions {
	/** The side of the anchor to prefer. */
	readonly placement: Placement;
	/** The gap between the anchor and the menu, in CSS px. Default 8. */
	readonly offset?: number;
	/** The least distance between the menu and the viewport's edges, in CSS px. Default 8. */
	readonly padding?: number;
}

/** Options for {@link bubbleMenu} and {@link floatingMenu}. */
export interface MenuOptions<E extends Extensions, P extends Plugins>
	extends Partial<PlacementOptions> {
	/**
	 * Whether to show the menu now. It replaces the default rule, which
	 * the menu's own function documents.
	 */
	readonly shouldShow?: (editor: Editor<E, P>) => boolean;
}

/** A menu attached to an editor. */
export interface Menu {
	/** Whether the menu is showing. */
	readonly visible: boolean;
	/** Show, hide, and place the menu now, as after a change you made outside the editor. */
	update(): void;
	/** Stop following the editor and hide the menu. The element stays in the page. */
	destroy(): void;
	[Symbol.dispose](): void;
}

/** The gap between a menu and its anchor, in CSS px. */
const DEFAULT_OFFSET = 8;
/** The least distance between a menu and the viewport's edges, in CSS px. */
const DEFAULT_PADDING = 8;

/**
 * Keep `element` above selected text. By default it shows while the editor
 * is editable and focused (or focus is in the menu) and the selection is a
 * range or whole blocks, never a caret.
 */
export function bubbleMenu<E extends Extensions, P extends Plugins>(
	editor: Editor<E, P>,
	element: HTMLElement,
	options: MenuOptions<E, P> = {},
): Menu {
	return attach(editor, element, {
		placement: options.placement ?? "top",
		offset: options.offset ?? DEFAULT_OFFSET,
		padding: options.padding ?? DEFAULT_PADDING,
		shouldShow: options.shouldShow ?? showsBubble,
	});
}

/**
 * Keep `element` beside the caret on an empty line, for inserting blocks.
 * By default it shows while the editor is editable and focused (or focus is
 * in the menu) and the caret is in an empty text block.
 */
export function floatingMenu<E extends Extensions, P extends Plugins>(
	editor: Editor<E, P>,
	element: HTMLElement,
	options: MenuOptions<E, P> = {},
): Menu {
	return attach(editor, element, {
		placement: options.placement ?? "left",
		offset: options.offset ?? DEFAULT_OFFSET,
		padding: options.padding ?? DEFAULT_PADDING,
		shouldShow: options.shouldShow ?? showsFloating,
	});
}

/**
 * Position `element` (`position: fixed`) beside `anchor`, a rectangle in
 * viewport coordinates, on the preferred side if it fits and the opposite
 * side if not, then shifted along that side to stay inside the viewport.
 */
export function placeMenu(
	element: HTMLElement,
	anchor: DOMRectReadOnly,
	options: PlacementOptions,
): void {
	const offset = options.offset ?? DEFAULT_OFFSET;
	const padding = options.padding ?? DEFAULT_PADDING;
	const view = element.ownerDocument.defaultView;
	const viewport = {
		width: view?.innerWidth ?? 0,
		height: view?.innerHeight ?? 0,
	};
	// Out of the flow first, so it measures at the size it shows at.
	element.style.position = "fixed";
	const size = element.getBoundingClientRect();
	const placement = fits(options.placement, anchor, size, viewport, offset)
		? options.placement
		: OPPOSITE[options.placement];
	const vertical = placement === "top" || placement === "bottom";
	const [x, y] = vertical
		? [
			anchor.x + anchor.width / 2 - size.width / 2,
			placement === "top"
				? anchor.top - offset - size.height
				: anchor.bottom + offset,
		]
		: [
			placement === "left"
				? anchor.left - offset - size.width
				: anchor.right + offset,
			anchor.y + anchor.height / 2 - size.height / 2,
		];

	Object.assign(element.style, {
		left: `${clamp(x, padding, viewport.width - size.width - padding)}px`,
		top: `${clamp(y, padding, viewport.height - size.height - padding)}px`,
	});
	element.dataset["placement"] = placement;
}

const OPPOSITE: Readonly<Record<Placement, Placement>> = {
	top: "bottom",
	bottom: "top",
	left: "right",
	right: "left",
};

interface Attachment<E extends Extensions, P extends Plugins>
	extends Required<PlacementOptions> {
	readonly shouldShow: (editor: Editor<E, P>) => boolean;
}

/** Follow `editor` with `element` until the returned menu is destroyed. */
function attach<E extends Extensions, P extends Plugins>(
	editor: Editor<E, P>,
	element: HTMLElement,
	attachment: Attachment<E, P>,
): Menu {
	const abort = new AbortController();
	const signal = abort.signal;
	const view = element.ownerDocument.defaultView;
	let visible = false;
	let frame = 0;

	const update = () => {
		cancelAnimationFrame(frame);
		frame = 0;
		const anchor = attachment.shouldShow(editor)
			? editor.selectionRect()
			: null;
		visible = anchor !== null;
		element.style.visibility = visible ? "visible" : "hidden";

		if (anchor !== null) {
			placeMenu(element, editor.toClientRect(anchor), attachment);
		}
	};
	// Edits and scrolling come in bursts: place once per frame.
	const schedule = () => {
		frame ||= requestAnimationFrame(update);
	};
	const unsubscribe = editor.subscribe(schedule);

	// Focus inside the menu counts as focus in the editor.
	element.dataset["canvistMenu"] = "";
	// Hidden by visibility, so it keeps its size for placing, whatever
	// `display` the page gives it.
	element.hidden = false;
	element.style.position = "fixed";

	// Keep the editor's focus, and its selection, when the menu is pressed.
	element.addEventListener("pointerdown", (event) => event.preventDefault(), {
		signal,
	});
	element.addEventListener("focusout", schedule, { signal });
	view?.addEventListener("scroll", schedule, {
		capture: true,
		passive: true,
		signal,
	});
	view?.addEventListener("resize", schedule, { signal });
	update();

	const destroy = () => {
		abort.abort();
		unsubscribe();
		cancelAnimationFrame(frame);
		element.style.visibility = "hidden";
		visible = false;
	};

	return {
		get visible() {
			return visible;
		},
		update,
		destroy,
		[Symbol.dispose]: destroy,
	};
}

/** Whether `editor`, or a menu inside its document, holds focus while it is editable. */
function active<E extends Extensions, P extends Plugins>(
	editor: Editor<E, P>,
): boolean {
	if (!editor.editable) {
		return false;
	}

	const focused = editor.canvas.ownerDocument.activeElement;

	return editor.focused ||
		(focused !== null && focused.closest("[data-canvist-menu]") !== null);
}

function showsBubble<E extends Extensions, P extends Plugins>(
	editor: Editor<E, P>,
): boolean {
	const selection = editor.selection;

	return active(editor) &&
		(selection === null || !samePosition(selection.anchor, selection.head));
}

function showsFloating<E extends Extensions, P extends Plugins>(
	editor: Editor<E, P>,
): boolean {
	const selection = editor.selection;

	return active(editor) && selection !== null &&
		samePosition(selection.anchor, selection.head) &&
		editor.blockText() === "";
}

function samePosition(
	a: { readonly path: readonly number[]; readonly offset: number },
	b: { readonly path: readonly number[]; readonly offset: number },
): boolean {
	return a.offset === b.offset && a.path.length === b.path.length &&
		a.path.every((step, index) => step === b.path[index]);
}

/** Whether a menu of `size` fits on `placement`'s side of `anchor`. */
function fits(
	placement: Placement,
	anchor: DOMRectReadOnly,
	size: DOMRectReadOnly,
	viewport: { readonly width: number; readonly height: number },
	offset: number,
): boolean {
	switch (placement) {
		case "top":
			return anchor.top - offset - size.height >= 0;
		case "bottom":
			return anchor.bottom + offset + size.height <= viewport.height;
		case "left":
			return anchor.left - offset - size.width >= 0;
		case "right":
			return anchor.right + offset + size.width <= viewport.width;
	}
}

function clamp(value: number, min: number, max: number): number {
	return Math.max(min, Math.min(value, Math.max(min, max)));
}
