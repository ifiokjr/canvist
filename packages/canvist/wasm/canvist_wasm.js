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
     * Add a single ruler at the given column.
     * @param {number} column
     */
    add_ruler(column) {
        wasm.canvisteditor_add_ruler(this.__wbg_ptr, column);
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
     * Whether bracket auto-closing is enabled.
     * @returns {boolean}
     */
    auto_close_brackets() {
        const ret = wasm.canvisteditor_auto_close_brackets(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Insert a newline at the cursor and auto-indent with the same leading
     * whitespace as the current line.
     *
     * Also continues list markers:
     * - Bullet lines (`• `, `- `, `* `) → new bullet line
     * - Numbered lines (`1. `, `2. `, …) → incremented number
     * - Empty list line (just the marker) → removes the marker instead
     *
     * Returns the number of characters inserted (1 for `\n` plus indent).
     * @returns {number}
     */
    auto_indent_newline() {
        const ret = wasm.canvisteditor_auto_indent_newline(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Whether auto-surround is enabled.
     * @returns {boolean}
     */
    auto_surround() {
        const ret = wasm.canvisteditor_auto_surround(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Base64-decode the selected text, replacing the selection.
     *
     * If the selected text is not valid base64, the selection is unchanged.
     */
    base64_decode_selection() {
        wasm.canvisteditor_base64_decode_selection(this.__wbg_ptr);
    }
    /**
     * Base64-encode the selected text, replacing the selection.
     */
    base64_encode_selection() {
        wasm.canvisteditor_base64_encode_selection(this.__wbg_ptr);
    }
    /**
     * Number of active bookmarks.
     * @returns {number}
     */
    bookmark_count() {
        const ret = wasm.canvisteditor_bookmark_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Return all bookmarked line numbers as a flat array (0-based).
     * @returns {Uint32Array}
     */
    bookmarked_lines() {
        const ret = wasm.canvisteditor_bookmarked_lines(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
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
     * Scroll so the cursor's line is vertically centered in the viewport.
     */
    center_line_in_viewport() {
        wasm.canvisteditor_center_line_in_viewport(this.__wbg_ptr);
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
     * Remove all bookmarks.
     */
    clear_bookmarks() {
        wasm.canvisteditor_clear_bookmarks(this.__wbg_ptr);
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
     * Get the current line comment prefix.
     * @returns {string}
     */
    comment_prefix() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_comment_prefix(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
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
     * Contract selection intelligently (reverse of expand).
     *
     * Shrinks: all → line → bracket → quote → word → collapsed.
     */
    contract_selection() {
        wasm.canvisteditor_contract_selection(this.__wbg_ptr);
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
     * Current column (1-based character offset from line start).
     * @returns {number}
     */
    current_column() {
        const ret = wasm.canvisteditor_current_column(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Current line number the cursor is on (1-based).
     * @returns {number}
     */
    current_line_number() {
        const ret = wasm.canvisteditor_current_line_number(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get the full text of the line the cursor is on (including the
     * trailing `\n` if present). Useful for "copy line" when nothing is
     * selected.
     * @returns {string}
     */
    current_line_text() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_current_line_text(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
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
     * Navigate backward in cursor history (Ctrl+Alt+←).
     *
     * Returns `true` if the cursor moved.
     * @returns {boolean}
     */
    cursor_history_back() {
        const ret = wasm.canvisteditor_cursor_history_back(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Navigate forward in cursor history (Ctrl+Alt+→).
     *
     * Returns `true` if the cursor moved.
     * @returns {boolean}
     */
    cursor_history_forward() {
        const ret = wasm.canvisteditor_cursor_history_forward(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Number of positions in cursor history.
     * @returns {number}
     */
    cursor_history_length() {
        const ret = wasm.canvisteditor_cursor_history_length(this.__wbg_ptr);
        return ret >>> 0;
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
     * Cut the current line (remove it and return its text).
     * This is the "cut line when nothing is selected" behavior.
     * @returns {string}
     */
    cut_line() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_cut_line(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Delete the entire line the cursor is on (Ctrl+Shift+K).
     *
     * If the deleted line is not the last, the trailing `\n` is also
     * removed so the next line moves up.
     */
    delete_line() {
        wasm.canvisteditor_delete_line(this.__wbg_ptr);
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
     * Delete the word to the left of the cursor (Ctrl+Backspace).
     *
     * Walks backwards from the cursor past whitespace, then past word
     * characters, and deletes the range.
     */
    delete_word_left() {
        wasm.canvisteditor_delete_word_left(this.__wbg_ptr);
    }
    /**
     * Delete the word to the right of the cursor (Ctrl+Delete).
     */
    delete_word_right() {
        wasm.canvisteditor_delete_word_right(this.__wbg_ptr);
    }
    /**
     * Duplicate the current line (or selected lines) below.
     */
    duplicate_line() {
        wasm.canvisteditor_duplicate_line(this.__wbg_ptr);
    }
    /**
     * Ensure the document ends with a newline character.
     *
     * Returns `true` if a newline was added.
     * @returns {boolean}
     */
    ensure_final_newline() {
        const ret = wasm.canvisteditor_ensure_final_newline(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Expand selection intelligently: word → quoted → bracketed → line → all.
     *
     * Each call expands to the next logical boundary.
     */
    expand_selection() {
        wasm.canvisteditor_expand_selection(this.__wbg_ptr);
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
     * Find all whole-word occurrences of `needle`.
     *
     * Returns offsets as `[start0, end0, start1, end1, ...]`.
     * A "whole word" match requires the char before and after the match
     * to be non-alphanumeric (or at document boundary).
     * @param {string} needle
     * @returns {Uint32Array}
     */
    find_all_whole_word(needle) {
        const ptr0 = passStringToWasm0(needle, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_find_all_whole_word(this.__wbg_ptr, ptr0, len0);
        var v2 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * Find the offset of the bracket matching the one at `offset`.
     *
     * Returns `None` (via -1 in WASM) if the char at `offset` is not a
     * bracket or no match is found.
     * @param {number} offset
     * @returns {number}
     */
    find_matching_bracket(offset) {
        const ret = wasm.canvisteditor_find_matching_bracket(this.__wbg_ptr, offset);
        return ret;
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
     * Import HTML content, replacing the current document.
     *
     * Parses basic inline elements (`<strong>`, `<em>`, `<u>`, `<s>`, `<br>`,
     * `<p>`) and HTML entities.
     * @param {string} html
     */
    from_html(html) {
        const ptr0 = passStringToWasm0(html, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_from_html(this.__wbg_ptr, ptr0, len0);
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
     * Move cursor to the very end of the document (Ctrl+End).
     */
    go_to_document_end() {
        wasm.canvisteditor_go_to_document_end(this.__wbg_ptr);
    }
    /**
     * Move cursor to the very beginning of the document (Ctrl+Home).
     */
    go_to_document_start() {
        wasm.canvisteditor_go_to_document_start(this.__wbg_ptr);
    }
    /**
     * Move the cursor to the start of the given 1-based paragraph line.
     *
     * If `line_number` exceeds the paragraph count, the cursor moves to
     * the end of the document.
     * @param {number} line_number
     */
    go_to_line(line_number) {
        wasm.canvisteditor_go_to_line(this.__wbg_ptr, line_number);
    }
    /**
     * Whether the current-line highlight is enabled.
     * @returns {boolean}
     */
    highlight_current_line() {
        const ret = wasm.canvisteditor_highlight_current_line(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Whether matching bracket highlighting is enabled.
     * @returns {boolean}
     */
    highlight_matching_brackets() {
        const ret = wasm.canvisteditor_highlight_matching_brackets(this.__wbg_ptr);
        return ret !== 0;
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
     * Indent the current selection: insert a tab character at the start
     * of each selected line. If the selection is collapsed, insert a tab
     * at the cursor position.
     */
    indent_selection() {
        wasm.canvisteditor_indent_selection(this.__wbg_ptr);
    }
    /**
     * Insert a snippet template. `$0` marks where the cursor should
     * be placed after insertion. Other text is inserted literally.
     *
     * Example: `insert_snippet("if ($0) {\n}")` inserts the template
     * and places the cursor between the parentheses.
     * @param {string} template
     */
    insert_snippet(template) {
        const ptr0 = passStringToWasm0(template, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_insert_snippet(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Insert one "tab" at the cursor — either spaces or a `\t`.
     */
    insert_tab() {
        wasm.canvisteditor_insert_tab(this.__wbg_ptr);
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
     * Insert text respecting overwrite mode. In overwrite mode,
     * characters after the cursor are replaced one-for-one rather
     * than pushing text forward.
     * @param {string} text
     */
    insert_text_overwrite(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_insert_text_overwrite(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Insert an opening bracket and its closing counterpart.
     *
     * Returns the number of characters inserted (always 2 when auto-close
     * fires, 1 otherwise). Cursor is placed between the pair.
     * @param {string} ch
     * @returns {number}
     */
    insert_with_auto_close(ch) {
        const ptr0 = passStringToWasm0(ch, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_insert_with_auto_close(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
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
     * Check if the current line has a bookmark.
     * @returns {boolean}
     */
    is_line_bookmarked() {
        const ret = wasm.canvisteditor_is_line_bookmarked(this.__wbg_ptr);
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
     * Join the current line with the line below (Ctrl+J).
     *
     * Replaces the newline between them with a single space.
     */
    join_lines() {
        wasm.canvisteditor_join_lines(this.__wbg_ptr);
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
     * Move the current line down by swapping it with the line below.
     */
    move_line_down() {
        wasm.canvisteditor_move_line_down(this.__wbg_ptr);
    }
    /**
     * Move the current line up by swapping it with the line above.
     */
    move_line_up() {
        wasm.canvisteditor_move_line_up(this.__wbg_ptr);
    }
    /**
     * Move text from `[src_start, src_end)` to `dest` offset.
     *
     * Used by drag-and-drop: extract the selected text, delete the source
     * range, then insert at the destination (adjusting for the shift).
     * @param {number} src_start
     * @param {number} src_end
     * @param {number} dest
     */
    move_text(src_start, src_end, dest) {
        wasm.canvisteditor_move_text(this.__wbg_ptr, src_start, src_end, dest);
    }
    /**
     * Move cursor to the matching bracket (Ctrl+Shift+\).
     *
     * Checks the character at the cursor and the one before it.
     * If a bracket is found, jumps the cursor to its match.
     */
    move_to_matching_bracket() {
        wasm.canvisteditor_move_to_matching_bracket(this.__wbg_ptr);
    }
    /**
     * Move cursor to the start of the next paragraph (Ctrl+↓).
     */
    move_to_next_paragraph() {
        wasm.canvisteditor_move_to_next_paragraph(this.__wbg_ptr);
    }
    /**
     * Move cursor to the start of the previous paragraph (Ctrl+↑).
     *
     * A paragraph boundary is an empty line or the document start.
     */
    move_to_prev_paragraph() {
        wasm.canvisteditor_move_to_prev_paragraph(this.__wbg_ptr);
    }
    /**
     * Jump to the next bookmark after the current line.
     *
     * Wraps around to the first bookmark if past the last one.
     * Returns `true` if a bookmark was found.
     * @returns {boolean}
     */
    next_bookmark() {
        const ret = wasm.canvisteditor_next_bookmark(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Return all occurrence offsets of the selected text as a flat array
     * `[start0, end0, start1, end1, ...]`.
     * @returns {Uint32Array}
     */
    occurrence_offsets() {
        const ret = wasm.canvisteditor_occurrence_offsets(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
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
     * Insert a new line above the current line and move cursor there
     * (Ctrl+Shift+Enter).
     */
    open_line_above() {
        wasm.canvisteditor_open_line_above(this.__wbg_ptr);
    }
    /**
     * Insert a new line below the current line and move cursor there
     * (Ctrl+Enter).
     */
    open_line_below() {
        wasm.canvisteditor_open_line_below(this.__wbg_ptr);
    }
    /**
     * Outdent the current selection: remove one leading tab or up to 4
     * spaces from the start of each selected line.
     */
    outdent_selection() {
        wasm.canvisteditor_outdent_selection(this.__wbg_ptr);
    }
    /**
     * Whether the editor is in overwrite mode.
     * @returns {boolean}
     */
    overwrite_mode() {
        const ret = wasm.canvisteditor_overwrite_mode(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Total paragraph count (non-empty lines).
     * @returns {number}
     */
    paragraph_count() {
        const ret = wasm.canvisteditor_paragraph_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Paste HTML at the current cursor position.
     *
     * Parses the HTML to extract styled text, deletes any current selection,
     * and inserts the parsed content with formatting preserved.
     * @param {string} html
     */
    paste_html(html) {
        const ptr0 = passStringToWasm0(html, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_paste_html(this.__wbg_ptr, ptr0, len0);
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
     * Jump to the previous bookmark before the current line.
     *
     * Wraps around to the last bookmark if before the first one.
     * Returns `true` if a bookmark was found.
     * @returns {boolean}
     */
    prev_bookmark() {
        const ret = wasm.canvisteditor_prev_bookmark(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Process all pending canonical events via the editor runtime.
     */
    process_events() {
        wasm.canvisteditor_process_events(this.__wbg_ptr);
    }
    /**
     * Record the current cursor position in the history stack.
     *
     * Call this before navigation jumps (go-to-line, bookmark jump, etc.)
     * so the user can navigate back. Deduplicates consecutive identical
     * positions and caps the stack at 100 entries.
     */
    push_cursor_history() {
        wasm.canvisteditor_push_cursor_history(this.__wbg_ptr);
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
     * Check whether the editor is in read-only mode.
     * @returns {boolean}
     */
    read_only() {
        const ret = wasm.canvisteditor_read_only(this.__wbg_ptr);
        return ret !== 0;
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
     * Remove consecutive duplicate lines from the document.
     *
     * Returns the number of lines removed.
     * @returns {number}
     */
    remove_duplicate_lines() {
        const ret = wasm.canvisteditor_remove_duplicate_lines(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Remove the background (highlight) colour from the current selection.
     */
    remove_highlight_color() {
        wasm.canvisteditor_remove_highlight_color(this.__wbg_ptr);
    }
    /**
     * Remove the ruler at the given column.
     * @param {number} column
     */
    remove_ruler(column) {
        wasm.canvisteditor_remove_ruler(this.__wbg_ptr, column);
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
     * Replace all occurrences of the selected text with `replacement`.
     *
     * Returns the number of replacements made. Processes from end to
     * start so offsets remain valid.
     * @param {string} replacement
     * @returns {number}
     */
    replace_all_occurrences(replacement) {
        const ptr0 = passStringToWasm0(replacement, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_replace_all_occurrences(this.__wbg_ptr, ptr0, len0);
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
     * Reverse the order of selected lines.
     */
    reverse_lines() {
        wasm.canvisteditor_reverse_lines(this.__wbg_ptr);
    }
    /**
     * Get the current ruler columns as a flat array.
     * @returns {Uint32Array}
     */
    rulers() {
        const ret = wasm.canvisteditor_rulers(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Scroll by a delta (positive = down, negative = up).
     * @param {number} delta_y
     */
    scroll_by(delta_y) {
        wasm.canvisteditor_scroll_by(this.__wbg_ptr, delta_y);
    }
    /**
     * Ensure the current selection (or cursor) is visible in the
     * viewport. Scrolls the minimum amount needed.
     */
    scroll_to_selection() {
        wasm.canvisteditor_scroll_to_selection(this.__wbg_ptr);
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
     * Find all occurrences of the currently selected text.
     *
     * Returns the count of matches found (0 if nothing is selected or no
     * matches). The offsets can be retrieved with `find_all`.
     * @returns {number}
     */
    select_all_occurrences() {
        const ret = wasm.canvisteditor_select_all_occurrences(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Select all text between the nearest enclosing bracket pair.
     *
     * Returns `true` if brackets were found and selection was made.
     * @returns {boolean}
     */
    select_between_brackets() {
        const ret = wasm.canvisteditor_select_between_brackets(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Select the entire current line (Ctrl+L).
     *
     * Repeated calls extend the selection by one line each time.
     */
    select_line() {
        wasm.canvisteditor_select_line(this.__wbg_ptr);
    }
    /**
     * Select from cursor to document end (Ctrl+Shift+End).
     */
    select_to_document_end() {
        wasm.canvisteditor_select_to_document_end(this.__wbg_ptr);
    }
    /**
     * Select from cursor to document start (Ctrl+Shift+Home).
     */
    select_to_document_start() {
        wasm.canvisteditor_select_to_document_start(this.__wbg_ptr);
    }
    /**
     * Select the word at the given character offset.
     * @param {number} offset
     */
    select_word_at(offset) {
        wasm.canvisteditor_select_word_at(this.__wbg_ptr, offset);
    }
    /**
     * Number of characters currently selected (0 if collapsed).
     * @returns {number}
     */
    selected_char_count() {
        const ret = wasm.canvisteditor_selected_char_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Number of words in the current selection (0 if collapsed).
     * @returns {number}
     */
    selected_word_count() {
        const ret = wasm.canvisteditor_selected_word_count(this.__wbg_ptr);
        return ret >>> 0;
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
     * Toggle bracket auto-closing.
     *
     * When enabled, typing `(`, `[`, `{`, `"`, or `'` automatically
     * inserts the closing counterpart and places the cursor between them.
     * @param {boolean} enabled
     */
    set_auto_close_brackets(enabled) {
        wasm.canvisteditor_set_auto_close_brackets(this.__wbg_ptr, enabled);
    }
    /**
     * Enable or disable auto-surround on selection.
     *
     * When enabled and text is selected, typing an opening bracket
     * wraps the selection instead of replacing it.
     * @param {boolean} enabled
     */
    set_auto_surround(enabled) {
        wasm.canvisteditor_set_auto_surround(this.__wbg_ptr, enabled);
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
     * Set the line comment prefix (default `"// "`).
     * @param {string} prefix
     */
    set_comment_prefix(prefix) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_set_comment_prefix(this.__wbg_ptr, ptr0, len0);
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
     * Set a background (highlight) colour on the current selection.
     *
     * The colour is stored via the style's `background` field.
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_highlight_color(r, g, b, a) {
        wasm.canvisteditor_set_highlight_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * Enable or disable the current-line highlight band.
     * @param {boolean} enabled
     */
    set_highlight_current_line(enabled) {
        wasm.canvisteditor_set_highlight_current_line(this.__wbg_ptr, enabled);
    }
    /**
     * Toggle matching bracket highlight.
     * @param {boolean} enabled
     */
    set_highlight_matching_brackets(enabled) {
        wasm.canvisteditor_set_highlight_matching_brackets(this.__wbg_ptr, enabled);
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
     * Set overwrite mode explicitly.
     * @param {boolean} enabled
     */
    set_overwrite_mode(enabled) {
        wasm.canvisteditor_set_overwrite_mode(this.__wbg_ptr, enabled);
    }
    /**
     * Set the editor to read-only mode. Editing operations are blocked;
     * selection, copy, and navigation still work.
     * @param {boolean} read_only
     */
    set_read_only(read_only) {
        wasm.canvisteditor_set_read_only(this.__wbg_ptr, read_only);
    }
    /**
     * Set column ruler positions (e.g. `[80, 120]`).
     *
     * Pass an empty array to remove all rulers. Rulers are drawn as
     * thin vertical lines at the specified column offsets.
     * @param {Uint32Array} columns
     */
    set_rulers(columns) {
        const ptr0 = passArray32ToWasm0(columns, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_set_rulers(this.__wbg_ptr, ptr0, len0);
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
     * Toggle indent guide rendering.
     * @param {boolean} show
     */
    set_show_indent_guides(show) {
        wasm.canvisteditor_set_show_indent_guides(this.__wbg_ptr, show);
    }
    /**
     * Enable or disable the line-number gutter.
     * @param {boolean} show
     */
    set_show_line_numbers(show) {
        wasm.canvisteditor_set_show_line_numbers(this.__wbg_ptr, show);
    }
    /**
     * Toggle the visual whitespace indicator.
     *
     * When enabled, the renderer draws `·` for spaces and `→` for tabs.
     * @param {boolean} show
     */
    set_show_whitespace(show) {
        wasm.canvisteditor_set_show_whitespace(this.__wbg_ptr, show);
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
     * Enable or disable soft tabs (spaces instead of `\t`).
     * @param {boolean} enabled
     */
    set_soft_tabs(enabled) {
        wasm.canvisteditor_set_soft_tabs(this.__wbg_ptr, enabled);
    }
    /**
     * Set the tab display/insert size (1–8). Default: 4.
     * @param {number} size
     */
    set_tab_size(size) {
        wasm.canvisteditor_set_tab_size(this.__wbg_ptr, size);
    }
    /**
     * Switch to the dark colour theme.
     */
    set_theme_dark() {
        wasm.canvisteditor_set_theme_dark(this.__wbg_ptr);
    }
    /**
     * Switch to the light colour theme.
     */
    set_theme_light() {
        wasm.canvisteditor_set_theme_light(this.__wbg_ptr);
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
     * Enable or disable word wrapping at the canvas edge.
     *
     * When disabled, lines extend horizontally and horizontal scrolling
     * may be needed.
     * @param {boolean} enabled
     */
    set_word_wrap(enabled) {
        wasm.canvisteditor_set_word_wrap(this.__wbg_ptr, enabled);
    }
    /**
     * Set the zoom level (1.0 = 100%, 1.5 = 150%, etc.). Clamped to [0.25, 4.0].
     * @param {number} level
     */
    set_zoom(level) {
        wasm.canvisteditor_set_zoom(this.__wbg_ptr, level);
    }
    /**
     * Whether indent guides are enabled.
     * @returns {boolean}
     */
    show_indent_guides() {
        const ret = wasm.canvisteditor_show_indent_guides(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Check whether line numbers are visible.
     * @returns {boolean}
     */
    show_line_numbers() {
        const ret = wasm.canvisteditor_show_line_numbers(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Whether whitespace visualization is enabled.
     * @returns {boolean}
     */
    show_whitespace() {
        const ret = wasm.canvisteditor_show_whitespace(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * If the cursor is between a matching bracket pair (e.g. `(|)`),
     * delete both characters. Otherwise, behave like normal backspace.
     *
     * Returns `true` if a pair was deleted, `false` for normal backspace.
     * @returns {boolean}
     */
    smart_backspace() {
        const ret = wasm.canvisteditor_smart_backspace(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Whether soft tabs are enabled.
     * @returns {boolean}
     */
    soft_tabs() {
        const ret = wasm.canvisteditor_soft_tabs(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Sort selected lines in ascending alphabetical order.
     */
    sort_lines_asc() {
        wasm.canvisteditor_sort_lines_asc(this.__wbg_ptr);
    }
    /**
     * Sort selected lines in descending alphabetical order.
     */
    sort_lines_desc() {
        wasm.canvisteditor_sort_lines_desc(this.__wbg_ptr);
    }
    /**
     * Convert leading spaces to tabs (using the current tab_size).
     *
     * Only converts groups of `tab_size` spaces at the start of lines.
     * Returns the number of conversions made.
     * @returns {number}
     */
    spaces_to_tabs() {
        const ret = wasm.canvisteditor_spaces_to_tabs(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get the current tab size.
     * @returns {number}
     */
    tab_size() {
        const ret = wasm.canvisteditor_tab_size(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Convert all tabs to spaces (using the current tab_size).
     *
     * Returns the number of tabs replaced.
     * @returns {number}
     */
    tabs_to_spaces() {
        const ret = wasm.canvisteditor_tabs_to_spaces(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Return `"dark"` or `"light"` depending on the active theme.
     * @returns {string}
     */
    theme_name() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_theme_name(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Export the document as HTML.
     *
     * Each paragraph becomes a `<p>` element. Styled text gets inline
     * elements (`<strong>`, `<em>`, `<u>`, `<s>`, `<span>`).
     * @returns {string}
     */
    to_html() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_to_html(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
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
     * Export the document as Markdown.
     *
     * Bold → `**text**`, italic → `*text*`, strikethrough → `~~text~~`.
     * @returns {string}
     */
    to_markdown() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_to_markdown(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
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
     * Toggle a bookmark on the current line.
     *
     * Returns `true` if the bookmark was added, `false` if removed.
     * @returns {boolean}
     */
    toggle_bookmark() {
        const ret = wasm.canvisteditor_toggle_bookmark(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Toggle a bullet list prefix (`• `) on the current line.
     *
     * If the line already starts with `• `, the prefix is removed.
     * Otherwise it is inserted at the line start (after leading whitespace).
     */
    toggle_bullet_list() {
        wasm.canvisteditor_toggle_bullet_list(this.__wbg_ptr);
    }
    /**
     * Toggle italic on the current selection. Preserves the current
     * selection.
     */
    toggle_italic() {
        wasm.canvisteditor_toggle_italic(this.__wbg_ptr);
    }
    /**
     * Toggle a line-comment prefix on the current line or all selected
     * lines.
     *
     * If all affected lines start with the prefix, it is removed from
     * each. Otherwise the prefix is added to every line.
     */
    toggle_line_comment() {
        wasm.canvisteditor_toggle_line_comment(this.__wbg_ptr);
    }
    /**
     * Toggle a numbered list prefix (`1. `) on the current line.
     *
     * If the line already starts with a number prefix, it is removed.
     * Otherwise `1. ` is inserted.
     */
    toggle_numbered_list() {
        wasm.canvisteditor_toggle_numbered_list(this.__wbg_ptr);
    }
    /**
     * Toggle between insert and overwrite mode (Insert key).
     */
    toggle_overwrite_mode() {
        wasm.canvisteditor_toggle_overwrite_mode(this.__wbg_ptr);
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
     * Convert selected text to lowercase.
     */
    transform_lowercase() {
        wasm.canvisteditor_transform_lowercase(this.__wbg_ptr);
    }
    /**
     * Convert selected text to Title Case.
     */
    transform_title_case() {
        wasm.canvisteditor_transform_title_case(this.__wbg_ptr);
    }
    /**
     * Swap the case of each character in the selection (a↔A).
     */
    transform_toggle_case() {
        wasm.canvisteditor_transform_toggle_case(this.__wbg_ptr);
    }
    /**
     * Convert selected text to UPPERCASE.
     */
    transform_uppercase() {
        wasm.canvisteditor_transform_uppercase(this.__wbg_ptr);
    }
    /**
     * Swap the two characters around the cursor (Ctrl+T).
     *
     * If the cursor is at the end of a line, swaps the two preceding
     * characters instead.
     */
    transpose_chars() {
        wasm.canvisteditor_transpose_chars(this.__wbg_ptr);
    }
    /**
     * Remove trailing whitespace (spaces and tabs) from every line.
     *
     * Returns the number of characters removed.
     * @returns {number}
     */
    trim_trailing_whitespace() {
        const ret = wasm.canvisteditor_trim_trailing_whitespace(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * If auto-surround is on and the selection is non-empty, wrap the
     * selection with the opening/closing pair. Returns `true` if wrapping
     * happened.
     * @param {string} ch
     * @returns {boolean}
     */
    try_auto_surround(ch) {
        const ptr0 = passStringToWasm0(ch, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_try_auto_surround(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
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
     * URL-decode the selected text, replacing the selection.
     */
    url_decode_selection() {
        wasm.canvisteditor_url_decode_selection(this.__wbg_ptr);
    }
    /**
     * URL-encode the selected text, replacing the selection.
     */
    url_encode_selection() {
        wasm.canvisteditor_url_encode_selection(this.__wbg_ptr);
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
    /**
     * Whether word wrapping is enabled.
     * @returns {boolean}
     */
    word_wrap() {
        const ret = wasm.canvisteditor_word_wrap(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Wrap the selected text with a pair of strings (e.g. brackets).
     *
     * Example: `wrap_selection("(", ")")` turns `hello` into `(hello)`.
     * Cursor is placed after the closing string.
     * @param {string} open
     * @param {string} close
     */
    wrap_selection(open, close) {
        const ptr0 = passStringToWasm0(open, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(close, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.canvisteditor_wrap_selection(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * Get the current zoom level.
     * @returns {number}
     */
    zoom() {
        const ret = wasm.canvisteditor_zoom(this.__wbg_ptr);
        return ret;
    }
    /**
     * Zoom in by one step (1.1× multiplier).
     */
    zoom_in() {
        wasm.canvisteditor_zoom_in(this.__wbg_ptr);
    }
    /**
     * Zoom out by one step (÷ 1.1).
     */
    zoom_out() {
        wasm.canvisteditor_zoom_out(this.__wbg_ptr);
    }
    /**
     * Reset zoom to 100%.
     */
    zoom_reset() {
        wasm.canvisteditor_zoom_reset(this.__wbg_ptr);
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

function passArray32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getUint32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
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
