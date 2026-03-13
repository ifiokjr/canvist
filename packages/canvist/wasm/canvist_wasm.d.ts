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
     * Apply style to the given character range.
     */
    apply_style_range(start: number, end: number, bold: boolean, italic: boolean, underline: boolean, font_size?: number | null, font_family?: string | null, color_rgba?: Uint8Array | null): void;
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
     * Return the character count.
     */
    char_count(): number;
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
     * Return the current undo-coalescing timeout in milliseconds.
     */
    coalesce_timeout(): number;
    /**
     * Create a new editor attached to the canvas element with the given ID.
     *
     * # Errors
     *
     * Returns an error if the canvas element is not found.
     */
    static create(canvas_id: string): CanvistEditor;
    /**
     * Delete a range of characters from `start` to `end`.
     */
    delete_range(start: number, end: number): void;
    /**
     * Find all occurrences of `needle`. Returns a flat array: [start0, end0,
     * start1, end1, …].
     */
    find_all(needle: string, case_sensitive: boolean): Uint32Array;
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
     * Return the currently selected text (empty string if selection is collapsed).
     */
    get_selected_text(): string;
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
     * Insert text at the current cursor position (start of document).
     */
    insert_text(text: string): void;
    /**
     * Insert text at a specific character offset.
     */
    insert_text_at(offset: number, text: string): void;
    /**
     * Check if the current selection is all bold.
     */
    is_bold(): boolean;
    /**
     * Check if the current selection is all italic.
     */
    is_italic(): boolean;
    /**
     * Check if the current selection is all underline.
     */
    is_underline(): boolean;
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
     * Return the full plain-text content of the document.
     */
    plain_text(): string;
    /**
     * Process all pending canonical events via the editor runtime.
     */
    process_events(): void;
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
     * Redo the most recently undone transaction.
     *
     * Re-applies the forward operations that were undone. Returns `true` if a
     * redo was performed, `false` if the redo stack was empty.
     */
    redo(): boolean;
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
     * Select the entire document.
     */
    select_all(): void;
    /**
     * Select the word at the given character offset.
     */
    select_word_at(offset: number): void;
    /**
     * Get selection end offset.
     */
    selection_end(): number;
    /**
     * Get selection start offset.
     */
    selection_start(): number;
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
     * Set font size on the current selection.
     */
    set_font_size(size: number): void;
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
     * Set selection range.
     */
    set_selection(start: number, end: number): void;
    /**
     * Set the logical (CSS) dimensions of the editor canvas.
     *
     * Call this after changing the canvas's CSS size so layout wrapping
     * and hit-testing use the correct dimensions (not the DPR-scaled pixel
     * dimensions).
     */
    set_size(width: number, height: number): void;
    /**
     * Set the document title.
     */
    set_title(title: string): void;
    /**
     * Export the document as a JSON string.
     */
    to_json(): string;
    /**
     * Toggle bold on the current selection.
     *
     * If all characters in the selection are already bold, removes bold.
     * Otherwise, applies bold. Preserves the current selection.
     */
    toggle_bold(): void;
    /**
     * Toggle italic on the current selection. Preserves the current
     * selection.
     */
    toggle_italic(): void;
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
     * Undo the most recent transaction.
     *
     * Applies inverse operations to restore the document to its previous state.
     * Returns `true` if an undo was performed, `false` if the undo stack was empty.
     */
    undo(): boolean;
    /**
     * Find the previous word boundary from a character offset.
     */
    word_boundary_left(offset: number): number;
    /**
     * Find the next word boundary from a character offset.
     */
    word_boundary_right(offset: number): number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_canvisteditor_free: (a: number, b: number) => void;
    readonly canvisteditor_apply_style_range: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => void;
    readonly canvisteditor_break_undo_coalescing: (a: number) => void;
    readonly canvisteditor_can_redo: (a: number) => number;
    readonly canvisteditor_can_undo: (a: number) => number;
    readonly canvisteditor_canvas_id: (a: number) => [number, number];
    readonly canvisteditor_char_count: (a: number) => number;
    readonly canvisteditor_clipboard_cut: (a: number) => void;
    readonly canvisteditor_clipboard_paste: (a: number, b: number, c: number) => void;
    readonly canvisteditor_coalesce_timeout: (a: number) => number;
    readonly canvisteditor_create: (a: number, b: number) => [number, number, number];
    readonly canvisteditor_delete_range: (a: number, b: number, c: number) => void;
    readonly canvisteditor_find_all: (a: number, b: number, c: number, d: number) => [number, number];
    readonly canvisteditor_find_next: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly canvisteditor_find_prev: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly canvisteditor_get_selected_text: (a: number) => [number, number];
    readonly canvisteditor_hit_test: (a: number, b: number, c: number) => [number, number, number];
    readonly canvisteditor_insert_text: (a: number, b: number, c: number) => void;
    readonly canvisteditor_insert_text_at: (a: number, b: number, c: number, d: number) => void;
    readonly canvisteditor_is_bold: (a: number) => number;
    readonly canvisteditor_is_italic: (a: number) => number;
    readonly canvisteditor_is_underline: (a: number) => number;
    readonly canvisteditor_line_end_for_offset: (a: number, b: number) => [number, number, number];
    readonly canvisteditor_line_start_for_offset: (a: number, b: number) => [number, number, number];
    readonly canvisteditor_move_cursor_left: (a: number, b: number) => void;
    readonly canvisteditor_move_cursor_right: (a: number, b: number) => void;
    readonly canvisteditor_move_cursor_to: (a: number, b: number, c: number) => void;
    readonly canvisteditor_offset_above: (a: number, b: number) => [number, number, number];
    readonly canvisteditor_offset_below: (a: number, b: number) => [number, number, number];
    readonly canvisteditor_plain_text: (a: number) => [number, number];
    readonly canvisteditor_process_events: (a: number) => void;
    readonly canvisteditor_queue_key_down: (a: number, b: number, c: number) => void;
    readonly canvisteditor_queue_key_down_with_modifiers: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly canvisteditor_redo: (a: number) => number;
    readonly canvisteditor_render: (a: number) => [number, number];
    readonly canvisteditor_replace_all: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly canvisteditor_replace_range: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly canvisteditor_replay_operations_json: (a: number, b: number, c: number) => [number, number];
    readonly canvisteditor_select_all: (a: number) => void;
    readonly canvisteditor_select_word_at: (a: number, b: number) => void;
    readonly canvisteditor_selection_end: (a: number) => number;
    readonly canvisteditor_selection_start: (a: number) => number;
    readonly canvisteditor_set_caret_visible: (a: number, b: number) => void;
    readonly canvisteditor_set_coalesce_timeout: (a: number, b: number) => void;
    readonly canvisteditor_set_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly canvisteditor_set_font_size: (a: number, b: number) => void;
    readonly canvisteditor_set_now_ms: (a: number, b: number) => void;
    readonly canvisteditor_set_selection: (a: number, b: number, c: number) => void;
    readonly canvisteditor_set_size: (a: number, b: number, c: number) => void;
    readonly canvisteditor_set_title: (a: number, b: number, c: number) => void;
    readonly canvisteditor_to_json: (a: number) => [number, number, number, number];
    readonly canvisteditor_toggle_bold: (a: number) => void;
    readonly canvisteditor_toggle_italic: (a: number) => void;
    readonly canvisteditor_toggle_strikethrough: (a: number) => void;
    readonly canvisteditor_toggle_underline: (a: number) => void;
    readonly canvisteditor_undo: (a: number) => number;
    readonly canvisteditor_word_boundary_left: (a: number, b: number) => number;
    readonly canvisteditor_word_boundary_right: (a: number, b: number) => number;
    readonly canvisteditor_queue_text_input: (a: number, b: number, c: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
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
