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

	// ── Word completion ────────────────────────────────────────────

	/** Suggest completions from document vocabulary. */
	completions(maxResults: number): string[];

	// ── Line range operations ───────────────────────────────────────

	/** Get text for a range of lines (0-based, start inclusive, end exclusive). */
	getLineRange(startLine: number, endLine: number): string;

	/** Replace text for a range of lines. */
	setLineRange(startLine: number, endLine: number, text: string): void;

	/** Total number of lines. */
	readonly lineCountTotal: number;

	/** Get text of a single line (0-based). */
	getLine(line: number): string;

	// ── Scroll metrics ──────────────────────────────────────────────

	/** Viewport height in pixels. */
	readonly viewportHeight: number;

	/** Ratio of viewport to content (0–1, 1 = all visible). */
	readonly scrollRatio: number;

	/** Scroll position as fraction (0 = top, 1 = bottom). */
	readonly scrollFraction: number;

	/** Scroll to a fraction of the document. */
	scrollToFraction(fraction: number): void;

	// ── Annotations ─────────────────────────────────────────────────

	/** Add an annotation to a range (kind: error/warning/info/spelling). */
	addAnnotation(
		start: number,
		end: number,
		kind: string,
		message: string,
	): void;

	/** Remove all annotations of a kind. */
	removeAnnotationsByKind(kind: string): void;

	/** Clear all annotations. */
	clearAnnotations(): void;

	/** Number of annotations. */
	readonly annotationCount: number;

	/** All annotations as [start, end, kind, message, ...]. */
	getAnnotations(): string[];

	/** Annotations overlapping an offset. */
	annotationsAt(offset: number): string[];

	// ── Search history ──────────────────────────────────────────────

	/** Push a search term into history. */
	searchHistoryPush(term: string): void;

	/** Get search history entry (0 = newest). */
	searchHistoryGet(index: number): string;

	/** Search history length. */
	readonly searchHistoryLength: number;

	/** Clear search history. */
	searchHistoryClear(): void;

	// ── Visible range ───────────────────────────────────────────────

	/** First visible line (0-based). */
	readonly firstVisibleLine: number;

	/** Last visible line (0-based). */
	readonly lastVisibleLine: number;

	/** Number of visible lines in viewport. */
	readonly visibleLineCount: number;

	// ── Minimap ─────────────────────────────────────────────────────

	/** Whether the minimap is shown. */
	readonly showMinimap: boolean;

	/** Toggle the minimap sidebar. */
	setShowMinimap(enabled: boolean): void;

	/** Minimap width in pixels. */
	readonly minimapWidth: number;

	/** Set minimap width (30–200). */
	setMinimapWidth(w: number): void;

	// ── Sticky scroll ───────────────────────────────────────────────

	/** Whether sticky scroll is enabled. */
	readonly stickyScroll: boolean;

	/** Toggle sticky scroll. */
	setStickyScroll(enabled: boolean): void;

	// ── Rename all ──────────────────────────────────────────────────

	/** Rename all whole-word occurrences of word under cursor. Returns count. */
	renameAll(newName: string): number;

	// ── Cursor style ────────────────────────────────────────────────

	/** Get cursor style (0=line, 1=block, 2=underline). */
	readonly cursorStyle: number;

	/** Set cursor style. */
	setCursorStyle(style: number): void;

	/** Get cursor width in pixels. */
	readonly cursorWidthPx: number;

	/** Set cursor width. */
	setCursorWidth(w: number): void;

	/** Set cursor colour (pass a=0 to reset to theme). */
	setCursorColor(r: number, g: number, b: number, a: number): void;

	// ── Snapshot diff ───────────────────────────────────────────────

	/** Take a snapshot of current text. */
	takeSnapshot(): void;

	/** Compare current text vs snapshot; returns changed line numbers. */
	diffFromSnapshot(): number[];

	/** Whether a snapshot exists. */
	readonly hasSnapshot: boolean;

	/** Clear the snapshot. */
	clearSnapshot(): void;

	// ── Macro recording ─────────────────────────────────────────────

	/** Start recording a macro. */
	macroStartRecording(): void;

	/** Stop recording. Returns number of steps. */
	macroStopRecording(): number;

	/** Whether recording is active. */
	readonly macroIsRecording: boolean;

	/** Record a step manually (kind: insert/delete/select). */
	macroRecordStep(kind: string, data: string): void;

	/** Number of recorded steps. */
	readonly macroStepCount: number;

	/** Replay the recorded macro. */
	macroReplay(): void;

	/** Save the recorded macro under a name. */
	macroSave(name: string): void;

	/** Replay a saved macro by name. Returns false if not found. */
	macroReplaySaved(name: string): boolean;

	/** List saved macro names. */
	macroListSaved(): string[];

	/** Delete a saved macro. */
	macroDeleteSaved(name: string): void;

	// ── Find match highlights ───────────────────────────────────────

	/** Set needle for visual find highlights. Empty = clear. */
	setFindHighlights(needle: string): void;

	/** Current find highlight needle. */
	readonly findHighlightNeedle: string;

	/** Whether find highlights are active. */
	readonly showFindHighlights: boolean;

	// ── Column/block selection ───────────────────────────────────────

	/** Get text from a rectangular block. */
	getBlockSelection(
		startLine: number,
		endLine: number,
		startCol: number,
		endCol: number,
	): string;

	/** Replace text in a rectangular block. */
	setBlockSelection(
		startLine: number,
		endLine: number,
		startCol: number,
		endCol: number,
		text: string,
	): void;

	// ── Smart paste ─────────────────────────────────────────────────

	/** Paste with auto-adjusted indentation. */
	pasteWithIndent(text: string): void;

	// ── Tokenize ────────────────────────────────────────────────────

	/** Tokenize document: [kind, text, kind, text, ...]. */
	tokenize(): string[];

	// ── Link detection ──────────────────────────────────────────────

	/** Toggle URL link detection. */
	setDetectLinks(enabled: boolean): void;

	/** Whether link detection is enabled. */
	readonly detectLinks: boolean;

	/** Find all URLs: [start, end, start, end, ...]. */
	findLinks(): number[];

	/** Get URL text at offset, or empty string. */
	linkAtOffset(offset: number): string;

	// ── Line folding ────────────────────────────────────────────────

	/** Fold a range of lines (0-based, inclusive). */
	foldLines(startLine: number, endLine: number): void;

	/** Unfold a specific range. */
	unfoldLines(startLine: number, endLine: number): void;

	/** Unfold all ranges. */
	unfoldAll(): void;

	/** Number of fold regions. */
	readonly foldCount: number;

	/** Whether a line is hidden inside a fold. */
	isLineFolded(line: number): boolean;

	/** Toggle fold at line (indent-based auto-detection). */
	toggleFoldAt(line: number): void;

	/** All folded ranges: [start, end, ...]. */
	foldedRanges(): number[];

	// ── Gutter click ────────────────────────────────────────────────

	/** Get line number at Y coordinate (-1 if outside). */
	lineAtY(y: number): number;

	// ── Configuration presets ───────────────────────────────────────

	/** Apply preset: "code", "prose", or "minimal". */
	applyPreset(name: string): void;

	// ── Content statistics ──────────────────────────────────────────

	/** Estimated reading time in seconds. */
	readonly readingTimeSeconds: number;

	/** Flesch reading ease score (0–100). */
	readonly fleschReadingEase: number;

	// ── Syntax highlighting ─────────────────────────────────────────

	/** Toggle syntax highlighting. */
	setSyntaxHighlight(enabled: boolean): void;

	/** Whether syntax highlighting is on. */
	readonly syntaxHighlight: boolean;

	/** Set colour for a token kind. */
	setTokenColor(kind: string, r: number, g: number, b: number, a: number): void;

	/** Get colour for a token kind: [r, g, b, a]. */
	getTokenColor(kind: string): number[];

	/** Reset all token colours to defaults. */
	resetTokenColors(): void;

	// ── Custom theme API ────────────────────────────────────────────

	/** Set a theme colour slot (background, text, caret, selection, etc). */
	setThemeColor(slot: string, r: number, g: number, b: number, a: number): void;

	/** Get a theme colour slot: [r, g, b, a]. */
	getThemeColor(slot: string): number[];

	// ── Range formatting ────────────────────────────────────────────

	/** Apply bold to a char range. */
	formatRangeBold(start: number, end: number): void;

	/** Apply italic to a char range. */
	formatRangeItalic(start: number, end: number): void;

	/** Apply underline to a char range. */
	formatRangeUnderline(start: number, end: number): void;

	/** Apply strikethrough to a char range. */
	formatRangeStrikethrough(start: number, end: number): void;

	/** Set font size for a char range. */
	formatRangeFontSize(start: number, end: number, size: number): void;

	// ── Scroll to line ──────────────────────────────────────────────

	/** Scroll viewport to show a specific line. */
	scrollToLine(line: number): void;

	// ── Extended statistics ──────────────────────────────────────────

	/** Average word length in characters. */
	readonly avgWordLength: number;

	/** The longest word in the document. */
	longestWord(): string;

	/** Count of unique words (case-insensitive). */
	readonly uniqueWordCount: number;

	/** Sentence count. */
	readonly sentenceCount: number;

	// ── Editor info ─────────────────────────────────────────────────

	/** Editor version string. */
	readonly editorVersion: string;

	/** Total API method count. */
	readonly apiCount: number;

	/** Feature categories as comma-separated string. */
	featureCategories(): string;

	// ── Multi-cursor ────────────────────────────────────────────────

	/** Add an extra cursor at offset. */
	addCursor(offset: number): void;

	/** Remove extra cursor at offset. */
	removeCursor(offset: number): void;

	/** Clear all extra cursors. */
	clearCursors(): void;

	/** Number of extra cursors. */
	readonly extraCursorCount: number;

	/** All extra cursor offsets. */
	extraCursorOffsets(): number[];

	/** Insert text at all cursors. Returns number of insertions. */
	multiCursorInsert(text: string): number;

	// ── Breadcrumbs ─────────────────────────────────────────────────

	/** Document breadcrumbs: [lineNumber, text, ...]. */
	breadcrumbs(): string[];

	/** Navigate to a breadcrumb line and scroll to it. */
	goToBreadcrumb(line: number): void;

	// ── Indent level ────────────────────────────────────────────────

	/** Indent level at cursor position. */
	readonly indentLevelAtCursor: number;

	/** Indent level of a specific line. */
	indentLevelOfLine(line: number): number;

	// ── Patch ───────────────────────────────────────────────────────

	/** Apply a patch: ["insert","offset","text","delete","start","end",...]. */
	applyPatch(operations: string[]): void;

	// ── Canvas export ───────────────────────────────────────────────

	/** Export canvas as PNG data URL. */
	exportCanvasDataUrl(): string;

	// ── Command palette ─────────────────────────────────────────────

	/** All commands: [name, keybinding, ...]. */
	commandList(): string[];

	/** Search commands by query. */
	searchCommands(query: string): string[];

	// ── Text diffing ────────────────────────────────────────────────

	/** Compare two texts: [kind, lineNumber, text, ...]. */
	diffTexts(a: string, b: string): string[];

	// ── Bidi info ───────────────────────────────────────────────────

	/** Whether text contains RTL characters. */
	readonly containsRtl: boolean;

	/** Whether text contains non-ASCII characters. */
	readonly containsNonAscii: boolean;

	// ── Selection to lines ──────────────────────────────────────────

	/** Lines covered by selection: [startLine, endLine]. */
	selectionLineRange(): number[];

	/** Select entire line range (inclusive). */
	selectLines(startLine: number, endLine: number): void;

	// ── Whitespace normalization ────────────────────────────────────

	/** Remove \\r characters. Returns count removed. */
	normalizeLineEndings(): number;

	/** Normalize indentation to current tab style. Returns lines modified. */
	normalizeIndentation(): number;

	// ── Document outline ────────────────────────────────────────────

	/** Outline: [indent, lineNumber, text, ...]. */
	documentOutline(): string[];

	// ── Collaborative cursors ───────────────────────────────────────

	/** Add a collaborator cursor with name and colour. */
	addCollabCursor(
		offset: number,
		name: string,
		r: number,
		g: number,
		b: number,
	): void;

	/** Update a collaborator cursor position. */
	updateCollabCursor(name: string, offset: number): void;

	/** Remove a collaborator cursor. */
	removeCollabCursor(name: string): void;

	/** Clear all collaborator cursors. */
	clearCollabCursors(): void;

	/** Number of collaborator cursors. */
	readonly collabCursorCount: number;

	/** All cursors: [offset, name, r, g, b, ...]. */
	collabCursorList(): string[];

	// ── Line ending ─────────────────────────────────────────────────

	/** Detect line ending style: "lf", "crlf", or "mixed". */
	detectLineEnding(): string;

	/** Convert all endings to CRLF. Returns count. */
	convertToCrlf(): number;

	/** Convert all endings to LF. Returns count. */
	convertToLf(): number;

	// ── File type ───────────────────────────────────────────────────

	/** Guess file type from content. */
	detectFileType(): string;

	// ── Emmet ───────────────────────────────────────────────────────

	/** Expand Emmet abbreviation at cursor. Returns true if expanded. */
	expandEmmet(): boolean;

	// ── Selection history ───────────────────────────────────────────

	/** Push current selection onto history. */
	pushSelectionHistory(): void;

	/** Go back in selection history. */
	selectionHistoryBack(): boolean;

	/** Go forward in selection history. */
	selectionHistoryForward(): boolean;

	/** Selection history length. */
	readonly selectionHistoryLength: number;

	// ── Focus ───────────────────────────────────────────────────────

	/** Whether the editor is focused. */
	readonly isFocused: boolean;

	// ── Custom keybindings ──────────────────────────────────────────

	/** Set a custom keybinding override: shortcut -> command. */
	setKeybinding(shortcut: string, command: string): void;

	/** Remove a custom keybinding override. */
	removeKeybinding(shortcut: string): void;

	/** Clear all custom keybinding overrides. */
	clearKeybindings(): void;

	/** Get command for shortcut (override or default). */
	getKeybinding(shortcut: string): string;

	/** Number of custom keybinding overrides. */
	readonly keybindingOverrideCount: number;

	/** Overrides as [shortcut, command, ...]. */
	keybindingOverridesList(): string[];

	/** Execute a command by name. */
	runCommand(command: string): boolean;

	/** Execute command bound to shortcut. */
	runShortcut(shortcut: string): boolean;

	// ── Text transform pipeline ─────────────────────────────────────

	/** Transform selection to camelCase. */
	transformCamelCase(): void;

	/** Transform selection to snake_case. */
	transformSnakeCase(): void;

	/** Transform selection to kebab-case. */
	transformKebabCase(): void;

	/** Transform selection to CONSTANT_CASE. */
	transformConstantCase(): void;

	/** Apply `|`-separated transform steps (upper|snake|...). */
	transformPipeline(pipeline: string): void;

	// ── Marker ranges ───────────────────────────────────────────────

	/** Add a marker highlight range with explicit RGBA and id. */
	addMarker(
		start: number,
		end: number,
		r: number,
		g: number,
		b: number,
		a: number,
		id: string,
	): void;

	/** Remove marker by id. */
	removeMarker(id: string): void;

	/** Remove markers whose ids start with prefix. */
	removeMarkersByPrefix(prefix: string): void;

	/** Clear all marker ranges. */
	clearMarkers(): void;

	/** Number of markers. */
	readonly markerCount: number;

	/** Marker list: [start, end, r, g, b, a, id, ...]. */
	markerList(): string[];

	/** Markers overlapping offset in same flat format. */
	markersAt(offset: number): string[];

	// ── Soft wrap info ──────────────────────────────────────────────

	/** Number of visual lines after wrapping. */
	readonly visualLineCount: number;

	/** Whether a logical line is wrapped. */
	isLineWrapped(line: number): boolean;

	// ── Extended stats ───────────────────────────────────────────────

	/** Paragraph blocks separated by blank lines. */
	readonly paragraphBlockCount: number;

	/** Average line length in characters. */
	readonly avgLineLength: number;

	/** Longest line length in characters. */
	readonly longestLineLength: number;

	/** 0-based line number of longest line. */
	readonly longestLineNumber: number;

	/** Total UTF-8 byte count. */
	readonly byteCount: number;

	// ── Completion context ───────────────────────────────────────────

	/** Context completions: [word, lineContext, ...]. */
	completionsWithContext(limit: number): string[];

	// ── Named anchors ───────────────────────────────────────────────

	/** Set a named anchor to a character offset. */
	setAnchor(name: string, offset: number): void;

	/** Get anchor offset, or -1 if missing. */
	anchorOffset(name: string): number;

	/** Remove a named anchor. */
	removeAnchor(name: string): void;

	/** Clear all anchors. */
	clearAnchors(): void;

	/** Number of named anchors. */
	readonly anchorCount: number;

	/** All anchor names (sorted). */
	anchorNames(): string[];

	/** Move cursor to anchor. */
	goToAnchor(name: string): boolean;

	/** Whether an anchor exists. */
	anchorExists(name: string): boolean;

	/** Rename an anchor key. */
	renameAnchor(oldName: string, newName: string): boolean;

	/** Nearest anchor at/before offset: [name, offset] or empty. */
	nearestAnchorBefore(offset: number): string[];

	/** Nearest anchor at/after offset: [name, offset] or empty. */
	nearestAnchorAfter(offset: number): string[];

	/** Anchor names exactly at offset (sorted). */
	anchorsAtOffset(offset: number): string[];

	/** Anchors in inclusive range as [name, offset, ...]. */
	anchorsInRange(startOffset: number, endOffset: number): string[];

	/** Shift a named anchor by signed delta, clamped to doc bounds. */
	shiftAnchor(name: string, delta: number): boolean;

	// ── Tasks / TODO scanner ────────────────────────────────────────

	/** Scan tasks: [line, kind, checked, text, ...]. */
	scanTasks(): string[];

	/** Number of task lines. */
	readonly taskCount: number;

	/** Next task line after fromLine, wraps, -1 if none. */
	nextTaskLine(fromLine: number): number;

	/** Previous task line before fromLine, wraps, -1 if none. */
	prevTaskLine(fromLine: number): number;

	/** Toggle markdown task checkbox on line. */
	toggleTaskCheckbox(line: number): boolean;

	// ── Lint helpers ────────────────────────────────────────────────

	/** Lines with trailing whitespace. */
	lintTrailingWhitespace(): number[];

	/** Lines longer than maxLen chars. */
	lintLongLines(maxLen: number): number[];

	/** Lines with mixed tabs/spaces in leading indent. */
	lintMixedIndentation(): number[];

	/** Lines containing non-ASCII chars. */
	lintNonAsciiLines(): number[];

	// ── Line occurrence navigation ──────────────────────────────────

	/** Lines containing needle. */
	lineOccurrences(needle: string, caseSensitive?: boolean): number[];

	/** Number of matching lines. */
	lineOccurrenceCount(needle: string, caseSensitive?: boolean): number;

	/** Next matching line after fromLine, wraps, -1 if none. */
	nextLineWith(
		needle: string,
		fromLine: number,
		caseSensitive?: boolean,
	): number;

	/** Previous matching line before fromLine, wraps, -1 if none. */
	prevLineWith(
		needle: string,
		fromLine: number,
		caseSensitive?: boolean,
	): number;

	// ── Cursor context helpers ──────────────────────────────────────

	/** Text before cursor, up to maxChars. */
	textBeforeCursor(maxChars: number): string;

	/** Text after cursor, up to maxChars. */
	textAfterCursor(maxChars: number): string;

	/** Line context window: [lineNumber, text, ...]. */
	lineContext(line: number, radius: number): string[];

	// ── Rotate lines ────────────────────────────────────────────────

	/** Rotate line range up by one. */
	rotateLinesUp(startLine: number, endLine: number): boolean;

	/** Rotate line range down by one. */
	rotateLinesDown(startLine: number, endLine: number): boolean;

	// ── Named state slots ───────────────────────────────────────────

	/** Save full editor state under a name. */
	saveNamedState(name: string): void;

	/** Load a named editor state. */
	loadNamedState(name: string): boolean;

	/** Delete a named state. */
	deleteNamedState(name: string): void;

	/** Clear all named states. */
	clearNamedStates(): void;

	/** Number of saved named states. */
	readonly namedStateCount: number;

	/** Named state keys (sorted). */
	namedStateNames(): string[];

	// ── Selection profiles ──────────────────────────────────────────

	/** Save current selection under a profile name. */
	saveSelectionProfile(name: string): void;

	/** Load a named selection profile. */
	loadSelectionProfile(name: string): boolean;

	/** Delete a named selection profile. */
	deleteSelectionProfile(name: string): void;

	/** Clear all selection profiles. */
	clearSelectionProfiles(): void;

	/** Number of selection profiles. */
	readonly selectionProfileCount: number;

	/** Selection profile names (sorted). */
	selectionProfileNames(): string[];

	// ── Task workflow helpers ───────────────────────────────────────

	/** Task progress: [checked, total]. */
	taskProgress(): number[];

	/** Insert markdown task line at current line start. */
	insertTaskLine(text: string, checked?: boolean): void;

	/** Next unchecked task line after fromLine, wraps. */
	nextUncheckedTaskLine(fromLine: number): number;

	/** Previous unchecked task line before fromLine, wraps. */
	prevUncheckedTaskLine(fromLine: number): number;

	/** Mark all unchecked tasks as checked. Returns line count. */
	completeAllTasks(): number;

	/** Remove completed markdown task lines. Returns removed count. */
	clearCompletedTasks(): number;

	// ── Cleanup utilities ───────────────────────────────────────────

	/** Trim leading spaces/tabs on each line. Returns line count changed. */
	trimLeadingWhitespace(): number;

	/** Collapse blank runs to at most maxConsecutive lines. Returns removed. */
	collapseBlankLines(maxConsecutive: number): number;

	/** Remove trailing blank lines. Returns removed count. */
	removeTrailingBlankLines(): number;

	/** Ensure document ends with exactly one trailing newline. */
	ensureSingleTrailingNewline(): boolean;

	// ── Line utilities ──────────────────────────────────────────────

	/** Swap two logical lines by index. */
	swapLines(a: number, b: number): boolean;

	/** Duplicate line range and insert below it. */
	duplicateLineRange(startLine: number, endLine: number): boolean;

	/** Prefix each line in range. Returns lines changed. */
	prefixLines(startLine: number, endLine: number, prefix: string): number;

	/** Suffix each line in range. Returns lines changed. */
	suffixLines(startLine: number, endLine: number, suffix: string): number;

	/** Remove prefix from lines in range when present. Returns lines changed. */
	unprefixLines(startLine: number, endLine: number, prefix: string): number;

	/** Remove suffix from lines in range when present. Returns lines changed. */
	unsuffixLines(startLine: number, endLine: number, suffix: string): number;

	/** Number each line in range with N. prefix. Returns lines changed. */
	numberLines(
		startLine: number,
		endLine: number,
		startNumber?: number,
		padWidth?: number,
	): number;

	/** Remove control chars except LF/CR/TAB. Returns chars removed. */
	stripNonPrintable(): number;

	/** Normalize Unicode spaces to ASCII spaces. Returns replacements. */
	normalizeUnicodeWhitespace(): number;

	/** Hash a logical line (FNV-1a 64-bit hex). */
	lineHash(line: number): string;

	/** All line hashes: [line, hash, ...]. */
	lineHashes(): string[];

	/** Compare hashes of two lines. */
	lineHashEquals(a: number, b: number): boolean;

	/** Whether a line is part of a duplicate-content set. */
	lineIsDuplicate(
		line: number,
		caseSensitive?: boolean,
		ignoreWhitespace?: boolean,
	): boolean;

	/** Duplicate line numbers by content matching. */
	duplicateLineNumbers(
		caseSensitive?: boolean,
		ignoreWhitespace?: boolean,
	): number[];

	/** Destroy the editor and release WASM resources. */
	destroy(): void;
}
