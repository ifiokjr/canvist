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

	// ── Transpose characters ────────────────────────────────────────

	/** Swap the two characters around the cursor (Ctrl+T). */
	transposeChars(): void;

	// ── Toggle line comment ─────────────────────────────────────────

	/** Get the line comment prefix. */
	readonly commentPrefix: string;

	/** Set the line comment prefix (default `"// "`). */
	setCommentPrefix(prefix: string): void;

	/** Toggle line-comment on current/selected lines. */
	toggleLineComment(): void;

	// ── Soft tabs ───────────────────────────────────────────────────

	/** Current tab size (1–8). */
	readonly tabSize: number;

	/** Set tab size in spaces. */
	setTabSize(size: number): void;

	/** Whether soft tabs (spaces) are used. */
	readonly softTabs: boolean;

	/** Enable or disable soft tabs. */
	setSoftTabs(enabled: boolean): void;

	/** Insert a tab (spaces or `\t` depending on soft tabs). */
	insertTab(): void;

	// ── Auto-surround ───────────────────────────────────────────────

	/** Whether auto-surround is enabled. */
	readonly autoSurround: boolean;

	/** Enable or disable auto-surround on selection. */
	setAutoSurround(enabled: boolean): void;

	// ── Expand / contract selection ─────────────────────────────────

	/** Expand selection: word → quoted → bracketed → line → all. */
	expandSelection(): void;

	/** Contract selection: all → line → bracket → word → collapsed. */
	contractSelection(): void;

	// ── Matching bracket highlight ──────────────────────────────────

	/** Whether matching bracket highlighting is enabled. */
	readonly highlightMatchingBrackets: boolean;

	/** Toggle matching bracket highlighting. */
	setHighlightMatchingBrackets(enabled: boolean): void;

	/** Find the offset of the bracket matching the one at `offset`. Returns -1 if none. */
	findMatchingBracket(offset: number): number;

	// ── Move to matching bracket ────────────────────────────────────

	/** Move cursor to the matching bracket (Ctrl+Shift+\). */
	moveToMatchingBracket(): void;

	// ── Document statistics (extras) ────────────────────────────────

	/** Total paragraph count (non-empty lines). */
	readonly paragraphCount: number;

	/** Current cursor line number (1-based). */
	readonly currentLineNumber: number;

	/** Current cursor column (1-based). */
	readonly currentColumn: number;

	// ── Indent guides ───────────────────────────────────────────────

	/** Whether indent guides are visible. */
	readonly showIndentGuides: boolean;

	/** Toggle indent guide rendering. */
	setShowIndentGuides(show: boolean): void;

	// ── Bookmarks ───────────────────────────────────────────────────

	/** Toggle bookmark on current line. Returns true if added. */
	toggleBookmark(): boolean;

	/** Jump to the next bookmark. Returns true if found. */
	nextBookmark(): boolean;

	/** Jump to the previous bookmark. Returns true if found. */
	prevBookmark(): boolean;

	/** Remove all bookmarks. */
	clearBookmarks(): void;

	/** Number of active bookmarks. */
	readonly bookmarkCount: number;

	/** Whether current line has a bookmark. */
	readonly isLineBookmarked: boolean;

	// ── Convert indentation ─────────────────────────────────────────

	/** Convert all tabs to spaces. Returns tabs replaced. */
	tabsToSpaces(): number;

	/** Convert leading spaces to tabs. Returns conversions made. */
	spacesToTabs(): number;

	// ── Open line above / below ─────────────────────────────────────

	/** Insert new line below current and move cursor there (Ctrl+Enter). */
	openLineBelow(): void;

	/** Insert new line above current and move cursor there (Ctrl+Shift+Enter). */
	openLineAbove(): void;

	// ── Copy / cut line (no selection) ──────────────────────────────

	/** Get the full text of the current line (including trailing \n). */
	readonly currentLineText: string;

	/** Cut the current line — removes it and returns its text. */
	cutLine(): string;

	// ── Overwrite mode ──────────────────────────────────────────────

	/** Whether the editor is in overwrite (replace) mode. */
	readonly overwriteMode: boolean;

	/** Toggle insert/overwrite mode. */
	toggleOverwriteMode(): void;

	/** Set overwrite mode explicitly. */
	setOverwriteMode(enabled: boolean): void;

	// ── Center line ─────────────────────────────────────────────────

	/** Scroll so the cursor's line is vertically centered. */
	centerLineInViewport(): void;

	// ── Document start / end ────────────────────────────────────────

	/** Move cursor to document start (Ctrl+Home). */
	goToDocumentStart(): void;

	/** Move cursor to document end (Ctrl+End). */
	goToDocumentEnd(): void;

	/** Select from cursor to document start (Ctrl+Shift+Home). */
	selectToDocumentStart(): void;

	/** Select from cursor to document end (Ctrl+Shift+End). */
	selectToDocumentEnd(): void;

	// ── Select between brackets ─────────────────────────────────────

	/** Select text between nearest enclosing bracket pair. Returns true if found. */
	selectBetweenBrackets(): boolean;

	// ── Cursor position history ────────────────────────────────────

	/** Record current cursor position in history stack. */
	pushCursorHistory(): void;

	/** Navigate backward in cursor history. Returns true if moved. */
	cursorHistoryBack(): boolean;

	/** Navigate forward in cursor history. Returns true if moved. */
	cursorHistoryForward(): boolean;

	/** Number of positions in cursor history. */
	readonly cursorHistoryLength: number;

	// ── Select all occurrences ──────────────────────────────────────

	/** Count of all occurrences of the current selection. */
	selectAllOccurrences(): number;

	/** Return all occurrence offsets as [start0, end0, start1, end1, ...]. */
	occurrenceOffsets(): number[];

	// ── Whole word find ─────────────────────────────────────────────

	/** Find all whole-word occurrences. Returns [start0, end0, ...]. */
	findAllWholeWord(needle: string): number[];

	// ── Paragraph navigation ────────────────────────────────────────

	/** Move cursor to the start of the previous paragraph (Ctrl+↑). */
	moveToPrevParagraph(): void;

	/** Move cursor to the start of the next paragraph (Ctrl+↓). */
	moveToNextParagraph(): void;

	// ── Snippet insertion ───────────────────────────────────────────

	/** Insert a snippet template. `$0` marks cursor position. */
	insertSnippet(template: string): void;

	// ── Scroll to selection ─────────────────────────────────────────

	/** Ensure the selection is visible in the viewport. */
	scrollToSelection(): void;

	// ── Column ruler ───────────────────────────────────────────────

	/** Set column ruler positions (e.g. [80, 120]). */
	setRulers(columns: number[]): void;

	/** Get current ruler columns. */
	readonly rulers: number[];

	/** Add a ruler at the given column. */
	addRuler(column: number): void;

	/** Remove the ruler at the given column. */
	removeRuler(column: number): void;

	// ── Ensure final newline ────────────────────────────────────────

	/** Ensure document ends with \n. Returns true if added. */
	ensureFinalNewline(): boolean;

	// ── Replace all occurrences ─────────────────────────────────────

	/** Replace all occurrences of selected text with replacement. Returns count. */
	replaceAllOccurrences(replacement: string): number;

	// ── Reverse lines ───────────────────────────────────────────────

	/** Reverse the order of selected lines. */
	reverseLines(): void;

	// ── Encode / decode ─────────────────────────────────────────────

	/** Base64-encode the selected text. */
	base64EncodeSelection(): void;

	/** Base64-decode the selected text. */
	base64DecodeSelection(): void;

	/** URL-encode the selected text. */
	urlEncodeSelection(): void;

	/** URL-decode the selected text. */
	urlDecodeSelection(): void;

	// ── Toggle case ─────────────────────────────────────────────────

	/** Swap case of each character in selection (a↔A). */
	transformToggleCase(): void;

	// ── Line decorations ───────────────────────────────────────────

	/** Add a coloured background decoration to a line (0-based). */
	addLineDecoration(
		line: number,
		r: number,
		g: number,
		b: number,
		a: number,
	): void;

	/** Remove all decorations from a specific line. */
	removeLineDecorations(line: number): void;

	/** Remove all line decorations. */
	clearLineDecorations(): void;

	/** Number of active line decorations. */
	readonly lineDecorationCount: number;

	// ── Modified state ──────────────────────────────────────────────

	/** Whether the document has been modified since last save. */
	readonly isModified: boolean;

	/** Mark the document as saved (clears modified flag). */
	markSaved(): void;

	/** Mark the document as modified. */
	markModified(): void;

	// ── Clipboard ring ──────────────────────────────────────────────

	/** Push text into the clipboard ring. */
	clipboardRingPush(text: string): void;

	/** Get clipboard ring entry at index (0 = most recent). */
	clipboardRingGet(index: number): string;

	/** Number of entries in the clipboard ring. */
	readonly clipboardRingLength: number;

	/** Clear the clipboard ring. */
	clipboardRingClear(): void;

	/** Paste clipboard ring entry at index at cursor. */
	clipboardRingPaste(index: number): void;

	// ── Word frequency ──────────────────────────────────────────────

	/** Return top N words as [word, count, word, count, ...]. */
	wordFrequency(topN: number): string[];

	// ── Highlight occurrences ───────────────────────────────────────

	/** Whether occurrence highlighting is enabled. */
	readonly highlightOccurrences: boolean;

	/** Toggle occurrence highlighting. */
	setHighlightOccurrences(enabled: boolean): void;

	/** Get the word under/adjacent to the cursor. */
	wordAtCursor(): string;

	// ── Text measurement ────────────────────────────────────────────

	/** Measure pixel width of a string using default style. */
	measureTextWidth(text: string): number;

	/** Measure pixel width of a single character. */
	measureCharWidth(ch: string): number;

	// ── State serialization ────────────────────────────────────────

	/** Serialize editor state to JSON (text, selection, scroll, settings). */
	saveState(): string;

	/** Restore editor state from JSON. */
	restoreState(json: string): void;

	// ── Placeholder text ────────────────────────────────────────────

	/** Get placeholder text. */
	readonly placeholder: string;

	/** Set placeholder text shown when document is empty. */
	setPlaceholder(text: string): void;

	// ── Max length ──────────────────────────────────────────────────

	/** Get max character count (0 = unlimited). */
	readonly maxLength: number;

	/** Set max character count. */
	setMaxLength(max: number): void;

	/** How many more characters can be inserted. */
	readonly remainingCapacity: number;

	/** Insert text respecting max length. Returns chars inserted. */
	insertTextClamped(text: string): number;

	// ── Batch operations ────────────────────────────────────────────

	/** Begin a batch of edits (for undo grouping). */
	beginBatch(): void;

	/** End a batch of edits. */
	endBatch(): void;

	// ── Regex find ──────────────────────────────────────────────────

	/** Case-insensitive find. Returns [start0, end0, ...]. */
	findAllRegex(pattern: string): number[];

	// ── Selection change detection ──────────────────────────────────

	/** Returns true if selection moved since last check. */
	selectionChanged(): boolean;

	// ── Wrap indicators ────────────────────────────────────────────

	/** Whether wrap continuation indicators are shown. */
	readonly showWrapIndicators: boolean;

	/** Toggle wrap continuation indicators (↪ in gutter). */
	setShowWrapIndicators(enabled: boolean): void;

	// ── Selection anchor ────────────────────────────────────────────

	/** Get selection anchor (start) offset. */
	readonly selectionAnchor: number;

	/** Whether the selection is collapsed (cursor, no range). */
	readonly selectionIsCollapsed: boolean;

	/** Character length of the current selection. */
	readonly selectionLength: number;

	// ── Character counts ────────────────────────────────────────────

	/** Count chars by type: [letters, digits, spaces, punctuation, other]. */
	charCounts(): number[];

	// ── Text hash ───────────────────────────────────────────────────

	/** FNV-1a 64-bit hash of document text (hex string). */
	textHash(): string;

	// ── Event log ───────────────────────────────────────────────────

	/** Log an editor event. */
	logEvent(event: string): void;

	/** Get event log entry at index (0 = newest). */
	eventLogGet(index: number): string;

	/** Number of entries in the event log. */
	readonly eventLogLength: number;

	/** Clear the event log. */
	eventLogClear(): void;

	/** Set max event log entries. */
	setEventLogMax(max: number): void;

	/** Destroy the editor and release WASM resources. */
	destroy(): void;
}
