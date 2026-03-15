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
     * Add an annotation to a text range.
     *
     * `kind` examples: "error", "warning", "info", "spelling".
     * `message` is optional descriptive text.
     * @param {number} start
     * @param {number} end
     * @param {string} kind
     * @param {string} message
     */
    add_annotation(start, end, kind, message) {
        const ptr0 = passStringToWasm0(kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(message, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.canvisteditor_add_annotation(this.__wbg_ptr, start, end, ptr0, len0, ptr1, len1);
    }
    /**
     * Add a collaborative cursor (another user's position).
     *
     * Each cursor has an offset, display name, and RGB colour.
     * @param {number} offset
     * @param {string} name
     * @param {number} r
     * @param {number} g
     * @param {number} b
     */
    add_collab_cursor(offset, name, r, g, b) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_add_collab_cursor(this.__wbg_ptr, offset, ptr0, len0, r, g, b);
    }
    /**
     * Add an extra cursor at a character offset.
     *
     * Extra cursors are rendered alongside the primary cursor.
     * Use `multi_cursor_insert` to type at all positions.
     * @param {number} offset
     */
    add_cursor(offset) {
        wasm.canvisteditor_add_cursor(this.__wbg_ptr, offset);
    }
    /**
     * Add a coloured background decoration to a line (0-based).
     *
     * Multiple decorations can be added to the same line. The colours
     * are blended in order.
     * @param {number} line
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    add_line_decoration(line, r, g, b, a) {
        wasm.canvisteditor_add_line_decoration(this.__wbg_ptr, line, r, g, b, a);
    }
    /**
     * Add a coloured marker highlight range.
     *
     * Returns the marker ID for later removal.
     * @param {number} start
     * @param {number} end
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     * @param {string} id
     */
    add_marker(start, end, r, g, b, a, id) {
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_add_marker(this.__wbg_ptr, start, end, r, g, b, a, ptr0, len0);
    }
    /**
     * Add a single ruler at the given column.
     * @param {number} column
     */
    add_ruler(column) {
        wasm.canvisteditor_add_ruler(this.__wbg_ptr, column);
    }
    /**
     * Number of named anchors.
     * @returns {number}
     */
    anchor_count() {
        const ret = wasm.canvisteditor_anchor_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Anchor entries as flat `[name, offset, ...]`, sorted by name.
     * @returns {string[]}
     */
    anchor_entries() {
        const ret = wasm.canvisteditor_anchor_entries(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Whether a named anchor exists.
     * @param {string} name
     * @returns {boolean}
     */
    anchor_exists(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_anchor_exists(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * List anchor names sorted alphabetically.
     * @returns {string[]}
     */
    anchor_names() {
        const ret = wasm.canvisteditor_anchor_names(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Anchor names inside an inclusive character-offset range.
     *
     * Sorted by offset then name.
     * @param {number} start_offset
     * @param {number} end_offset
     * @returns {string[]}
     */
    anchor_names_in_range(start_offset, end_offset) {
        const ret = wasm.canvisteditor_anchor_names_in_range(this.__wbg_ptr, start_offset, end_offset);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Anchor names that start with a prefix, sorted by name.
     * @param {string} prefix
     * @returns {string[]}
     */
    anchor_names_with_prefix(prefix) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_anchor_names_with_prefix(this.__wbg_ptr, ptr0, len0);
        var v2 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * Get a named anchor offset, or -1 if not found.
     * @param {string} name
     * @returns {number}
     */
    anchor_offset(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_anchor_offset(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Anchor names set exactly at a given offset.
     * @param {number} offset
     * @returns {string[]}
     */
    anchors_at_offset(offset) {
        const ret = wasm.canvisteditor_anchors_at_offset(this.__wbg_ptr, offset);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Anchors inside an inclusive character-offset range.
     *
     * Returns `[name, offset, ...]` sorted by offset then name.
     * @param {number} start_offset
     * @param {number} end_offset
     * @returns {string[]}
     */
    anchors_in_range(start_offset, end_offset) {
        const ret = wasm.canvisteditor_anchors_in_range(this.__wbg_ptr, start_offset, end_offset);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Number of active annotations.
     * @returns {number}
     */
    annotation_count() {
        const ret = wasm.canvisteditor_annotation_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get annotations overlapping a character offset.
     *
     * Returns flat array: [start, end, kind, message, ...].
     * @param {number} offset
     * @returns {string[]}
     */
    annotations_at(offset) {
        const ret = wasm.canvisteditor_annotations_at(this.__wbg_ptr, offset);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Total number of public API methods.
     * @returns {number}
     */
    api_count() {
        const ret = wasm.canvisteditor_api_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Apply a simple text patch.
     *
     * `operations` is a flat array of strings: ["insert", "offset", "text",
     * "delete", "start", "end", ...]. Processed from end to start.
     * @param {string[]} operations
     */
    apply_patch(operations) {
        const ptr0 = passArrayJsValueToWasm0(operations, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_apply_patch(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Apply a named configuration preset.
     *
     * - `"code"`: line numbers, indent guides, whitespace, bracket
     *   highlight, occurrence highlight, auto-close brackets, soft tabs
     * - `"prose"`: word wrap, no line numbers, no whitespace, no
     *   indent guides, placeholder
     * - `"minimal"`: minimal chrome, no gutter, no highlights
     * @param {string} name
     */
    apply_preset(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_apply_preset(this.__wbg_ptr, ptr0, len0);
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
     * Average number of characters per line.
     * @returns {number}
     */
    avg_line_length() {
        const ret = wasm.canvisteditor_avg_line_length(this.__wbg_ptr);
        return ret;
    }
    /**
     * Average word length in characters.
     * @returns {number}
     */
    avg_word_length() {
        const ret = wasm.canvisteditor_avg_word_length(this.__wbg_ptr);
        return ret;
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
     * Begin a batch of operations that will be grouped into a single
     * undo step. Call `end_batch` when done.
     *
     * The runtime coalesces rapid edits automatically. This method
     * serves as a logical marker — all edits between `begin_batch`
     * and `end_batch` happen in quick succession and are treated as
     * one undo group.
     */
    begin_batch() {
        wasm.canvisteditor_begin_batch(this.__wbg_ptr);
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
     * Get document breadcrumbs — lines that start with #, //, or are
     * all-caps (treated as section headers).
     *
     * Returns flat array: [line_number, text, line_number, text, ...].
     * @returns {string[]}
     */
    breadcrumbs() {
        const ret = wasm.canvisteditor_breadcrumbs(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
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
     * Total byte count of the document (UTF-8).
     * @returns {number}
     */
    byte_count() {
        const ret = wasm.canvisteditor_byte_count(this.__wbg_ptr);
        return ret >>> 0;
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
     * Count characters by type: [letters, digits, spaces, punctuation, other].
     *
     * Returns a 5-element array.
     * @returns {Uint32Array}
     */
    char_counts() {
        const ret = wasm.canvisteditor_char_counts(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Clear all named anchors.
     */
    clear_anchors() {
        wasm.canvisteditor_clear_anchors(this.__wbg_ptr);
    }
    /**
     * Remove all annotations.
     */
    clear_annotations() {
        wasm.canvisteditor_clear_annotations(this.__wbg_ptr);
    }
    /**
     * Remove all bookmarks.
     */
    clear_bookmarks() {
        wasm.canvisteditor_clear_bookmarks(this.__wbg_ptr);
    }
    /**
     * Clear all collaborative cursors.
     */
    clear_collab_cursors() {
        wasm.canvisteditor_clear_collab_cursors(this.__wbg_ptr);
    }
    /**
     * Remove completed markdown task lines (`[x]` / `[X]`).
     *
     * Returns number of lines removed.
     * @returns {number}
     */
    clear_completed_tasks() {
        const ret = wasm.canvisteditor_clear_completed_tasks(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Clear all extra cursors.
     */
    clear_cursors() {
        wasm.canvisteditor_clear_cursors(this.__wbg_ptr);
    }
    /**
     * Clear all custom keybinding overrides.
     */
    clear_keybindings() {
        wasm.canvisteditor_clear_keybindings(this.__wbg_ptr);
    }
    /**
     * Remove all line decorations.
     */
    clear_line_decorations() {
        wasm.canvisteditor_clear_line_decorations(this.__wbg_ptr);
    }
    /**
     * Clear all markers.
     */
    clear_markers() {
        wasm.canvisteditor_clear_markers(this.__wbg_ptr);
    }
    /**
     * Remove all named states.
     */
    clear_named_states() {
        wasm.canvisteditor_clear_named_states(this.__wbg_ptr);
    }
    /**
     * Clear all selection profiles.
     */
    clear_selection_profiles() {
        wasm.canvisteditor_clear_selection_profiles(this.__wbg_ptr);
    }
    /**
     * Clear the saved snapshot.
     */
    clear_snapshot() {
        wasm.canvisteditor_clear_snapshot(this.__wbg_ptr);
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
     * Clear the clipboard ring.
     */
    clipboard_ring_clear() {
        wasm.canvisteditor_clipboard_ring_clear(this.__wbg_ptr);
    }
    /**
     * Get the clipboard ring entry at `index` (0 = most recent).
     *
     * Returns empty string if index is out of range.
     * @param {number} index
     * @returns {string}
     */
    clipboard_ring_get(index) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_clipboard_ring_get(this.__wbg_ptr, index);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Number of entries in the clipboard ring.
     * @returns {number}
     */
    clipboard_ring_length() {
        const ret = wasm.canvisteditor_clipboard_ring_length(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Paste the clipboard ring entry at `index` at the cursor.
     * @param {number} index
     */
    clipboard_ring_paste(index) {
        wasm.canvisteditor_clipboard_ring_paste(this.__wbg_ptr, index);
    }
    /**
     * Push a text entry into the clipboard ring.
     *
     * The ring holds the most recent `clipboard_ring_max` entries
     * (default 10). Newest entry is at index 0.
     * @param {string} text
     */
    clipboard_ring_push(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_clipboard_ring_push(this.__wbg_ptr, ptr0, len0);
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
     * Number of collaborative cursors.
     * @returns {number}
     */
    collab_cursor_count() {
        const ret = wasm.canvisteditor_collab_cursor_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get all collaborative cursors as [offset, name, r, g, b, ...].
     * @returns {string[]}
     */
    collab_cursor_list() {
        const ret = wasm.canvisteditor_collab_cursor_list(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Collapse blank-line runs to at most `max_consecutive` lines.
     *
     * Returns number of lines removed.
     * @param {number} max_consecutive
     * @returns {number}
     */
    collapse_blank_lines(max_consecutive) {
        const ret = wasm.canvisteditor_collapse_blank_lines(this.__wbg_ptr, max_consecutive);
        return ret >>> 0;
    }
    /**
     * Return all available editor commands as a flat array:
     * [name, keybinding, name, keybinding, ...].
     *
     * Useful for building a command palette UI.
     * @returns {string[]}
     */
    command_list() {
        const ret = wasm.canvisteditor_command_list(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
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
     * Mark all unchecked markdown tasks as checked.
     *
     * Returns number of lines updated.
     * @returns {number}
     */
    complete_all_tasks() {
        const ret = wasm.canvisteditor_complete_all_tasks(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Suggest completions for the word currently being typed.
     *
     * Returns up to `max_results` words from the document that start
     * with the prefix at the cursor. Sorted alphabetically, deduplicated.
     * @param {number} max_results
     * @returns {string[]}
     */
    completions(max_results) {
        const ret = wasm.canvisteditor_completions(this.__wbg_ptr, max_results);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Get filtered word completions with context.
     *
     * Returns [word, lineContext, ...] where lineContext is the line
     * where the word appears. Max `limit` results.
     * @param {number} limit
     * @returns {string[]}
     */
    completions_with_context(limit) {
        const ret = wasm.canvisteditor_completions_with_context(this.__wbg_ptr, limit);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Whether the document contains any non-ASCII characters.
     * @returns {boolean}
     */
    contains_non_ascii() {
        const ret = wasm.canvisteditor_contains_non_ascii(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Whether the document contains any RTL (right-to-left) characters.
     *
     * Detects Arabic, Hebrew, and other RTL scripts.
     * @returns {boolean}
     */
    contains_rtl() {
        const ret = wasm.canvisteditor_contains_rtl(this.__wbg_ptr);
        return ret !== 0;
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
     * Convert all line endings to CRLF.
     *
     * Returns the number of conversions made.
     * @returns {number}
     */
    convert_to_crlf() {
        const ret = wasm.canvisteditor_convert_to_crlf(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Convert all line endings to LF.
     * @returns {number}
     */
    convert_to_lf() {
        const ret = wasm.canvisteditor_convert_to_lf(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Count lines whose text starts with prefix.
     * @param {string} prefix
     * @param {boolean} case_sensitive
     * @returns {number}
     */
    count_lines_with_prefix(prefix, case_sensitive) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_count_lines_with_prefix(this.__wbg_ptr, ptr0, len0, case_sensitive);
        return ret >>> 0;
    }
    /**
     * Count lines whose text ends with suffix.
     * @param {string} suffix
     * @param {boolean} case_sensitive
     * @returns {number}
     */
    count_lines_with_suffix(suffix, case_sensitive) {
        const ptr0 = passStringToWasm0(suffix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_count_lines_with_suffix(this.__wbg_ptr, ptr0, len0, case_sensitive);
        return ret >>> 0;
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
     * Get cursor style (0=line, 1=block, 2=underline).
     * @returns {number}
     */
    cursor_style() {
        const ret = wasm.canvisteditor_cursor_style(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get cursor width.
     * @returns {number}
     */
    cursor_width_px() {
        const ret = wasm.canvisteditor_cursor_width_px(this.__wbg_ptr);
        return ret;
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
     * Remove a named state.
     * @param {string} name
     */
    delete_named_state(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_delete_named_state(this.__wbg_ptr, ptr0, len0);
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
     * Remove a named selection profile.
     * @param {string} name
     */
    delete_selection_profile(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_delete_selection_profile(this.__wbg_ptr, ptr0, len0);
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
     * Guess the file type from content.
     *
     * Returns a string like "javascript", "python", "html", "css",
     * "json", "markdown", "xml", "rust", "text".
     * @returns {string}
     */
    detect_file_type() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_detect_file_type(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Detect the dominant line ending style.
     *
     * Returns "lf", "crlf", or "mixed".
     * @returns {string}
     */
    detect_line_ending() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_detect_line_ending(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Whether link detection is enabled.
     * @returns {boolean}
     */
    detect_links() {
        const ret = wasm.canvisteditor_detect_links(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Compare current text against the last snapshot.
     *
     * Returns a list of changed line numbers (0-based) as a flat array.
     * A line is "changed" if it differs from the snapshot.
     * @returns {Uint32Array}
     */
    diff_from_snapshot() {
        const ret = wasm.canvisteditor_diff_from_snapshot(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Compare two texts line by line.
     *
     * Returns flat array: [kind, lineNumber, text, ...] where kind is
     * "added", "removed", or "changed".
     * @param {string} a
     * @param {string} b
     * @returns {string[]}
     */
    static diff_texts(a, b) {
        const ptr0 = passStringToWasm0(a, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(b, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_diff_texts(ptr0, len0, ptr1, len1);
        var v3 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v3;
    }
    /**
     * Build a document outline from indentation levels.
     *
     * Returns flat array: [indent, lineNumber, text, ...] for non-empty
     * lines. The indent value can be used to build a tree structure.
     * @returns {string[]}
     */
    document_outline() {
        const ret = wasm.canvisteditor_document_outline(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Duplicate the current line (or selected lines) below.
     */
    duplicate_line() {
        wasm.canvisteditor_duplicate_line(this.__wbg_ptr);
    }
    /**
     * Number of lines that belong to duplicate-content groups.
     * @param {boolean} case_sensitive
     * @param {boolean} ignore_whitespace
     * @returns {number}
     */
    duplicate_line_count(case_sensitive, ignore_whitespace) {
        const ret = wasm.canvisteditor_duplicate_line_count(this.__wbg_ptr, case_sensitive, ignore_whitespace);
        return ret >>> 0;
    }
    /**
     * Line numbers that have duplicated content.
     *
     * `ignore_whitespace` collapses whitespace and trims ends before
     * comparison. Returns sorted 0-based line numbers.
     * @param {boolean} case_sensitive
     * @param {boolean} ignore_whitespace
     * @returns {Uint32Array}
     */
    duplicate_line_numbers(case_sensitive, ignore_whitespace) {
        const ret = wasm.canvisteditor_duplicate_line_numbers(this.__wbg_ptr, case_sensitive, ignore_whitespace);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Duplicate a line range and insert it below the range.
     *
     * Returns `true` on success.
     * @param {number} start_line
     * @param {number} end_line
     * @returns {boolean}
     */
    duplicate_line_range(start_line, end_line) {
        const ret = wasm.canvisteditor_duplicate_line_range(this.__wbg_ptr, start_line, end_line);
        return ret !== 0;
    }
    /**
     * Ratio of duplicate lines to total lines.
     * @param {boolean} case_sensitive
     * @param {boolean} ignore_whitespace
     * @returns {number}
     */
    duplicate_line_ratio(case_sensitive, ignore_whitespace) {
        const ret = wasm.canvisteditor_duplicate_line_ratio(this.__wbg_ptr, case_sensitive, ignore_whitespace);
        return ret;
    }
    /**
     * Editor version string.
     * @returns {string}
     */
    editor_version() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_editor_version(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * End a batch of operations.
     *
     * After this call, the next edit will start a new undo group
     * (once the coalesce timeout expires).
     */
    end_batch() {
        wasm.canvisteditor_end_batch(this.__wbg_ptr);
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
     * Ensure the document ends with exactly one trailing newline.
     *
     * Returns `true` if content was changed.
     * @returns {boolean}
     */
    ensure_single_trailing_newline() {
        const ret = wasm.canvisteditor_ensure_single_trailing_newline(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Clear the event log.
     */
    event_log_clear() {
        wasm.canvisteditor_event_log_clear(this.__wbg_ptr);
    }
    /**
     * Get event log entry at index (0 = newest).
     * @param {number} index
     * @returns {string}
     */
    event_log_get(index) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_event_log_get(this.__wbg_ptr, index);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Number of entries in the event log.
     * @returns {number}
     */
    event_log_length() {
        const ret = wasm.canvisteditor_event_log_length(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Expand an Emmet-style abbreviation at the cursor.
     *
     * Supports simple patterns:
     * - `tag` → `<tag></tag>`
     * - `tag.class` → `<tag class="class"></tag>`
     * - `tag#id` → `<tag id="id"></tag>`
     * - `tag*n` → `<tag></tag>` repeated n times
     * - `lorem` → placeholder lorem ipsum text
     *
     * Returns true if an expansion was performed.
     * @returns {boolean}
     */
    expand_emmet() {
        const ret = wasm.canvisteditor_expand_emmet(this.__wbg_ptr);
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
     * Export the current canvas as a PNG data URL.
     *
     * Returns empty string if the canvas is not available.
     * @returns {string}
     */
    export_canvas_data_url() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_export_canvas_data_url(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Number of extra cursors (not counting the primary).
     * @returns {number}
     */
    extra_cursor_count() {
        const ret = wasm.canvisteditor_extra_cursor_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get all extra cursor offsets.
     * @returns {Uint32Array}
     */
    extra_cursor_offsets() {
        const ret = wasm.canvisteditor_extra_cursor_offsets(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Feature summary as a comma-separated list of categories.
     * @returns {string}
     */
    feature_categories() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_feature_categories(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
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
     * Find all matches of a regex pattern in the document.
     *
     * Returns offsets as `[start0, end0, start1, end1, ...]`.
     * Returns empty array if the pattern is invalid.
     *
     * Note: uses a simple character-by-character implementation since
     * the `regex` crate is heavy for WASM. Supports: `.` `*` `+` `?`
     * `^` `$` `\d` `\w` `\s` and character classes `[abc]`.
     * For full regex, use the JS `RegExp` in the host and pass offsets.
     * @param {string} pattern
     * @returns {Uint32Array}
     */
    find_all_regex(pattern) {
        const ptr0 = passStringToWasm0(pattern, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_find_all_regex(this.__wbg_ptr, ptr0, len0);
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
     * Get the current find highlight needle.
     * @returns {string}
     */
    find_highlight_needle() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_find_highlight_needle(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Find all URLs in the document.
     *
     * Returns flat array: [start, end, start, end, ...] of char offsets.
     * @returns {Uint32Array}
     */
    find_links() {
        const ret = wasm.canvisteditor_find_links(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
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
     * First duplicate line number, or -1 when no duplicates.
     * @param {boolean} case_sensitive
     * @param {boolean} ignore_whitespace
     * @returns {number}
     */
    first_duplicate_line(case_sensitive, ignore_whitespace) {
        const ret = wasm.canvisteditor_first_duplicate_line(this.__wbg_ptr, case_sensitive, ignore_whitespace);
        return ret;
    }
    /**
     * Get the first visible line number (0-based).
     * @returns {number}
     */
    first_visible_line() {
        const ret = wasm.canvisteditor_first_visible_line(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Flesch reading ease score (0–100, higher = easier).
     *
     * Simplified: uses average words per sentence and average
     * syllables per word.
     * @returns {number}
     */
    flesch_reading_ease() {
        const ret = wasm.canvisteditor_flesch_reading_ease(this.__wbg_ptr);
        return ret;
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
     * Number of active fold regions.
     * @returns {number}
     */
    fold_count() {
        const ret = wasm.canvisteditor_fold_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Fold (collapse) a range of lines (0-based, inclusive).
     *
     * The first line remains visible; subsequent lines are hidden.
     * @param {number} start_line
     * @param {number} end_line
     */
    fold_lines(start_line, end_line) {
        wasm.canvisteditor_fold_lines(this.__wbg_ptr, start_line, end_line);
    }
    /**
     * Get all folded ranges as flat array: [start0, end0, start1, end1, ...].
     * @returns {Uint32Array}
     */
    folded_ranges() {
        const ret = wasm.canvisteditor_folded_ranges(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Apply bold to a character range.
     * @param {number} start
     * @param {number} end
     */
    format_range_bold(start, end) {
        wasm.canvisteditor_format_range_bold(this.__wbg_ptr, start, end);
    }
    /**
     * Set font size for a character range.
     * @param {number} start
     * @param {number} end
     * @param {number} size
     */
    format_range_font_size(start, end, size) {
        wasm.canvisteditor_format_range_font_size(this.__wbg_ptr, start, end, size);
    }
    /**
     * Apply italic to a character range.
     * @param {number} start
     * @param {number} end
     */
    format_range_italic(start, end) {
        wasm.canvisteditor_format_range_italic(this.__wbg_ptr, start, end);
    }
    /**
     * Apply strikethrough to a character range.
     * @param {number} start
     * @param {number} end
     */
    format_range_strikethrough(start, end) {
        wasm.canvisteditor_format_range_strikethrough(this.__wbg_ptr, start, end);
    }
    /**
     * Apply underline to a character range.
     * @param {number} start
     * @param {number} end
     */
    format_range_underline(start, end) {
        wasm.canvisteditor_format_range_underline(this.__wbg_ptr, start, end);
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
     * Get annotations as flat array: [start, end, kind, message, ...].
     * @returns {string[]}
     */
    get_annotations() {
        const ret = wasm.canvisteditor_get_annotations(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Get text from a rectangular block selection.
     *
     * Returns lines from `start_line` to `end_line` (inclusive),
     * each trimmed to columns `start_col` to `end_col` (char-based).
     * @param {number} start_line
     * @param {number} end_line
     * @param {number} start_col
     * @param {number} end_col
     * @returns {string}
     */
    get_block_selection(start_line, end_line, start_col, end_col) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_get_block_selection(this.__wbg_ptr, start_line, end_line, start_col, end_col);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the command bound to a shortcut (custom override or default).
     * @param {string} shortcut
     * @returns {string}
     */
    get_keybinding(shortcut) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(shortcut, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.canvisteditor_get_keybinding(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Get text of a single line (0-based).
     * @param {number} line
     * @returns {string}
     */
    get_line(line) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_get_line(this.__wbg_ptr, line);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get text for a range of lines (0-based, inclusive start, exclusive end).
     * @param {number} start_line
     * @param {number} end_line
     * @returns {string}
     */
    get_line_range(start_line, end_line) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_get_line_range(this.__wbg_ptr, start_line, end_line);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
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
     * Get a theme colour slot as [r, g, b, a].
     * @param {string} slot
     * @returns {Uint8Array}
     */
    get_theme_color(slot) {
        const ptr0 = passStringToWasm0(slot, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_get_theme_color(this.__wbg_ptr, ptr0, len0);
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * Get the colour for a token kind as [r, g, b, a].
     *
     * Returns default colours if not customised.
     * @param {string} kind
     * @returns {Uint8Array}
     */
    get_token_color(kind) {
        const ptr0 = passStringToWasm0(kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_get_token_color(this.__wbg_ptr, ptr0, len0);
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * Move cursor to a named anchor.
     *
     * Returns true if the anchor exists.
     * @param {string} name
     * @returns {boolean}
     */
    go_to_anchor(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_go_to_anchor(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Navigate to a breadcrumb by index in the breadcrumbs array.
     *
     * Sets cursor to the beginning of that line and scrolls to it.
     * @param {number} line
     */
    go_to_breadcrumb(line) {
        wasm.canvisteditor_go_to_breadcrumb(this.__wbg_ptr, line);
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
     * Whether a snapshot has been taken.
     * @returns {boolean}
     */
    has_snapshot() {
        const ret = wasm.canvisteditor_has_snapshot(this.__wbg_ptr);
        return ret !== 0;
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
     * Whether occurrence highlighting is enabled.
     * @returns {boolean}
     */
    highlight_occurrences() {
        const ret = wasm.canvisteditor_highlight_occurrences(this.__wbg_ptr);
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
     * Get the indentation level (number of leading whitespace chars)
     * of the current line.
     * @returns {number}
     */
    indent_level_at_cursor() {
        const ret = wasm.canvisteditor_indent_level_at_cursor(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get the indent level of a specific line (0-based).
     * @param {number} line
     * @returns {number}
     */
    indent_level_of_line(line) {
        const ret = wasm.canvisteditor_indent_level_of_line(this.__wbg_ptr, line);
        return ret >>> 0;
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
     * Insert a markdown task line at the current line start.
     * @param {string} text
     * @param {boolean} checked
     */
    insert_task_line(text, checked) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_insert_task_line(this.__wbg_ptr, ptr0, len0, checked);
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
     * Insert text respecting the max_length constraint.
     *
     * Truncates the input so the total never exceeds the limit.
     * Returns the number of characters actually inserted.
     * @param {string} text
     * @returns {number}
     */
    insert_text_clamped(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_insert_text_clamped(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
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
     * Whether the editor is currently focused.
     * @returns {boolean}
     */
    is_focused() {
        const ret = wasm.canvisteditor_is_focused(this.__wbg_ptr);
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
     * Whether a specific line is inside a folded (hidden) region.
     *
     * Returns true for lines that are hidden — NOT the first line of
     * a fold which remains visible.
     * @param {number} line
     * @returns {boolean}
     */
    is_line_folded(line) {
        const ret = wasm.canvisteditor_is_line_folded(this.__wbg_ptr, line);
        return ret !== 0;
    }
    /**
     * Whether a specific logical line (0-based) is soft-wrapped into
     * multiple visual lines.
     * @param {number} line
     * @returns {boolean}
     */
    is_line_wrapped(line) {
        const ret = wasm.canvisteditor_is_line_wrapped(this.__wbg_ptr, line);
        return ret !== 0;
    }
    /**
     * Whether the document has been modified since last save.
     * @returns {boolean}
     */
    is_modified() {
        const ret = wasm.canvisteditor_is_modified(this.__wbg_ptr);
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
     * Number of custom keybinding overrides.
     * @returns {number}
     */
    keybinding_override_count() {
        const ret = wasm.canvisteditor_keybinding_override_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get all keybinding overrides as [shortcut, command, ...].
     * @returns {string[]}
     */
    keybinding_overrides_list() {
        const ret = wasm.canvisteditor_keybinding_overrides_list(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Last duplicate line number, or -1 when no duplicates.
     * @param {boolean} case_sensitive
     * @param {boolean} ignore_whitespace
     * @returns {number}
     */
    last_duplicate_line(case_sensitive, ignore_whitespace) {
        const ret = wasm.canvisteditor_last_duplicate_line(this.__wbg_ptr, case_sensitive, ignore_whitespace);
        return ret;
    }
    /**
     * Get the last recorded selection end offset (from `selection_changed`).
     * @returns {number}
     */
    last_selection_end() {
        const ret = wasm.canvisteditor_last_selection_end(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get the last visible line number (0-based).
     * @returns {number}
     */
    last_visible_line() {
        const ret = wasm.canvisteditor_last_visible_line(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Determine which line number a Y-coordinate in the gutter maps to.
     *
     * Returns the 0-based line number, or -1 if outside content.
     * @param {number} y
     * @returns {number}
     */
    line_at_y(y) {
        const ret = wasm.canvisteditor_line_at_y(this.__wbg_ptr, y);
        return ret;
    }
    /**
     * Return line context window around a target line.
     *
     * Flat format: [lineNumber, text, lineNumber, text, ...].
     * @param {number} line
     * @param {number} radius
     * @returns {string[]}
     */
    line_context(line, radius) {
        const ret = wasm.canvisteditor_line_context(this.__wbg_ptr, line, radius);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
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
     * Get the total number of lines in the document.
     * @returns {number}
     */
    line_count_total() {
        const ret = wasm.canvisteditor_line_count_total(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Number of active line decorations.
     * @returns {number}
     */
    line_decoration_count() {
        const ret = wasm.canvisteditor_line_decoration_count(this.__wbg_ptr);
        return ret >>> 0;
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
     * Whether a line starts with a prefix.
     * @param {number} line
     * @param {string} prefix
     * @param {boolean} case_sensitive
     * @returns {boolean}
     */
    line_has_prefix(line, prefix, case_sensitive) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_line_has_prefix(this.__wbg_ptr, line, ptr0, len0, case_sensitive);
        return ret !== 0;
    }
    /**
     * Whether a line ends with a suffix.
     * @param {number} line
     * @param {string} suffix
     * @param {boolean} case_sensitive
     * @returns {boolean}
     */
    line_has_suffix(line, suffix, case_sensitive) {
        const ptr0 = passStringToWasm0(suffix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_line_has_suffix(this.__wbg_ptr, line, ptr0, len0, case_sensitive);
        return ret !== 0;
    }
    /**
     * Compute FNV-1a 64-bit hash of a logical line.
     *
     * Returns empty string when line is out of range.
     * @param {number} line
     * @returns {string}
     */
    line_hash(line) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_line_hash(this.__wbg_ptr, line);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Compare two line hashes for equality.
     *
     * Returns `false` if either line is out of range.
     * @param {number} a
     * @param {number} b
     * @returns {boolean}
     */
    line_hash_equals(a, b) {
        const ret = wasm.canvisteditor_line_hash_equals(this.__wbg_ptr, a, b);
        return ret !== 0;
    }
    /**
     * Return all line hashes as flat array: [line, hash, ...].
     * @returns {string[]}
     */
    line_hashes() {
        const ret = wasm.canvisteditor_line_hashes(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Return line hashes for an inclusive range as `[line, hash, ...]`.
     * @param {number} start_line
     * @param {number} end_line
     * @returns {string[]}
     */
    line_hashes_in_range(start_line, end_line) {
        const ret = wasm.canvisteditor_line_hashes_in_range(this.__wbg_ptr, start_line, end_line);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Whether a line participates in a duplicate-content set.
     * @param {number} line
     * @param {boolean} case_sensitive
     * @param {boolean} ignore_whitespace
     * @returns {boolean}
     */
    line_is_duplicate(line, case_sensitive, ignore_whitespace) {
        const ret = wasm.canvisteditor_line_is_duplicate(this.__wbg_ptr, line, case_sensitive, ignore_whitespace);
        return ret !== 0;
    }
    /**
     * Count lines containing `needle`.
     * @param {string} needle
     * @param {boolean} case_sensitive
     * @returns {number}
     */
    line_occurrence_count(needle, case_sensitive) {
        const ptr0 = passStringToWasm0(needle, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_line_occurrence_count(this.__wbg_ptr, ptr0, len0, case_sensitive);
        return ret >>> 0;
    }
    /**
     * Return line numbers containing `needle`.
     * @param {string} needle
     * @param {boolean} case_sensitive
     * @returns {Uint32Array}
     */
    line_occurrences(needle, case_sensitive) {
        const ptr0 = passStringToWasm0(needle, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_line_occurrences(this.__wbg_ptr, ptr0, len0, case_sensitive);
        var v2 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
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
     * Line numbers whose text starts with prefix.
     * @param {string} prefix
     * @param {boolean} case_sensitive
     * @returns {Uint32Array}
     */
    lines_with_prefix(prefix, case_sensitive) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_lines_with_prefix(this.__wbg_ptr, ptr0, len0, case_sensitive);
        var v2 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * Line numbers whose text ends with suffix.
     * @param {string} suffix
     * @param {boolean} case_sensitive
     * @returns {Uint32Array}
     */
    lines_with_suffix(suffix, case_sensitive) {
        const ptr0 = passStringToWasm0(suffix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_lines_with_suffix(this.__wbg_ptr, ptr0, len0, case_sensitive);
        var v2 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * Get the URL text at a character offset, if any.
     *
     * Returns empty string if offset is not inside a URL.
     * @param {number} offset
     * @returns {string}
     */
    link_at_offset(offset) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_link_at_offset(this.__wbg_ptr, offset);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Return line numbers longer than `max_len` characters.
     * @param {number} max_len
     * @returns {Uint32Array}
     */
    lint_long_lines(max_len) {
        const ret = wasm.canvisteditor_lint_long_lines(this.__wbg_ptr, max_len);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Return line numbers with mixed leading tabs and spaces.
     * @returns {Uint32Array}
     */
    lint_mixed_indentation() {
        const ret = wasm.canvisteditor_lint_mixed_indentation(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Return line numbers containing non-ASCII characters.
     * @returns {Uint32Array}
     */
    lint_non_ascii_lines() {
        const ret = wasm.canvisteditor_lint_non_ascii_lines(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Return line numbers that end with trailing spaces or tabs.
     * @returns {Uint32Array}
     */
    lint_trailing_whitespace() {
        const ret = wasm.canvisteditor_lint_trailing_whitespace(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Load a previously saved named editor state.
     *
     * Returns `true` when found and restored.
     * @param {string} name
     * @returns {boolean}
     */
    load_named_state(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_load_named_state(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Restore a named selection profile.
     *
     * Returns `true` when found.
     * @param {string} name
     * @returns {boolean}
     */
    load_selection_profile(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_load_selection_profile(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Log an editor event. Newest entries are at index 0.
     *
     * The log is capped at `event_log_max` (default 50).
     * Call from JS to record significant actions.
     * @param {string} event
     */
    log_event(event) {
        const ptr0 = passStringToWasm0(event, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_log_event(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Longest line length in characters.
     * @returns {number}
     */
    longest_line_length() {
        const ret = wasm.canvisteditor_longest_line_length(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Line number of the longest line (0-based).
     * @returns {number}
     */
    longest_line_number() {
        const ret = wasm.canvisteditor_longest_line_number(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * The longest word in the document.
     * @returns {string}
     */
    longest_word() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_longest_word(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Delete a saved macro.
     * @param {string} name
     */
    macro_delete_saved(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_macro_delete_saved(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Whether macro recording is active.
     * @returns {boolean}
     */
    macro_is_recording() {
        const ret = wasm.canvisteditor_macro_is_recording(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * List saved macro names.
     * @returns {string[]}
     */
    macro_list_saved() {
        const ret = wasm.canvisteditor_macro_list_saved(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Record a macro step manually.
     *
     * `kind`: "insert", "delete", "select"
     * `data`: for insert = text; for delete = "start,end";
     *         for select = "start,end"
     * @param {string} kind
     * @param {string} data
     */
    macro_record_step(kind, data) {
        const ptr0 = passStringToWasm0(kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(data, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.canvisteditor_macro_record_step(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * Replay the recorded macro once.
     */
    macro_replay() {
        wasm.canvisteditor_macro_replay(this.__wbg_ptr);
    }
    /**
     * Replay a saved macro by name. Returns false if not found.
     * @param {string} name
     * @returns {boolean}
     */
    macro_replay_saved(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_macro_replay_saved(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Save the current recorded macro under a name.
     * @param {string} name
     */
    macro_save(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_macro_save(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Start recording a macro.
     */
    macro_start_recording() {
        wasm.canvisteditor_macro_start_recording(this.__wbg_ptr);
    }
    /**
     * Number of steps in the current macro recording.
     * @returns {number}
     */
    macro_step_count() {
        const ret = wasm.canvisteditor_macro_step_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Stop recording and return the number of steps recorded.
     * @returns {number}
     */
    macro_stop_recording() {
        const ret = wasm.canvisteditor_macro_stop_recording(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Mark the document as modified.
     *
     * Called automatically by mutating operations. You can also call
     * it manually to force the dirty state.
     */
    mark_modified() {
        wasm.canvisteditor_mark_modified(this.__wbg_ptr);
    }
    /**
     * Mark the document as saved (clears the modified flag).
     */
    mark_saved() {
        wasm.canvisteditor_mark_saved(this.__wbg_ptr);
    }
    /**
     * Number of active markers.
     * @returns {number}
     */
    marker_count() {
        const ret = wasm.canvisteditor_marker_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get all markers as [start, end, r, g, b, a, id, ...].
     * @returns {string[]}
     */
    marker_list() {
        const ret = wasm.canvisteditor_marker_list(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Get markers overlapping a character offset.
     * @param {number} offset
     * @returns {string[]}
     */
    markers_at(offset) {
        const ret = wasm.canvisteditor_markers_at(this.__wbg_ptr, offset);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Get the current max character count (0 = unlimited).
     * @returns {number}
     */
    max_length() {
        const ret = wasm.canvisteditor_max_length(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Measure the pixel width of a single character using the default
     * style.
     * @param {string} ch
     * @returns {number}
     */
    measure_char_width(ch) {
        const ptr0 = passStringToWasm0(ch, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_measure_char_width(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Measure the pixel width of a string using the default style.
     *
     * Useful for external layout calculations. Returns 0.0 if the
     * canvas context is not available.
     * @param {string} text
     * @returns {number}
     */
    measure_text_width(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_measure_text_width(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get the minimap width.
     * @returns {number}
     */
    minimap_width() {
        const ret = wasm.canvisteditor_minimap_width(this.__wbg_ptr);
        return ret;
    }
    /**
     * Move an anchor to the current cursor position.
     *
     * Returns false when the anchor does not exist.
     * @param {string} name
     * @returns {boolean}
     */
    move_anchor_to_cursor(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_move_anchor_to_cursor(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
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
     * Insert text at all cursor positions (primary + extras).
     *
     * Returns the number of insertions performed. Offsets are adjusted
     * as text is inserted (processed from end to start).
     * @param {string} text
     * @returns {number}
     */
    multi_cursor_insert(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_multi_cursor_insert(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Number of named states currently saved.
     * @returns {number}
     */
    named_state_count() {
        const ret = wasm.canvisteditor_named_state_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Saved state names (sorted).
     * @returns {string[]}
     */
    named_state_names() {
        const ret = wasm.canvisteditor_named_state_names(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Find nearest anchor at or after the given offset.
     *
     * Returns `[name, offset]` or empty when none.
     * @param {number} offset
     * @returns {string[]}
     */
    nearest_anchor_after(offset) {
        const ret = wasm.canvisteditor_nearest_anchor_after(this.__wbg_ptr, offset);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Find nearest anchor at or before the given offset.
     *
     * Returns `[name, offset]` or empty when none.
     * @param {number} offset
     * @returns {string[]}
     */
    nearest_anchor_before(offset) {
        const ret = wasm.canvisteditor_nearest_anchor_before(this.__wbg_ptr, offset);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
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
     * Next line containing `needle` after `from_line` (wraps), or -1.
     * @param {string} needle
     * @param {number} from_line
     * @param {boolean} case_sensitive
     * @returns {number}
     */
    next_line_with(needle, from_line, case_sensitive) {
        const ptr0 = passStringToWasm0(needle, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_next_line_with(this.__wbg_ptr, ptr0, len0, from_line, case_sensitive);
        return ret;
    }
    /**
     * Return the next task line after `from_line` (wraps), or -1.
     * @param {number} from_line
     * @returns {number}
     */
    next_task_line(from_line) {
        const ret = wasm.canvisteditor_next_task_line(this.__wbg_ptr, from_line);
        return ret;
    }
    /**
     * Next unchecked task line after `from_line` (wraps), or -1.
     * @param {number} from_line
     * @returns {number}
     */
    next_unchecked_task_line(from_line) {
        const ret = wasm.canvisteditor_next_unchecked_task_line(this.__wbg_ptr, from_line);
        return ret;
    }
    /**
     * Normalize all indentation to the current tab style.
     *
     * If soft_tabs is true, converts tabs to spaces (tab_size).
     * If soft_tabs is false, converts leading spaces to tabs.
     * Returns number of lines modified.
     * @returns {number}
     */
    normalize_indentation() {
        const ret = wasm.canvisteditor_normalize_indentation(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Normalize line endings to LF (remove \r).
     *
     * Returns the number of \r characters removed.
     * @returns {number}
     */
    normalize_line_endings() {
        const ret = wasm.canvisteditor_normalize_line_endings(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Normalize common Unicode whitespace characters to ASCII space.
     *
     * Returns number of replaced characters.
     * @returns {number}
     */
    normalize_unicode_whitespace() {
        const ret = wasm.canvisteditor_normalize_unicode_whitespace(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Number a line range with `N. ` prefix.
     *
     * Returns number of lines changed.
     * @param {number} start_line
     * @param {number} end_line
     * @param {number} start_number
     * @param {number} pad_width
     * @returns {number}
     */
    number_lines(start_line, end_line, start_number, pad_width) {
        const ret = wasm.canvisteditor_number_lines(this.__wbg_ptr, start_line, end_line, start_number, pad_width);
        return ret >>> 0;
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
     * Number of paragraph blocks (text groups separated by blank lines).
     * @returns {number}
     */
    paragraph_block_count() {
        const ret = wasm.canvisteditor_paragraph_block_count(this.__wbg_ptr);
        return ret >>> 0;
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
     * Paste text with auto-adjusted indentation.
     *
     * Detects the indentation level at the cursor and adjusts the
     * pasted text to match.
     * @param {string} text
     */
    paste_with_indent(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_paste_with_indent(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Get the current placeholder text.
     * @returns {string}
     */
    placeholder() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_placeholder(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
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
     * Prefix each line in range with `prefix`.
     *
     * Returns number of lines changed.
     * @param {number} start_line
     * @param {number} end_line
     * @param {string} prefix
     * @returns {number}
     */
    prefix_lines(start_line, end_line, prefix) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_prefix_lines(this.__wbg_ptr, start_line, end_line, ptr0, len0);
        return ret >>> 0;
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
     * Previous line containing `needle` before `from_line` (wraps), or -1.
     * @param {string} needle
     * @param {number} from_line
     * @param {boolean} case_sensitive
     * @returns {number}
     */
    prev_line_with(needle, from_line, case_sensitive) {
        const ptr0 = passStringToWasm0(needle, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_prev_line_with(this.__wbg_ptr, ptr0, len0, from_line, case_sensitive);
        return ret;
    }
    /**
     * Return the previous task line before `from_line` (wraps), or -1.
     * @param {number} from_line
     * @returns {number}
     */
    prev_task_line(from_line) {
        const ret = wasm.canvisteditor_prev_task_line(this.__wbg_ptr, from_line);
        return ret;
    }
    /**
     * Previous unchecked task line before `from_line` (wraps), or -1.
     * @param {number} from_line
     * @returns {number}
     */
    prev_unchecked_task_line(from_line) {
        const ret = wasm.canvisteditor_prev_unchecked_task_line(this.__wbg_ptr, from_line);
        return ret;
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
     * Push the current selection onto the selection history stack.
     */
    push_selection_history() {
        wasm.canvisteditor_push_selection_history(this.__wbg_ptr);
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
     * Estimated reading time in seconds (assumes 250 words/minute).
     * @returns {number}
     */
    reading_time_seconds() {
        const ret = wasm.canvisteditor_reading_time_seconds(this.__wbg_ptr);
        return ret;
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
     * How many more characters can be inserted before hitting the limit.
     *
     * Returns `usize::MAX` when max_length is 0 (unlimited).
     * @returns {number}
     */
    remaining_capacity() {
        const ret = wasm.canvisteditor_remaining_capacity(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Remove a named anchor.
     * @param {string} name
     */
    remove_anchor(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_remove_anchor(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Remove anchors whose offsets are inside an inclusive range.
     *
     * Returns number removed.
     * @param {number} start_offset
     * @param {number} end_offset
     * @returns {number}
     */
    remove_anchors_in_range(start_offset, end_offset) {
        const ret = wasm.canvisteditor_remove_anchors_in_range(this.__wbg_ptr, start_offset, end_offset);
        return ret >>> 0;
    }
    /**
     * Remove anchors whose names start with prefix.
     *
     * Returns number removed.
     * @param {string} prefix
     * @returns {number}
     */
    remove_anchors_with_prefix(prefix) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_remove_anchors_with_prefix(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Remove all annotations matching a kind (e.g. "error").
     * @param {string} kind
     */
    remove_annotations_by_kind(kind) {
        const ptr0 = passStringToWasm0(kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_remove_annotations_by_kind(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Remove a collaborative cursor by name.
     * @param {string} name
     */
    remove_collab_cursor(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_remove_collab_cursor(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Remove an extra cursor at a specific offset.
     * @param {number} offset
     */
    remove_cursor(offset) {
        wasm.canvisteditor_remove_cursor(this.__wbg_ptr, offset);
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
     * Remove a custom keybinding override.
     * @param {string} shortcut
     */
    remove_keybinding(shortcut) {
        const ptr0 = passStringToWasm0(shortcut, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_remove_keybinding(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Remove all decorations from a specific line.
     * @param {number} line
     */
    remove_line_decorations(line) {
        wasm.canvisteditor_remove_line_decorations(this.__wbg_ptr, line);
    }
    /**
     * Remove a marker by ID.
     * @param {string} id
     */
    remove_marker(id) {
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_remove_marker(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Remove all markers with IDs starting with a prefix.
     * @param {string} prefix
     */
    remove_markers_by_prefix(prefix) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_remove_markers_by_prefix(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Remove the ruler at the given column.
     * @param {number} column
     */
    remove_ruler(column) {
        wasm.canvisteditor_remove_ruler(this.__wbg_ptr, column);
    }
    /**
     * Remove blank lines at end of document.
     *
     * Returns number of trailing blank lines removed.
     * @returns {number}
     */
    remove_trailing_blank_lines() {
        const ret = wasm.canvisteditor_remove_trailing_blank_lines(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Rename all occurrences of the word under cursor to `new_name`.
     *
     * Uses whole-word matching. Returns the number of replacements.
     * @param {string} new_name
     * @returns {number}
     */
    rename_all(new_name) {
        const ptr0 = passStringToWasm0(new_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_rename_all(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Rename an anchor key.
     *
     * Returns `true` when source anchor existed.
     * @param {string} old_name
     * @param {string} new_name
     * @returns {boolean}
     */
    rename_anchor(old_name, new_name) {
        const ptr0 = passStringToWasm0(old_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(new_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_rename_anchor(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * Rename anchors with a shared prefix.
     *
     * Returns number renamed. Existing destination names are overwritten.
     * @param {string} old_prefix
     * @param {string} new_prefix
     * @returns {number}
     */
    rename_anchor_prefix(old_prefix, new_prefix) {
        const ptr0 = passStringToWasm0(old_prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(new_prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_rename_anchor_prefix(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret >>> 0;
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
     * Reset all token colours to defaults.
     */
    reset_token_colors() {
        wasm.canvisteditor_reset_token_colors(this.__wbg_ptr);
    }
    /**
     * Restore editor state from a JSON string produced by `save_state`.
     * @param {string} json
     */
    restore_state(json) {
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_restore_state(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Reverse the order of selected lines.
     */
    reverse_lines() {
        wasm.canvisteditor_reverse_lines(this.__wbg_ptr);
    }
    /**
     * Rotate a line range down by one (last line moves to start).
     * @param {number} start_line
     * @param {number} end_line
     * @returns {boolean}
     */
    rotate_lines_down(start_line, end_line) {
        const ret = wasm.canvisteditor_rotate_lines_down(this.__wbg_ptr, start_line, end_line);
        return ret !== 0;
    }
    /**
     * Rotate a line range up by one (first line moves to end).
     * @param {number} start_line
     * @param {number} end_line
     * @returns {boolean}
     */
    rotate_lines_up(start_line, end_line) {
        const ret = wasm.canvisteditor_rotate_lines_up(this.__wbg_ptr, start_line, end_line);
        return ret !== 0;
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
     * Execute a command by name.
     *
     * Returns `true` if the command is recognized and executed.
     * @param {string} command
     * @returns {boolean}
     */
    run_command(command) {
        const ptr0 = passStringToWasm0(command, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_run_command(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Execute the command bound to a shortcut.
     *
     * Custom overrides are checked first, then defaults.
     * @param {string} shortcut
     * @returns {boolean}
     */
    run_shortcut(shortcut) {
        const ptr0 = passStringToWasm0(shortcut, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_run_shortcut(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Save the full editor state under a name.
     * @param {string} name
     */
    save_named_state(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_save_named_state(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Save the current selection range under a name.
     * @param {string} name
     */
    save_selection_profile(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_save_selection_profile(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Serialize the editor state to a JSON string.
     *
     * Includes text, selection, scroll position, theme, and settings.
     * Use `restore_state` to reload.
     * @returns {string}
     */
    save_state() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_save_state(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Scan the document for task-style lines.
     *
     * Returns flat array [line, kind, checked, text, ...].
     * Kinds: `task`, `todo`, `fixme`, `note`, `hack`.
     * @returns {string[]}
     */
    scan_tasks() {
        const ret = wasm.canvisteditor_scan_tasks(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
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
     * The scroll position as a fraction (0.0 = top, 1.0 = bottom).
     * @returns {number}
     */
    scroll_fraction() {
        const ret = wasm.canvisteditor_scroll_fraction(this.__wbg_ptr);
        return ret;
    }
    /**
     * The ratio of viewport to content (0.0–1.0). 1.0 = all visible.
     * @returns {number}
     */
    scroll_ratio() {
        const ret = wasm.canvisteditor_scroll_ratio(this.__wbg_ptr);
        return ret;
    }
    /**
     * Scroll to a fraction of the document (0.0 = top, 1.0 = bottom).
     * @param {number} fraction
     */
    scroll_to_fraction(fraction) {
        wasm.canvisteditor_scroll_to_fraction(this.__wbg_ptr, fraction);
    }
    /**
     * Scroll the viewport to make a specific line visible.
     *
     * The line will be positioned near the top of the viewport with
     * a 2-line padding.
     * @param {number} line
     */
    scroll_to_line(line) {
        wasm.canvisteditor_scroll_to_line(this.__wbg_ptr, line);
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
     * Search commands by query string.
     *
     * Returns matching commands as [name, keybinding, ...].
     * Case-insensitive substring match on command name.
     * @param {string} query
     * @returns {string[]}
     */
    search_commands(query) {
        const ptr0 = passStringToWasm0(query, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_search_commands(this.__wbg_ptr, ptr0, len0);
        var v2 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * Clear search history.
     */
    search_history_clear() {
        wasm.canvisteditor_search_history_clear(this.__wbg_ptr);
    }
    /**
     * Get search history entry at index (0 = most recent).
     * @param {number} index
     * @returns {string}
     */
    search_history_get(index) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_search_history_get(this.__wbg_ptr, index);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Number of search history entries.
     * @returns {number}
     */
    search_history_length() {
        const ret = wasm.canvisteditor_search_history_length(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Push a search term into the search history.
     * @param {string} term
     */
    search_history_push(term) {
        const ptr0 = passStringToWasm0(term, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_search_history_push(this.__wbg_ptr, ptr0, len0);
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
     * Select an entire range of lines (0-based, inclusive).
     * @param {number} start_line
     * @param {number} end_line
     */
    select_lines(start_line, end_line) {
        wasm.canvisteditor_select_lines(this.__wbg_ptr, start_line, end_line);
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
     * Get the selection anchor (start) offset.
     *
     * When selecting left-to-right, anchor < focus (end).
     * When selecting right-to-left, anchor > focus.
     * When collapsed, anchor == focus.
     * @returns {number}
     */
    selection_anchor() {
        const ret = wasm.canvisteditor_selection_anchor(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Check if the selection has changed since the last call to this
     * method.
     *
     * Returns `true` the first time the selection moves to a new
     * position. Useful for triggering UI updates only when needed.
     * @returns {boolean}
     */
    selection_changed() {
        const ret = wasm.canvisteditor_selection_changed(this.__wbg_ptr);
        return ret !== 0;
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
     * Go back in selection history.
     * @returns {boolean}
     */
    selection_history_back() {
        const ret = wasm.canvisteditor_selection_history_back(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Go forward in selection history.
     * @returns {boolean}
     */
    selection_history_forward() {
        const ret = wasm.canvisteditor_selection_history_forward(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Selection history length.
     * @returns {number}
     */
    selection_history_length() {
        const ret = wasm.canvisteditor_selection_history_length(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Whether the selection is collapsed (no text selected).
     * @returns {boolean}
     */
    selection_is_collapsed() {
        const ret = wasm.canvisteditor_selection_is_collapsed(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Length of the current selection in characters.
     * @returns {number}
     */
    selection_length() {
        const ret = wasm.canvisteditor_selection_length(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get the line numbers covered by the current selection.
     *
     * Returns [startLine, endLine] (0-based, inclusive).
     * @returns {Uint32Array}
     */
    selection_line_range() {
        const ret = wasm.canvisteditor_selection_line_range(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Number of saved selection profiles.
     * @returns {number}
     */
    selection_profile_count() {
        const ret = wasm.canvisteditor_selection_profile_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Selection profile names (sorted).
     * @returns {string[]}
     */
    selection_profile_names() {
        const ret = wasm.canvisteditor_selection_profile_names(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
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
     * Sentence count (split on . ! ?).
     * @returns {number}
     */
    sentence_count() {
        const ret = wasm.canvisteditor_sentence_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Set a named anchor to a character offset.
     *
     * If the anchor already exists, it is updated.
     * @param {string} name
     * @param {number} offset
     */
    set_anchor(name, offset) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_set_anchor(this.__wbg_ptr, ptr0, len0, offset);
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
     * Replace text in a rectangular block.
     *
     * Each line of `text` replaces the corresponding column range.
     * @param {number} start_line
     * @param {number} end_line
     * @param {number} start_col
     * @param {number} end_col
     * @param {string} text
     */
    set_block_selection(start_line, end_line, start_col, end_col, text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_set_block_selection(this.__wbg_ptr, start_line, end_line, start_col, end_col, ptr0, len0);
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
     * Set cursor colour override. Pass 0,0,0,0 to reset to theme default.
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_cursor_color(r, g, b, a) {
        wasm.canvisteditor_set_cursor_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * Set cursor style: 0=line (default), 1=block, 2=underline.
     * @param {number} style
     */
    set_cursor_style(style) {
        wasm.canvisteditor_set_cursor_style(this.__wbg_ptr, style);
    }
    /**
     * Set cursor width in pixels (line style only, default 2.0).
     * @param {number} w
     */
    set_cursor_width(w) {
        wasm.canvisteditor_set_cursor_width(this.__wbg_ptr, w);
    }
    /**
     * Enable or disable URL link detection.
     * @param {boolean} enabled
     */
    set_detect_links(enabled) {
        wasm.canvisteditor_set_detect_links(this.__wbg_ptr, enabled);
    }
    /**
     * Set the maximum number of event log entries.
     * @param {number} max
     */
    set_event_log_max(max) {
        wasm.canvisteditor_set_event_log_max(this.__wbg_ptr, max);
    }
    /**
     * Set the needle for visual find highlights.
     *
     * All occurrences are highlighted with a translucent overlay.
     * Pass empty string to clear highlights.
     * @param {string} needle
     */
    set_find_highlights(needle) {
        const ptr0 = passStringToWasm0(needle, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_set_find_highlights(this.__wbg_ptr, ptr0, len0);
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
     * Enable or disable highlighting all occurrences of the word under
     * the cursor.
     * @param {boolean} enabled
     */
    set_highlight_occurrences(enabled) {
        wasm.canvisteditor_set_highlight_occurrences(this.__wbg_ptr, enabled);
    }
    /**
     * Rebind a keyboard shortcut to a command.
     *
     * `shortcut` is e.g. "Ctrl+B", `command` is the command name from
     * `command_list()` e.g. "Bold".
     * @param {string} shortcut
     * @param {string} command
     */
    set_keybinding(shortcut, command) {
        const ptr0 = passStringToWasm0(shortcut, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(command, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.canvisteditor_set_keybinding(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * Replace text for a range of lines (0-based, inclusive start, exclusive end).
     * @param {number} start_line
     * @param {number} end_line
     * @param {string} text
     */
    set_line_range(start_line, end_line, text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_set_line_range(this.__wbg_ptr, start_line, end_line, ptr0, len0);
    }
    /**
     * Set maximum character count (0 = unlimited).
     *
     * When set, `insert_text` and similar operations will be truncated
     * to stay within the limit.
     * @param {number} max
     */
    set_max_length(max) {
        wasm.canvisteditor_set_max_length(this.__wbg_ptr, max);
    }
    /**
     * Set the minimap width in pixels (default 60).
     * @param {number} w
     */
    set_minimap_width(w) {
        wasm.canvisteditor_set_minimap_width(this.__wbg_ptr, w);
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
     * Set placeholder text shown when the document is empty.
     * @param {string} text
     */
    set_placeholder(text) {
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_set_placeholder(this.__wbg_ptr, ptr0, len0);
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
     * Toggle the minimap sidebar.
     * @param {boolean} enabled
     */
    set_show_minimap(enabled) {
        wasm.canvisteditor_set_show_minimap(this.__wbg_ptr, enabled);
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
     * Enable/disable wrap continuation indicators in the gutter.
     *
     * When enabled, wrapped continuation lines show a `↪` glyph in
     * the gutter to distinguish them from real line breaks.
     * @param {boolean} enabled
     */
    set_show_wrap_indicators(enabled) {
        wasm.canvisteditor_set_show_wrap_indicators(this.__wbg_ptr, enabled);
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
     * Toggle sticky scroll — shows the first line of the document at
     * the top when scrolled past it.
     * @param {boolean} enabled
     */
    set_sticky_scroll(enabled) {
        wasm.canvisteditor_set_sticky_scroll(this.__wbg_ptr, enabled);
    }
    /**
     * Enable or disable syntax highlighting.
     *
     * When enabled, the tokenizer colours are applied during rendering.
     * @param {boolean} enabled
     */
    set_syntax_highlight(enabled) {
        wasm.canvisteditor_set_syntax_highlight(this.__wbg_ptr, enabled);
    }
    /**
     * Set the tab display/insert size (1–8). Default: 4.
     * @param {number} size
     */
    set_tab_size(size) {
        wasm.canvisteditor_set_tab_size(this.__wbg_ptr, size);
    }
    /**
     * Set a single theme colour slot.
     *
     * Slot names: "background", "text", "caret", "caret_blur",
     * "selection", "selection_blur", "line_highlight",
     * "gutter_bg", "gutter_text", "gutter_border",
     * "scrollbar_track", "scrollbar_thumb".
     * @param {string} slot
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_theme_color(slot, r, g, b, a) {
        const ptr0 = passStringToWasm0(slot, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_set_theme_color(this.__wbg_ptr, ptr0, len0, r, g, b, a);
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
     * Set a colour for a token kind.
     *
     * Kinds: "word", "number", "whitespace", "punctuation", "newline".
     * Use this to customise syntax colours.
     * @param {string} kind
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_token_color(kind, r, g, b, a) {
        const ptr0 = passStringToWasm0(kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_set_token_color(this.__wbg_ptr, ptr0, len0, r, g, b, a);
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
     * Shift a named anchor by a signed delta.
     *
     * The resulting offset is clamped to the current document bounds.
     * Returns `false` when the anchor does not exist.
     * @param {string} name
     * @param {number} delta
     * @returns {boolean}
     */
    shift_anchor(name, delta) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_shift_anchor(this.__wbg_ptr, ptr0, len0, delta);
        return ret !== 0;
    }
    /**
     * Whether find highlights are active.
     * @returns {boolean}
     */
    show_find_highlights() {
        const ret = wasm.canvisteditor_show_find_highlights(this.__wbg_ptr);
        return ret !== 0;
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
     * Whether the minimap is shown.
     * @returns {boolean}
     */
    show_minimap() {
        const ret = wasm.canvisteditor_show_minimap(this.__wbg_ptr);
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
     * Whether wrap indicators are shown.
     * @returns {boolean}
     */
    show_wrap_indicators() {
        const ret = wasm.canvisteditor_show_wrap_indicators(this.__wbg_ptr);
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
     * Whether sticky scroll is enabled.
     * @returns {boolean}
     */
    sticky_scroll() {
        const ret = wasm.canvisteditor_sticky_scroll(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Remove non-printable control characters.
     *
     * Keeps `\n`, `\r`, and `\t`. Returns chars removed.
     * @returns {number}
     */
    strip_non_printable() {
        const ret = wasm.canvisteditor_strip_non_printable(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Suffix each line in range with `suffix`.
     *
     * Returns number of lines changed.
     * @param {number} start_line
     * @param {number} end_line
     * @param {string} suffix
     * @returns {number}
     */
    suffix_lines(start_line, end_line, suffix) {
        const ptr0 = passStringToWasm0(suffix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_suffix_lines(this.__wbg_ptr, start_line, end_line, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Swap two logical lines by index.
     *
     * Returns `true` on success.
     * @param {number} a
     * @param {number} b
     * @returns {boolean}
     */
    swap_lines(a, b) {
        const ret = wasm.canvisteditor_swap_lines(this.__wbg_ptr, a, b);
        return ret !== 0;
    }
    /**
     * Whether syntax highlighting is enabled.
     * @returns {boolean}
     */
    syntax_highlight() {
        const ret = wasm.canvisteditor_syntax_highlight(this.__wbg_ptr);
        return ret !== 0;
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
     * Take a snapshot of the current text for later diff.
     */
    take_snapshot() {
        wasm.canvisteditor_take_snapshot(this.__wbg_ptr);
    }
    /**
     * Count task-style lines in the document.
     * @returns {number}
     */
    task_count() {
        const ret = wasm.canvisteditor_task_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Task progress as [checked, total].
     * @returns {Uint32Array}
     */
    task_progress() {
        const ret = wasm.canvisteditor_task_progress(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Return up to `max_chars` immediately after the cursor.
     * @param {number} max_chars
     * @returns {string}
     */
    text_after_cursor(max_chars) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_text_after_cursor(this.__wbg_ptr, max_chars);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Return up to `max_chars` immediately before the cursor.
     * @param {number} max_chars
     * @returns {string}
     */
    text_before_cursor(max_chars) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_text_before_cursor(this.__wbg_ptr, max_chars);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Fast content fingerprint (FNV-1a 64-bit hash as hex string).
     *
     * Useful for external change detection: compare hashes to check
     * if content has changed without comparing full text.
     * @returns {string}
     */
    text_hash() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_text_hash(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
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
     * Toggle fold at a line. If the line starts a fold, unfold it.
     * Otherwise, try to fold from this line using indentation.
     * @param {number} line
     */
    toggle_fold_at(line) {
        wasm.canvisteditor_toggle_fold_at(this.__wbg_ptr, line);
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
     * Toggle markdown checkbox state on a line.
     *
     * Supports `- [ ]` <-> `- [x]` and `* [ ]` <-> `* [x]`.
     * Returns true if a toggle occurred.
     * @param {number} line
     * @returns {boolean}
     */
    toggle_task_checkbox(line) {
        const ret = wasm.canvisteditor_toggle_task_checkbox(this.__wbg_ptr, line);
        return ret !== 0;
    }
    /**
     * Toggle underline on the current selection. Preserves the current
     * selection.
     */
    toggle_underline() {
        wasm.canvisteditor_toggle_underline(this.__wbg_ptr);
    }
    /**
     * Simple tokenization of the document text.
     *
     * Returns alternating [kind, text, kind, text, ...] where kind is
     * one of: "word", "number", "whitespace", "punctuation", "newline".
     * @returns {string[]}
     */
    tokenize() {
        const ret = wasm.canvisteditor_tokenize(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Transform selected text to camelCase.
     */
    transform_camel_case() {
        wasm.canvisteditor_transform_camel_case(this.__wbg_ptr);
    }
    /**
     * Transform selected text to CONSTANT_CASE (upper snake).
     */
    transform_constant_case() {
        wasm.canvisteditor_transform_constant_case(this.__wbg_ptr);
    }
    /**
     * Transform selected text to kebab-case.
     */
    transform_kebab_case() {
        wasm.canvisteditor_transform_kebab_case(this.__wbg_ptr);
    }
    /**
     * Convert selected text to lowercase.
     */
    transform_lowercase() {
        wasm.canvisteditor_transform_lowercase(this.__wbg_ptr);
    }
    /**
     * Apply a transformation pipeline to the current selection.
     *
     * Supported step names (case-insensitive, `|` separated):
     * `upper`, `lower`, `title`, `camel`, `snake`, `kebab`, `constant`,
     * `reverse`.
     * @param {string} pipeline
     */
    transform_pipeline(pipeline) {
        const ptr0 = passStringToWasm0(pipeline, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_transform_pipeline(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Transform selected text to snake_case.
     */
    transform_snake_case() {
        wasm.canvisteditor_transform_snake_case(this.__wbg_ptr);
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
     * Trim leading spaces/tabs from every line.
     *
     * Returns number of lines changed.
     * @returns {number}
     */
    trim_leading_whitespace() {
        const ret = wasm.canvisteditor_trim_leading_whitespace(this.__wbg_ptr);
        return ret >>> 0;
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
     * Unfold all ranges.
     */
    unfold_all() {
        wasm.canvisteditor_unfold_all(this.__wbg_ptr);
    }
    /**
     * Unfold a specific range.
     * @param {number} start_line
     * @param {number} end_line
     */
    unfold_lines(start_line, end_line) {
        wasm.canvisteditor_unfold_lines(this.__wbg_ptr, start_line, end_line);
    }
    /**
     * Number of lines that are unique by content matching.
     * @param {boolean} case_sensitive
     * @param {boolean} ignore_whitespace
     * @returns {number}
     */
    unique_line_count(case_sensitive, ignore_whitespace) {
        const ret = wasm.canvisteditor_unique_line_count(this.__wbg_ptr, case_sensitive, ignore_whitespace);
        return ret >>> 0;
    }
    /**
     * Line numbers that are unique by content matching.
     *
     * Returns sorted 0-based line numbers.
     * @param {boolean} case_sensitive
     * @param {boolean} ignore_whitespace
     * @returns {Uint32Array}
     */
    unique_line_numbers(case_sensitive, ignore_whitespace) {
        const ret = wasm.canvisteditor_unique_line_numbers(this.__wbg_ptr, case_sensitive, ignore_whitespace);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Count of unique words (case-insensitive).
     * @returns {number}
     */
    unique_word_count() {
        const ret = wasm.canvisteditor_unique_word_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Remove `prefix` from each line in range when present.
     *
     * Returns number of lines changed.
     * @param {number} start_line
     * @param {number} end_line
     * @param {string} prefix
     * @returns {number}
     */
    unprefix_lines(start_line, end_line, prefix) {
        const ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_unprefix_lines(this.__wbg_ptr, start_line, end_line, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Remove `suffix` from each line in range when present.
     *
     * Returns number of lines changed.
     * @param {number} start_line
     * @param {number} end_line
     * @param {string} suffix
     * @returns {number}
     */
    unsuffix_lines(start_line, end_line, suffix) {
        const ptr0 = passStringToWasm0(suffix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.canvisteditor_unsuffix_lines(this.__wbg_ptr, start_line, end_line, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Update a collaborative cursor's position.
     * @param {string} name
     * @param {number} offset
     */
    update_collab_cursor(name, offset) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.canvisteditor_update_collab_cursor(this.__wbg_ptr, ptr0, len0, offset);
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
     * The viewport height in pixels (same as canvas height / zoom).
     * @returns {number}
     */
    viewport_height() {
        const ret = wasm.canvisteditor_viewport_height(this.__wbg_ptr);
        return ret;
    }
    /**
     * Number of lines visible in the viewport.
     * @returns {number}
     */
    visible_line_count() {
        const ret = wasm.canvisteditor_visible_line_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Number of visual (display) lines after word wrapping.
     * @returns {number}
     */
    visual_line_count() {
        const ret = wasm.canvisteditor_visual_line_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get the word under (or adjacent to) the cursor.
     *
     * Returns empty string if the cursor is not on a word.
     * @returns {string}
     */
    word_at_cursor() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.canvisteditor_word_at_cursor(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
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
     * Return the top N most frequent words as alternating
     * `[word, count, word, count, ...]` strings.
     * @param {number} top_n
     * @returns {string[]}
     */
    word_frequency(top_n) {
        const ret = wasm.canvisteditor_word_frequency(this.__wbg_ptr, top_n);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
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
        __wbg___wbindgen_string_get_395e606bd0ee4427: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
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
        __wbg_toDataURL_bf99d85b39ce57cc: function() { return handleError(function (arg0, arg1) {
            const ret = arg1.toDataURL();
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
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

function getArrayJsValueFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    const mem = getDataViewMemory0();
    const result = [];
    for (let i = ptr; i < ptr + 4 * len; i += 4) {
        result.push(wasm.__wbindgen_externrefs.get(mem.getUint32(i, true)));
    }
    wasm.__externref_drop_slice(ptr, len);
    return result;
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
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

function passArrayJsValueToWasm0(array, malloc) {
    const ptr = malloc(array.length * 4, 4) >>> 0;
    for (let i = 0; i < array.length; i++) {
        const add = addToExternrefTable0(array[i]);
        getDataViewMemory0().setUint32(ptr + 4 * i, add, true);
    }
    WASM_VECTOR_LEN = array.length;
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
    cachedDataViewMemory0 = null;
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
