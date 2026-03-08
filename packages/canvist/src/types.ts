/**
 * The canvist editor instance.
 *
 * Wraps the WASM-backed editor with a friendly TypeScript API and manages
 * keyboard/mouse/IME input, cursor rendering, selection highlights, and
 * accessibility.
 */
export interface CanvistEditor {
	/** The ID of the canvas element this editor is attached to. */
	readonly canvasId: string;

	/** The current plain-text content of the document. */
	readonly text: string;

	/** The number of characters in the document. */
	readonly charCount: number;

	/** Insert text at the current cursor position. */
	insertText(text: string): void;

	/** Set the document title (metadata). */
	setTitle(title: string): void;

	/** Re-render the document to the canvas. */
	render(): void;

	/** Export the document as a JSON string. */
	toJSON(): string;

	/** Destroy the editor and release WASM resources. */
	destroy(): void;
}
