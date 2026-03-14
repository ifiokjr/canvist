/* tslint:disable */
/* eslint-disable */

/**
 * The main editor handle exposed to JavaScript.
 *
 * Wraps a [`Document`] and a Canvas2D rendering backend. Create one per
 * `<canvas>` element.
 */
export class CanvistEditor {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Add an annotation to a text range.
     *
     * `kind` examples: "error", "warning", "info", "spelling".
     * `message` is optional descriptive text.
     */
    add_annotation(start: number, end: number, kind: string, message: string): void;
    /**
     * Add a coloured background decoration to a line (0-based).
     *
     * Multiple decorations can be added to the same line. The colours
     * are blended in order.
     */
    add_line_decoration(line: number, r: number, g: number, b: number, a: number): void;
    /**
     * Add a single ruler at the given column.
     */
    add_ruler(column: number): void;
    /**
     * Number of active annotations.
     */
    annotation_count(): number;
    /**
     * Get annotations overlapping a character offset.
     *
     * Returns flat array: [start, end, kind, message, ...].
     */
    annotations_at(offset: number): string[];
    /**
     * Apply a named configuration preset.
     *
     * - `"code"`: line numbers, indent guides, whitespace, bracket
     *   highlight, occurrence highlight, auto-close brackets, soft tabs
     * - `"prose"`: word wrap, no line numbers, no whitespace, no
     *   indent guides, placeholder
     * - `"minimal"`: minimal chrome, no gutter, no highlights
     */
    apply_preset(name: string): void;
    /**
     * Apply style to the given character range.
     */
    apply_style_range(start: number, end: number, bold: boolean, italic: boolean, underline: boolean, font_size?: number | null, font_family?: string | null, color_rgba?: Uint8Array | null): void;
    /**
     * Whether bracket auto-closing is enabled.
     */
    auto_close_brackets(): boolean;
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
     */
    auto_indent_newline(): number;
    /**
     * Whether auto-surround is enabled.
     */
    auto_surround(): boolean;
    /**
     * Base64-decode the selected text, replacing the selection.
     *
     * If the selected text is not valid base64, the selection is unchanged.
     */
    base64_decode_selection(): void;
    /**
     * Base64-encode the selected text, replacing the selection.
     */
    base64_encode_selection(): void;
    /**
     * Begin a batch of operations that will be grouped into a single
     * undo step. Call `end_batch` when done.
     *
     * The runtime coalesces rapid edits automatically. This method
     * serves as a logical marker — all edits between `begin_batch`
     * and `end_batch` happen in quick succession and are treated as
     * one undo group.
     */
    begin_batch(): void;
    /**
     * Number of active bookmarks.
     */
    bookmark_count(): number;
    /**
     * Return all bookmarked line numbers as a flat array (0-based).
     */
    bookmarked_lines(): Uint32Array;
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
    break_undo_coalescing(): void;
    /**
     * Whether there are entries on the redo stack.
     */
    can_redo(): boolean;
    /**
     * Whether there are entries on the undo stack.
     */
    can_undo(): boolean;
    /**
     * Return the canvas element ID this editor is attached to.
     */
    canvas_id(): string;
    /**
     * Compute the Y position of the caret in content coordinates.
     *
     * Returns `(y, height)` for the caret line. Useful for scroll-into-view.
     */
    caret_y(): Float32Array;
    /**
     * Scroll so the cursor's line is vertically centered in the viewport.
     */
    center_line_in_viewport(): void;
    /**
     * Return the character count.
     */
    char_count(): number;
    /**
     * Count characters by type: [letters, digits, spaces, punctuation, other].
     *
     * Returns a 5-element array.
     */
    char_counts(): Uint32Array;
    /**
     * Remove all annotations.
     */
    clear_annotations(): void;
    /**
     * Remove all bookmarks.
     */
    clear_bookmarks(): void;
    /**
     * Remove all line decorations.
     */
    clear_line_decorations(): void;
    /**
     * Clear the saved snapshot.
     */
    clear_snapshot(): void;
    /**
     * Perform a clipboard cut: delete the current selection.
     *
     * The caller is expected to have already read `get_selected_text()` and
     * written it to the system clipboard before calling this method.
     */
    clipboard_cut(): void;
    /**
     * Paste text at the current cursor position (replacing any selection).
     */
    clipboard_paste(text: string): void;
    /**
     * Clear the clipboard ring.
     */
    clipboard_ring_clear(): void;
    /**
     * Get the clipboard ring entry at `index` (0 = most recent).
     *
     * Returns empty string if index is out of range.
     */
    clipboard_ring_get(index: number): string;
    /**
     * Number of entries in the clipboard ring.
     */
    clipboard_ring_length(): number;
    /**
     * Paste the clipboard ring entry at `index` at the cursor.
     */
    clipboard_ring_paste(index: number): void;
    /**
     * Push a text entry into the clipboard ring.
     *
     * The ring holds the most recent `clipboard_ring_max` entries
     * (default 10). Newest entry is at index 0.
     */
    clipboard_ring_push(text: string): void;
    /**
     * Return the current undo-coalescing timeout in milliseconds.
     */
    coalesce_timeout(): number;
    /**
     * Get the current line comment prefix.
     */
    comment_prefix(): string;
    /**
     * Suggest completions for the word currently being typed.
     *
     * Returns up to `max_results` words from the document that start
     * with the prefix at the cursor. Sorted alphabetically, deduplicated.
     */
    completions(max_results: number): string[];
    /**
     * Compute the total content height in logical pixels.
     *
     * Uses the paragraph layout engine to determine the full document
     * height including padding and paragraph spacing.
     */
    content_height(): number;
    /**
     * Contract selection intelligently (reverse of expand).
     *
     * Shrinks: all → line → bracket → quote → word → collapsed.
     */
    contract_selection(): void;
    /**
     * Create a new editor attached to the canvas element with the given ID.
     *
     * # Errors
     *
     * Returns an error if the canvas element is not found.
     */
    static create(canvas_id: string): CanvistEditor;
    /**
     * Current column (1-based character offset from line start).
     */
    current_column(): number;
    /**
     * Current line number the cursor is on (1-based).
     */
    current_line_number(): number;
    /**
     * Get the full text of the line the cursor is on (including the
     * trailing `\n` if present). Useful for "copy line" when nothing is
     * selected.
     */
    current_line_text(): string;
    /**
     * Return the 1-based column (character position within the visual line).
     */
    cursor_column(): number;
    /**
     * Navigate backward in cursor history (Ctrl+Alt+←).
     *
     * Returns `true` if the cursor moved.
     */
    cursor_history_back(): boolean;
    /**
     * Navigate forward in cursor history (Ctrl+Alt+→).
     *
     * Returns `true` if the cursor moved.
     */
    cursor_history_forward(): boolean;
    /**
     * Number of positions in cursor history.
     */
    cursor_history_length(): number;
    /**
     * Return the 1-based visual line number the caret is on.
     */
    cursor_line(): number;
    /**
     * Get cursor style (0=line, 1=block, 2=underline).
     */
    cursor_style(): number;
    /**
     * Get cursor width.
     */
    cursor_width_px(): number;
    /**
     * Cut the current line (remove it and return its text).
     * This is the "cut line when nothing is selected" behavior.
     */
    cut_line(): string;
    /**
     * Delete the entire line the cursor is on (Ctrl+Shift+K).
     *
     * If the deleted line is not the last, the trailing `\n` is also
     * removed so the next line moves up.
     */
    delete_line(): void;
    /**
     * Delete a range of characters from `start` to `end`.
     */
    delete_range(start: number, end: number): void;
    /**
     * Delete the word to the left of the cursor (Ctrl+Backspace).
     *
     * Walks backwards from the cursor past whitespace, then past word
     * characters, and deletes the range.
     */
    delete_word_left(): void;
    /**
     * Delete the word to the right of the cursor (Ctrl+Delete).
     */
    delete_word_right(): void;
    /**
     * Whether link detection is enabled.
     */
    detect_links(): boolean;
    /**
     * Compare current text against the last snapshot.
     *
     * Returns a list of changed line numbers (0-based) as a flat array.
     * A line is "changed" if it differs from the snapshot.
     */
    diff_from_snapshot(): Uint32Array;
    /**
     * Duplicate the current line (or selected lines) below.
     */
    duplicate_line(): void;
    /**
     * End a batch of operations.
     *
     * After this call, the next edit will start a new undo group
     * (once the coalesce timeout expires).
     */
    end_batch(): void;
    /**
     * Ensure the document ends with a newline character.
     *
     * Returns `true` if a newline was added.
     */
    ensure_final_newline(): boolean;
    /**
     * Clear the event log.
     */
    event_log_clear(): void;
    /**
     * Get event log entry at index (0 = newest).
     */
    event_log_get(index: number): string;
    /**
     * Number of entries in the event log.
     */
    event_log_length(): number;
    /**
     * Expand selection intelligently: word → quoted → bracketed → line → all.
     *
     * Each call expands to the next logical boundary.
     */
    expand_selection(): void;
    /**
     * Find all occurrences of `needle`. Returns a flat array: [start0, end0,
     * start1, end1, …].
     */
    find_all(needle: string, case_sensitive: boolean): Uint32Array;
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
     */
    find_all_regex(pattern: string): Uint32Array;
    /**
     * Find all whole-word occurrences of `needle`.
     *
     * Returns offsets as `[start0, end0, start1, end1, ...]`.
     * A "whole word" match requires the char before and after the match
     * to be non-alphanumeric (or at document boundary).
     */
    find_all_whole_word(needle: string): Uint32Array;
    /**
     * Get the current find highlight needle.
     */
    find_highlight_needle(): string;
    /**
     * Find all URLs in the document.
     *
     * Returns flat array: [start, end, start, end, ...] of char offsets.
     */
    find_links(): Uint32Array;
    /**
     * Find the offset of the bracket matching the one at `offset`.
     *
     * Returns `None` (via -1 in WASM) if the char at `offset` is not a
     * bracket or no match is found.
     */
    find_matching_bracket(offset: number): number;
    /**
     * Find the next occurrence of `needle` at or after `from_offset`.
     * Returns `[start, end]` or an empty array if not found.
     */
    find_next(needle: string, from_offset: number, case_sensitive: boolean): Uint32Array;
    /**
     * Find the previous occurrence before `from_offset`.
     */
    find_prev(needle: string, from_offset: number, case_sensitive: boolean): Uint32Array;
    /**
     * Get the first visible line number (0-based).
     */
    first_visible_line(): number;
    /**
     * Flesch reading ease score (0–100, higher = easier).
     *
     * Simplified: uses average words per sentence and average
     * syllables per word.
     */
    flesch_reading_ease(): number;
    /**
     * Get the current focus state.
     */
    focused(): boolean;
    /**
     * Number of active fold regions.
     */
    fold_count(): number;
    /**
     * Fold (collapse) a range of lines (0-based, inclusive).
     *
     * The first line remains visible; subsequent lines are hidden.
     */
    fold_lines(start_line: number, end_line: number): void;
    /**
     * Get all folded ranges as flat array: [start0, end0, start1, end1, ...].
     */
    folded_ranges(): Uint32Array;
    /**
     * Import HTML content, replacing the current document.
     *
     * Parses basic inline elements (`<strong>`, `<em>`, `<u>`, `<s>`, `<br>`,
     * `<p>`) and HTML entities.
     */
    from_html(html: string): void;
    /**
     * Get annotations as flat array: [start, end, kind, message, ...].
     */
    get_annotations(): string[];
    /**
     * Get text from a rectangular block selection.
     *
     * Returns lines from `start_line` to `end_line` (inclusive),
     * each trimmed to columns `start_col` to `end_col` (char-based).
     */
    get_block_selection(start_line: number, end_line: number, start_col: number, end_col: number): string;
    /**
     * Get text of a single line (0-based).
     */
    get_line(line: number): string;
    /**
     * Get text for a range of lines (0-based, inclusive start, exclusive end).
     */
    get_line_range(start_line: number, end_line: number): string;
    /**
     * Return the currently selected text (empty string if selection is collapsed).
     */
    get_selected_text(): string;
    /**
     * Move cursor to the very end of the document (Ctrl+End).
     */
    go_to_document_end(): void;
    /**
     * Move cursor to the very beginning of the document (Ctrl+Home).
     */
    go_to_document_start(): void;
    /**
     * Move the cursor to the start of the given 1-based paragraph line.
     *
     * If `line_number` exceeds the paragraph count, the cursor moves to
     * the end of the document.
     */
    go_to_line(line_number: number): void;
    /**
     * Whether a snapshot has been taken.
     */
    has_snapshot(): boolean;
    /**
     * Whether the current-line highlight is enabled.
     */
    highlight_current_line(): boolean;
    /**
     * Whether matching bracket highlighting is enabled.
     */
    highlight_matching_brackets(): boolean;
    /**
     * Whether occurrence highlighting is enabled.
     */
    highlight_occurrences(): boolean;
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
     */
    hit_test(screen_x: number, screen_y: number): number;
    /**
     * Indent the current selection: insert a tab character at the start
     * of each selected line. If the selection is collapsed, insert a tab
     * at the cursor position.
     */
    indent_selection(): void;
    /**
     * Insert a snippet template. `$0` marks where the cursor should
     * be placed after insertion. Other text is inserted literally.
     *
     * Example: `insert_snippet("if ($0) {\n}")` inserts the template
     * and places the cursor between the parentheses.
     */
    insert_snippet(template: string): void;
    /**
     * Insert one "tab" at the cursor — either spaces or a `\t`.
     */
    insert_tab(): void;
    /**
     * Insert text at the current cursor position (start of document).
     */
    insert_text(text: string): void;
    /**
     * Insert text at a specific character offset.
     */
    insert_text_at(offset: number, text: string): void;
    /**
     * Insert text respecting the max_length constraint.
     *
     * Truncates the input so the total never exceeds the limit.
     * Returns the number of characters actually inserted.
     */
    insert_text_clamped(text: string): number;
    /**
     * Insert text respecting overwrite mode. In overwrite mode,
     * characters after the cursor are replaced one-for-one rather
     * than pushing text forward.
     */
    insert_text_overwrite(text: string): void;
    /**
     * Insert an opening bracket and its closing counterpart.
     *
     * Returns the number of characters inserted (always 2 when auto-close
     * fires, 1 otherwise). Cursor is placed between the pair.
     */
    insert_with_auto_close(ch: string): number;
    /**
     * Check if the current selection is all bold.
     */
    is_bold(): boolean;
    /**
     * Check if the current selection is all italic.
     */
    is_italic(): boolean;
    /**
     * Check if the current line has a bookmark.
     */
    is_line_bookmarked(): boolean;
    /**
     * Whether a specific line is inside a folded (hidden) region.
     *
     * Returns true for lines that are hidden — NOT the first line of
     * a fold which remains visible.
     */
    is_line_folded(line: number): boolean;
    /**
     * Whether the document has been modified since last save.
     */
    is_modified(): boolean;
    /**
     * Check if the current selection is all underline.
     */
    is_underline(): boolean;
    /**
     * Join the current line with the line below (Ctrl+J).
     *
     * Replaces the newline between them with a single space.
     */
    join_lines(): void;
    /**
     * Get the last recorded selection end offset (from `selection_changed`).
     */
    last_selection_end(): number;
    /**
     * Get the last visible line number (0-based).
     */
    last_visible_line(): number;
    /**
     * Determine which line number a Y-coordinate in the gutter maps to.
     *
     * Returns the 0-based line number, or -1 if outside content.
     */
    line_at_y(y: number): number;
    /**
     * Count the number of visual lines using the paragraph layout engine.
     */
    line_count(): number;
    /**
     * Get the total number of lines in the document.
     */
    line_count_total(): number;
    /**
     * Number of active line decorations.
     */
    line_decoration_count(): number;
    /**
     * Return the end offset of the visual line containing `offset`.
     */
    line_end_for_offset(offset: number): number;
    /**
     * Return the start offset of the visual line containing `offset`.
     *
     * This performs a full paragraph layout to determine where lines wrap,
     * then returns the character offset where that visual line begins.
     */
    line_start_for_offset(offset: number): number;
    /**
     * Get the URL text at a character offset, if any.
     *
     * Returns empty string if offset is not inside a URL.
     */
    link_at_offset(offset: number): string;
    /**
     * Log an editor event. Newest entries are at index 0.
     *
     * The log is capped at `event_log_max` (default 50).
     * Call from JS to record significant actions.
     */
    log_event(event: string): void;
    /**
     * Delete a saved macro.
     */
    macro_delete_saved(name: string): void;
    /**
     * Whether macro recording is active.
     */
    macro_is_recording(): boolean;
    /**
     * List saved macro names.
     */
    macro_list_saved(): string[];
    /**
     * Record a macro step manually.
     *
     * `kind`: "insert", "delete", "select"
     * `data`: for insert = text; for delete = "start,end";
     *         for select = "start,end"
     */
    macro_record_step(kind: string, data: string): void;
    /**
     * Replay the recorded macro once.
     */
    macro_replay(): void;
    /**
     * Replay a saved macro by name. Returns false if not found.
     */
    macro_replay_saved(name: string): boolean;
    /**
     * Save the current recorded macro under a name.
     */
    macro_save(name: string): void;
    /**
     * Start recording a macro.
     */
    macro_start_recording(): void;
    /**
     * Number of steps in the current macro recording.
     */
    macro_step_count(): number;
    /**
     * Stop recording and return the number of steps recorded.
     */
    macro_stop_recording(): number;
    /**
     * Mark the document as modified.
     *
     * Called automatically by mutating operations. You can also call
     * it manually to force the dirty state.
     */
    mark_modified(): void;
    /**
     * Mark the document as saved (clears the modified flag).
     */
    mark_saved(): void;
    /**
     * Get the current max character count (0 = unlimited).
     */
    max_length(): number;
    /**
     * Measure the pixel width of a single character using the default
     * style.
     */
    measure_char_width(ch: string): number;
    /**
     * Measure the pixel width of a string using the default style.
     *
     * Useful for external layout calculations. Returns 0.0 if the
     * canvas context is not available.
     */
    measure_text_width(text: string): number;
    /**
     * Get the minimap width.
     */
    minimap_width(): number;
    /**
     * Move cursor one character left.
     */
    move_cursor_left(extend: boolean): void;
    /**
     * Move cursor one character right.
     */
    move_cursor_right(extend: boolean): void;
    /**
     * Move cursor to an absolute position; extend toggles range selection.
     */
    move_cursor_to(position: number, extend: boolean): void;
    /**
     * Move the current line down by swapping it with the line below.
     */
    move_line_down(): void;
    /**
     * Move the current line up by swapping it with the line above.
     */
    move_line_up(): void;
    /**
     * Move text from `[src_start, src_end)` to `dest` offset.
     *
     * Used by drag-and-drop: extract the selected text, delete the source
     * range, then insert at the destination (adjusting for the shift).
     */
    move_text(src_start: number, src_end: number, dest: number): void;
    /**
     * Move cursor to the matching bracket (Ctrl+Shift+\).
     *
     * Checks the character at the cursor and the one before it.
     * If a bracket is found, jumps the cursor to its match.
     */
    move_to_matching_bracket(): void;
    /**
     * Move cursor to the start of the next paragraph (Ctrl+↓).
     */
    move_to_next_paragraph(): void;
    /**
     * Move cursor to the start of the previous paragraph (Ctrl+↑).
     *
     * A paragraph boundary is an empty line or the document start.
     */
    move_to_prev_paragraph(): void;
    /**
     * Jump to the next bookmark after the current line.
     *
     * Wraps around to the first bookmark if past the last one.
     * Returns `true` if a bookmark was found.
     */
    next_bookmark(): boolean;
    /**
     * Return all occurrence offsets of the selected text as a flat array
     * `[start0, end0, start1, end1, ...]`.
     */
    occurrence_offsets(): Uint32Array;
    /**
     * Return the character offset on the line directly above `offset`.
     *
     * Preserves the horizontal (x) pixel position of the caret when moving
     * between lines.
     */
    offset_above(offset: number): number;
    /**
     * Return the character offset on the line directly below `offset`.
     */
    offset_below(offset: number): number;
    /**
     * Insert a new line above the current line and move cursor there
     * (Ctrl+Shift+Enter).
     */
    open_line_above(): void;
    /**
     * Insert a new line below the current line and move cursor there
     * (Ctrl+Enter).
     */
    open_line_below(): void;
    /**
     * Outdent the current selection: remove one leading tab or up to 4
     * spaces from the start of each selected line.
     */
    outdent_selection(): void;
    /**
     * Whether the editor is in overwrite mode.
     */
    overwrite_mode(): boolean;
    /**
     * Total paragraph count (non-empty lines).
     */
    paragraph_count(): number;
    /**
     * Paste HTML at the current cursor position.
     *
     * Parses the HTML to extract styled text, deletes any current selection,
     * and inserts the parsed content with formatting preserved.
     */
    paste_html(html: string): void;
    /**
     * Paste text with auto-adjusted indentation.
     *
     * Detects the indentation level at the cursor and adjusts the
     * pasted text to match.
     */
    paste_with_indent(text: string): void;
    /**
     * Get the current placeholder text.
     */
    placeholder(): string;
    /**
     * Return the full plain-text content of the document.
     */
    plain_text(): string;
    /**
     * Jump to the previous bookmark before the current line.
     *
     * Wraps around to the last bookmark if before the first one.
     * Returns `true` if a bookmark was found.
     */
    prev_bookmark(): boolean;
    /**
     * Process all pending canonical events via the editor runtime.
     */
    process_events(): void;
    /**
     * Record the current cursor position in the history stack.
     *
     * Call this before navigation jumps (go-to-line, bookmark jump, etc.)
     * so the user can navigate back. Deduplicates consecutive identical
     * positions and caps the stack at 100 entries.
     */
    push_cursor_history(): void;
    /**
     * Queue a key down event and process resulting operations.
     */
    queue_key_down(key: string): void;
    /**
     * Queue key down with explicit modifier + repeat state.
     */
    queue_key_down_with_modifiers(key: string, shift: boolean, control: boolean, alt: boolean, meta: boolean, repeat: boolean): void;
    /**
     * Queue canonical text input and process it into operations.
     */
    queue_text_input(text: string): void;
    /**
     * Check whether the editor is in read-only mode.
     */
    read_only(): boolean;
    /**
     * Estimated reading time in seconds (assumes 250 words/minute).
     */
    reading_time_seconds(): number;
    /**
     * Redo the most recently undone transaction.
     *
     * Re-applies the forward operations that were undone. Returns `true` if a
     * redo was performed, `false` if the redo stack was empty.
     */
    redo(): boolean;
    /**
     * How many more characters can be inserted before hitting the limit.
     *
     * Returns `usize::MAX` when max_length is 0 (unlimited).
     */
    remaining_capacity(): number;
    /**
     * Remove all annotations matching a kind (e.g. "error").
     */
    remove_annotations_by_kind(kind: string): void;
    /**
     * Remove consecutive duplicate lines from the document.
     *
     * Returns the number of lines removed.
     */
    remove_duplicate_lines(): number;
    /**
     * Remove the background (highlight) colour from the current selection.
     */
    remove_highlight_color(): void;
    /**
     * Remove all decorations from a specific line.
     */
    remove_line_decorations(line: number): void;
    /**
     * Remove the ruler at the given column.
     */
    remove_ruler(column: number): void;
    /**
     * Rename all occurrences of the word under cursor to `new_name`.
     *
     * Uses whole-word matching. Returns the number of replacements.
     */
    rename_all(new_name: string): number;
    /**
     * Request a re-render of the document to the canvas.
     *
     * Performs multi-paragraph, multi-line text rendering with styled runs,
     * selection highlights, and a blinking caret. Each paragraph in the
     * document tree is laid out independently with configurable paragraph
     * spacing between them.
     */
    render(): void;
    /**
     * Replace all occurrences of `needle` with `replacement`.
     * Returns the number of replacements made.
     */
    replace_all(needle: string, replacement: string, case_sensitive: boolean): number;
    /**
     * Replace all occurrences of the selected text with `replacement`.
     *
     * Returns the number of replacements made. Processes from end to
     * start so offsets remain valid.
     */
    replace_all_occurrences(replacement: string): number;
    /**
     * Replace the text in range `[start, end)` with `replacement`.
     *
     * This is a delete + insert.
     */
    replace_range(start: number, end: number, replacement: string): void;
    /**
     * Replay a JSON-encoded operation list into current runtime.
     */
    replay_operations_json(operations_json: string): void;
    /**
     * Restore editor state from a JSON string produced by `save_state`.
     */
    restore_state(json: string): void;
    /**
     * Reverse the order of selected lines.
     */
    reverse_lines(): void;
    /**
     * Get the current ruler columns as a flat array.
     */
    rulers(): Uint32Array;
    /**
     * Serialize the editor state to a JSON string.
     *
     * Includes text, selection, scroll position, theme, and settings.
     * Use `restore_state` to reload.
     */
    save_state(): string;
    /**
     * Scroll by a delta (positive = down, negative = up).
     */
    scroll_by(delta_y: number): void;
    /**
     * The scroll position as a fraction (0.0 = top, 1.0 = bottom).
     */
    scroll_fraction(): number;
    /**
     * The ratio of viewport to content (0.0–1.0). 1.0 = all visible.
     */
    scroll_ratio(): number;
    /**
     * Scroll to a fraction of the document (0.0 = top, 1.0 = bottom).
     */
    scroll_to_fraction(fraction: number): void;
    /**
     * Ensure the current selection (or cursor) is visible in the
     * viewport. Scrolls the minimum amount needed.
     */
    scroll_to_selection(): void;
    /**
     * Get the current vertical scroll offset.
     */
    scroll_y(): number;
    /**
     * Clear search history.
     */
    search_history_clear(): void;
    /**
     * Get search history entry at index (0 = most recent).
     */
    search_history_get(index: number): string;
    /**
     * Number of search history entries.
     */
    search_history_length(): number;
    /**
     * Push a search term into the search history.
     */
    search_history_push(term: string): void;
    /**
     * Select the entire document.
     */
    select_all(): void;
    /**
     * Find all occurrences of the currently selected text.
     *
     * Returns the count of matches found (0 if nothing is selected or no
     * matches). The offsets can be retrieved with `find_all`.
     */
    select_all_occurrences(): number;
    /**
     * Select all text between the nearest enclosing bracket pair.
     *
     * Returns `true` if brackets were found and selection was made.
     */
    select_between_brackets(): boolean;
    /**
     * Select the entire current line (Ctrl+L).
     *
     * Repeated calls extend the selection by one line each time.
     */
    select_line(): void;
    /**
     * Select from cursor to document end (Ctrl+Shift+End).
     */
    select_to_document_end(): void;
    /**
     * Select from cursor to document start (Ctrl+Shift+Home).
     */
    select_to_document_start(): void;
    /**
     * Select the word at the given character offset.
     */
    select_word_at(offset: number): void;
    /**
     * Number of characters currently selected (0 if collapsed).
     */
    selected_char_count(): number;
    /**
     * Number of words in the current selection (0 if collapsed).
     */
    selected_word_count(): number;
    /**
     * Get the selection anchor (start) offset.
     *
     * When selecting left-to-right, anchor < focus (end).
     * When selecting right-to-left, anchor > focus.
     * When collapsed, anchor == focus.
     */
    selection_anchor(): number;
    /**
     * Check if the selection has changed since the last call to this
     * method.
     *
     * Returns `true` the first time the selection moves to a new
     * position. Useful for triggering UI updates only when needed.
     */
    selection_changed(): boolean;
    /**
     * Get selection end offset.
     */
    selection_end(): number;
    /**
     * Whether the selection is collapsed (no text selected).
     */
    selection_is_collapsed(): boolean;
    /**
     * Length of the current selection in characters.
     */
    selection_length(): number;
    /**
     * Get selection start offset.
     */
    selection_start(): number;
    /**
     * Toggle bracket auto-closing.
     *
     * When enabled, typing `(`, `[`, `{`, `"`, or `'` automatically
     * inserts the closing counterpart and places the cursor between them.
     */
    set_auto_close_brackets(enabled: boolean): void;
    /**
     * Enable or disable auto-surround on selection.
     *
     * When enabled and text is selected, typing an opening bracket
     * wraps the selection instead of replacing it.
     */
    set_auto_surround(enabled: boolean): void;
    /**
     * Replace text in a rectangular block.
     *
     * Each line of `text` replaces the corresponding column range.
     */
    set_block_selection(start_line: number, end_line: number, start_col: number, end_col: number, text: string): void;
    /**
     * Set whether the caret (text cursor) is visible.
     *
     * Called by the JS blink controller on a 530 ms interval to toggle the
     * caret on and off. When `visible` is `false`, `render()` skips drawing
     * the caret line, producing the classic blinking effect.
     */
    set_caret_visible(visible: boolean): void;
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
     */
    set_coalesce_timeout(ms: number): void;
    /**
     * Set text color on the current selection.
     */
    set_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set the line comment prefix (default `"// "`).
     */
    set_comment_prefix(prefix: string): void;
    /**
     * Set cursor colour override. Pass 0,0,0,0 to reset to theme default.
     */
    set_cursor_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set cursor style: 0=line (default), 1=block, 2=underline.
     */
    set_cursor_style(style: number): void;
    /**
     * Set cursor width in pixels (line style only, default 2.0).
     */
    set_cursor_width(w: number): void;
    /**
     * Enable or disable URL link detection.
     */
    set_detect_links(enabled: boolean): void;
    /**
     * Set the maximum number of event log entries.
     */
    set_event_log_max(max: number): void;
    /**
     * Set the needle for visual find highlights.
     *
     * All occurrences are highlighted with a translucent overlay.
     * Pass empty string to clear highlights.
     */
    set_find_highlights(needle: string): void;
    /**
     * Set whether the editor has focus.
     *
     * When unfocused, the caret is drawn as a gray line and selection
     * uses a lighter highlight color.
     */
    set_focused(focused: boolean): void;
    /**
     * Set font size on the current selection.
     */
    set_font_size(size: number): void;
    /**
     * Set a background (highlight) colour on the current selection.
     *
     * The colour is stored via the style's `background` field.
     */
    set_highlight_color(r: number, g: number, b: number, a: number): void;
    /**
     * Enable or disable the current-line highlight band.
     */
    set_highlight_current_line(enabled: boolean): void;
    /**
     * Toggle matching bracket highlight.
     */
    set_highlight_matching_brackets(enabled: boolean): void;
    /**
     * Enable or disable highlighting all occurrences of the word under
     * the cursor.
     */
    set_highlight_occurrences(enabled: boolean): void;
    /**
     * Replace text for a range of lines (0-based, inclusive start, exclusive end).
     */
    set_line_range(start_line: number, end_line: number, text: string): void;
    /**
     * Set maximum character count (0 = unlimited).
     *
     * When set, `insert_text` and similar operations will be truncated
     * to stay within the limit.
     */
    set_max_length(max: number): void;
    /**
     * Set the minimap width in pixels (default 60).
     */
    set_minimap_width(w: number): void;
    /**
     * Set the current wall-clock time (milliseconds since epoch) for the
     * undo coalescing timer.
     *
     * Call this with `Date.now()` before every user action so the runtime
     * can measure real time gaps between keystrokes. Without this, the
     * runtime falls back to its monotonic counter which doesn't reflect
     * actual typing speed.
     */
    set_now_ms(ms: number): void;
    /**
     * Set overwrite mode explicitly.
     */
    set_overwrite_mode(enabled: boolean): void;
    /**
     * Set placeholder text shown when the document is empty.
     */
    set_placeholder(text: string): void;
    /**
     * Set the editor to read-only mode. Editing operations are blocked;
     * selection, copy, and navigation still work.
     */
    set_read_only(read_only: boolean): void;
    /**
     * Set column ruler positions (e.g. `[80, 120]`).
     *
     * Pass an empty array to remove all rulers. Rulers are drawn as
     * thin vertical lines at the specified column offsets.
     */
    set_rulers(columns: Uint32Array): void;
    /**
     * Set the vertical scroll offset (clamped to valid range).
     */
    set_scroll_y(y: number): void;
    /**
     * Set selection range.
     */
    set_selection(start: number, end: number): void;
    /**
     * Toggle indent guide rendering.
     */
    set_show_indent_guides(show: boolean): void;
    /**
     * Enable or disable the line-number gutter.
     */
    set_show_line_numbers(show: boolean): void;
    /**
     * Toggle the minimap sidebar.
     */
    set_show_minimap(enabled: boolean): void;
    /**
     * Toggle the visual whitespace indicator.
     *
     * When enabled, the renderer draws `·` for spaces and `→` for tabs.
     */
    set_show_whitespace(show: boolean): void;
    /**
     * Enable/disable wrap continuation indicators in the gutter.
     *
     * When enabled, wrapped continuation lines show a `↪` glyph in
     * the gutter to distinguish them from real line breaks.
     */
    set_show_wrap_indicators(enabled: boolean): void;
    /**
     * Set the logical (CSS) dimensions of the editor canvas.
     *
     * Call this after changing the canvas's CSS size so layout wrapping
     * and hit-testing use the correct dimensions (not the DPR-scaled pixel
     * dimensions).
     */
    set_size(width: number, height: number): void;
    /**
     * Enable or disable soft tabs (spaces instead of `\t`).
     */
    set_soft_tabs(enabled: boolean): void;
    /**
     * Toggle sticky scroll — shows the first line of the document at
     * the top when scrolled past it.
     */
    set_sticky_scroll(enabled: boolean): void;
    /**
     * Set the tab display/insert size (1–8). Default: 4.
     */
    set_tab_size(size: number): void;
    /**
     * Switch to the dark colour theme.
     */
    set_theme_dark(): void;
    /**
     * Switch to the light colour theme.
     */
    set_theme_light(): void;
    /**
     * Set the document title.
     */
    set_title(title: string): void;
    /**
     * Enable or disable word wrapping at the canvas edge.
     *
     * When disabled, lines extend horizontally and horizontal scrolling
     * may be needed.
     */
    set_word_wrap(enabled: boolean): void;
    /**
     * Set the zoom level (1.0 = 100%, 1.5 = 150%, etc.). Clamped to [0.25, 4.0].
     */
    set_zoom(level: number): void;
    /**
     * Whether find highlights are active.
     */
    show_find_highlights(): boolean;
    /**
     * Whether indent guides are enabled.
     */
    show_indent_guides(): boolean;
    /**
     * Check whether line numbers are visible.
     */
    show_line_numbers(): boolean;
    /**
     * Whether the minimap is shown.
     */
    show_minimap(): boolean;
    /**
     * Whether whitespace visualization is enabled.
     */
    show_whitespace(): boolean;
    /**
     * Whether wrap indicators are shown.
     */
    show_wrap_indicators(): boolean;
    /**
     * If the cursor is between a matching bracket pair (e.g. `(|)`),
     * delete both characters. Otherwise, behave like normal backspace.
     *
     * Returns `true` if a pair was deleted, `false` for normal backspace.
     */
    smart_backspace(): boolean;
    /**
     * Whether soft tabs are enabled.
     */
    soft_tabs(): boolean;
    /**
     * Sort selected lines in ascending alphabetical order.
     */
    sort_lines_asc(): void;
    /**
     * Sort selected lines in descending alphabetical order.
     */
    sort_lines_desc(): void;
    /**
     * Convert leading spaces to tabs (using the current tab_size).
     *
     * Only converts groups of `tab_size` spaces at the start of lines.
     * Returns the number of conversions made.
     */
    spaces_to_tabs(): number;
    /**
     * Whether sticky scroll is enabled.
     */
    sticky_scroll(): boolean;
    /**
     * Get the current tab size.
     */
    tab_size(): number;
    /**
     * Convert all tabs to spaces (using the current tab_size).
     *
     * Returns the number of tabs replaced.
     */
    tabs_to_spaces(): number;
    /**
     * Take a snapshot of the current text for later diff.
     */
    take_snapshot(): void;
    /**
     * Fast content fingerprint (FNV-1a 64-bit hash as hex string).
     *
     * Useful for external change detection: compare hashes to check
     * if content has changed without comparing full text.
     */
    text_hash(): string;
    /**
     * Return `"dark"` or `"light"` depending on the active theme.
     */
    theme_name(): string;
    /**
     * Export the document as HTML.
     *
     * Each paragraph becomes a `<p>` element. Styled text gets inline
     * elements (`<strong>`, `<em>`, `<u>`, `<s>`, `<span>`).
     */
    to_html(): string;
    /**
     * Export the document as a JSON string.
     */
    to_json(): string;
    /**
     * Export the document as Markdown.
     *
     * Bold → `**text**`, italic → `*text*`, strikethrough → `~~text~~`.
     */
    to_markdown(): string;
    /**
     * Toggle bold on the current selection.
     *
     * If all characters in the selection are already bold, removes bold.
     * Otherwise, applies bold. Preserves the current selection.
     */
    toggle_bold(): void;
    /**
     * Toggle a bookmark on the current line.
     *
     * Returns `true` if the bookmark was added, `false` if removed.
     */
    toggle_bookmark(): boolean;
    /**
     * Toggle a bullet list prefix (`• `) on the current line.
     *
     * If the line already starts with `• `, the prefix is removed.
     * Otherwise it is inserted at the line start (after leading whitespace).
     */
    toggle_bullet_list(): void;
    /**
     * Toggle fold at a line. If the line starts a fold, unfold it.
     * Otherwise, try to fold from this line using indentation.
     */
    toggle_fold_at(line: number): void;
    /**
     * Toggle italic on the current selection. Preserves the current
     * selection.
     */
    toggle_italic(): void;
    /**
     * Toggle a line-comment prefix on the current line or all selected
     * lines.
     *
     * If all affected lines start with the prefix, it is removed from
     * each. Otherwise the prefix is added to every line.
     */
    toggle_line_comment(): void;
    /**
     * Toggle a numbered list prefix (`1. `) on the current line.
     *
     * If the line already starts with a number prefix, it is removed.
     * Otherwise `1. ` is inserted.
     */
    toggle_numbered_list(): void;
    /**
     * Toggle between insert and overwrite mode (Insert key).
     */
    toggle_overwrite_mode(): void;
    /**
     * Toggle strikethrough on the current selection.
     */
    toggle_strikethrough(): void;
    /**
     * Toggle underline on the current selection. Preserves the current
     * selection.
     */
    toggle_underline(): void;
    /**
     * Simple tokenization of the document text.
     *
     * Returns alternating [kind, text, kind, text, ...] where kind is
     * one of: "word", "number", "whitespace", "punctuation", "newline".
     */
    tokenize(): string[];
    /**
     * Convert selected text to lowercase.
     */
    transform_lowercase(): void;
    /**
     * Convert selected text to Title Case.
     */
    transform_title_case(): void;
    /**
     * Swap the case of each character in the selection (a↔A).
     */
    transform_toggle_case(): void;
    /**
     * Convert selected text to UPPERCASE.
     */
    transform_uppercase(): void;
    /**
     * Swap the two characters around the cursor (Ctrl+T).
     *
     * If the cursor is at the end of a line, swaps the two preceding
     * characters instead.
     */
    transpose_chars(): void;
    /**
     * Remove trailing whitespace (spaces and tabs) from every line.
     *
     * Returns the number of characters removed.
     */
    trim_trailing_whitespace(): number;
    /**
     * If auto-surround is on and the selection is non-empty, wrap the
     * selection with the opening/closing pair. Returns `true` if wrapping
     * happened.
     */
    try_auto_surround(ch: string): boolean;
    /**
     * Undo the most recent transaction.
     *
     * Applies inverse operations to restore the document to its previous state.
     * Returns `true` if an undo was performed, `false` if the undo stack was empty.
     */
    undo(): boolean;
    /**
     * Unfold all ranges.
     */
    unfold_all(): void;
    /**
     * Unfold a specific range.
     */
    unfold_lines(start_line: number, end_line: number): void;
    /**
     * URL-decode the selected text, replacing the selection.
     */
    url_decode_selection(): void;
    /**
     * URL-encode the selected text, replacing the selection.
     */
    url_encode_selection(): void;
    /**
     * The viewport height in pixels (same as canvas height / zoom).
     */
    viewport_height(): number;
    /**
     * Number of lines visible in the viewport.
     */
    visible_line_count(): number;
    /**
     * Get the word under (or adjacent to) the cursor.
     *
     * Returns empty string if the cursor is not on a word.
     */
    word_at_cursor(): string;
    /**
     * Find the previous word boundary from a character offset.
     */
    word_boundary_left(offset: number): number;
    /**
     * Find the next word boundary from a character offset.
     */
    word_boundary_right(offset: number): number;
    /**
     * Count the number of words (whitespace-separated tokens).
     */
    word_count(): number;
    /**
     * Return the top N most frequent words as alternating
     * `[word, count, word, count, ...]` strings.
     */
    word_frequency(top_n: number): string[];
    /**
     * Whether word wrapping is enabled.
     */
    word_wrap(): boolean;
    /**
     * Wrap the selected text with a pair of strings (e.g. brackets).
     *
     * Example: `wrap_selection("(", ")")` turns `hello` into `(hello)`.
     * Cursor is placed after the closing string.
     */
    wrap_selection(open: string, close: string): void;
    /**
     * Get the current zoom level.
     */
    zoom(): number;
    /**
     * Zoom in by one step (1.1× multiplier).
     */
    zoom_in(): void;
    /**
     * Zoom out by one step (÷ 1.1).
     */
    zoom_out(): void;
    /**
     * Reset zoom to 100%.
     */
    zoom_reset(): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_canvisteditor_free: (a: number, b: number) => void;
    readonly canvisteditor_add_annotation: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly canvisteditor_add_line_decoration: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly canvisteditor_add_ruler: (a: number, b: number) => void;
    readonly canvisteditor_annotation_count: (a: number) => number;
    readonly canvisteditor_annotations_at: (a: number, b: number) => [number, number];
    readonly canvisteditor_apply_preset: (a: number, b: number, c: number) => void;
    readonly canvisteditor_apply_style_range: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => void;
    readonly canvisteditor_auto_close_brackets: (a: number) => number;
    readonly canvisteditor_auto_indent_newline: (a: number) => number;
    readonly canvisteditor_auto_surround: (a: number) => number;
    readonly canvisteditor_base64_decode_selection: (a: number) => void;
    readonly canvisteditor_base64_encode_selection: (a: number) => void;
    readonly canvisteditor_begin_batch: (a: number) => void;
    readonly canvisteditor_bookmark_count: (a: number) => number;
    readonly canvisteditor_bookmarked_lines: (a: number) => [number, number];
    readonly canvisteditor_break_undo_coalescing: (a: number) => void;
    readonly canvisteditor_can_redo: (a: number) => number;
    readonly canvisteditor_can_undo: (a: number) => number;
    readonly canvisteditor_canvas_id: (a: number) => [number, number];
    readonly canvisteditor_caret_y: (a: number) => [number, number, number, number];
    readonly canvisteditor_center_line_in_viewport: (a: number) => void;
    readonly canvisteditor_char_count: (a: number) => number;
    readonly canvisteditor_char_counts: (a: number) => [number, number];
    readonly canvisteditor_clear_annotations: (a: number) => void;
    readonly canvisteditor_clear_bookmarks: (a: number) => void;
    readonly canvisteditor_clear_line_decorations: (a: number) => void;
    readonly canvisteditor_clear_snapshot: (a: number) => void;
    readonly canvisteditor_clipboard_cut: (a: number) => void;
    readonly canvisteditor_clipboard_paste: (a: number, b: number, c: number) => void;
    readonly canvisteditor_clipboard_ring_clear: (a: number) => void;
    readonly canvisteditor_clipboard_ring_get: (a: number, b: number) => [number, number];
    readonly canvisteditor_clipboard_ring_length: (a: number) => number;
    readonly canvisteditor_clipboard_ring_paste: (a: number, b: number) => void;
    readonly canvisteditor_clipboard_ring_push: (a: number, b: number, c: number) => void;
    readonly canvisteditor_coalesce_timeout: (a: number) => number;
    readonly canvisteditor_comment_prefix: (a: number) => [number, number];
    readonly canvisteditor_completions: (a: number, b: number) => [number, number];
    readonly canvisteditor_content_height: (a: number) => [number, number, number];
    readonly canvisteditor_contract_selection: (a: number) => void;
    readonly canvisteditor_create: (a: number, b: number) => [number, number, number];
    readonly canvisteditor_current_column: (a: number) => number;
    readonly canvisteditor_current_line_number: (a: number) => number;
    readonly canvisteditor_current_line_text: (a: number) => [number, number];
    readonly canvisteditor_cursor_column: (a: number) => [number, number, number];
    readonly canvisteditor_cursor_history_back: (a: number) => number;
    readonly canvisteditor_cursor_history_forward: (a: number) => number;
    readonly canvisteditor_cursor_history_length: (a: number) => number;
    readonly canvisteditor_cursor_line: (a: number) => [number, number, number];
    readonly canvisteditor_cursor_style: (a: number) => number;
    readonly canvisteditor_cursor_width_px: (a: number) => number;
    readonly canvisteditor_cut_line: (a: number) => [number, number];
    readonly canvisteditor_delete_line: (a: number) => void;
    readonly canvisteditor_delete_range: (a: number, b: number, c: number) => void;
    readonly canvisteditor_delete_word_left: (a: number) => void;
    readonly canvisteditor_delete_word_right: (a: number) => void;
    readonly canvisteditor_detect_links: (a: number) => number;
    readonly canvisteditor_diff_from_snapshot: (a: number) => [number, number];
    readonly canvisteditor_duplicate_line: (a: number) => void;
    readonly canvisteditor_ensure_final_newline: (a: number) => number;
    readonly canvisteditor_event_log_clear: (a: number) => void;
    readonly canvisteditor_event_log_get: (a: number, b: number) => [number, number];
    readonly canvisteditor_event_log_length: (a: number) => number;
    readonly canvisteditor_expand_selection: (a: number) => void;
    readonly canvisteditor_find_all: (a: number, b: number, c: number, d: number) => [number, number];
    readonly canvisteditor_find_all_regex: (a: number, b: number, c: number) => [number, number];
    readonly canvisteditor_find_all_whole_word: (a: number, b: number, c: number) => [number, number];
    readonly canvisteditor_find_highlight_needle: (a: number) => [number, number];
    readonly canvisteditor_find_links: (a: number) => [number, number];
    readonly canvisteditor_find_matching_bracket: (a: number, b: number) => number;
    readonly canvisteditor_find_next: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly canvisteditor_find_prev: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly canvisteditor_first_visible_line: (a: number) => number;
    readonly canvisteditor_flesch_reading_ease: (a: number) => number;
    readonly canvisteditor_focused: (a: number) => number;
    readonly canvisteditor_fold_count: (a: number) => number;
    readonly canvisteditor_fold_lines: (a: number, b: number, c: number) => void;
    readonly canvisteditor_folded_ranges: (a: number) => [number, number];
    readonly canvisteditor_from_html: (a: number, b: number, c: number) => void;
    readonly canvisteditor_get_annotations: (a: number) => [number, number];
    readonly canvisteditor_get_block_selection: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly canvisteditor_get_line: (a: number, b: number) => [number, number];
    readonly canvisteditor_get_line_range: (a: number, b: number, c: number) => [number, number];
    readonly canvisteditor_get_selected_text: (a: number) => [number, number];
    readonly canvisteditor_go_to_document_end: (a: number) => void;
    readonly canvisteditor_go_to_document_start: (a: number) => void;
    readonly canvisteditor_go_to_line: (a: number, b: number) => void;
    readonly canvisteditor_has_snapshot: (a: number) => number;
    readonly canvisteditor_highlight_current_line: (a: number) => number;
    readonly canvisteditor_highlight_matching_brackets: (a: number) => number;
    readonly canvisteditor_highlight_occurrences: (a: number) => number;
    readonly canvisteditor_hit_test: (a: number, b: number, c: number) => [number, number, number];
    readonly canvisteditor_indent_selection: (a: number) => void;
    readonly canvisteditor_insert_snippet: (a: number, b: number, c: number) => void;
    readonly canvisteditor_insert_tab: (a: number) => void;
    readonly canvisteditor_insert_text: (a: number, b: number, c: number) => void;
    readonly canvisteditor_insert_text_at: (a: number, b: number, c: number, d: number) => void;
    readonly canvisteditor_insert_text_clamped: (a: number, b: number, c: number) => number;
    readonly canvisteditor_insert_text_overwrite: (a: number, b: number, c: number) => void;
    readonly canvisteditor_insert_with_auto_close: (a: number, b: number, c: number) => number;
    readonly canvisteditor_is_bold: (a: number) => number;
    readonly canvisteditor_is_italic: (a: number) => number;
    readonly canvisteditor_is_line_bookmarked: (a: number) => number;
    readonly canvisteditor_is_line_folded: (a: number, b: number) => number;
    readonly canvisteditor_is_modified: (a: number) => number;
    readonly canvisteditor_is_underline: (a: number) => number;
    readonly canvisteditor_join_lines: (a: number) => void;
    readonly canvisteditor_last_selection_end: (a: number) => number;
    readonly canvisteditor_last_visible_line: (a: number) => number;
    readonly canvisteditor_line_at_y: (a: number, b: number) => number;
    readonly canvisteditor_line_count: (a: number) => [number, number, number];
    readonly canvisteditor_line_count_total: (a: number) => number;
    readonly canvisteditor_line_decoration_count: (a: number) => number;
    readonly canvisteditor_line_end_for_offset: (a: number, b: number) => [number, number, number];
    readonly canvisteditor_line_start_for_offset: (a: number, b: number) => [number, number, number];
    readonly canvisteditor_link_at_offset: (a: number, b: number) => [number, number];
    readonly canvisteditor_log_event: (a: number, b: number, c: number) => void;
    readonly canvisteditor_macro_delete_saved: (a: number, b: number, c: number) => void;
    readonly canvisteditor_macro_is_recording: (a: number) => number;
    readonly canvisteditor_macro_list_saved: (a: number) => [number, number];
    readonly canvisteditor_macro_record_step: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly canvisteditor_macro_replay: (a: number) => void;
    readonly canvisteditor_macro_replay_saved: (a: number, b: number, c: number) => number;
    readonly canvisteditor_macro_save: (a: number, b: number, c: number) => void;
    readonly canvisteditor_macro_start_recording: (a: number) => void;
    readonly canvisteditor_macro_step_count: (a: number) => number;
    readonly canvisteditor_macro_stop_recording: (a: number) => number;
    readonly canvisteditor_mark_modified: (a: number) => void;
    readonly canvisteditor_mark_saved: (a: number) => void;
    readonly canvisteditor_max_length: (a: number) => number;
    readonly canvisteditor_measure_char_width: (a: number, b: number, c: number) => number;
    readonly canvisteditor_minimap_width: (a: number) => number;
    readonly canvisteditor_move_cursor_left: (a: number, b: number) => void;
    readonly canvisteditor_move_cursor_right: (a: number, b: number) => void;
    readonly canvisteditor_move_cursor_to: (a: number, b: number, c: number) => void;
    readonly canvisteditor_move_line_down: (a: number) => void;
    readonly canvisteditor_move_line_up: (a: number) => void;
    readonly canvisteditor_move_text: (a: number, b: number, c: number, d: number) => void;
    readonly canvisteditor_move_to_matching_bracket: (a: number) => void;
    readonly canvisteditor_move_to_next_paragraph: (a: number) => void;
    readonly canvisteditor_move_to_prev_paragraph: (a: number) => void;
    readonly canvisteditor_next_bookmark: (a: number) => number;
    readonly canvisteditor_occurrence_offsets: (a: number) => [number, number];
    readonly canvisteditor_offset_above: (a: number, b: number) => [number, number, number];
    readonly canvisteditor_offset_below: (a: number, b: number) => [number, number, number];
    readonly canvisteditor_open_line_above: (a: number) => void;
    readonly canvisteditor_open_line_below: (a: number) => void;
    readonly canvisteditor_outdent_selection: (a: number) => void;
    readonly canvisteditor_overwrite_mode: (a: number) => number;
    readonly canvisteditor_paragraph_count: (a: number) => number;
    readonly canvisteditor_paste_html: (a: number, b: number, c: number) => void;
    readonly canvisteditor_paste_with_indent: (a: number, b: number, c: number) => void;
    readonly canvisteditor_placeholder: (a: number) => [number, number];
    readonly canvisteditor_plain_text: (a: number) => [number, number];
    readonly canvisteditor_prev_bookmark: (a: number) => number;
    readonly canvisteditor_process_events: (a: number) => void;
    readonly canvisteditor_push_cursor_history: (a: number) => void;
    readonly canvisteditor_queue_key_down: (a: number, b: number, c: number) => void;
    readonly canvisteditor_queue_key_down_with_modifiers: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly canvisteditor_queue_text_input: (a: number, b: number, c: number) => void;
    readonly canvisteditor_read_only: (a: number) => number;
    readonly canvisteditor_reading_time_seconds: (a: number) => number;
    readonly canvisteditor_redo: (a: number) => number;
    readonly canvisteditor_remaining_capacity: (a: number) => number;
    readonly canvisteditor_remove_annotations_by_kind: (a: number, b: number, c: number) => void;
    readonly canvisteditor_remove_duplicate_lines: (a: number) => number;
    readonly canvisteditor_remove_highlight_color: (a: number) => void;
    readonly canvisteditor_remove_line_decorations: (a: number, b: number) => void;
    readonly canvisteditor_remove_ruler: (a: number, b: number) => void;
    readonly canvisteditor_rename_all: (a: number, b: number, c: number) => number;
    readonly canvisteditor_render: (a: number) => [number, number];
    readonly canvisteditor_replace_all: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly canvisteditor_replace_all_occurrences: (a: number, b: number, c: number) => number;
    readonly canvisteditor_replace_range: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly canvisteditor_replay_operations_json: (a: number, b: number, c: number) => [number, number];
    readonly canvisteditor_restore_state: (a: number, b: number, c: number) => void;
    readonly canvisteditor_reverse_lines: (a: number) => void;
    readonly canvisteditor_rulers: (a: number) => [number, number];
    readonly canvisteditor_save_state: (a: number) => [number, number];
    readonly canvisteditor_scroll_by: (a: number, b: number) => void;
    readonly canvisteditor_scroll_fraction: (a: number) => number;
    readonly canvisteditor_scroll_ratio: (a: number) => number;
    readonly canvisteditor_scroll_to_fraction: (a: number, b: number) => void;
    readonly canvisteditor_scroll_to_selection: (a: number) => void;
    readonly canvisteditor_scroll_y: (a: number) => number;
    readonly canvisteditor_search_history_clear: (a: number) => void;
    readonly canvisteditor_search_history_get: (a: number, b: number) => [number, number];
    readonly canvisteditor_search_history_length: (a: number) => number;
    readonly canvisteditor_search_history_push: (a: number, b: number, c: number) => void;
    readonly canvisteditor_select_all: (a: number) => void;
    readonly canvisteditor_select_all_occurrences: (a: number) => number;
    readonly canvisteditor_select_between_brackets: (a: number) => number;
    readonly canvisteditor_select_line: (a: number) => void;
    readonly canvisteditor_select_to_document_end: (a: number) => void;
    readonly canvisteditor_select_to_document_start: (a: number) => void;
    readonly canvisteditor_select_word_at: (a: number, b: number) => void;
    readonly canvisteditor_selected_char_count: (a: number) => number;
    readonly canvisteditor_selected_word_count: (a: number) => number;
    readonly canvisteditor_selection_anchor: (a: number) => number;
    readonly canvisteditor_selection_changed: (a: number) => number;
    readonly canvisteditor_selection_end: (a: number) => number;
    readonly canvisteditor_selection_is_collapsed: (a: number) => number;
    readonly canvisteditor_selection_length: (a: number) => number;
    readonly canvisteditor_set_auto_close_brackets: (a: number, b: number) => void;
    readonly canvisteditor_set_auto_surround: (a: number, b: number) => void;
    readonly canvisteditor_set_block_selection: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly canvisteditor_set_caret_visible: (a: number, b: number) => void;
    readonly canvisteditor_set_coalesce_timeout: (a: number, b: number) => void;
    readonly canvisteditor_set_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly canvisteditor_set_comment_prefix: (a: number, b: number, c: number) => void;
    readonly canvisteditor_set_cursor_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly canvisteditor_set_cursor_style: (a: number, b: number) => void;
    readonly canvisteditor_set_cursor_width: (a: number, b: number) => void;
    readonly canvisteditor_set_detect_links: (a: number, b: number) => void;
    readonly canvisteditor_set_event_log_max: (a: number, b: number) => void;
    readonly canvisteditor_set_find_highlights: (a: number, b: number, c: number) => void;
    readonly canvisteditor_set_focused: (a: number, b: number) => void;
    readonly canvisteditor_set_font_size: (a: number, b: number) => void;
    readonly canvisteditor_set_highlight_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly canvisteditor_set_highlight_current_line: (a: number, b: number) => void;
    readonly canvisteditor_set_highlight_matching_brackets: (a: number, b: number) => void;
    readonly canvisteditor_set_highlight_occurrences: (a: number, b: number) => void;
    readonly canvisteditor_set_line_range: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly canvisteditor_set_max_length: (a: number, b: number) => void;
    readonly canvisteditor_set_minimap_width: (a: number, b: number) => void;
    readonly canvisteditor_set_now_ms: (a: number, b: number) => void;
    readonly canvisteditor_set_overwrite_mode: (a: number, b: number) => void;
    readonly canvisteditor_set_placeholder: (a: number, b: number, c: number) => void;
    readonly canvisteditor_set_read_only: (a: number, b: number) => void;
    readonly canvisteditor_set_rulers: (a: number, b: number, c: number) => void;
    readonly canvisteditor_set_scroll_y: (a: number, b: number) => void;
    readonly canvisteditor_set_selection: (a: number, b: number, c: number) => void;
    readonly canvisteditor_set_show_indent_guides: (a: number, b: number) => void;
    readonly canvisteditor_set_show_line_numbers: (a: number, b: number) => void;
    readonly canvisteditor_set_show_minimap: (a: number, b: number) => void;
    readonly canvisteditor_set_show_whitespace: (a: number, b: number) => void;
    readonly canvisteditor_set_show_wrap_indicators: (a: number, b: number) => void;
    readonly canvisteditor_set_size: (a: number, b: number, c: number) => void;
    readonly canvisteditor_set_soft_tabs: (a: number, b: number) => void;
    readonly canvisteditor_set_sticky_scroll: (a: number, b: number) => void;
    readonly canvisteditor_set_tab_size: (a: number, b: number) => void;
    readonly canvisteditor_set_theme_dark: (a: number) => void;
    readonly canvisteditor_set_theme_light: (a: number) => void;
    readonly canvisteditor_set_title: (a: number, b: number, c: number) => void;
    readonly canvisteditor_set_word_wrap: (a: number, b: number) => void;
    readonly canvisteditor_set_zoom: (a: number, b: number) => void;
    readonly canvisteditor_show_find_highlights: (a: number) => number;
    readonly canvisteditor_show_indent_guides: (a: number) => number;
    readonly canvisteditor_show_line_numbers: (a: number) => number;
    readonly canvisteditor_show_minimap: (a: number) => number;
    readonly canvisteditor_show_whitespace: (a: number) => number;
    readonly canvisteditor_show_wrap_indicators: (a: number) => number;
    readonly canvisteditor_smart_backspace: (a: number) => number;
    readonly canvisteditor_soft_tabs: (a: number) => number;
    readonly canvisteditor_sort_lines_asc: (a: number) => void;
    readonly canvisteditor_sort_lines_desc: (a: number) => void;
    readonly canvisteditor_spaces_to_tabs: (a: number) => number;
    readonly canvisteditor_sticky_scroll: (a: number) => number;
    readonly canvisteditor_tab_size: (a: number) => number;
    readonly canvisteditor_tabs_to_spaces: (a: number) => number;
    readonly canvisteditor_take_snapshot: (a: number) => void;
    readonly canvisteditor_text_hash: (a: number) => [number, number];
    readonly canvisteditor_theme_name: (a: number) => [number, number];
    readonly canvisteditor_to_html: (a: number) => [number, number];
    readonly canvisteditor_to_json: (a: number) => [number, number, number, number];
    readonly canvisteditor_to_markdown: (a: number) => [number, number];
    readonly canvisteditor_toggle_bold: (a: number) => void;
    readonly canvisteditor_toggle_bookmark: (a: number) => number;
    readonly canvisteditor_toggle_bullet_list: (a: number) => void;
    readonly canvisteditor_toggle_fold_at: (a: number, b: number) => void;
    readonly canvisteditor_toggle_italic: (a: number) => void;
    readonly canvisteditor_toggle_line_comment: (a: number) => void;
    readonly canvisteditor_toggle_numbered_list: (a: number) => void;
    readonly canvisteditor_toggle_overwrite_mode: (a: number) => void;
    readonly canvisteditor_toggle_strikethrough: (a: number) => void;
    readonly canvisteditor_toggle_underline: (a: number) => void;
    readonly canvisteditor_tokenize: (a: number) => [number, number];
    readonly canvisteditor_transform_lowercase: (a: number) => void;
    readonly canvisteditor_transform_title_case: (a: number) => void;
    readonly canvisteditor_transform_toggle_case: (a: number) => void;
    readonly canvisteditor_transform_uppercase: (a: number) => void;
    readonly canvisteditor_transpose_chars: (a: number) => void;
    readonly canvisteditor_trim_trailing_whitespace: (a: number) => number;
    readonly canvisteditor_try_auto_surround: (a: number, b: number, c: number) => number;
    readonly canvisteditor_undo: (a: number) => number;
    readonly canvisteditor_unfold_all: (a: number) => void;
    readonly canvisteditor_unfold_lines: (a: number, b: number, c: number) => void;
    readonly canvisteditor_url_decode_selection: (a: number) => void;
    readonly canvisteditor_url_encode_selection: (a: number) => void;
    readonly canvisteditor_viewport_height: (a: number) => number;
    readonly canvisteditor_visible_line_count: (a: number) => number;
    readonly canvisteditor_word_at_cursor: (a: number) => [number, number];
    readonly canvisteditor_word_boundary_left: (a: number, b: number) => number;
    readonly canvisteditor_word_boundary_right: (a: number, b: number) => number;
    readonly canvisteditor_word_count: (a: number) => number;
    readonly canvisteditor_word_frequency: (a: number, b: number) => [number, number];
    readonly canvisteditor_word_wrap: (a: number) => number;
    readonly canvisteditor_wrap_selection: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly canvisteditor_zoom: (a: number) => number;
    readonly canvisteditor_zoom_in: (a: number) => void;
    readonly canvisteditor_zoom_out: (a: number) => void;
    readonly canvisteditor_zoom_reset: (a: number) => void;
    readonly canvisteditor_end_batch: (a: number) => void;
    readonly canvisteditor_selection_start: (a: number) => number;
    readonly canvisteditor_measure_text_width: (a: number, b: number, c: number) => number;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
