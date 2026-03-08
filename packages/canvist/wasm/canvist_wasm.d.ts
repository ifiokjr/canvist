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
     * Return the canvas element ID this editor is attached to.
     */
    canvas_id(): string;
    /**
     * Return the character count.
     */
    char_count(): number;
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
     * Insert text at the current cursor position (start of document).
     */
    insert_text(text: string): void;
    /**
     * Insert text at a specific character offset.
     */
    insert_text_at(offset: number, text: string): void;
    /**
     * Return the full plain-text content of the document.
     */
    plain_text(): string;
    /**
     * Request a re-render of the document to the canvas.
     *
     * This reads the document state and draws it using the Canvas 2D API.
     */
    render(): void;
    /**
     * Set the document title.
     */
    set_title(title: string): void;
    /**
     * Export the document as a JSON string.
     */
    to_json(): string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_canvisteditor_free: (a: number, b: number) => void;
    readonly canvisteditor_canvas_id: (a: number) => [number, number];
    readonly canvisteditor_char_count: (a: number) => number;
    readonly canvisteditor_create: (a: number, b: number) => [number, number, number];
    readonly canvisteditor_delete_range: (a: number, b: number, c: number) => void;
    readonly canvisteditor_insert_text: (a: number, b: number, c: number) => void;
    readonly canvisteditor_insert_text_at: (a: number, b: number, c: number, d: number) => void;
    readonly canvisteditor_plain_text: (a: number) => [number, number];
    readonly canvisteditor_render: (a: number) => [number, number];
    readonly canvisteditor_set_title: (a: number, b: number, c: number) => void;
    readonly canvisteditor_to_json: (a: number) => [number, number, number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
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
