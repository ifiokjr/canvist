/**
 * Input: the keyboard, IME composition, the pointer, the clipboard, and
 * drag and drop, turned into editor changes.
 *
 * Text arrives through an `EditContext` attached to the canvas where the
 * browser has one, so input methods see the paragraph being edited and
 * place their windows at the caret. Elsewhere, or when the `input` option
 * forces it, a transparent textarea at the caret takes the focus and the
 * text. Either way, key bindings run first and every change is painted in
 * the same task as its event.
 *
 * @module
 */
import type { WebEditor } from "../wasm/canvist_web.js";
import {
	type CharacterBoundsUpdateEvent,
	type EditContext,
	editContextApi,
	type EditContextHost,
	type TextUpdateEvent,
} from "./edit-context.ts";
import { detectPlatform } from "./platform.ts";

/**
 * How the editor receives text: through `EditContext` where the browser
 * has it (`"auto"`), always through it, or always through a hidden
 * textarea. `"textarea"` is the kill switch for EditContext bugs.
 */
export type InputMode = "auto" | "editcontext" | "textarea";

/** Modifier bits `WebEditor.keyDown` takes. */
const SHIFT = 1;
const CTRL = 2;
const ALT = 4;
const META = 8;

/** Presses this close in time (ms) and space (CSS px) count as one multi-click. */
const MULTI_CLICK_MS = 500;
const MULTI_CLICK_SLOP = 4;

/** The last press, for counting double and triple clicks. */
interface Press {
	readonly count: number;
	readonly time: number;
	readonly x: number;
	readonly y: number;
}

/** What the input layer needs from the editor that owns it. */
export interface InputHost {
	readonly element: HTMLElement;
	readonly canvas: HTMLCanvasElement;
	readonly wasm: WebEditor;
	readonly signal: AbortSignal;
	/** An accessible name for the text field. */
	readonly label: string;
	/** The document coordinate at the canvas's top edge. */
	viewportTop(): number;
	/**
	 * Input changed the document or selection: paint now. `since` is the
	 * input's timestamp, on the `performance.now()` clock.
	 */
	changed(since: number): void;
	/** The editor gained or lost focus. */
	focusChanged(focused: boolean): void;
}

/** Whether `mode` resolves to EditContext in this browser. */
export function usesEditContext(mode: InputMode): boolean {
	return mode !== "textarea" && editContextApi() !== undefined;
}

/** Keyboard, text, pointer, clipboard, and drop input for one editor. */
export class Input {
	readonly #host: InputHost;
	readonly #context: EditContext | undefined;
	readonly #textarea: HTMLTextAreaElement | undefined;
	/** The element that holds focus while the editor is focused. */
	readonly #target: HTMLElement;
	#composing = false;
	/** The preedit text of the composition in progress. */
	#preedit = "";
	/** Where the composition starts in the EditContext's text. */
	#compositionFrom = 0;
	/** The pointer is pressed and selecting. */
	#pressed = false;
	/** This editor's selection is being dragged. */
	#dragging = false;
	#droppedHere = false;
	#lastPress: Press = { count: 0, time: -Infinity, x: 0, y: 0 };
	/**
	 * When the last key without a binding went down: the text it types
	 * arrives in a later event whose timestamp is its own dispatch, so
	 * latency is measured from the key.
	 */
	#keyTime: number | undefined;

	constructor(host: InputHost, mode: InputMode) {
		this.#host = host;
		const EditContextApi = mode === "textarea" ? undefined : editContextApi();

		if (EditContextApi === undefined) {
			this.#textarea = createTextarea(host.element.ownerDocument);
			host.element.append(this.#textarea);
			this.#target = this.#textarea;
			this.#listenToTextarea(this.#textarea);
		} else {
			this.#context = new EditContextApi();
			const canvas = host.canvas as HTMLCanvasElement & EditContextHost;
			canvas.tabIndex = 0;
			canvas.editContext = this.#context;
			this.#target = canvas;
			this.#listenToEditContext(this.#context);
		}

		this.#target.setAttribute("role", "textbox");
		this.#target.setAttribute("aria-multiline", "true");
		this.#target.setAttribute("aria-label", host.label);
		this.#listen();
	}

	/** Which input path is in use. */
	get mode(): "editcontext" | "textarea" {
		return this.#context === undefined ? "textarea" : "editcontext";
	}

	/** The element that takes focus. */
	get target(): HTMLElement {
		return this.#target;
	}

	focus(): void {
		this.#target.focus({ preventScroll: true });
	}

	/** Remove the textarea or detach the EditContext. Listeners go with the host's signal. */
	dispose(): void {
		this.#textarea?.remove();

		if (this.#context !== undefined) {
			(this.#host.canvas as HTMLCanvasElement & EditContextHost).editContext =
				null;
		}
	}

	/**
	 * Tell the platform what changed outside its own input: the paragraph
	 * and selection for an EditContext, and where the caret is. Left alone
	 * during a composition, which the input method owns.
	 */
	sync(): void {
		if (this.#composing) {
			return;
		}

		const context = this.#context;

		if (context !== undefined) {
			const [text = "", start = 0, end = 0] = this.#host.wasm
				.editContextState();

			if (context.text !== text) {
				context.updateText(0, context.text.length, text);
			}

			if (context.selectionStart !== start || context.selectionEnd !== end) {
				context.updateSelection(start, end);
			}
		}

		this.syncBounds();
	}

	/** Tell the platform where the editor and caret are on screen, for IME windows. */
	syncBounds(): void {
		const caret = this.#caretClientRect();

		if (this.#context !== undefined) {
			this.#context.updateControlBounds(
				this.#host.element.getBoundingClientRect(),
			);

			if (caret !== undefined) {
				this.#context.updateSelectionBounds(caret);
			}
		}

		if (this.#textarea !== undefined && caret !== undefined) {
			const element = this.#host.element.getBoundingClientRect();
			Object.assign(this.#textarea.style, {
				left: `${caret.x - element.left + this.#host.element.scrollLeft}px`,
				top: `${caret.y - element.top + this.#host.element.scrollTop}px`,
				height: `${caret.height}px`,
			});
		}
	}

	#listen(): void {
		const { canvas, element, signal } = this.#host;
		const target = this.#target;

		target.addEventListener("keydown", (event) => this.#keyDown(event), {
			signal,
		});
		target.addEventListener("focus", () => this.#host.focusChanged(true), {
			signal,
		});
		target.addEventListener("blur", () => this.#host.focusChanged(false), {
			signal,
		});
		canvas.addEventListener(
			"pointerdown",
			(event) => this.#pointerDown(event),
			{
				signal,
			},
		);
		canvas.addEventListener(
			"pointermove",
			(event) => this.#pointerMove(event),
			{
				signal,
			},
		);
		canvas.addEventListener("pointerup", (event) => this.#pointerUp(event), {
			signal,
		});
		canvas.addEventListener("pointercancel", () => this.#release(), { signal });
		element.addEventListener("copy", (event) => this.#copy(event, false), {
			signal,
		});
		element.addEventListener("cut", (event) => this.#copy(event, true), {
			signal,
		});
		element.addEventListener("paste", (event) => this.#paste(event), {
			signal,
		});
		canvas.addEventListener("dragstart", (event) => this.#dragStart(event), {
			signal,
		});
		canvas.addEventListener("dragend", (event) => this.#dragEnd(event), {
			signal,
		});
		canvas.addEventListener("dragover", (event) => this.#dragOver(event), {
			signal,
		});
		canvas.addEventListener("drop", (event) => this.#drop(event), { signal });
	}

	#listenToEditContext(context: EditContext): void {
		const signal = this.#host.signal;

		context.addEventListener("textupdate", (event) => this.#textUpdate(event), {
			signal,
		});
		context.addEventListener("compositionstart", () => {
			this.#composing = true;
			this.#preedit = "";
			this.#compositionFrom = Math.min(
				context.selectionStart,
				context.selectionEnd,
			);
		}, { signal });
		context.addEventListener("compositionend", (event) => {
			this.#composing = false;
			this.#edit(event, (wasm) => wasm.commitComposition(this.#preedit));
			this.#preedit = "";
		}, { signal });
		context.addEventListener(
			"characterboundsupdate",
			(event) => this.#characterBounds(context, event),
			{ signal },
		);
	}

	#listenToTextarea(textarea: HTMLTextAreaElement): void {
		const signal = this.#host.signal;

		textarea.addEventListener(
			"beforeinput",
			(event) => this.#beforeInput(event),
			{
				signal,
			},
		);
		textarea.addEventListener("compositionstart", () => {
			this.#composing = true;
		}, { signal });
		textarea.addEventListener("compositionupdate", (event) => {
			this.#edit(event, (wasm) => wasm.setComposition(event.data));
		}, { signal });
		textarea.addEventListener("compositionend", (event) => {
			this.#composing = false;
			this.#edit(event, (wasm) => wasm.commitComposition(event.data));
			textarea.value = "";
		}, { signal });
		// Whatever the browser put in the textarea has been applied already.
		textarea.addEventListener("input", () => {
			if (!this.#composing) {
				textarea.value = "";
			}
		}, { signal });
	}

	/** Run the key binding for `event`; keys without one reach the browser as text. */
	#keyDown(event: KeyboardEvent): void {
		// A key that an input method is handling (`Process`, key code 229)
		// belongs to the composition.
		if (event.isComposing || this.#composing || event.keyCode === 229) {
			return;
		}

		if (this.#host.wasm.keyDown(event.key, modifiers(event))) {
			event.preventDefault();
			this.#changed(event);
		} else {
			this.#keyTime = event.timeStamp;
		}
	}

	/** Text input through the EditContext, composed or typed. */
	#textUpdate(event: TextUpdateEvent): void {
		if (!this.#composing) {
			this.#edit(
				event,
				(wasm) =>
					wasm.replaceText(
						event.updateRangeStart,
						event.updateRangeEnd,
						event.text,
					),
			);
			return;
		}

		// The update replaces part of the preedit, in the context's offsets.
		const length = this.#preedit.length;
		const from = clamp(event.updateRangeStart - this.#compositionFrom, length);
		const to = clamp(event.updateRangeEnd - this.#compositionFrom, length);
		this.#preedit = this.#preedit.slice(0, from) + event.text +
			this.#preedit.slice(to);
		this.#edit(event, (wasm) => wasm.setComposition(this.#preedit));
	}

	/** Text input through the textarea: typed text and edits without a key binding (as from soft keyboards). */
	#beforeInput(event: InputEvent): void {
		if (this.#composing || event.inputType === "insertCompositionText") {
			return;
		}

		const wasm = this.#host.wasm;
		const apply = (): boolean => {
			switch (event.inputType) {
				case "insertText":
				case "insertReplacementText":
					return wasm.insertText(
						event.data ?? event.dataTransfer?.getData("text/plain") ?? "",
					);
				case "insertLineBreak":
				case "insertParagraph":
					return wasm.keyDown(
						"Enter",
						event.inputType === "insertLineBreak" ? SHIFT : 0,
					);
				case "deleteContentBackward":
					return wasm.keyDown("Backspace", 0);
				case "deleteContentForward":
					return wasm.keyDown("Delete", 0);
				case "historyUndo":
					return wasm.undo();
				case "historyRedo":
					return wasm.redo();
				default:
					return false;
			}
		};

		// Pastes and drops arrive as their own events.
		if (
			event.inputType === "insertFromPaste" ||
			event.inputType === "insertFromDrop"
		) {
			return;
		}

		event.preventDefault();

		if (apply()) {
			this.#changed(event);
		}
	}

	/** Where the characters an input method asks about are drawn, in client coordinates. */
	#characterBounds(
		context: EditContext,
		event: CharacterBoundsUpdateEvent,
	): void {
		const flat = this.#host.wasm.characterBounds(
			event.rangeStart,
			event.rangeEnd,
		);
		const rects: DOMRect[] = [];

		for (let index = 0; index + 3 < flat.length; index += 4) {
			rects.push(
				this.#clientRect(
					flat[index] ?? 0,
					flat[index + 1] ?? 0,
					flat[index + 2] ?? 0,
					flat[index + 3] ?? 0,
				),
			);
		}

		context.updateCharacterBounds(event.rangeStart, rects);
	}

	#pointerDown(event: PointerEvent): void {
		if (event.button !== 0) {
			return;
		}

		const [x, y] = this.#documentPoint(event);
		const wasm = this.#host.wasm;
		const canvas = this.#host.canvas;
		const clicks = this.#count(event);

		// A press inside the selection may start dragging it: let the
		// browser decide, and place the caret on release if it doesn't.
		if (clicks === 1 && !event.shiftKey && wasm.isInSelection(x, y)) {
			canvas.draggable = true;
			return;
		}

		canvas.draggable = false;
		// Keep focus in the textarea rather than letting the press blur it.
		event.preventDefault();
		this.focus();
		this.#pressed = true;
		canvas.setPointerCapture(event.pointerId);
		this.#edit(
			event,
			(editor) => editor.pointerDown(x, y, event.shiftKey, clicks),
		);
	}

	/**
	 * How many clicks `event` makes in a row. Pointer events don't count
	 * clicks in every browser, so presses close in time and place are
	 * counted here when `detail` is zero.
	 */
	#count(event: PointerEvent): number {
		const last = this.#lastPress;
		const near = Math.abs(event.clientX - last.x) <= MULTI_CLICK_SLOP &&
			Math.abs(event.clientY - last.y) <= MULTI_CLICK_SLOP &&
			event.timeStamp - last.time <= MULTI_CLICK_MS;
		const count = event.detail > 0 ? event.detail : near ? last.count + 1 : 1;
		this.#lastPress = {
			count,
			time: event.timeStamp,
			x: event.clientX,
			y: event.clientY,
		};

		return count;
	}

	#pointerMove(event: PointerEvent): void {
		if (!this.#pressed) {
			return;
		}

		const [x, y] = this.#documentPoint(event);
		this.#edit(event, (wasm) => wasm.pointerMove(x, y));
	}

	#pointerUp(event: PointerEvent): void {
		const canvas = this.#host.canvas;

		// A press inside the selection that didn't become a drag is a click.
		if (canvas.draggable && !this.#dragging) {
			canvas.draggable = false;
			const [x, y] = this.#documentPoint(event);
			this.focus();
			this.#edit(event, (wasm) => wasm.pointerDown(x, y, false, 1));
		}

		this.#release();
	}

	#release(): void {
		this.#pressed = false;
		this.#host.wasm.pointerUp();
	}

	#copy(event: ClipboardEvent, cut: boolean): void {
		const wasm = this.#host.wasm;
		const [html, text] = cut ? wasm.cutSelection() : wasm.copySelection();

		if (
			html === undefined || text === undefined || event.clipboardData === null
		) {
			return;
		}

		event.preventDefault();
		event.clipboardData.setData("text/html", html);
		event.clipboardData.setData("text/plain", text);

		if (cut) {
			this.#changed(event);
		}
	}

	#paste(event: ClipboardEvent): void {
		const data = event.clipboardData;

		if (data === null) {
			return;
		}

		event.preventDefault();
		const html = data.getData("text/html");
		this.#edit(
			event,
			(wasm) =>
				html === ""
					? wasm.pasteText(data.getData("text/plain"))
					: wasm.pasteHtml(html),
		);
	}

	#dragStart(event: DragEvent): void {
		const [html, text] = this.#host.wasm.copySelection();

		if (
			html === undefined || text === undefined || event.dataTransfer === null
		) {
			event.preventDefault();
			return;
		}

		this.#dragging = true;
		this.#droppedHere = false;
		event.dataTransfer.setData("text/html", html);
		event.dataTransfer.setData("text/plain", text);
		event.dataTransfer.effectAllowed = "copyMove";
	}

	#dragEnd(event: DragEvent): void {
		const moved = event.dataTransfer?.dropEffect === "move";

		// Moved to another application: remove it here.
		if (moved && !this.#droppedHere) {
			this.#host.wasm.cutSelection();
			this.#changed(event);
		}

		this.#dragging = false;
		this.#host.canvas.draggable = false;
	}

	#dragOver(event: DragEvent): void {
		if (event.dataTransfer === null) {
			return;
		}

		event.preventDefault();
		event.dataTransfer.dropEffect = this.#dragging && !copyModifier(event)
			? "move"
			: "copy";
	}

	#drop(event: DragEvent): void {
		const data = event.dataTransfer;

		if (data === null) {
			return;
		}

		event.preventDefault();
		const [x, y] = this.#documentPoint(event);
		const html = data.getData("text/html");
		const moving = this.#dragging && !copyModifier(event);
		this.#droppedHere = true;
		this.focus();
		this.#edit(
			event,
			(wasm) =>
				wasm.dropAt(
					x,
					y,
					html === "" ? undefined : html,
					data.getData("text/plain"),
					moving,
				),
		);
	}

	/** Apply `change` and, when it changed anything, paint for `event`. */
	#edit(event: Event, change: (wasm: WebEditor) => boolean): void {
		if (change(this.#host.wasm)) {
			this.#changed(event);
		}
	}

	#changed(event: Event): void {
		const text = event.type === "textupdate" || event.type === "beforeinput";
		this.#host.changed(
			text ? this.#keyTime ?? event.timeStamp : event.timeStamp,
		);
		this.#keyTime = undefined;
		this.sync();
	}

	/** A pointer event's position in document coordinates. */
	#documentPoint(event: MouseEvent): [number, number] {
		const canvas = this.#host.canvas.getBoundingClientRect();

		return [
			event.clientX - canvas.left,
			event.clientY - canvas.top + this.#host.viewportTop(),
		];
	}

	/** A rectangle in document coordinates, in client coordinates. */
	#clientRect(x: number, y: number, width: number, height: number): DOMRect {
		const canvas = this.#host.canvas.getBoundingClientRect();

		return new DOMRect(
			canvas.left + x,
			canvas.top + y - this.#host.viewportTop(),
			width,
			height,
		);
	}

	#caretClientRect(): DOMRect | undefined {
		const [x, y, width, height] = this.#host.wasm.caretRect();

		return x === undefined || y === undefined || width === undefined ||
				height === undefined
			? undefined
			: this.#clientRect(x, y, width, height);
	}
}

/** `event`'s modifiers as `WebEditor.keyDown` takes them. */
function modifiers(event: KeyboardEvent): number {
	return (event.shiftKey ? SHIFT : 0) | (event.ctrlKey ? CTRL : 0) |
		(event.altKey ? ALT : 0) | (event.metaKey ? META : 0);
}

/** Whether a drop copies rather than moves: Option on Apple platforms, Ctrl elsewhere. */
function copyModifier(event: DragEvent): boolean {
	return detectPlatform(navigator) === "apple" ? event.altKey : event.ctrlKey;
}

function clamp(value: number, max: number): number {
	return Math.min(Math.max(value, 0), max);
}

/**
 * The transparent textarea that receives text where EditContext is
 * unavailable. It follows the caret so input methods open their windows
 * there.
 */
function createTextarea(document: Document): HTMLTextAreaElement {
	const textarea = document.createElement("textarea");
	textarea.className = "cv-input";
	textarea.autocapitalize = "off";
	textarea.autocomplete = "off";
	textarea.spellcheck = false;
	textarea.setAttribute("autocorrect", "off");
	Object.assign(textarea.style, {
		position: "absolute",
		left: "0",
		top: "0",
		width: "1px",
		height: "1em",
		padding: "0",
		border: "0",
		margin: "0",
		outline: "none",
		resize: "none",
		overflow: "hidden",
		whiteSpace: "pre",
		opacity: "0",
		caretColor: "transparent",
		pointerEvents: "none",
	});

	return textarea;
}
