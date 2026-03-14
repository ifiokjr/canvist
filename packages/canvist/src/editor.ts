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
	 * Convert a MouseEvent's client coordinates to canvas logical coordinates.
	 * WASM hit_test uses CSS (logical) dimensions, not DPR-scaled pixels.
	 */
	function canvasCoordsFromEvent(e: MouseEvent): { x: number; y: number } {
		const rect = canvas.getBoundingClientRect();
		return {
			x: e.clientX - rect.left,
			y: e.clientY - rect.top,
		};
	}

	// --- Multi-click tracking (double/triple-click) ---
	let clickCount = 0;
	let lastClickTime = 0;

	canvas.addEventListener("mousedown", (e: MouseEvent) => {
		const { x, y } = canvasCoordsFromEvent(e);
		const now = Date.now();
		clickCount = now - lastClickTime < 400 ? clickCount + 1 : 1;
		lastClickTime = now;

		try {
			const offset = inner.hit_test(x, y);
			textarea.focus();

			if (clickCount === 3) {
				// Triple-click: select entire line/paragraph.
				const text = inner.plain_text();
				let start = offset;
				while (start > 0 && text[start - 1] !== "\n") start--;
				let end = offset;
				while (end < text.length && text[end] !== "\n") end++;
				cursorOffset = end;
				dragAnchorOffset = start;
				inner.set_selection(start, end);
				clickCount = 0;
			} else if (clickCount === 2) {
				// Double-click: select word.
				inner.select_word_at(offset);
				cursorOffset = inner.selection_end();
				dragAnchorOffset = inner.selection_start();
			} else {
				// Single click: position cursor, start drag.
				cursorOffset = offset;
				dragAnchorOffset = offset;
				isDragging = true;
				inner.set_selection(offset, offset);
			}

			renderFrame();
		} catch {
			// hit_test may fail if canvas is detached.
		}

		e.preventDefault();
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

	// --- Mouse wheel scroll ---
	canvas.addEventListener(
		"wheel",
		(e: WheelEvent) => {
			e.preventDefault();
			inner.scroll_by(e.deltaY);
			try {
				inner.render();
			} catch {
				// Canvas may be detached.
			}
		},
		{ passive: false },
	);

	// --- Focus / blur handling ---
	textarea.addEventListener("focus", () => {
		inner.set_focused(true);
		try {
			inner.render();
		} catch {}
	});
	textarea.addEventListener("blur", () => {
		inner.set_focused(false);
		try {
			inner.render();
		} catch {}
	});

	// --- Scroll-to-caret helper ---
	function scrollToCaret() {
		try {
			const caretInfo = inner.caret_y(); // [y, height]
			const caretTop = caretInfo[0];
			const caretBottom = caretTop + caretInfo[1];
			const viewTop = inner.scroll_y();
			const viewBottom = viewTop + canvas.getBoundingClientRect().height;
			const margin = 24;
			if (caretTop < viewTop + margin) {
				inner.set_scroll_y(Math.max(0, caretTop - margin));
			} else if (caretBottom > viewBottom - margin) {
				inner.set_scroll_y(
					caretBottom - canvas.getBoundingClientRect().height + margin,
				);
			}
		} catch {
			// scroll APIs may not be available in tests.
		}
	}

	// --- Rendering helper ---
	function renderFrame() {
		// Reset the caret blink so it's fully visible right after an action.
		resetCaretBlink();
		try {
			scrollToCaret();
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
			// Auto-surround: if text is selected and auto-surround is on,
			// wrap the selection instead of replacing it.
			const autoCloseChars = "([{\"'`";
			if (
				data.length === 1 && autoCloseChars.includes(data) &&
				inner.auto_surround() &&
				inner.selection_start() !== inner.selection_end()
			) {
				inner.try_auto_surround(data);
				cursorOffset = inner.selection_end();
				renderFrame();
				return;
			}
			// Bracket auto-close: if the input is an opening bracket and
			// auto-close is enabled, insert the pair.
			if (
				data.length === 1 && autoCloseChars.includes(data) &&
				inner.auto_close_brackets()
			) {
				inner.insert_with_auto_close(data);
				cursorOffset = inner.selection_end();
			} else if (inner.overwrite_mode()) {
				inner.insert_text_overwrite(data);
				cursorOffset = inner.selection_end();
			} else {
				inner.insert_text_at(cursorOffset, data);
				cursorOffset += data.length;
			}
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
			inner.clipboard_ring_push(selected);
		} else if (e.clipboardData) {
			const lineText = inner.current_line_text();
			if (lineText) {
				e.clipboardData.setData("text/plain", lineText);
				inner.clipboard_ring_push(lineText);
			}
		}
	});

	textarea.addEventListener("cut", (e: ClipboardEvent) => {
		e.preventDefault();
		const selected = inner.get_selected_text();
		if (selected && e.clipboardData) {
			e.clipboardData.setData("text/plain", selected);
			inner.clipboard_ring_push(selected);
			syncTime();
			inner.clipboard_cut();
			cursorOffset = inner.selection_start();
			renderFrame();
		} else if (e.clipboardData) {
			syncTime();
			const lineText = inner.cut_line();
			if (lineText) {
				e.clipboardData.setData("text/plain", lineText);
				inner.clipboard_ring_push(lineText);
			}
			cursorOffset = inner.selection_end();
			renderFrame();
		}
	});

	textarea.addEventListener("paste", (e: ClipboardEvent) => {
		e.preventDefault();
		// Prefer HTML paste for rich formatting preservation.
		const html = e.clipboardData?.getData("text/html");
		if (html) {
			syncTime();
			inner.paste_html(html);
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}
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
		const mod = e.ctrlKey || e.metaKey;
		const textLength = inner.plain_text().length;

		function startOrExtendSelection(newOffset: number) {
			if (e.shiftKey) {
				const anchor = inner.selection_start() === cursorOffset
					? inner.selection_end()
					: inner.selection_start();
				cursorOffset = newOffset;
				inner.set_selection(anchor, cursorOffset);
			} else {
				cursorOffset = newOffset;
				inner.set_selection(cursorOffset, cursorOffset);
			}
		}

		// Clipboard — let browser fire native events.
		if (mod && !e.shiftKey && !e.altKey) {
			if (e.key === "c" || e.key === "x" || e.key === "v") return;
		}

		// Select all.
		if (mod && e.key === "a") {
			e.preventDefault();
			inner.select_all();
			cursorOffset = textLength;
			renderFrame();
			return;
		}

		// Formatting toggles.
		if (mod && e.key === "b") {
			e.preventDefault();
			syncTime();
			inner.toggle_bold();
			renderFrame();
			return;
		}
		if (mod && e.key === "i") {
			e.preventDefault();
			syncTime();
			inner.toggle_italic();
			renderFrame();
			return;
		}
		if (mod && e.key === "u") {
			e.preventDefault();
			syncTime();
			inner.toggle_underline();
			renderFrame();
			return;
		}

		// Strikethrough — Ctrl+Shift+S.
		if (mod && e.shiftKey && e.key === "S") {
			e.preventDefault();
			syncTime();
			inner.toggle_strikethrough();
			renderFrame();
			return;
		}

		// Undo / Redo.
		if (e.key === "z" && mod && !e.shiftKey) {
			e.preventDefault();
			syncTime();
			if (inner.undo()) {
				cursorOffset = inner.selection_start();
				renderFrame();
			}
			return;
		}
		if (
			(e.key === "z" && mod && e.shiftKey) ||
			(e.key === "y" && mod)
		) {
			e.preventDefault();
			syncTime();
			if (inner.redo()) {
				cursorOffset = inner.selection_start();
				renderFrame();
			}
			return;
		}

		// Enter — auto-indent + list continuation.
		if (e.key === "Enter") {
			e.preventDefault();
			syncTime();
			const inserted = inner.auto_indent_newline();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Delete line — Ctrl+Shift+K.
		if (mod && e.shiftKey && e.key === "K") {
			e.preventDefault();
			syncTime();
			inner.delete_line();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Join lines — Ctrl+J.
		if (mod && e.key === "j") {
			e.preventDefault();
			syncTime();
			inner.join_lines();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Transform uppercase — Ctrl+Shift+U.
		if (mod && e.shiftKey && e.key === "U") {
			e.preventDefault();
			syncTime();
			inner.transform_uppercase();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Transform lowercase — Ctrl+Shift+L.
		if (mod && e.shiftKey && e.key === "L") {
			e.preventDefault();
			syncTime();
			inner.transform_lowercase();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Duplicate line — Ctrl+Shift+D.
		if (mod && e.shiftKey && e.key === "D") {
			e.preventDefault();
			syncTime();
			inner.duplicate_line();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Move line up — Alt+ArrowUp.
		if (e.altKey && e.key === "ArrowUp" && !mod) {
			e.preventDefault();
			syncTime();
			inner.move_line_up();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Move line down — Alt+ArrowDown.
		if (e.altKey && e.key === "ArrowDown" && !mod) {
			e.preventDefault();
			syncTime();
			inner.move_line_down();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Zoom — Ctrl+= / Ctrl+- / Ctrl+0.
		if (mod && (e.key === "=" || e.key === "+")) {
			e.preventDefault();
			inner.zoom_in();
			renderFrame();
			return;
		}
		if (mod && e.key === "-") {
			e.preventDefault();
			inner.zoom_out();
			renderFrame();
			return;
		}
		if (mod && e.key === "0") {
			e.preventDefault();
			inner.zoom_reset();
			renderFrame();
			return;
		}

		// Tab / Shift+Tab for indent/outdent.
		if (e.key === "Tab") {
			e.preventDefault();
			syncTime();
			if (e.shiftKey) {
				inner.outdent_selection();
			} else {
				const sel = {
					start: inner.selection_start(),
					end: inner.selection_end(),
				};
				if (sel.start !== sel.end) {
					inner.indent_selection();
				} else {
					inner.insert_tab();
				}
			}
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Page Up / Page Down.
		if (e.key === "PageUp") {
			e.preventDefault();
			let pos = cursorOffset;
			for (let i = 0; i < 20; i++) {
				try {
					const above = inner.offset_above(pos);
					if (above === pos) break;
					pos = above;
				} catch {
					break;
				}
			}
			startOrExtendSelection(pos);
			renderFrame();
			return;
		}
		if (e.key === "PageDown") {
			e.preventDefault();
			let pos = cursorOffset;
			for (let i = 0; i < 20; i++) {
				try {
					const below = inner.offset_below(pos);
					if (below === pos) break;
					pos = below;
				} catch {
					break;
				}
			}
			startOrExtendSelection(pos);
			renderFrame();
			return;
		}

		// Cursor history back — Ctrl+Alt+Left.
		if (mod && e.altKey && e.key === "ArrowLeft") {
			e.preventDefault();
			inner.cursor_history_back();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Cursor history forward — Ctrl+Alt+Right.
		if (mod && e.altKey && e.key === "ArrowRight") {
			e.preventDefault();
			inner.cursor_history_forward();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Paragraph navigation — Ctrl+Up / Ctrl+Down.
		if (mod && !e.shiftKey && e.key === "ArrowUp") {
			e.preventDefault();
			inner.push_cursor_history();
			inner.move_to_prev_paragraph();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}
		if (mod && !e.shiftKey && e.key === "ArrowDown") {
			e.preventDefault();
			inner.push_cursor_history();
			inner.move_to_next_paragraph();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Insert key — toggle overwrite mode.
		if (e.key === "Insert" && !mod && !e.shiftKey) {
			e.preventDefault();
			inner.toggle_overwrite_mode();
			return;
		}

		// Ctrl+Home — go to document start.
		if (mod && !e.shiftKey && e.key === "Home") {
			e.preventDefault();
			inner.go_to_document_start();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Ctrl+Shift+Home — select to document start.
		if (mod && e.shiftKey && e.key === "Home") {
			e.preventDefault();
			inner.select_to_document_start();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Ctrl+End — go to document end.
		if (mod && !e.shiftKey && e.key === "End") {
			e.preventDefault();
			inner.go_to_document_end();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Ctrl+Shift+End — select to document end.
		if (mod && e.shiftKey && e.key === "End") {
			e.preventDefault();
			inner.select_to_document_end();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Move to matching bracket — Ctrl+Shift+\.
		if (mod && e.shiftKey && e.key === "|") {
			e.preventDefault();
			inner.move_to_matching_bracket();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Open line below — Ctrl+Enter.
		if (mod && !e.shiftKey && e.key === "Enter") {
			e.preventDefault();
			syncTime();
			inner.open_line_below();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Open line above — Ctrl+Shift+Enter.
		if (mod && e.shiftKey && e.key === "Enter") {
			e.preventDefault();
			syncTime();
			inner.open_line_above();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Transpose characters — Ctrl+T.
		if (mod && e.key === "t") {
			e.preventDefault();
			syncTime();
			inner.transpose_chars();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Toggle line comment — Ctrl+/.
		if (mod && e.key === "/") {
			e.preventDefault();
			syncTime();
			inner.toggle_line_comment();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Expand selection — Ctrl+Shift+E.
		if (mod && e.shiftKey && e.key === "E") {
			e.preventDefault();
			inner.expand_selection();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Contract selection — Ctrl+Shift+R (not reload, intercepted).
		if (mod && e.shiftKey && e.key === "W") {
			e.preventDefault();
			inner.contract_selection();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Select line — Ctrl+L.
		if (mod && e.key === "l") {
			e.preventDefault();
			inner.select_line();
			cursorOffset = inner.selection_end();
			renderFrame();
			return;
		}

		// Backspace.
		if (e.key === "Backspace") {
			e.preventDefault();
			syncTime();
			const selStart = inner.selection_start();
			const selEnd = inner.selection_end();
			if (selStart !== selEnd) {
				inner.delete_range(selStart, selEnd);
				cursorOffset = selStart;
			} else if (mod) {
				const wb = inner.word_boundary_left(cursorOffset);
				if (wb < cursorOffset) {
					inner.delete_range(wb, cursorOffset);
					cursorOffset = wb;
				}
			} else if (inner.auto_close_brackets() && inner.smart_backspace()) {
				// Smart backspace deleted a bracket pair.
				cursorOffset = inner.selection_end();
			} else if (cursorOffset > 0) {
				inner.delete_range(cursorOffset - 1, cursorOffset);
				cursorOffset -= 1;
			}
			inner.set_selection(cursorOffset, cursorOffset);
			renderFrame();
			return;
		}

		// Delete.
		if (e.key === "Delete") {
			e.preventDefault();
			syncTime();
			const selStart = inner.selection_start();
			const selEnd = inner.selection_end();
			if (selStart !== selEnd) {
				inner.delete_range(selStart, selEnd);
				cursorOffset = selStart;
			} else if (mod) {
				const wb = inner.word_boundary_right(cursorOffset);
				if (wb > cursorOffset) {
					inner.delete_range(cursorOffset, wb);
				}
			} else if (cursorOffset < textLength) {
				inner.delete_range(cursorOffset, cursorOffset + 1);
			}
			inner.set_selection(cursorOffset, cursorOffset);
			renderFrame();
			return;
		}

		// Home / End.
		if (e.key === "Home") {
			e.preventDefault();
			try {
				const target = mod ? 0 : inner.line_start_for_offset(cursorOffset);
				startOrExtendSelection(target);
			} catch {
				startOrExtendSelection(0);
			}
			renderFrame();
			return;
		}
		if (e.key === "End") {
			e.preventDefault();
			try {
				const target = mod
					? textLength
					: inner.line_end_for_offset(cursorOffset);
				startOrExtendSelection(target);
			} catch {
				startOrExtendSelection(textLength);
			}
			renderFrame();
			return;
		}

		// Arrow Up / Down.
		if (e.key === "ArrowUp") {
			e.preventDefault();
			try {
				startOrExtendSelection(inner.offset_above(cursorOffset));
			} catch {
				startOrExtendSelection(0);
			}
			renderFrame();
			return;
		}
		if (e.key === "ArrowDown") {
			e.preventDefault();
			try {
				startOrExtendSelection(inner.offset_below(cursorOffset));
			} catch {
				startOrExtendSelection(textLength);
			}
			renderFrame();
			return;
		}

		// Arrow Left / Right.
		if (e.key === "ArrowLeft") {
			e.preventDefault();
			if (mod) {
				startOrExtendSelection(inner.word_boundary_left(cursorOffset));
			} else {
				const selStart = inner.selection_start();
				const selEnd = inner.selection_end();
				if (!e.shiftKey && selStart !== selEnd) {
					cursorOffset = selStart;
					inner.set_selection(cursorOffset, cursorOffset);
				} else {
					startOrExtendSelection(Math.max(0, cursorOffset - 1));
				}
			}
			renderFrame();
			return;
		}
		if (e.key === "ArrowRight") {
			e.preventDefault();
			if (mod) {
				startOrExtendSelection(inner.word_boundary_right(cursorOffset));
			} else {
				const selStart = inner.selection_start();
				const selEnd = inner.selection_end();
				if (!e.shiftKey && selStart !== selEnd) {
					cursorOffset = selEnd;
					inner.set_selection(cursorOffset, cursorOffset);
				} else {
					startOrExtendSelection(Math.min(textLength, cursorOffset + 1));
				}
			}
			renderFrame();
			return;
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
			toHTML() {
				return ref.to_html();
			},
			toMarkdown() {
				return ref.to_markdown();
			},
			fromHTML(html: string) {
				ref.from_html(html);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			pasteHTML(html: string) {
				syncTime();
				ref.paste_html(html);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			toggleBold() {
				syncTime();
				ref.toggle_bold();
				renderFrame();
			},
			toggleItalic() {
				syncTime();
				ref.toggle_italic();
				renderFrame();
			},
			toggleUnderline() {
				syncTime();
				ref.toggle_underline();
				renderFrame();
			},
			toggleStrikethrough() {
				syncTime();
				ref.toggle_strikethrough();
				renderFrame();
			},
			get isBold() {
				return ref.is_bold();
			},
			get isItalic() {
				return ref.is_italic();
			},
			get isUnderline() {
				return ref.is_underline();
			},
			setFontSize(size: number) {
				syncTime();
				ref.set_font_size(size);
				renderFrame();
			},
			setColor(r: number, g: number, b: number, a: number) {
				syncTime();
				ref.set_color(r, g, b, a);
				renderFrame();
			},
			selectAll() {
				ref.select_all();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			selectWordAt(offset: number) {
				ref.select_word_at(offset);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			get selectedText() {
				return ref.get_selected_text();
			},
			findAll(needle: string, caseSensitive = false) {
				const flat = ref.find_all(needle, caseSensitive);
				const results: Array<{ start: number; end: number }> = [];
				for (let i = 0; i < flat.length; i += 2) {
					results.push({ start: flat[i], end: flat[i + 1] });
				}
				return results;
			},
			findNext(needle: string, fromOffset: number, caseSensitive = false) {
				const r = ref.find_next(needle, fromOffset, caseSensitive);
				return r.length === 2 ? { start: r[0], end: r[1] } : null;
			},
			findPrev(needle: string, fromOffset: number, caseSensitive = false) {
				const r = ref.find_prev(needle, fromOffset, caseSensitive);
				return r.length === 2 ? { start: r[0], end: r[1] } : null;
			},
			replaceRange(start: number, end: number, replacement: string) {
				syncTime();
				ref.replace_range(start, end, replacement);
				cursorOffset = start + replacement.length;
				renderFrame();
			},
			replaceAll(needle: string, replacement: string, caseSensitive = false) {
				syncTime();
				const count = ref.replace_all(needle, replacement, caseSensitive);
				cursorOffset = ref.selection_end();
				renderFrame();
				return count;
			},
			// ── Scroll ──────────────────────────────────
			get scrollY() {
				return ref.scroll_y();
			},
			setScrollY(y: number) {
				ref.set_scroll_y(y);
				renderFrame();
			},
			scrollBy(deltaY: number) {
				ref.scroll_by(deltaY);
				renderFrame();
			},
			get contentHeight() {
				return ref.content_height();
			},
			get caretY(): [number, number] {
				const r = ref.caret_y();
				return [r[0], r[1]];
			},
			// ── Focus ───────────────────────────────────
			get focused() {
				return ref.focused();
			},
			setFocused(focused: boolean) {
				ref.set_focused(focused);
				renderFrame();
			},
			// ── Statistics ──────────────────────────────
			get wordCount() {
				return ref.word_count();
			},
			get lineCount() {
				return ref.line_count();
			},
			get cursorLine() {
				return ref.cursor_line();
			},
			get cursorColumn() {
				return ref.cursor_column();
			},
			// ── Size ────────────────────────────────────
			setSize(width: number, height: number) {
				ref.set_size(width, height);
				renderFrame();
			},
			// ── Read-only ───────────────────────────────
			get readOnly() {
				return ref.read_only();
			},
			setReadOnly(readOnly: boolean) {
				ref.set_read_only(readOnly);
			},
			// ── Line numbers ────────────────────────────
			get showLineNumbers() {
				return ref.show_line_numbers();
			},
			setShowLineNumbers(show: boolean) {
				ref.set_show_line_numbers(show);
				renderFrame();
			},
			// ── Indentation ─────────────────────────────
			indentSelection() {
				syncTime();
				ref.indent_selection();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			outdentSelection() {
				syncTime();
				ref.outdent_selection();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Theme ───────────────────────────────────
			setThemeDark() {
				ref.set_theme_dark();
				renderFrame();
			},
			setThemeLight() {
				ref.set_theme_light();
				renderFrame();
			},
			get themeName() {
				return ref.theme_name();
			},
			// ── Zoom ────────────────────────────────────
			get zoom() {
				return ref.zoom();
			},
			setZoom(level: number) {
				ref.set_zoom(level);
				renderFrame();
			},
			zoomIn() {
				ref.zoom_in();
				renderFrame();
			},
			zoomOut() {
				ref.zoom_out();
				renderFrame();
			},
			zoomReset() {
				ref.zoom_reset();
				renderFrame();
			},
			// ── Current line highlight ──────────────────
			get highlightCurrentLine() {
				return ref.highlight_current_line();
			},
			setHighlightCurrentLine(enabled: boolean) {
				ref.set_highlight_current_line(enabled);
				renderFrame();
			},
			// ── Drag and drop ───────────────────────────
			moveText(srcStart: number, srcEnd: number, destOffset: number) {
				syncTime();
				ref.move_text(srcStart, srcEnd, destOffset);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Word wrap ───────────────────────────────
			get wordWrap() {
				return ref.word_wrap();
			},
			setWordWrap(enabled: boolean) {
				ref.set_word_wrap(enabled);
				renderFrame();
			},
			// ── Lists ───────────────────────────────────
			toggleBulletList() {
				syncTime();
				ref.toggle_bullet_list();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			toggleNumberedList() {
				syncTime();
				ref.toggle_numbered_list();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Delete / join line ───────────────────────
			deleteLine() {
				syncTime();
				ref.delete_line();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			joinLines() {
				syncTime();
				ref.join_lines();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Transform case ──────────────────────────
			transformUppercase() {
				syncTime();
				ref.transform_uppercase();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			transformLowercase() {
				syncTime();
				ref.transform_lowercase();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			transformTitleCase() {
				syncTime();
				ref.transform_title_case();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Sort lines ──────────────────────────────
			sortLinesAsc() {
				syncTime();
				ref.sort_lines_asc();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			sortLinesDesc() {
				syncTime();
				ref.sort_lines_desc();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Show whitespace ─────────────────────────
			get showWhitespace() {
				return ref.show_whitespace();
			},
			setShowWhitespace(show: boolean) {
				ref.set_show_whitespace(show);
				renderFrame();
			},
			// ── Bracket auto-close ──────────────────────
			get autoCloseBrackets() {
				return ref.auto_close_brackets();
			},
			setAutoCloseBrackets(enabled: boolean) {
				ref.set_auto_close_brackets(enabled);
			},
			// ── Delete word ─────────────────────────────
			deleteWordLeft() {
				syncTime();
				ref.delete_word_left();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			deleteWordRight() {
				syncTime();
				ref.delete_word_right();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Select line ─────────────────────────────
			selectLine() {
				ref.select_line();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Utility commands ────────────────────────
			trimTrailingWhitespace() {
				syncTime();
				const n = ref.trim_trailing_whitespace();
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			removeDuplicateLines() {
				syncTime();
				const n = ref.remove_duplicate_lines();
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			wrapSelection(open: string, close: string) {
				syncTime();
				ref.wrap_selection(open, close);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Transpose characters ────────────────────
			transposeChars() {
				syncTime();
				ref.transpose_chars();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Toggle line comment ─────────────────────
			get commentPrefix() {
				return ref.comment_prefix();
			},
			setCommentPrefix(prefix: string) {
				ref.set_comment_prefix(prefix);
			},
			toggleLineComment() {
				syncTime();
				ref.toggle_line_comment();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Soft tabs ───────────────────────────────
			get tabSize() {
				return ref.tab_size();
			},
			setTabSize(size: number) {
				ref.set_tab_size(size);
			},
			get softTabs() {
				return ref.soft_tabs();
			},
			setSoftTabs(enabled: boolean) {
				ref.set_soft_tabs(enabled);
			},
			insertTab() {
				syncTime();
				ref.insert_tab();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Auto-surround ───────────────────────────
			get autoSurround() {
				return ref.auto_surround();
			},
			setAutoSurround(enabled: boolean) {
				ref.set_auto_surround(enabled);
			},
			// ── Expand / contract selection ──────────────
			expandSelection() {
				ref.expand_selection();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			contractSelection() {
				ref.contract_selection();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Matching bracket highlight ───────────────
			get highlightMatchingBrackets() {
				return ref.highlight_matching_brackets();
			},
			setHighlightMatchingBrackets(enabled: boolean) {
				ref.set_highlight_matching_brackets(enabled);
				renderFrame();
			},
			findMatchingBracket(offset: number) {
				return ref.find_matching_bracket(offset);
			},
			// ── Move to matching bracket ────────────────
			moveToMatchingBracket() {
				ref.move_to_matching_bracket();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Document statistics (extras) ────────────
			get paragraphCount() {
				return ref.paragraph_count();
			},
			get currentLineNumber() {
				return ref.current_line_number();
			},
			get currentColumn() {
				return ref.current_column();
			},
			// ── Indent guides ───────────────────────────
			get showIndentGuides() {
				return ref.show_indent_guides();
			},
			setShowIndentGuides(show: boolean) {
				ref.set_show_indent_guides(show);
				renderFrame();
			},
			// ── Bookmarks ───────────────────────────────
			toggleBookmark() {
				const added = ref.toggle_bookmark();
				renderFrame();
				return added;
			},
			nextBookmark() {
				const found = ref.next_bookmark();
				if (found) {
					cursorOffset = ref.selection_end();
					renderFrame();
				}
				return found;
			},
			prevBookmark() {
				const found = ref.prev_bookmark();
				if (found) {
					cursorOffset = ref.selection_end();
					renderFrame();
				}
				return found;
			},
			clearBookmarks() {
				ref.clear_bookmarks();
				renderFrame();
			},
			get bookmarkCount() {
				return ref.bookmark_count();
			},
			get isLineBookmarked() {
				return ref.is_line_bookmarked();
			},
			// ── Convert indentation ─────────────────────
			tabsToSpaces() {
				syncTime();
				const n = ref.tabs_to_spaces();
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			spacesToTabs() {
				syncTime();
				const n = ref.spaces_to_tabs();
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			// ── Open line above / below ─────────────────
			openLineBelow() {
				syncTime();
				ref.open_line_below();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			openLineAbove() {
				syncTime();
				ref.open_line_above();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Copy / cut line (no selection) ──────────
			get currentLineText() {
				return ref.current_line_text();
			},
			cutLine() {
				syncTime();
				const text = ref.cut_line();
				cursorOffset = ref.selection_end();
				renderFrame();
				return text;
			},
			// ── Overwrite mode ──────────────────────────
			get overwriteMode() {
				return ref.overwrite_mode();
			},
			toggleOverwriteMode() {
				ref.toggle_overwrite_mode();
			},
			setOverwriteMode(enabled: boolean) {
				ref.set_overwrite_mode(enabled);
			},
			// ── Center line ─────────────────────────────
			centerLineInViewport() {
				ref.center_line_in_viewport();
				renderFrame();
			},
			// ── Document start / end ────────────────────
			goToDocumentStart() {
				ref.go_to_document_start();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			goToDocumentEnd() {
				ref.go_to_document_end();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			selectToDocumentStart() {
				ref.select_to_document_start();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			selectToDocumentEnd() {
				ref.select_to_document_end();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Select between brackets ─────────────────
			selectBetweenBrackets() {
				const found = ref.select_between_brackets();
				if (found) {
					cursorOffset = ref.selection_end();
					renderFrame();
				}
				return found;
			},
			// ── Cursor position history ─────────────────
			pushCursorHistory() {
				ref.push_cursor_history();
			},
			cursorHistoryBack() {
				const moved = ref.cursor_history_back();
				if (moved) {
					cursorOffset = ref.selection_end();
					renderFrame();
				}
				return moved;
			},
			cursorHistoryForward() {
				const moved = ref.cursor_history_forward();
				if (moved) {
					cursorOffset = ref.selection_end();
					renderFrame();
				}
				return moved;
			},
			get cursorHistoryLength() {
				return ref.cursor_history_length();
			},
			// ── Select all occurrences ──────────────────
			selectAllOccurrences() {
				return ref.select_all_occurrences();
			},
			occurrenceOffsets() {
				return Array.from(ref.occurrence_offsets());
			},
			// ── Whole word find ─────────────────────────
			findAllWholeWord(needle: string) {
				return Array.from(ref.find_all_whole_word(needle));
			},
			// ── Paragraph navigation ────────────────────
			moveToPrevParagraph() {
				ref.move_to_prev_paragraph();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			moveToNextParagraph() {
				ref.move_to_next_paragraph();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Snippet insertion ───────────────────────
			insertSnippet(template: string) {
				syncTime();
				ref.insert_snippet(template);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Scroll to selection ─────────────────────
			scrollToSelection() {
				ref.scroll_to_selection();
				renderFrame();
			},
			// ── Line decorations ────────────────────────
			addLineDecoration(
				line: number,
				r: number,
				g: number,
				b: number,
				a: number,
			) {
				ref.add_line_decoration(line, r, g, b, a);
				renderFrame();
			},
			removeLineDecorations(line: number) {
				ref.remove_line_decorations(line);
				renderFrame();
			},
			clearLineDecorations() {
				ref.clear_line_decorations();
				renderFrame();
			},
			get lineDecorationCount() {
				return ref.line_decoration_count();
			},
			// ── Modified state ──────────────────────────
			get isModified() {
				return ref.is_modified();
			},
			markSaved() {
				ref.mark_saved();
			},
			markModified() {
				ref.mark_modified();
			},
			// ── Clipboard ring ──────────────────────────
			clipboardRingPush(text: string) {
				ref.clipboard_ring_push(text);
			},
			clipboardRingGet(index: number) {
				return ref.clipboard_ring_get(index);
			},
			get clipboardRingLength() {
				return ref.clipboard_ring_length();
			},
			clipboardRingClear() {
				ref.clipboard_ring_clear();
			},
			clipboardRingPaste(index: number) {
				syncTime();
				ref.clipboard_ring_paste(index);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Word frequency ──────────────────────────
			wordFrequency(topN: number) {
				return Array.from(ref.word_frequency(topN));
			},
			// ── Highlight occurrences ───────────────────
			get highlightOccurrences() {
				return ref.highlight_occurrences();
			},
			setHighlightOccurrences(enabled: boolean) {
				ref.set_highlight_occurrences(enabled);
				renderFrame();
			},
			wordAtCursor() {
				return ref.word_at_cursor();
			},
			// ── Text measurement ────────────────────────
			measureTextWidth(text: string) {
				return ref.measure_text_width(text);
			},
			measureCharWidth(ch: string) {
				return ref.measure_char_width(ch);
			},
			// ── Column ruler ────────────────────────────
			setRulers(columns: number[]) {
				ref.set_rulers(new Uint32Array(columns));
				renderFrame();
			},
			get rulers() {
				return Array.from(ref.rulers());
			},
			addRuler(column: number) {
				ref.add_ruler(column);
				renderFrame();
			},
			removeRuler(column: number) {
				ref.remove_ruler(column);
				renderFrame();
			},
			// ── Ensure final newline ────────────────────
			ensureFinalNewline() {
				syncTime();
				const added = ref.ensure_final_newline();
				cursorOffset = ref.selection_end();
				renderFrame();
				return added;
			},
			// ── Replace all occurrences ─────────────────
			replaceAllOccurrences(replacement: string) {
				syncTime();
				const count = ref.replace_all_occurrences(replacement);
				cursorOffset = ref.selection_end();
				renderFrame();
				return count;
			},
			// ── Reverse lines ───────────────────────────
			reverseLines() {
				syncTime();
				ref.reverse_lines();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Encode / decode ─────────────────────────
			base64EncodeSelection() {
				syncTime();
				ref.base64_encode_selection();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			base64DecodeSelection() {
				syncTime();
				ref.base64_decode_selection();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			urlEncodeSelection() {
				syncTime();
				ref.url_encode_selection();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			urlDecodeSelection() {
				syncTime();
				ref.url_decode_selection();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Toggle case ─────────────────────────────
			transformToggleCase() {
				syncTime();
				ref.transform_toggle_case();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Auto-indent ─────────────────────────────
			autoIndentNewline() {
				syncTime();
				const n = ref.auto_indent_newline();
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			// ── Selection statistics ────────────────────
			get selectedCharCount() {
				return ref.selected_char_count();
			},
			get selectedWordCount() {
				return ref.selected_word_count();
			},
			// ── Go to line ──────────────────────────────
			goToLine(lineNumber: number) {
				ref.go_to_line(lineNumber);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Line operations ─────────────────────────
			duplicateLine() {
				syncTime();
				ref.duplicate_line();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			moveLineUp() {
				syncTime();
				ref.move_line_up();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			moveLineDown() {
				syncTime();
				ref.move_line_down();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Highlight colour ────────────────────────
			setHighlightColor(r: number, g: number, b: number, a: number) {
				syncTime();
				ref.set_highlight_color(r, g, b, a);
				renderFrame();
			},
			removeHighlightColor() {
				syncTime();
				ref.remove_highlight_color();
				renderFrame();
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
