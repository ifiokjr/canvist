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

	/** Insert text at a specific character offset. */
	insertTextAt(offset: number, text: string): void;

	/** Delete text in the half-open range `[start, end)`. */
	deleteRange(start: number, end: number): void;

	/** Undo the most recent transaction. Returns `true` if undo was performed. */
	undo(): boolean;

	/** Redo the most recently undone transaction. Returns `true` if redo was performed. */
	redo(): boolean;

	/** Whether there are entries on the undo stack. */
	readonly canUndo: boolean;

	/** Whether there are entries on the redo stack. */
	readonly canRedo: boolean;

	/**
	 * Force the next edit to start a new undo group, even if it would normally
	 * be coalesced with the previous one (e.g. consecutive single-char inserts).
	 */
	breakUndoCoalescing(): void;

	/**
	 * Set the coalesce timeout in milliseconds. Consecutive edits within this
	 * window are merged into a single undo entry.
	 */
	setCoalesceTimeout(ms: number): void;

	/** The current coalesce timeout in milliseconds. */
	readonly coalesceTimeout: number;

	/** Queue plain text input for runtime event processing. */
	queueTextInput(text: string): void;

	/** Queue a keydown event for runtime event processing. */
	queueKeyDown(key: string): void;

	/** Queue keydown with explicit modifiers/repeat state. */
	queueKeyDownWithModifiers(
		key: string,
		modifiers: {
			shift?: boolean;
			control?: boolean;
			alt?: boolean;
			meta?: boolean;
			repeat?: boolean;
		},
	): void;

	/** Process queued runtime events. */
	processEvents(): void;

	/** Current selection start offset. */
	readonly selectionStart: number;

	/** Current selection end offset. */
	readonly selectionEnd: number;

	/** Set selection range `[start, end]` (order normalized by runtime). */
	setSelection(start: number, end: number): void;

	/** Move cursor to an absolute position. */
	moveCursorTo(position: number, extend?: boolean): void;

	/** Move cursor one character left. */
	moveCursorLeft(extend?: boolean): void;

	/** Move cursor one character right. */
	moveCursorRight(extend?: boolean): void;

	/**
	 * Apply style flags/options to a range.
	 */
	applyStyleRange(
		start: number,
		end: number,
		style?: {
			bold?: boolean;
			italic?: boolean;
			underline?: boolean;
			fontSize?: number;
			fontFamily?: string;
			colorRgba?: [number, number, number, number];
		},
	): void;

	/** Replay a JSON-encoded transaction payload. */
	replayOperationsJson(operationsJson: string): void;

	/** Set the document title (metadata). */
	setTitle(title: string): void;

	/** Re-render the document to the canvas. */
	render(): void;

	/** Export the document as a JSON string. */
	toJSON(): string;

	/** Export the document as HTML with inline styles and semantic elements. */
	toHTML(): string;

	/** Export the document as Markdown. */
	toMarkdown(): string;

	/** Import HTML content, replacing the current document. */
	fromHTML(html: string): void;

	/** Paste HTML at the current cursor, preserving bold/italic/underline. */
	pasteHTML(html: string): void;

	/** Toggle bold on the current selection. */
	toggleBold(): void;

	/** Toggle italic on the current selection. */
	toggleItalic(): void;

	/** Toggle underline on the current selection. */
	toggleUnderline(): void;

	/** Toggle strikethrough on the current selection. */
	toggleStrikethrough(): void;

	/** Whether the current selection is all bold. */
	readonly isBold: boolean;

	/** Whether the current selection is all italic. */
	readonly isItalic: boolean;

	/** Whether the current selection is all underline. */
	readonly isUnderline: boolean;

	/** Set font size on the current selection. */
	setFontSize(size: number): void;

	/** Set text color (RGBA) on the current selection. */
	setColor(r: number, g: number, b: number, a: number): void;

	/** Select the entire document. */
	selectAll(): void;

	/** Select the word at the given offset. */
	selectWordAt(offset: number): void;

	/** Get the selected text (empty if selection is collapsed). */
	readonly selectedText: string;

	/**
	 * Find all occurrences of `needle`. Returns array of `{start, end}`.
	 */
	findAll(needle: string, caseSensitive?: boolean): Array<{
		start: number;
		end: number;
	}>;

	/**
	 * Find the next occurrence at or after `fromOffset`.
	 */
	findNext(
		needle: string,
		fromOffset: number,
		caseSensitive?: boolean,
	): { start: number; end: number } | null;

	/**
	 * Find the previous occurrence before `fromOffset`.
	 */
	findPrev(
		needle: string,
		fromOffset: number,
		caseSensitive?: boolean,
	): { start: number; end: number } | null;

	/** Replace text in `[start, end)` with `replacement`. */
	replaceRange(start: number, end: number, replacement: string): void;

	/** Replace all occurrences of `needle` with `replacement`. Returns count. */
	replaceAll(
		needle: string,
		replacement: string,
		caseSensitive?: boolean,
	): number;

	// ── Scroll ─────────────────────────────────────────────────────

	/** Current vertical scroll offset in logical pixels. */
	readonly scrollY: number;

	/** Set the vertical scroll offset (clamped to valid range). */
	setScrollY(y: number): void;

	/** Scroll by a delta (positive = down, negative = up). */
	scrollBy(deltaY: number): void;

	/** Total content height in logical pixels (computed from layout). */
	readonly contentHeight: number;

	/** Y position and height of the caret line: `[y, height]`. */
	readonly caretY: [number, number];

	// ── Focus ───────────────────────────────────────────────────────

	/** Whether the editor currently has focus. */
	readonly focused: boolean;

	/** Set the focus state (affects caret/selection rendering). */
	setFocused(focused: boolean): void;

	// ── Statistics ──────────────────────────────────────────────────

	/** Number of words (whitespace-separated) in the document. */
	readonly wordCount: number;

	/** Number of visual lines (computed from layout). */
	readonly lineCount: number;

	/** 1-based visual line number the caret is on. */
	readonly cursorLine: number;

	/** 1-based column (character position) within the visual line. */
	readonly cursorColumn: number;

	// ── Size ────────────────────────────────────────────────────────

	/** Set the logical (CSS) dimensions. */
	setSize(width: number, height: number): void;

	// ── Read-only ───────────────────────────────────────────────────

	/** Whether the editor is in read-only mode. */
	readonly readOnly: boolean;

	/** Enable or disable read-only mode. */
	setReadOnly(readOnly: boolean): void;

	// ── Line numbers ────────────────────────────────────────────────

	/** Whether line numbers are visible. */
	readonly showLineNumbers: boolean;

	/** Enable or disable the line-number gutter. */
	setShowLineNumbers(show: boolean): void;

	// ── Indentation ─────────────────────────────────────────────────

	/** Indent the current selection (insert tab at start of each line). */
	indentSelection(): void;

	/** Outdent the current selection (remove leading tab/spaces). */
	outdentSelection(): void;

	// ── Theme ───────────────────────────────────────────────────────

	/** Switch to dark theme. */
	setThemeDark(): void;

	/** Switch to light theme. */
	setThemeLight(): void;

	/** Current theme name: `"dark"` or `"light"`. */
	readonly themeName: string;

	// ── Zoom ────────────────────────────────────────────────────────

	/** Current zoom level (1.0 = 100%). */
	readonly zoom: number;

	/** Set zoom level (clamped to [0.25, 4.0]). */
	setZoom(level: number): void;

	/** Zoom in by one step. */
	zoomIn(): void;

	/** Zoom out by one step. */
	zoomOut(): void;

	/** Reset zoom to 100%. */
	zoomReset(): void;

	// ── Current line highlight ──────────────────────────────────────

	/** Whether the current-line highlight is enabled. */
	readonly highlightCurrentLine: boolean;

	/** Enable or disable current-line highlight. */
	setHighlightCurrentLine(enabled: boolean): void;

	// ── Drag and drop ───────────────────────────────────────────────

	/** Move text from [srcStart, srcEnd) to destOffset. */
	moveText(srcStart: number, srcEnd: number, destOffset: number): void;

	// ── Word wrap ───────────────────────────────────────────────────

	/** Whether word wrapping is enabled. */
	readonly wordWrap: boolean;

	/** Enable or disable word wrapping. */
	setWordWrap(enabled: boolean): void;

	// ── Lists ───────────────────────────────────────────────────────

	/** Toggle bullet list prefix (• ) on the current line. */
	toggleBulletList(): void;

	/** Toggle numbered list prefix (1. ) on the current line. */
	toggleNumberedList(): void;

	// ── Auto-indent ─────────────────────────────────────────────────

	/** Insert newline with auto-indent and list continuation. Returns chars inserted. */
	autoIndentNewline(): number;

	// ── Selection statistics ────────────────────────────────────────

	/** Number of characters currently selected. */
	readonly selectedCharCount: number;

	/** Number of words currently selected. */
	readonly selectedWordCount: number;

	// ── Go to line ──────────────────────────────────────────────────

	/** Move cursor to the start of the given 1-based line number. */
	goToLine(lineNumber: number): void;

	// ── Line operations ─────────────────────────────────────────────

	/** Duplicate the current line below. */
	duplicateLine(): void;

	/** Move the current line up. */
	moveLineUp(): void;

	/** Move the current line down. */
	moveLineDown(): void;

	// ── Highlight colour ────────────────────────────────────────────

	/** Set a background highlight colour on the current selection. */
	setHighlightColor(r: number, g: number, b: number, a: number): void;

	/** Remove the background highlight from the current selection. */
	removeHighlightColor(): void;

	// ── Delete / join line ───────────────────────────────────────────

	/** Delete the entire current line (Ctrl+Shift+K). */
	deleteLine(): void;

	/** Join the current line with the line below (Ctrl+J). */
	joinLines(): void;

	// ── Transform case ──────────────────────────────────────────────

	/** Convert selected text to UPPERCASE. */
	transformUppercase(): void;

	/** Convert selected text to lowercase. */
	transformLowercase(): void;

	/** Convert selected text to Title Case. */
	transformTitleCase(): void;

	// ── Sort lines ──────────────────────────────────────────────────

	/** Sort selected lines ascending. */
	sortLinesAsc(): void;

	/** Sort selected lines descending. */
	sortLinesDesc(): void;

	// ── Show whitespace ─────────────────────────────────────────────

	/** Whether whitespace indicators are visible. */
	readonly showWhitespace: boolean;

	/** Toggle whitespace visualization. */
	setShowWhitespace(show: boolean): void;

	// ── Bracket auto-close ──────────────────────────────────────────

	/** Whether bracket auto-closing is enabled. */
	readonly autoCloseBrackets: boolean;

	/** Toggle bracket auto-closing. */
	setAutoCloseBrackets(enabled: boolean): void;

	// ── Delete word ─────────────────────────────────────────────────

	/** Delete the word to the left of the cursor (Ctrl+Backspace). */
	deleteWordLeft(): void;

	/** Delete the word to the right of the cursor (Ctrl+Delete). */
	deleteWordRight(): void;

	// ── Select line ─────────────────────────────────────────────────

	/** Select the entire current line (Ctrl+L). */
	selectLine(): void;

	// ── Utility commands ────────────────────────────────────────────

	/** Remove trailing whitespace from every line. Returns chars removed. */
	trimTrailingWhitespace(): number;

	/** Remove consecutive duplicate lines. Returns lines removed. */
	removeDuplicateLines(): number;

	/** Wrap selected text with open/close strings (e.g. brackets). */
	wrapSelection(open: string, close: string): void;

	/** Destroy the editor and release WASM resources. */
	destroy(): void;
}
