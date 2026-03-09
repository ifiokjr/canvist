/**
 * High-level editor creation and input handling.
 *
 * {@link createEditor} is the main entry point. It initialises WASM, wraps the
 * raw `CanvistEditor` from Rust, and wires up DOM event listeners to give
 * feature parity with `contenteditable`:
 *
 * - Keyboard input (printable characters, Enter, Backspace, Delete)
 * - IME / composition events
 * - Mouse click-to-position cursor (uses WASM hit-testing)
 * - Mouse drag-to-select (range selection via hit-testing on mousemove)
 * - Selection rendering (TODO: multi-line highlights)
 * - Clipboard (copy, cut, paste via system clipboard)
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

	// --- Caret blink controller ---
	const CARET_BLINK_INTERVAL_MS = 530;
	let caretBlinkTimer: ReturnType<typeof setInterval> | null = null;
	let caretVisible = true;

	/**
	 * Sync the current wall-clock time to the WASM runtime so that undo
	 * coalescing can measure real time gaps between keystrokes. Call this
	 * before every user action that may mutate the document.
	 */
	function syncTime() {
		inner.set_now_ms(Date.now());
	}

	/**
	 * Reset the blink timer so the caret is immediately visible for one full
	 * cycle, then blinks on/off every 530 ms. Called after every user action
	 * that should "wake" the caret (typing, clicking, arrow keys, etc.).
	 */
	function resetCaretBlink() {
		// Show the caret immediately.
		caretVisible = true;
		inner.set_caret_visible(true);

		// Clear any existing timer.
		if (caretBlinkTimer !== null) {
			clearInterval(caretBlinkTimer);
		}

		// Start a fresh blink cycle.
		caretBlinkTimer = setInterval(() => {
			caretVisible = !caretVisible;
			inner.set_caret_visible(caretVisible);
			try {
				inner.render();
			} catch {
				// Canvas may be detached.
			}
		}, CARET_BLINK_INTERVAL_MS);
	}

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
	canvas.addEventListener("focus", () => textarea.focus());
	canvas.setAttribute("tabindex", "0");

	// --- Mouse click-to-cursor hit-testing & drag-to-select ---

	/** Whether a mouse drag selection is currently active. */
	let isDragging = false;
	/** The character offset where the drag started (the "anchor" of the selection). */
	let dragAnchorOffset = 0;

	/**
	 * Convert a MouseEvent's client coordinates to canvas-space coordinates,
	 * accounting for CSS scaling (i.e. canvas.width vs getBoundingClientRect).
	 */
	function canvasCoordsFromEvent(e: MouseEvent): { x: number; y: number } {
		const rect = canvas.getBoundingClientRect();
		const scaleX = canvas.width / rect.width;
		const scaleY = canvas.height / rect.height;
		return {
			x: (e.clientX - rect.left) * scaleX,
			y: (e.clientY - rect.top) * scaleY,
		};
	}

	canvas.addEventListener("mousedown", (e: MouseEvent) => {
		const { x, y } = canvasCoordsFromEvent(e);

		try {
			const offset = inner.hit_test(x, y);
			cursorOffset = offset;
			dragAnchorOffset = offset;
			isDragging = true;
			inner.set_selection(offset, offset);
			renderFrame();
		} catch {
			// hit_test may fail if canvas is detached; fall back to focusing.
		}

		textarea.focus();
	});

	/**
	 * During a drag, extend the selection from the anchor to the current
	 * mouse position on every mousemove.
	 */
	function onMouseMove(e: MouseEvent) {
		if (!isDragging) return;

		const { x, y } = canvasCoordsFromEvent(e);

		try {
			const offset = inner.hit_test(x, y);
			cursorOffset = offset;
			inner.set_selection(dragAnchorOffset, offset);
			renderFrame();
		} catch {
			// hit_test may fail if canvas is detached.
		}
	}

	/** End the drag on mouseup. */
	function onMouseUp(_e: MouseEvent) {
		if (!isDragging) return;
		isDragging = false;
	}

	// Listen on `document` so we capture mousemove/mouseup even when the
	// pointer leaves the canvas bounds (standard drag-select UX).
	document.addEventListener("mousemove", onMouseMove);
	document.addEventListener("mouseup", onMouseUp);

	// --- Rendering helper ---
	function renderFrame() {
		// Reset the caret blink so it's fully visible right after an action.
		resetCaretBlink();
		try {
			inner.render();
			syncA11yState();
		} catch {
			// Canvas may not be in DOM during tests.
		}
	}

	// --- Input handling (contenteditable parity) ---

	textarea.addEventListener("input", (e: Event) => {
		const ie = e as InputEvent;
		if (ie.isComposing) return; // handled by compositionend

		const data = ie.data;
		if (data) {
			syncTime();
			inner.insert_text_at(cursorOffset, data);
			cursorOffset += data.length;
			renderFrame();
		}
	});

	textarea.addEventListener("compositionend", (e: Event) => {
		const ce = e as CompositionEvent;
		if (ce.data) {
			syncTime();
			inner.insert_text_at(cursorOffset, ce.data);
			cursorOffset += ce.data.length;
			renderFrame();
		}
		// Clear the textarea so next composition starts fresh.
		textarea.value = "";
	});

	// --- Clipboard handling (copy / cut / paste) ---

	textarea.addEventListener("copy", (e: ClipboardEvent) => {
		e.preventDefault();
		const selected = inner.get_selected_text();
		if (selected && e.clipboardData) {
			e.clipboardData.setData("text/plain", selected);
		}
	});

	textarea.addEventListener("cut", (e: ClipboardEvent) => {
		e.preventDefault();
		const selected = inner.get_selected_text();
		if (selected && e.clipboardData) {
			e.clipboardData.setData("text/plain", selected);
			syncTime();
			inner.clipboard_cut();
			cursorOffset = inner.selection_start();
			renderFrame();
		}
	});

	textarea.addEventListener("paste", (e: ClipboardEvent) => {
		e.preventDefault();
		const text = e.clipboardData?.getData("text/plain");
		if (text) {
			syncTime();
			inner.clipboard_paste(text);
			cursorOffset = inner.selection_start();
			renderFrame();
		}
	});

	function syncA11yState() {
		const text = inner.plain_text();
		textarea.value = text;
		const start = inner.selection_start();
		const end = inner.selection_end();
		textarea.setSelectionRange(start, end);
		canvas.setAttribute("aria-valuetext", text);
	}

	textarea.addEventListener("keydown", (e: KeyboardEvent) => {
		if (e.isComposing) return;

		// Clipboard shortcuts — let the browser fire native clipboard events
		// on the textarea. We do NOT preventDefault so copy/cut/paste events
		// propagate normally.
		if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey) {
			if (e.key === "c" || e.key === "x" || e.key === "v") {
				// Allow default so the browser fires the clipboard event.
				return;
			}
		}

		// Undo: Ctrl+Z / Cmd+Z
		if (e.key === "z" && (e.ctrlKey || e.metaKey) && !e.shiftKey) {
			e.preventDefault();
			syncTime();
			if (inner.undo()) {
				cursorOffset = inner.selection_start();
				renderFrame();
			}
			return;
		}

		// Redo: Ctrl+Shift+Z / Cmd+Shift+Z  or  Ctrl+Y / Cmd+Y
		if (
			(e.key === "z" && (e.ctrlKey || e.metaKey) && e.shiftKey) ||
			(e.key === "y" && (e.ctrlKey || e.metaKey))
		) {
			e.preventDefault();
			syncTime();
			if (inner.redo()) {
				cursorOffset = inner.selection_start();
				renderFrame();
			}
			return;
		}

		switch (e.key) {
			case "Enter": {
				e.preventDefault();
				syncTime();
				inner.insert_text_at(cursorOffset, "\n");
				cursorOffset += 1;
				renderFrame();
				break;
			}
			case "Backspace": {
				e.preventDefault();
				if (cursorOffset > 0) {
					syncTime();
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
					syncTime();
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
				syncTime();
				ref.insert_text_at(cursorOffset, text);
				cursorOffset += text.length;
				renderFrame();
			},
			insertTextAt(offset: number, text: string) {
				syncTime();
				ref.insert_text_at(offset, text);
				cursorOffset = offset + text.length;
				renderFrame();
			},
			deleteRange(start: number, end: number) {
				syncTime();
				ref.delete_range(start, end);
				cursorOffset = start;
				renderFrame();
			},
			undo() {
				syncTime();
				const did = ref.undo();
				if (did) {
					cursorOffset = ref.selection_start();
					renderFrame();
				}
				return did;
			},
			redo() {
				syncTime();
				const did = ref.redo();
				if (did) {
					cursorOffset = ref.selection_start();
					renderFrame();
				}
				return did;
			},
			get canUndo() {
				return ref.can_undo();
			},
			get canRedo() {
				return ref.can_redo();
			},
			breakUndoCoalescing() {
				ref.break_undo_coalescing();
			},
			setCoalesceTimeout(ms: number) {
				ref.set_coalesce_timeout(ms);
			},
			get coalesceTimeout() {
				return ref.coalesce_timeout();
			},
			queueTextInput(text: string) {
				syncTime();
				ref.queue_text_input(text);
			},
			queueKeyDown(key: string) {
				syncTime();
				ref.queue_key_down(key);
			},
			queueKeyDownWithModifiers(key: string, modifiers) {
				syncTime();
				ref.queue_key_down_with_modifiers(
					key,
					Boolean(modifiers?.shift),
					Boolean(modifiers?.control),
					Boolean(modifiers?.alt),
					Boolean(modifiers?.meta),
					Boolean(modifiers?.repeat),
				);
			},
			processEvents() {
				syncTime();
				ref.process_events();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			get selectionStart() {
				return ref.selection_start();
			},
			get selectionEnd() {
				return ref.selection_end();
			},
			setSelection(start: number, end: number) {
				ref.set_selection(start, end);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			moveCursorTo(position: number, extend = false) {
				ref.move_cursor_to(position, extend);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			moveCursorLeft(extend = false) {
				ref.move_cursor_left(extend);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			moveCursorRight(extend = false) {
				ref.move_cursor_right(extend);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			applyStyleRange(start: number, end: number, style = {}) {
				syncTime();
				ref.apply_style_range(
					start,
					end,
					Boolean(style.bold),
					Boolean(style.italic),
					Boolean(style.underline),
					style.fontSize,
					style.fontFamily,
					style.colorRgba,
				);
				renderFrame();
			},
			replayOperationsJson(operationsJson: string) {
				syncTime();
				ref.replay_operations_json(operationsJson);
				cursorOffset = ref.selection_end();
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
			// Stop the caret blink timer.
			if (caretBlinkTimer !== null) {
				clearInterval(caretBlinkTimer);
				caretBlinkTimer = null;
			}
			// Remove document-level drag listeners to prevent leaks.
			document.removeEventListener("mousemove", onMouseMove);
			document.removeEventListener("mouseup", onMouseUp);
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
