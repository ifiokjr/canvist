/* @ts-self-types="./canvist_wasm.d.ts" */

/**
 * The main editor handle exposed to JavaScript.
 *
 * Wraps a [`Document`] and a Canvas2D rendering backend. Create one per
 * `<canvas>` element.
 */
export class CanvistEditor {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(CanvistEditor.prototype);
        obj.__wbg_ptr = ptr;
        CanvistEditorFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        CanvistEditorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_canvisteditor_free(ptr, 0);
    }
    /**
     * Apply style to the given character range.
     * @param {number} start
     * @param {number} end
     * @param {boolean} bold
     * @param {boolean} italic
     * @param {boolean} underline
     * @param {number | null} [font_size]
     * @param {string | null} [font_family]
     * @param {Uint8Array | null} [color_rgba]
     */
    apply_style_range(start, end, bold, italic, underline, font_size, font_family, color_rgba) {
        var ptr0 = isLikeNone(font_family) ? 0 : passStringToWasm0(font_family, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(color_rgba) ? 0 : passArray8ToWasm0(color_rgba, wasm.__wbindgen_malloc);
        var len1 = WASM_VECTOR_LEN;
        wasm.canvisteditor_apply_style_range(this.__wbg_ptr, start, end, bold, italic, underline, isLikeNone(font_size) ? 0x100000001 : Math.fround(font_size), ptr0, len0, ptr1, len1);
    }
    /**
     * Force-break the current undo coalescing chain.
     *
     * Normally, rapid single-character inserts are merged into a single undo
     * group so that `undo()` reverses a whole burst of typing at once. Call
     * this method to ensure that the *next* insert starts a fresh undo
     * group, even if it would otherwise be coalesced with the previous one.
     *
     * Typical use-cases:
     * - Before programmatic (non-user) edits, so they form their own undo
     *   entry.
     * - After a focus change or explicit "save-point".
     */
    break_undo_coalescing() {
        wasm.canvisteditor_break_undo_coalescing(this.__wbg_ptr);
    }
    /**
     * Whether there are entries on the redo stack.
     * @returns {boolean}
     */
    can_redo() {
        const ret = wasm.canvisteditor_can_redo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Whether there are entries on the undo stack.
     * @returns {boolean}
     */
    can_undo() {
        const ret = wasm.canvisteditor_can_undo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Return the canvas element ID this editor is attached to.
     * @returns {string}
     */
    canvas_id() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_canvas_id(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Compute the Y position of the caret in content coordinates.
     *
     * Returns `(y, height)` for the caret line. Useful for scroll-into-view.
     * @returns {Float32Array}
     */
    caret_y() {
        const ret = wasm.canvisteditor_caret_y(this.__wbg_ptr);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Return the character count.
     * @returns {number}
     */
    char_count() {
        const ret = wasm.canvisteditor_char_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Perform a clipboard cut: delete the current selection.
     *
     * The caller is expected to have already read `get_selected_text()` and
     * written it to the system clipboard before calling this method.
     */
    clipboard_cut() {
        wasm.canvisteditor_clipboard_cut(this.__wbg_ptr);
    }
    /**
     * Paste text at the current cursor position (replacing any selection).
     * @param {string} text
     */
    clipboard_paste(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_clipboard_paste(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Return the current undo-coalescing timeout in milliseconds.
     * @returns {number}
     */
    coalesce_timeout() {
        const ret = wasm.canvisteditor_coalesce_timeout(this.__wbg_ptr);
        return ret;
    }
    /**
     * Compute the total content height in logical pixels.
     *
     * Uses the paragraph layout engine to determine the full document
     * height including padding and paragraph spacing.
     * @returns {number}
     */
    content_height() {
        const ret = wasm.canvisteditor_content_height(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0];
    }
    /**
     * Create a new editor attached to the canvas element with the given ID.
     *
     * # Errors
     *
     * Returns an error if the canvas element is not found.
     * @param {string} canvas_id
     * @returns {CanvistEditor}
     */
    static create(canvas_id) {
        const ptr0 = passStringToWasm0(canvas_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_create(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return CanvistEditor.__wrap(ret[0]);
    }
    /**
     * Return the 1-based column (character position within the visual line).
     * @returns {number}
     */
    cursor_column() {
        const ret = wasm.canvisteditor_cursor_column(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Return the 1-based visual line number the caret is on.
     * @returns {number}
     */
    cursor_line() {
        const ret = wasm.canvisteditor_cursor_line(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Delete a range of characters from `start` to `end`.
     * @param {number} start
     * @param {number} end
     */
    delete_range(start, end) {
        wasm.canvisteditor_delete_range(this.__wbg_ptr, start, end);
    }
    /**
     * Find all occurrences of `needle`. Returns a flat array: [start0, end0,
     * start1, end1, …].
     * @param {string} needle
     * @param {boolean} case_sensitive
     * @returns {Uint32Array}
     */
    find_all(needle, case_sensitive) {
        const ptr0 = passStringToWasm0(needle, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_find_all(this.__wbg_ptr, ptr0, len0, case_sensitive);
        var v2 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * Find the next occurrence of `needle` at or after `from_offset`.
     * Returns `[start, end]` or an empty array if not found.
     * @param {string} needle
     * @param {number} from_offset
     * @param {boolean} case_sensitive
     * @returns {Uint32Array}
     */
    find_next(needle, from_offset, case_sensitive) {
        const ptr0 = passStringToWasm0(needle, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_find_next(this.__wbg_ptr, ptr0, len0, from_offset, case_sensitive);
        var v2 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * Find the previous occurrence before `from_offset`.
     * @param {string} needle
     * @param {number} from_offset
     * @param {boolean} case_sensitive
     * @returns {Uint32Array}
     */
    find_prev(needle, from_offset, case_sensitive) {
        const ptr0 = passStringToWasm0(needle, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_find_prev(this.__wbg_ptr, ptr0, len0, from_offset, case_sensitive);
        var v2 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * Get the current focus state.
     * @returns {boolean}
     */
    focused() {
        const ret = wasm.canvisteditor_focused(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Return the currently selected text (empty string if selection is collapsed).
     * @returns {string}
     */
    get_selected_text() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_get_selected_text(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Hit-test a screen-space point to determine the character offset at that
     * position.
     *
     * Converts screen coordinates to document coordinates (accounting for
     * scroll and zoom via `Viewport`), then performs layout and walks the
     * resulting lines/characters to find the closest inter-character boundary.
     *
     * Returns a character offset suitable for setting the cursor position.
     *
     * # Arguments
     *
     * - `screen_x` — X coordinate in canvas/screen pixels
     * - `screen_y` — Y coordinate in canvas/screen pixels
     * Hit-test a screen-space point to determine the character offset at that
     * position.
     *
     * Converts screen coordinates to document coordinates (accounting for
     * scroll and zoom via `Viewport`), then walks the multi-paragraph layout
     * to find the closest inter-character boundary. Each paragraph is laid out
     * independently, so the hit-test accounts for paragraph spacing.
     *
     * Returns a character offset suitable for setting the cursor position.
     *
     * # Arguments
     *
     * - `screen_x` — X coordinate in canvas/screen pixels
     * - `screen_y` — Y coordinate in canvas/screen pixels
     * @param {number} screen_x
     * @param {number} screen_y
     * @returns {number}
     */
    hit_test(screen_x, screen_y) {
        const ret = wasm.canvisteditor_hit_test(this.__wbg_ptr, screen_x, screen_y);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Insert text at the current cursor position (start of document).
     * @param {string} text
     */
    insert_text(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_insert_text(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Insert text at a specific character offset.
     * @param {number} offset
     * @param {string} text
     */
    insert_text_at(offset, text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_insert_text_at(this.__wbg_ptr, offset, ptr0, len0);
    }
    /**
     * Check if the current selection is all bold.
     * @returns {boolean}
     */
    is_bold() {
        const ret = wasm.canvisteditor_is_bold(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Check if the current selection is all italic.
     * @returns {boolean}
     */
    is_italic() {
        const ret = wasm.canvisteditor_is_italic(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Check if the current selection is all underline.
     * @returns {boolean}
     */
    is_underline() {
        const ret = wasm.canvisteditor_is_underline(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Count the number of visual lines using the paragraph layout engine.
     * @returns {number}
     */
    line_count() {
        const ret = wasm.canvisteditor_line_count(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Return the end offset of the visual line containing `offset`.
     * @param {number} offset
     * @returns {number}
     */
    line_end_for_offset(offset) {
        const ret = wasm.canvisteditor_line_end_for_offset(this.__wbg_ptr, offset);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Return the start offset of the visual line containing `offset`.
     *
     * This performs a full paragraph layout to determine where lines wrap,
     * then returns the character offset where that visual line begins.
     * @param {number} offset
     * @returns {number}
     */
    line_start_for_offset(offset) {
        const ret = wasm.canvisteditor_line_start_for_offset(this.__wbg_ptr, offset);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Move cursor one character left.
     * @param {boolean} extend
     */
    move_cursor_left(extend) {
        wasm.canvisteditor_move_cursor_left(this.__wbg_ptr, extend);
    }
    /**
     * Move cursor one character right.
     * @param {boolean} extend
     */
    move_cursor_right(extend) {
        wasm.canvisteditor_move_cursor_right(this.__wbg_ptr, extend);
    }
    /**
     * Move cursor to an absolute position; extend toggles range selection.
     * @param {number} position
     * @param {boolean} extend
     */
    move_cursor_to(position, extend) {
        wasm.canvisteditor_move_cursor_to(this.__wbg_ptr, position, extend);
    }
    /**
     * Return the character offset on the line directly above `offset`.
     *
     * Preserves the horizontal (x) pixel position of the caret when moving
     * between lines.
     * @param {number} offset
     * @returns {number}
     */
    offset_above(offset) {
        const ret = wasm.canvisteditor_offset_above(this.__wbg_ptr, offset);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Return the character offset on the line directly below `offset`.
     * @param {number} offset
     * @returns {number}
     */
    offset_below(offset) {
        const ret = wasm.canvisteditor_offset_below(this.__wbg_ptr, offset);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Return the full plain-text content of the document.
     * @returns {string}
     */
    plain_text() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_plain_text(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Process all pending canonical events via the editor runtime.
     */
    process_events() {
        wasm.canvisteditor_process_events(this.__wbg_ptr);
    }
    /**
     * Queue a key down event and process resulting operations.
     * @param {string} key
     */
    queue_key_down(key) {
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_queue_key_down(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Queue key down with explicit modifier + repeat state.
     * @param {string} key
     * @param {boolean} shift
     * @param {boolean} control
     * @param {boolean} alt
     * @param {boolean} meta
     * @param {boolean} repeat
     */
    queue_key_down_with_modifiers(key, shift, control, alt, meta, repeat) {
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_queue_key_down_with_modifiers(this.__wbg_ptr, ptr0, len0, shift, control, alt, meta, repeat);
    }
    /**
     * Queue canonical text input and process it into operations.
     * @param {string} text
     */
    queue_text_input(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_queue_text_input(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Redo the most recently undone transaction.
     *
     * Re-applies the forward operations that were undone. Returns `true` if a
     * redo was performed, `false` if the redo stack was empty.
     * @returns {boolean}
     */
    redo() {
        const ret = wasm.canvisteditor_redo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Request a re-render of the document to the canvas.
     *
     * Performs multi-paragraph, multi-line text rendering with styled runs,
     * selection highlights, and a blinking caret. Each paragraph in the
     * document tree is laid out independently with configurable paragraph
     * spacing between them.
     */
    render() {
        const ret = wasm.canvisteditor_render(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Replace all occurrences of `needle` with `replacement`.
     * Returns the number of replacements made.
     * @param {string} needle
     * @param {string} replacement
     * @param {boolean} case_sensitive
     * @returns {number}
     */
    replace_all(needle, replacement, case_sensitive) {
        const ptr0 = passStringToWasm0(needle, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(replacement, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_replace_all(this.__wbg_ptr, ptr0, len0, ptr1, len1, case_sensitive);
        return ret >>> 0;
    }
    /**
     * Replace the text in range `[start, end)` with `replacement`.
     *
     * This is a delete + insert.
     * @param {number} start
     * @param {number} end
     * @param {string} replacement
     */
    replace_range(start, end, replacement) {
        const ptr0 = passStringToWasm0(replacement, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_replace_range(this.__wbg_ptr, start, end, ptr0, len0);
    }
    /**
     * Replay a JSON-encoded operation list into current runtime.
     * @param {string} operations_json
     */
    replay_operations_json(operations_json) {
        const ptr0 = passStringToWasm0(operations_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_replay_operations_json(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Scroll by a delta (positive = down, negative = up).
     * @param {number} delta_y
     */
    scroll_by(delta_y) {
        wasm.canvisteditor_scroll_by(this.__wbg_ptr, delta_y);
    }
    /**
     * Get the current vertical scroll offset.
     * @returns {number}
     */
    scroll_y() {
        const ret = wasm.canvisteditor_scroll_y(this.__wbg_ptr);
        return ret;
    }
    /**
     * Select the entire document.
     */
    select_all() {
        wasm.canvisteditor_select_all(this.__wbg_ptr);
    }
    /**
     * Select the word at the given character offset.
     * @param {number} offset
     */
    select_word_at(offset) {
        wasm.canvisteditor_select_word_at(this.__wbg_ptr, offset);
    }
    /**
     * Get selection end offset.
     * @returns {number}
     */
    selection_end() {
        const ret = wasm.canvisteditor_selection_end(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get selection start offset.
     * @returns {number}
     */
    selection_start() {
        const ret = wasm.canvisteditor_selection_start(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Set whether the caret (text cursor) is visible.
     *
     * Called by the JS blink controller on a 530 ms interval to toggle the
     * caret on and off. When `visible` is `false`, `render()` skips drawing
     * the caret line, producing the classic blinking effect.
     * @param {boolean} visible
     */
    set_caret_visible(visible) {
        wasm.canvisteditor_set_caret_visible(this.__wbg_ptr, visible);
    }
    /**
     * Set the undo-coalescing timeout in milliseconds.
     *
     * Single-character inserts that arrive within this interval (and satisfy
     * position/boundary checks) are merged into a single undo entry.
     * Increasing this value makes undo steps coarser; decreasing it makes
     * them finer.
     *
     * The default is 500 ms.
     *
     * # Arguments
     *
     * - `ms` — timeout in milliseconds (as `f64` because JS numbers are
     *   doubles; the value is truncated to `u64` internally).
     * @param {number} ms
     */
    set_coalesce_timeout(ms) {
        wasm.canvisteditor_set_coalesce_timeout(this.__wbg_ptr, ms);
    }
    /**
     * Set text color on the current selection.
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_color(r, g, b, a) {
        wasm.canvisteditor_set_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * Set whether the editor has focus.
     *
     * When unfocused, the caret is drawn as a gray line and selection
     * uses a lighter highlight color.
     * @param {boolean} focused
     */
    set_focused(focused) {
        wasm.canvisteditor_set_focused(this.__wbg_ptr, focused);
    }
    /**
     * Set font size on the current selection.
     * @param {number} size
     */
    set_font_size(size) {
        wasm.canvisteditor_set_font_size(this.__wbg_ptr, size);
    }
    /**
     * Set the current wall-clock time (milliseconds since epoch) for the
     * undo coalescing timer.
     *
     * Call this with `Date.now()` before every user action so the runtime
     * can measure real time gaps between keystrokes. Without this, the
     * runtime falls back to its monotonic counter which doesn't reflect
     * actual typing speed.
     * @param {number} ms
     */
    set_now_ms(ms) {
        wasm.canvisteditor_set_now_ms(this.__wbg_ptr, ms);
    }
    /**
     * Set the vertical scroll offset (clamped to valid range).
     * @param {number} y
     */
    set_scroll_y(y) {
        wasm.canvisteditor_set_scroll_y(this.__wbg_ptr, y);
    }
    /**
     * Set selection range.
     * @param {number} start
     * @param {number} end
     */
    set_selection(start, end) {
        wasm.canvisteditor_set_selection(this.__wbg_ptr, start, end);
    }
    /**
     * Set the logical (CSS) dimensions of the editor canvas.
     *
     * Call this after changing the canvas's CSS size so layout wrapping
     * and hit-testing use the correct dimensions (not the DPR-scaled pixel
     * dimensions).
     * @param {number} width
     * @param {number} height
     */
    set_size(width, height) {
        wasm.canvisteditor_set_size(this.__wbg_ptr, width, height);
    }
    /**
     * Set the document title.
     * @param {string} title
     */
    set_title(title) {
        const ptr0 = passStringToWasm0(title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_set_title(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Export the document as a JSON string.
     * @returns {string}
     */
    to_json() {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.canvisteditor_to_json(this.__wbg_ptr);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Toggle bold on the current selection.
     *
     * If all characters in the selection are already bold, removes bold.
     * Otherwise, applies bold. Preserves the current selection.
     */
    toggle_bold() {
        wasm.canvisteditor_toggle_bold(this.__wbg_ptr);
    }
    /**
     * Toggle italic on the current selection. Preserves the current
     * selection.
     */
    toggle_italic() {
        wasm.canvisteditor_toggle_italic(this.__wbg_ptr);
    }
    /**
     * Toggle strikethrough on the current selection.
     */
    toggle_strikethrough() {
        wasm.canvisteditor_toggle_strikethrough(this.__wbg_ptr);
    }
    /**
     * Toggle underline on the current selection. Preserves the current
     * selection.
     */
    toggle_underline() {
        wasm.canvisteditor_toggle_underline(this.__wbg_ptr);
    }
    /**
     * Undo the most recent transaction.
     *
     * Applies inverse operations to restore the document to its previous state.
     * Returns `true` if an undo was performed, `false` if the undo stack was empty.
     * @returns {boolean}
     */
    undo() {
        const ret = wasm.canvisteditor_undo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Find the previous word boundary from a character offset.
     * @param {number} offset
     * @returns {number}
     */
    word_boundary_left(offset) {
        const ret = wasm.canvisteditor_word_boundary_left(this.__wbg_ptr, offset);
        return ret >>> 0;
    }
    /**
     * Find the next word boundary from a character offset.
     * @param {number} offset
     * @returns {number}
     */
    word_boundary_right(offset) {
        const ret = wasm.canvisteditor_word_boundary_right(this.__wbg_ptr, offset);
        return ret >>> 0;
    }
    /**
     * Count the number of words (whitespace-separated tokens).
     * @returns {number}
     */
    word_count() {
        const ret = wasm.canvisteditor_word_count(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) CanvistEditor.prototype[Symbol.dispose] = CanvistEditor.prototype.free;

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_is_undefined_52709e72fb9f179c: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_throw_6ddd609b62940d55: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_beginPath_596efed55075dbc3: function(arg0) {
            arg0.beginPath();
        },
        __wbg_document_c0320cd4183c6d9b: function(arg0) {
            const ret = arg0.document;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_fillRect_4e5596ca954226e7: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.fillRect(arg1, arg2, arg3, arg4);
        },
        __wbg_fillText_b1722b6179692b85: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.fillText(getStringFromWasm0(arg1, arg2), arg3, arg4);
        }, arguments); },
        __wbg_getContext_f04bf8f22dcb2d53: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getElementById_d1f25d287b19a833: function(arg0, arg1, arg2) {
            const ret = arg0.getElementById(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_height_6568c4427c3b889d: function(arg0) {
            const ret = arg0.height;
            return ret;
        },
        __wbg_instanceof_CanvasRenderingContext2d_08b9d193c22fa886: function(arg0) {
            let result;
            try {
                result = arg0 instanceof CanvasRenderingContext2D;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlCanvasElement_26125339f936be50: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLCanvasElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Window_23e677d2c6843922: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Window;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_lineTo_8ea7db5b5d763030: function(arg0, arg1, arg2) {
            arg0.lineTo(arg1, arg2);
        },
        __wbg_measureText_a914720e0a913aef: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.measureText(getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_moveTo_6d04ca2f71946754: function(arg0, arg1, arg2) {
            arg0.moveTo(arg1, arg2);
        },
        __wbg_set_fillStyle_58417b6b548ae475: function(arg0, arg1, arg2) {
            arg0.fillStyle = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_font_b038797b3573ae5e: function(arg0, arg1, arg2) {
            arg0.font = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_imageSmoothingEnabled_f9f883202f4f3d5e: function(arg0, arg1) {
            arg0.imageSmoothingEnabled = arg1 !== 0;
        },
        __wbg_set_strokeStyle_a5baa9565d8b6485: function(arg0, arg1, arg2) {
            arg0.strokeStyle = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_textBaseline_a9304886c3f7ea50: function(arg0, arg1, arg2) {
            arg0.textBaseline = getStringFromWasm0(arg1, arg2);
        },
        __wbg_static_accessor_GLOBAL_8adb955bd33fac2f: function() {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_THIS_ad356e0db91c7913: function() {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_f207c857566db248: function() {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_bb9f1ba69d61b386: function() {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_stroke_affa71c0888c6f31: function(arg0) {
            arg0.stroke();
        },
        __wbg_width_4d6fc7fecd877217: function(arg0) {
            const ret = arg0.width;
            return ret;
        },
        __wbg_width_eebf2967f114717c: function(arg0) {
            const ret = arg0.width;
            return ret;
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./canvist_wasm_bg.js": import0,
    };
}

const CanvistEditorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_canvisteditor_free(ptr >>> 0, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedFloat32ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('canvist_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
