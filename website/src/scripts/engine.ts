import type { Content, StarterKit } from "@canvist/web";
import {
	PIXEL_CONTENT,
	pixelTheme,
	type Scheme,
} from "../lib/pixel-document.ts";

/**
 * The live editor behind "Type into the real engine": `@canvist/web`,
 * taking over from the document the page was built with.
 *
 * The section talks to this interface, so the editor's module (about
 * 340 KB of WebAssembly, gzipped) loads only when `loadEditor` is called.
 */
export interface LiveEditor {
	/** Inserts text at the caret, as if typed. */
	type(text: string): void;
	/** Puts the caret at the end of the first paragraph. */
	caretToEnd(): void;
	focus(): void;
	/** Shows the caret blinking, as when focused, or hides it. Focus shows it too. */
	showCaret(on: boolean): void;
	/** Repaints in the other colour scheme, keeping the document and selection. */
	setScheme(scheme: Scheme): Promise<void>;
}

const LABEL = "canvist editor";

/**
 * Starts `@canvist/web` in `element`, which holds the server-rendered
 * `.cv-doc`, and resolves once the canvas has replaced that HTML.
 */
export async function loadEditor(
	element: HTMLElement,
	scheme: Scheme,
): Promise<LiveEditor> {
	const { createEditor, load } = await import("@canvist/web");
	// Download and compile first, so creating the editor below runs within
	// microtasks, with no pause in which the visitor could scroll.
	await load();

	const create = (
		current: Scheme,
		content: Content<StarterKit> = PIXEL_CONTENT,
	) =>
		keepingFocus(() =>
			createEditor(element, {
				content,
				theme: pixelTheme(current),
				label: LABEL,
			})
		);
	type PixelEditor = Awaited<ReturnType<typeof create>>;

	let editor: PixelEditor = await create(scheme);
	let current = scheme;
	// The canvas stays hidden behind the HTML until the fonts have loaded.
	await editor.ready();

	// The theme is fixed when an editor is created, so a new one takes over
	// the document, selection, focus, and scroll position.
	const swap = async (next: Scheme) => {
		if (next === current) return;
		current = next;
		const content = editor.toJson();
		const selection = editor.selection;
		const hadFocus = element.contains(document.activeElement);
		const caretShown = editor.focused;
		const scrollTop = element.scrollTop;
		editor.destroy();
		editor = await create(next, content);
		if (selection) editor.select(selection.anchor, selection.head);
		element.scrollTop = scrollTop;
		if (hadFocus) editor.focus();
		else editor.setFocused(caretShown);
	};
	// One swap at a time: a second must not destroy the editor the first is
	// replacing.
	let swapping = Promise.resolve();

	return {
		type(text) {
			editor.insertText(text);
		},
		caretToEnd() {
			const [first] = editor.toJson().content ?? [];
			const inline = first && "content" in first ? first.content ?? [] : [];
			const offset = inline.reduce(
				(length, node) => length + ("text" in node ? node.text.length : 0),
				0,
			);
			editor.select({ path: [0], offset });
		},
		focus() {
			editor.focus();
		},
		showCaret(on) {
			editor.setFocused(on);
		},
		setScheme(next) {
			swapping = swapping.then(() => swap(next));
			return swapping;
		},
	};
}

/**
 * Runs `start` and then puts focus and the scroll position back as they
 * were, in case starting the editor moved either: the visitor focuses the
 * editor by clicking in. `start` must settle within microtasks, so nothing
 * the visitor does in between is undone.
 */
async function keepingFocus<T>(start: () => Promise<T>): Promise<T> {
	const previousFocus = document.activeElement;
	const previousScroll = { x: scrollX, y: scrollY };
	const result = await start();
	if (document.activeElement !== previousFocus) {
		if (
			previousFocus instanceof HTMLElement && previousFocus !== document.body
		) {
			previousFocus.focus({ preventScroll: true });
		} else if (document.activeElement instanceof HTMLElement) {
			document.activeElement.blur();
		}
	}
	if (scrollX !== previousScroll.x || scrollY !== previousScroll.y) {
		scrollTo(previousScroll.x, previousScroll.y);
	}
	return result;
}
