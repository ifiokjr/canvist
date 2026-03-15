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
			// ── State serialization ─────────────────────
			saveState() {
				return ref.save_state();
			},
			restoreState(json: string) {
				ref.restore_state(json);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Placeholder text ────────────────────────
			get placeholder() {
				return ref.placeholder();
			},
			setPlaceholder(text: string) {
				ref.set_placeholder(text);
				renderFrame();
			},
			// ── Max length ──────────────────────────────
			get maxLength() {
				return ref.max_length();
			},
			setMaxLength(max: number) {
				ref.set_max_length(max);
			},
			get remainingCapacity() {
				return ref.remaining_capacity();
			},
			insertTextClamped(text: string) {
				syncTime();
				const n = ref.insert_text_clamped(text);
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			// ── Batch operations ────────────────────────
			beginBatch() {
				ref.begin_batch();
			},
			endBatch() {
				ref.end_batch();
			},
			// ── Regex find ──────────────────────────────
			findAllRegex(pattern: string) {
				return Array.from(ref.find_all_regex(pattern));
			},
			// ── Selection change detection ──────────────
			selectionChanged() {
				return ref.selection_changed();
			},
			// ── Wrap indicators ─────────────────────────
			get showWrapIndicators() {
				return ref.show_wrap_indicators();
			},
			setShowWrapIndicators(enabled: boolean) {
				ref.set_show_wrap_indicators(enabled);
				renderFrame();
			},
			// ── Selection anchor ────────────────────────
			get selectionAnchor() {
				return ref.selection_anchor();
			},
			get selectionIsCollapsed() {
				return ref.selection_is_collapsed();
			},
			get selectionLength() {
				return ref.selection_length();
			},
			// ── Character counts ────────────────────────
			charCounts() {
				return Array.from(ref.char_counts());
			},
			// ── Text hash ───────────────────────────────
			textHash() {
				return ref.text_hash();
			},
			// ── Event log ───────────────────────────────
			logEvent(event: string) {
				ref.log_event(event);
			},
			eventLogGet(index: number) {
				return ref.event_log_get(index);
			},
			get eventLogLength() {
				return ref.event_log_length();
			},
			eventLogClear() {
				ref.event_log_clear();
			},
			setEventLogMax(max: number) {
				ref.set_event_log_max(max);
			},
			// ── Word completion ─────────────────────────
			completions(maxResults: number) {
				return Array.from(ref.completions(maxResults));
			},
			// ── Line range operations ───────────────────
			getLineRange(startLine: number, endLine: number) {
				return ref.get_line_range(startLine, endLine);
			},
			setLineRange(startLine: number, endLine: number, text: string) {
				syncTime();
				ref.set_line_range(startLine, endLine, text);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			get lineCountTotal() {
				return ref.line_count_total();
			},
			getLine(line: number) {
				return ref.get_line(line);
			},
			// ── Scroll metrics ──────────────────────────
			get viewportHeight() {
				return ref.viewport_height();
			},
			get scrollRatio() {
				return ref.scroll_ratio();
			},
			get scrollFraction() {
				return ref.scroll_fraction();
			},
			scrollToFraction(fraction: number) {
				ref.scroll_to_fraction(fraction);
				renderFrame();
			},
			// ── Annotations ─────────────────────────────
			addAnnotation(start: number, end: number, kind: string, message: string) {
				ref.add_annotation(start, end, kind, message);
				renderFrame();
			},
			removeAnnotationsByKind(kind: string) {
				ref.remove_annotations_by_kind(kind);
				renderFrame();
			},
			clearAnnotations() {
				ref.clear_annotations();
				renderFrame();
			},
			get annotationCount() {
				return ref.annotation_count();
			},
			getAnnotations() {
				return Array.from(ref.get_annotations());
			},
			annotationsAt(offset: number) {
				return Array.from(ref.annotations_at(offset));
			},
			// ── Search history ──────────────────────────
			searchHistoryPush(term: string) {
				ref.search_history_push(term);
			},
			searchHistoryGet(index: number) {
				return ref.search_history_get(index);
			},
			get searchHistoryLength() {
				return ref.search_history_length();
			},
			searchHistoryClear() {
				ref.search_history_clear();
			},
			// ── Visible range ───────────────────────────
			get firstVisibleLine() {
				return ref.first_visible_line();
			},
			get lastVisibleLine() {
				return ref.last_visible_line();
			},
			get visibleLineCount() {
				return ref.visible_line_count();
			},
			// ── Minimap ─────────────────────────────────
			get showMinimap() {
				return ref.show_minimap();
			},
			setShowMinimap(enabled: boolean) {
				ref.set_show_minimap(enabled);
				renderFrame();
			},
			get minimapWidth() {
				return ref.minimap_width();
			},
			setMinimapWidth(w: number) {
				ref.set_minimap_width(w);
				renderFrame();
			},
			// ── Sticky scroll ───────────────────────────
			get stickyScroll() {
				return ref.sticky_scroll();
			},
			setStickyScroll(enabled: boolean) {
				ref.set_sticky_scroll(enabled);
				renderFrame();
			},
			// ── Rename all ──────────────────────────────
			renameAll(newName: string) {
				syncTime();
				const n = ref.rename_all(newName);
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			// ── Cursor style ────────────────────────────
			get cursorStyle() {
				return ref.cursor_style();
			},
			setCursorStyle(style: number) {
				ref.set_cursor_style(style);
				renderFrame();
			},
			get cursorWidthPx() {
				return ref.cursor_width_px();
			},
			setCursorWidth(w: number) {
				ref.set_cursor_width(w);
				renderFrame();
			},
			setCursorColor(r: number, g: number, b: number, a: number) {
				ref.set_cursor_color(r, g, b, a);
				renderFrame();
			},
			// ── Snapshot diff ───────────────────────────
			takeSnapshot() {
				ref.take_snapshot();
			},
			diffFromSnapshot() {
				return Array.from(ref.diff_from_snapshot());
			},
			get hasSnapshot() {
				return ref.has_snapshot();
			},
			clearSnapshot() {
				ref.clear_snapshot();
			},
			// ── Macro recording ─────────────────────────
			macroStartRecording() {
				ref.macro_start_recording();
			},
			macroStopRecording() {
				return ref.macro_stop_recording();
			},
			get macroIsRecording() {
				return ref.macro_is_recording();
			},
			macroRecordStep(kind: string, data: string) {
				ref.macro_record_step(kind, data);
			},
			get macroStepCount() {
				return ref.macro_step_count();
			},
			macroReplay() {
				syncTime();
				ref.macro_replay();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			macroSave(name: string) {
				ref.macro_save(name);
			},
			macroReplaySaved(name: string) {
				syncTime();
				const ok = ref.macro_replay_saved(name);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			macroListSaved() {
				return Array.from(ref.macro_list_saved());
			},
			macroDeleteSaved(name: string) {
				ref.macro_delete_saved(name);
			},
			// ── Find match highlights ───────────────────
			setFindHighlights(needle: string) {
				ref.set_find_highlights(needle);
				renderFrame();
			},
			get findHighlightNeedle() {
				return ref.find_highlight_needle();
			},
			get showFindHighlights() {
				return ref.show_find_highlights();
			},
			// ── Column/block selection ──────────────────
			getBlockSelection(sl: number, el: number, sc: number, ec: number) {
				return ref.get_block_selection(sl, el, sc, ec);
			},
			setBlockSelection(
				sl: number,
				el: number,
				sc: number,
				ec: number,
				text: string,
			) {
				syncTime();
				ref.set_block_selection(sl, el, sc, ec, text);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Smart paste ─────────────────────────────
			pasteWithIndent(text: string) {
				syncTime();
				ref.paste_with_indent(text);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Tokenize ────────────────────────────────
			tokenize() {
				return Array.from(ref.tokenize());
			},
			// ── Link detection ──────────────────────────
			setDetectLinks(enabled: boolean) {
				ref.set_detect_links(enabled);
				renderFrame();
			},
			get detectLinks() {
				return ref.detect_links();
			},
			findLinks() {
				return Array.from(ref.find_links());
			},
			linkAtOffset(offset: number) {
				return ref.link_at_offset(offset);
			},
			// ── Line folding ────────────────────────────
			foldLines(startLine: number, endLine: number) {
				ref.fold_lines(startLine, endLine);
				renderFrame();
			},
			unfoldLines(startLine: number, endLine: number) {
				ref.unfold_lines(startLine, endLine);
				renderFrame();
			},
			unfoldAll() {
				ref.unfold_all();
				renderFrame();
			},
			get foldCount() {
				return ref.fold_count();
			},
			isLineFolded(line: number) {
				return ref.is_line_folded(line);
			},
			toggleFoldAt(line: number) {
				ref.toggle_fold_at(line);
				renderFrame();
			},
			foldedRanges() {
				return Array.from(ref.folded_ranges());
			},
			// ── Gutter click ────────────────────────────
			lineAtY(y: number) {
				return ref.line_at_y(y);
			},
			// ── Configuration presets ───────────────────
			applyPreset(name: string) {
				ref.apply_preset(name);
				renderFrame();
			},
			// ── Content statistics ──────────────────────
			get readingTimeSeconds() {
				return ref.reading_time_seconds();
			},
			get fleschReadingEase() {
				return ref.flesch_reading_ease();
			},
			// ── Syntax highlighting ─────────────────────
			setSyntaxHighlight(enabled: boolean) {
				ref.set_syntax_highlight(enabled);
				renderFrame();
			},
			get syntaxHighlight() {
				return ref.syntax_highlight();
			},
			setTokenColor(kind: string, r: number, g: number, b: number, a: number) {
				ref.set_token_color(kind, r, g, b, a);
				renderFrame();
			},
			getTokenColor(kind: string) {
				return Array.from(ref.get_token_color(kind));
			},
			resetTokenColors() {
				ref.reset_token_colors();
				renderFrame();
			},
			// ── Custom theme API ────────────────────────
			setThemeColor(slot: string, r: number, g: number, b: number, a: number) {
				ref.set_theme_color(slot, r, g, b, a);
				renderFrame();
			},
			getThemeColor(slot: string) {
				return Array.from(ref.get_theme_color(slot));
			},
			// ── Range formatting ────────────────────────
			formatRangeBold(start: number, end: number) {
				syncTime();
				ref.format_range_bold(start, end);
				renderFrame();
			},
			formatRangeItalic(start: number, end: number) {
				syncTime();
				ref.format_range_italic(start, end);
				renderFrame();
			},
			formatRangeUnderline(start: number, end: number) {
				syncTime();
				ref.format_range_underline(start, end);
				renderFrame();
			},
			formatRangeStrikethrough(start: number, end: number) {
				syncTime();
				ref.format_range_strikethrough(start, end);
				renderFrame();
			},
			formatRangeFontSize(start: number, end: number, size: number) {
				syncTime();
				ref.format_range_font_size(start, end, size);
				renderFrame();
			},
			// ── Scroll to line ──────────────────────────
			scrollToLine(line: number) {
				ref.scroll_to_line(line);
				renderFrame();
			},
			// ── Extended statistics ─────────────────────
			get avgWordLength() {
				return ref.avg_word_length();
			},
			longestWord() {
				return ref.longest_word();
			},
			get uniqueWordCount() {
				return ref.unique_word_count();
			},
			get sentenceCount() {
				return ref.sentence_count();
			},
			// ── Editor info ─────────────────────────────
			get editorVersion() {
				return ref.editor_version();
			},
			get apiCount() {
				return ref.api_count();
			},
			featureCategories() {
				return ref.feature_categories();
			},
			// ── Multi-cursor ────────────────────────────
			addCursor(offset: number) {
				ref.add_cursor(offset);
				renderFrame();
			},
			removeCursor(offset: number) {
				ref.remove_cursor(offset);
				renderFrame();
			},
			clearCursors() {
				ref.clear_cursors();
				renderFrame();
			},
			get extraCursorCount() {
				return ref.extra_cursor_count();
			},
			extraCursorOffsets() {
				return Array.from(ref.extra_cursor_offsets());
			},
			multiCursorInsert(text: string) {
				syncTime();
				const n = ref.multi_cursor_insert(text);
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			// ── Breadcrumbs ─────────────────────────────
			breadcrumbs() {
				return Array.from(ref.breadcrumbs());
			},
			goToBreadcrumb(line: number) {
				ref.go_to_breadcrumb(line);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Indent level ────────────────────────────
			get indentLevelAtCursor() {
				return ref.indent_level_at_cursor();
			},
			indentLevelOfLine(line: number) {
				return ref.indent_level_of_line(line);
			},
			// ── Patch ───────────────────────────────────
			applyPatch(operations: string[]) {
				syncTime();
				ref.apply_patch(operations);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Canvas export ───────────────────────────
			exportCanvasDataUrl() {
				return ref.export_canvas_data_url();
			},
			// ── Command palette ─────────────────────────
			commandList() {
				return Array.from(ref.command_list());
			},
			searchCommands(query: string) {
				return Array.from(ref.search_commands(query));
			},
			// ── Text diffing ────────────────────────────
			diffTexts(a: string, b: string) {
				return Array.from(wasm.CanvistEditor.diff_texts(a, b));
			},
			// ── Bidi info ───────────────────────────────
			get containsRtl() {
				return ref.contains_rtl();
			},
			get containsNonAscii() {
				return ref.contains_non_ascii();
			},
			// ── Selection to lines ──────────────────────
			selectionLineRange() {
				return Array.from(ref.selection_line_range());
			},
			selectLines(startLine: number, endLine: number) {
				ref.select_lines(startLine, endLine);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Whitespace normalization ────────────────
			normalizeLineEndings() {
				syncTime();
				const n = ref.normalize_line_endings();
				renderFrame();
				return n;
			},
			normalizeIndentation() {
				syncTime();
				const n = ref.normalize_indentation();
				renderFrame();
				return n;
			},
			// ── Document outline ────────────────────────
			documentOutline() {
				return Array.from(ref.document_outline());
			},
			// ── Collaborative cursors ───────────────────
			addCollabCursor(
				offset: number,
				name: string,
				r: number,
				g: number,
				b: number,
			) {
				ref.add_collab_cursor(offset, name, r, g, b);
				renderFrame();
			},
			updateCollabCursor(name: string, offset: number) {
				ref.update_collab_cursor(name, offset);
				renderFrame();
			},
			removeCollabCursor(name: string) {
				ref.remove_collab_cursor(name);
				renderFrame();
			},
			clearCollabCursors() {
				ref.clear_collab_cursors();
				renderFrame();
			},
			get collabCursorCount() {
				return ref.collab_cursor_count();
			},
			collabCursorList() {
				return Array.from(ref.collab_cursor_list());
			},
			// ── Line ending ─────────────────────────────
			detectLineEnding() {
				return ref.detect_line_ending();
			},
			convertToCrlf() {
				syncTime();
				const n = ref.convert_to_crlf();
				renderFrame();
				return n;
			},
			convertToLf() {
				syncTime();
				const n = ref.convert_to_lf();
				renderFrame();
				return n;
			},
			// ── File type ───────────────────────────────
			detectFileType() {
				return ref.detect_file_type();
			},
			// ── Emmet ───────────────────────────────────
			expandEmmet() {
				syncTime();
				const ok = ref.expand_emmet();
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			// ── Selection history ───────────────────────
			pushSelectionHistory() {
				ref.push_selection_history();
			},
			selectionHistoryBack() {
				const ok = ref.selection_history_back();
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			selectionHistoryForward() {
				const ok = ref.selection_history_forward();
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			get selectionHistoryLength() {
				return ref.selection_history_length();
			},
			// ── Focus ───────────────────────────────────
			get isFocused() {
				return ref.is_focused();
			},
			// ── Custom keybindings ──────────────────────
			setKeybinding(shortcut: string, command: string) {
				ref.set_keybinding(shortcut, command);
			},
			removeKeybinding(shortcut: string) {
				ref.remove_keybinding(shortcut);
			},
			clearKeybindings() {
				ref.clear_keybindings();
			},
			getKeybinding(shortcut: string) {
				return ref.get_keybinding(shortcut);
			},
			get keybindingOverrideCount() {
				return ref.keybinding_override_count();
			},
			keybindingOverridesList() {
				return Array.from(ref.keybinding_overrides_list());
			},
			runCommand(command: string) {
				syncTime();
				const ok = ref.run_command(command);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			runShortcut(shortcut: string) {
				syncTime();
				const ok = ref.run_shortcut(shortcut);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			// ── Text transform pipeline ─────────────────
			transformCamelCase() {
				syncTime();
				ref.transform_camel_case();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			transformSnakeCase() {
				syncTime();
				ref.transform_snake_case();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			transformKebabCase() {
				syncTime();
				ref.transform_kebab_case();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			transformConstantCase() {
				syncTime();
				ref.transform_constant_case();
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			transformPipeline(pipeline: string) {
				syncTime();
				ref.transform_pipeline(pipeline);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			// ── Marker ranges ───────────────────────────
			addMarker(
				start: number,
				end: number,
				r: number,
				g: number,
				b: number,
				a: number,
				id: string,
			) {
				ref.add_marker(start, end, r, g, b, a, id);
				renderFrame();
			},
			removeMarker(id: string) {
				ref.remove_marker(id);
				renderFrame();
			},
			removeMarkersByPrefix(prefix: string) {
				ref.remove_markers_by_prefix(prefix);
				renderFrame();
			},
			clearMarkers() {
				ref.clear_markers();
				renderFrame();
			},
			get markerCount() {
				return ref.marker_count();
			},
			markerList() {
				return Array.from(ref.marker_list());
			},
			markersAt(offset: number) {
				return Array.from(ref.markers_at(offset));
			},
			// ── Soft wrap info ──────────────────────────
			get visualLineCount() {
				return ref.visual_line_count();
			},
			isLineWrapped(line: number) {
				return ref.is_line_wrapped(line);
			},
			// ── Extended stats ──────────────────────────
			get paragraphBlockCount() {
				return ref.paragraph_block_count();
			},
			get avgLineLength() {
				return ref.avg_line_length();
			},
			get longestLineLength() {
				return ref.longest_line_length();
			},
			get longestLineNumber() {
				return ref.longest_line_number();
			},
			get byteCount() {
				return ref.byte_count();
			},
			// ── Completion context ──────────────────────
			completionsWithContext(limit: number) {
				return Array.from(ref.completions_with_context(limit));
			},
			// ── Named anchors ───────────────────────────
			setAnchor(name: string, offset: number) {
				ref.set_anchor(name, offset);
			},
			anchorOffset(name: string) {
				return ref.anchor_offset(name);
			},
			removeAnchor(name: string) {
				ref.remove_anchor(name);
			},
			clearAnchors() {
				ref.clear_anchors();
			},
			get anchorCount() {
				return ref.anchor_count();
			},
			anchorNames() {
				return Array.from(ref.anchor_names());
			},
			goToAnchor(name: string) {
				const ok = ref.go_to_anchor(name);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			anchorExists(name: string) {
				return ref.anchor_exists(name);
			},
			renameAnchor(oldName: string, newName: string) {
				return ref.rename_anchor(oldName, newName);
			},
			nearestAnchorBefore(offset: number) {
				return Array.from(ref.nearest_anchor_before(offset));
			},
			nearestAnchorAfter(offset: number) {
				return Array.from(ref.nearest_anchor_after(offset));
			},
			nextAnchorAfterCursor() {
				return Array.from(ref.next_anchor_after_cursor());
			},
			prevAnchorBeforeCursor() {
				return Array.from(ref.prev_anchor_before_cursor());
			},
			goToNextAnchor(wrap = true) {
				const ok = ref.go_to_next_anchor(wrap);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			goToPrevAnchor(wrap = true) {
				const ok = ref.go_to_prev_anchor(wrap);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			anchorsAtOffset(offset: number) {
				return Array.from(ref.anchors_at_offset(offset));
			},
			anchorsInRange(startOffset: number, endOffset: number) {
				return Array.from(ref.anchors_in_range(startOffset, endOffset));
			},
			shiftAnchor(name: string, delta: number) {
				return ref.shift_anchor(name, delta);
			},
			anchorEntries() {
				return Array.from(ref.anchor_entries());
			},
			removeAnchorsWithPrefix(prefix: string) {
				return ref.remove_anchors_with_prefix(prefix);
			},
			renameAnchorPrefix(oldPrefix: string, newPrefix: string) {
				return ref.rename_anchor_prefix(oldPrefix, newPrefix);
			},
			setAnchorIfAbsent(name: string, offset: number) {
				return ref.set_anchor_if_absent(name, offset);
			},
			anchorNamesByOffset() {
				return Array.from(ref.anchor_names_by_offset());
			},
			firstAnchorEntry() {
				return Array.from(ref.first_anchor_entry());
			},
			lastAnchorEntry() {
				return Array.from(ref.last_anchor_entry());
			},
			anchorNamesBeforeOffset(offset: number, inclusive = true) {
				return Array.from(ref.anchor_names_before_offset(offset, inclusive));
			},
			anchorNamesAfterOffset(offset: number, inclusive = true) {
				return Array.from(ref.anchor_names_after_offset(offset, inclusive));
			},
			anchorSpanOffsets(startName: string, endName: string) {
				return Array.from(ref.anchor_span_offsets(startName, endName));
			},
			anchorNamesBetween(
				startName: string,
				endName: string,
				inclusive = true,
			) {
				return Array.from(
					ref.anchor_names_between(startName, endName, inclusive),
				);
			},
			anchorCountBetween(
				startName: string,
				endName: string,
				inclusive = true,
			) {
				return ref.anchor_count_between(startName, endName, inclusive);
			},
			shiftAnchorsBetween(
				startName: string,
				endName: string,
				delta: number,
				inclusive = true,
			) {
				return ref.shift_anchors_between(startName, endName, delta, inclusive);
			},
			removeAnchorsBetween(
				startName: string,
				endName: string,
				inclusive = true,
			) {
				return ref.remove_anchors_between(startName, endName, inclusive);
			},
			anchorOffsetsInRange(startOffset: number, endOffset: number) {
				return Array.from(ref.anchor_offsets_in_range(startOffset, endOffset));
			},
			shiftAnchorsInRange(
				startOffset: number,
				endOffset: number,
				delta: number,
			) {
				return ref.shift_anchors_in_range(startOffset, endOffset, delta);
			},
			anchorNamesWithPrefix(prefix: string) {
				return Array.from(ref.anchor_names_with_prefix(prefix));
			},
			anchorNamesInRange(startOffset: number, endOffset: number) {
				return Array.from(ref.anchor_names_in_range(startOffset, endOffset));
			},
			removeAnchorsInRange(startOffset: number, endOffset: number) {
				return ref.remove_anchors_in_range(startOffset, endOffset);
			},
			moveAnchorToCursor(name: string) {
				return ref.move_anchor_to_cursor(name);
			},
			// ── Tasks / TODO scanner ────────────────────
			scanTasks() {
				return Array.from(ref.scan_tasks());
			},
			get taskCount() {
				return ref.task_count();
			},
			nextTaskLine(fromLine: number) {
				return ref.next_task_line(fromLine);
			},
			prevTaskLine(fromLine: number) {
				return ref.prev_task_line(fromLine);
			},
			toggleTaskCheckbox(line: number) {
				syncTime();
				const ok = ref.toggle_task_checkbox(line);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			// ── Lint helpers ────────────────────────────
			lintTrailingWhitespace() {
				return Array.from(ref.lint_trailing_whitespace());
			},
			lintLongLines(maxLen: number) {
				return Array.from(ref.lint_long_lines(maxLen));
			},
			lintMixedIndentation() {
				return Array.from(ref.lint_mixed_indentation());
			},
			lintNonAsciiLines() {
				return Array.from(ref.lint_non_ascii_lines());
			},
			// ── Line occurrence navigation ──────────────
			lineOccurrences(needle: string, caseSensitive = false) {
				return Array.from(ref.line_occurrences(needle, caseSensitive));
			},
			lineOccurrenceCount(needle: string, caseSensitive = false) {
				return ref.line_occurrence_count(needle, caseSensitive);
			},
			nextLineWith(needle: string, fromLine: number, caseSensitive = false) {
				return ref.next_line_with(needle, fromLine, caseSensitive);
			},
			prevLineWith(needle: string, fromLine: number, caseSensitive = false) {
				return ref.prev_line_with(needle, fromLine, caseSensitive);
			},
			// ── Cursor context helpers ──────────────────
			textBeforeCursor(maxChars: number) {
				return ref.text_before_cursor(maxChars);
			},
			textAfterCursor(maxChars: number) {
				return ref.text_after_cursor(maxChars);
			},
			lineContext(line: number, radius: number) {
				return Array.from(ref.line_context(line, radius));
			},
			// ── Rotate lines ────────────────────────────
			rotateLinesUp(startLine: number, endLine: number) {
				syncTime();
				const ok = ref.rotate_lines_up(startLine, endLine);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			rotateLinesDown(startLine: number, endLine: number) {
				syncTime();
				const ok = ref.rotate_lines_down(startLine, endLine);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			// ── Named state slots ───────────────────────
			saveNamedState(name: string) {
				ref.save_named_state(name);
			},
			loadNamedState(name: string) {
				const ok = ref.load_named_state(name);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			deleteNamedState(name: string) {
				ref.delete_named_state(name);
			},
			clearNamedStates() {
				ref.clear_named_states();
			},
			get namedStateCount() {
				return ref.named_state_count();
			},
			namedStateNames() {
				return Array.from(ref.named_state_names());
			},
			// ── Selection profiles ──────────────────────
			saveSelectionProfile(name: string) {
				ref.save_selection_profile(name);
			},
			loadSelectionProfile(name: string) {
				const ok = ref.load_selection_profile(name);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			deleteSelectionProfile(name: string) {
				ref.delete_selection_profile(name);
			},
			clearSelectionProfiles() {
				ref.clear_selection_profiles();
			},
			get selectionProfileCount() {
				return ref.selection_profile_count();
			},
			selectionProfileNames() {
				return Array.from(ref.selection_profile_names());
			},
			// ── Task workflow helpers ───────────────────
			taskProgress() {
				return Array.from(ref.task_progress());
			},
			insertTaskLine(text: string, checked = false) {
				syncTime();
				ref.insert_task_line(text, checked);
				cursorOffset = ref.selection_end();
				renderFrame();
			},
			nextUncheckedTaskLine(fromLine: number) {
				return ref.next_unchecked_task_line(fromLine);
			},
			prevUncheckedTaskLine(fromLine: number) {
				return ref.prev_unchecked_task_line(fromLine);
			},
			completeAllTasks() {
				syncTime();
				const n = ref.complete_all_tasks();
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			clearCompletedTasks() {
				syncTime();
				const n = ref.clear_completed_tasks();
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			// ── Cleanup utilities ───────────────────────
			trimLeadingWhitespace() {
				syncTime();
				const n = ref.trim_leading_whitespace();
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			collapseBlankLines(maxConsecutive: number) {
				syncTime();
				const n = ref.collapse_blank_lines(maxConsecutive);
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			removeTrailingBlankLines() {
				syncTime();
				const n = ref.remove_trailing_blank_lines();
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			ensureSingleTrailingNewline() {
				syncTime();
				const ok = ref.ensure_single_trailing_newline();
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			// ── Line utilities ──────────────────────────
			swapLines(a: number, b: number) {
				syncTime();
				const ok = ref.swap_lines(a, b);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			duplicateLineRange(startLine: number, endLine: number) {
				syncTime();
				const ok = ref.duplicate_line_range(startLine, endLine);
				cursorOffset = ref.selection_end();
				renderFrame();
				return ok;
			},
			prefixLines(startLine: number, endLine: number, prefix: string) {
				syncTime();
				const n = ref.prefix_lines(startLine, endLine, prefix);
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			suffixLines(startLine: number, endLine: number, suffix: string) {
				syncTime();
				const n = ref.suffix_lines(startLine, endLine, suffix);
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			unprefixLines(startLine: number, endLine: number, prefix: string) {
				syncTime();
				const n = ref.unprefix_lines(startLine, endLine, prefix);
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			unsuffixLines(startLine: number, endLine: number, suffix: string) {
				syncTime();
				const n = ref.unsuffix_lines(startLine, endLine, suffix);
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			lineHasPrefix(line: number, prefix: string, caseSensitive = false) {
				return ref.line_has_prefix(line, prefix, caseSensitive);
			},
			lineHasSuffix(line: number, suffix: string, caseSensitive = false) {
				return ref.line_has_suffix(line, suffix, caseSensitive);
			},
			linesWithPrefix(prefix: string, caseSensitive = false) {
				return Array.from(ref.lines_with_prefix(prefix, caseSensitive));
			},
			linesWithSuffix(suffix: string, caseSensitive = false) {
				return Array.from(ref.lines_with_suffix(suffix, caseSensitive));
			},
			countLinesWithPrefix(prefix: string, caseSensitive = false) {
				return ref.count_lines_with_prefix(prefix, caseSensitive);
			},
			countLinesWithSuffix(suffix: string, caseSensitive = false) {
				return ref.count_lines_with_suffix(suffix, caseSensitive);
			},
			numberLines(
				startLine: number,
				endLine: number,
				startNumber = 1,
				padWidth = 0,
			) {
				syncTime();
				const n = ref.number_lines(startLine, endLine, startNumber, padWidth);
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			stripNonPrintable() {
				syncTime();
				const n = ref.strip_non_printable();
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			normalizeUnicodeWhitespace() {
				syncTime();
				const n = ref.normalize_unicode_whitespace();
				cursorOffset = ref.selection_end();
				renderFrame();
				return n;
			},
			lineHash(line: number) {
				return ref.line_hash(line);
			},
			lineHashes() {
				return Array.from(ref.line_hashes());
			},
			lineHashesInRange(startLine: number, endLine: number) {
				return Array.from(ref.line_hashes_in_range(startLine, endLine));
			},
			lineHashEquals(a: number, b: number) {
				return ref.line_hash_equals(a, b);
			},
			lineIsDuplicate(
				line: number,
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return ref.line_is_duplicate(line, caseSensitive, ignoreWhitespace);
			},
			duplicateLineNumbers(caseSensitive = false, ignoreWhitespace = false) {
				return Array.from(
					ref.duplicate_line_numbers(caseSensitive, ignoreWhitespace),
				);
			},
			duplicateLineCount(caseSensitive = false, ignoreWhitespace = false) {
				return ref.duplicate_line_count(caseSensitive, ignoreWhitespace);
			},
			duplicateLineRatio(caseSensitive = false, ignoreWhitespace = false) {
				return ref.duplicate_line_ratio(caseSensitive, ignoreWhitespace);
			},
			uniqueLineNumbers(caseSensitive = false, ignoreWhitespace = false) {
				return Array.from(
					ref.unique_line_numbers(caseSensitive, ignoreWhitespace),
				);
			},
			uniqueLineCount(caseSensitive = false, ignoreWhitespace = false) {
				return ref.unique_line_count(caseSensitive, ignoreWhitespace);
			},
			firstDuplicateLine(caseSensitive = false, ignoreWhitespace = false) {
				return ref.first_duplicate_line(caseSensitive, ignoreWhitespace);
			},
			lastDuplicateLine(caseSensitive = false, ignoreWhitespace = false) {
				return ref.last_duplicate_line(caseSensitive, ignoreWhitespace);
			},
			duplicateGroupCount(caseSensitive = false, ignoreWhitespace = false) {
				return ref.duplicate_group_count(caseSensitive, ignoreWhitespace);
			},
			largestDuplicateGroupSize(
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return ref.largest_duplicate_group_size(
					caseSensitive,
					ignoreWhitespace,
				);
			},
			largestDuplicateGroupLines(
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return Array.from(
					ref.largest_duplicate_group_lines(caseSensitive, ignoreWhitespace),
				);
			},
			duplicateGroupSizes(caseSensitive = false, ignoreWhitespace = false) {
				return Array.from(
					ref.duplicate_group_sizes(caseSensitive, ignoreWhitespace),
				);
			},
			duplicateGroupLinesForLine(
				line: number,
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return Array.from(
					ref.duplicate_group_lines_for_line(
						line,
						caseSensitive,
						ignoreWhitespace,
					),
				);
			},
			duplicateGroupSizeForLine(
				line: number,
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return ref.duplicate_group_size_for_line(
					line,
					caseSensitive,
					ignoreWhitespace,
				);
			},
			duplicateGroupFirstLineForLine(
				line: number,
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return ref.duplicate_group_first_line_for_line(
					line,
					caseSensitive,
					ignoreWhitespace,
				);
			},
			duplicateGroupLastLineForLine(
				line: number,
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return ref.duplicate_group_last_line_for_line(
					line,
					caseSensitive,
					ignoreWhitespace,
				);
			},
			lineOccurrenceLinesForLine(
				line: number,
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return Array.from(
					ref.line_occurrence_lines_for_line(
						line,
						caseSensitive,
						ignoreWhitespace,
					),
				);
			},
			lineOccurrenceCountForLine(
				line: number,
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return ref.line_occurrence_count_for_line(
					line,
					caseSensitive,
					ignoreWhitespace,
				);
			},
			lineOccurrenceRatioForLine(
				line: number,
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return ref.line_occurrence_ratio_for_line(
					line,
					caseSensitive,
					ignoreWhitespace,
				);
			},
			lineOccurrenceGroupCount(
				caseSensitive = false,
				ignoreWhitespace = false,
				minCount = 1,
			) {
				return ref.line_occurrence_group_count(
					caseSensitive,
					ignoreWhitespace,
					minCount,
				);
			},
			lineOccurrenceRankings(
				caseSensitive = false,
				ignoreWhitespace = false,
				minCount = 1,
			) {
				return Array.from(
					ref.line_occurrence_rankings(
						caseSensitive,
						ignoreWhitespace,
						minCount,
					),
				);
			},
			mostCommonLineOccurrenceCount(
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return ref.most_common_line_occurrence_count(
					caseSensitive,
					ignoreWhitespace,
				);
			},
			mostCommonLineOccurrenceLines(
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return Array.from(
					ref.most_common_line_occurrence_lines(
						caseSensitive,
						ignoreWhitespace,
					),
				);
			},
			lineIsUniqueByContent(
				line: number,
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return ref.line_is_unique_by_content(
					line,
					caseSensitive,
					ignoreWhitespace,
				);
			},
			duplicatePeerLinesForLine(
				line: number,
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return Array.from(
					ref.duplicate_peer_lines_for_line(
						line,
						caseSensitive,
						ignoreWhitespace,
					),
				);
			},
			duplicatePeerLineCount(
				line: number,
				caseSensitive = false,
				ignoreWhitespace = false,
			) {
				return ref.duplicate_peer_line_count(
					line,
					caseSensitive,
					ignoreWhitespace,
				);
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
