/**
 * High-level editor creation and input handling.
 *
 * {@link createEditor} is the main entry point. It initialises WASM, wraps the
 * raw `CanvistEditor` from Rust, and wires up DOM event listeners to give
 * feature parity with `contenteditable`:
 *
 * - Keyboard input (printable characters, Enter, Backspace, Delete)
 * - IME / composition events
 * - Mouse click-to-position cursor (TODO: full hit-testing)
 * - Selection rendering (TODO: multi-line highlights)
 * - Clipboard (TODO: copy/paste)
 */

import type { CanvistEditor } from "./types.ts";
import { getWasmModule } from "./wasm.ts";

/** Options for creating an editor. */
export interface EditorOptions {
	/** Title metadata for the document. */
	title?: string;
}

/**
 * Create a canvist editor attached to the given `<canvas>` element.
 *
 * The canvas must already exist in the DOM. WASM is initialised automatically
 * on first call.
 *
 * ```ts
 * import { createEditor } from "@canvist/canvist";
 *
 * const editor = await createEditor("my-canvas");
 * editor.insertText("Hello!");
 * editor.render();
 * ```
 */
export async function createEditor(
	canvasId: string,
	options?: EditorOptions,
): Promise<CanvistEditor> {
	const wasm = await getWasmModule();
	const inner = wasm.CanvistEditor.create(canvasId);

	if (options?.title) {
		inner.set_title(options.title);
	}

	// --- Cursor state (character offset) ---
	let cursorOffset = 0;

	// --- Hidden textarea for capturing keyboard input ---
	const canvas = document.getElementById(canvasId) as HTMLCanvasElement;
	const textarea = document.createElement("textarea");
	textarea.id = "canvist-input";
	textarea.setAttribute("autocomplete", "off");
	textarea.setAttribute("autocorrect", "off");
	textarea.setAttribute("autocapitalize", "off");
	textarea.setAttribute("spellcheck", "false");
	textarea.setAttribute("aria-label", "Document editor input");
	textarea.style.cssText =
		"position:absolute;left:-9999px;top:-9999px;width:1px;height:1px;opacity:0;";
	canvas.parentElement?.appendChild(textarea);
	canvas.setAttribute("role", "textbox");
	canvas.setAttribute("aria-multiline", "true");
	canvas.setAttribute("aria-label", "Canvist document editor");
	canvas.setAttribute("aria-controls", textarea.id);

	// Keep textarea focused so we receive keyboard events.
	canvas.addEventListener("click", () => textarea.focus());
	canvas.addEventListener("focus", () => textarea.focus());
	canvas.setAttribute("tabindex", "0");

	// --- Rendering helper ---
	function renderFrame() {
		try {
			inner.render();
			renderCursor();
			syncA11yState();
		} catch {
			// Canvas may not be in DOM during tests.
		}
	}

	function renderCursor() {
		try {
			const ctx = canvas.getContext("2d");
			if (!ctx) return;
			// Simple blinking caret: compute x position from character offset.
			const text = inner.plain_text();
			const before = text.slice(0, cursorOffset);
			ctx.font = "16px sans-serif";
			const metrics = ctx.measureText(before);
			const x = 20 + metrics.width;
			const y = 24;
			ctx.fillStyle = "black";
			ctx.fillRect(x, y, 1.5, 20);
		} catch {
			// Ignore in headless.
		}
	}

	// --- Input handling (contenteditable parity) ---

	textarea.addEventListener("input", (e: Event) => {
		const ie = e as InputEvent;
		if (ie.isComposing) return; // handled by compositionend

		const data = ie.data;
		if (data) {
			inner.insert_text_at(cursorOffset, data);
			cursorOffset += data.length;
			renderFrame();
		}
	});

	textarea.addEventListener("compositionend", (e: Event) => {
		const ce = e as CompositionEvent;
		if (ce.data) {
			inner.insert_text_at(cursorOffset, ce.data);
			cursorOffset += ce.data.length;
			renderFrame();
		}
		// Clear the textarea so next composition starts fresh.
		textarea.value = "";
	});

	function syncA11yState() {
		const text = inner.plain_text();
		textarea.value = text;
		textarea.setSelectionRange(cursorOffset, cursorOffset);
		canvas.setAttribute("aria-valuetext", text);
	}

	textarea.addEventListener("keydown", (e: KeyboardEvent) => {
		if (e.isComposing) return;

		switch (e.key) {
			case "Enter": {
				e.preventDefault();
				inner.insert_text_at(cursorOffset, "\n");
				cursorOffset += 1;
				renderFrame();
				break;
			}
			case "Backspace": {
				e.preventDefault();
				if (cursorOffset > 0) {
					inner.delete_range(cursorOffset - 1, cursorOffset);
					cursorOffset -= 1;
					renderFrame();
				}
				break;
			}
			case "Delete": {
				e.preventDefault();
				const text = inner.plain_text();
				if (cursorOffset < text.length) {
					inner.delete_range(cursorOffset, cursorOffset + 1);
					renderFrame();
				}
				break;
			}
			case "ArrowLeft": {
				e.preventDefault();
				if (cursorOffset > 0) cursorOffset -= 1;
				renderFrame();
				break;
			}
			case "ArrowRight": {
				e.preventDefault();
				const text = inner.plain_text();
				if (cursorOffset < text.length) cursorOffset += 1;
				renderFrame();
				break;
			}
		}
	});

	function buildPublicApi(ref: typeof inner): Omit<CanvistEditor, "destroy"> {
		return {
			get canvasId() {
				return ref.canvas_id();
			},
			get text() {
				return ref.plain_text();
			},
			get charCount() {
				return ref.char_count();
			},
			insertText(text: string) {
				ref.insert_text_at(cursorOffset, text);
				cursorOffset += text.length;
				renderFrame();
			},
			insertTextAt(offset: number, text: string) {
				ref.insert_text_at(offset, text);
				cursorOffset = offset + text.length;
				renderFrame();
			},
			deleteRange(start: number, end: number) {
				ref.delete_range(start, end);
				cursorOffset = start;
				renderFrame();
			},
			queueTextInput(text: string) {
				ref.queue_text_input(text);
			},
			queueKeyDown(key: string) {
				ref.queue_key_down(key);
			},
			processEvents() {
				ref.process_events();
				renderFrame();
			},
			setTitle(title: string) {
				ref.set_title(title);
			},
			render() {
				renderFrame();
			},
			toJSON() {
				return ref.to_json();
			},
		};
	}

	const editor: CanvistEditor = {
		...buildPublicApi(inner),
		destroy() {
			textarea.remove();
			try {
				inner.free();
			} catch {
				// Already freed.
			}
		},
	};

	// Initial render.
	renderFrame();
	textarea.focus();

	return editor;
}
