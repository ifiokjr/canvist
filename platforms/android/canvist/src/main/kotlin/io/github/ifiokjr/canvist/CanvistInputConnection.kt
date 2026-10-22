package io.github.ifiokjr.canvist

import android.graphics.Matrix
import android.os.Build
import android.os.Bundle
import android.text.InputType
import android.text.TextUtils
import android.view.KeyEvent
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.CompletionInfo
import android.view.inputmethod.CorrectionInfo
import android.view.inputmethod.CursorAnchorInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.view.inputmethod.SurroundingText

/**
 * How input methods edit a [CanvistView]: Gboard's composition and
 * suggestions, autocorrect, voice typing, and hardware keys sent through the
 * input method.
 *
 * Offsets are UTF-16 code units into the editor's text, the document's text
 * blocks joined by line feeds, which is what the framework expects. Every
 * change goes through the editor, so key bindings, input rules, and undo
 * apply as they do for typing.
 */
internal class CanvistInputConnection(
    private val view: CanvistView,
) : BaseInputConnection(view, false) {
    private val editor get() = view.editor

    /** What the input method asked for with [requestCursorUpdates]. */
    private var cursorUpdates = 0

    /** Describe the editor to the input method. */
    fun configure(info: EditorInfo) {
        info.inputType = InputType.TYPE_CLASS_TEXT or
            InputType.TYPE_TEXT_FLAG_MULTI_LINE or
            InputType.TYPE_TEXT_FLAG_CAP_SENTENCES or
            InputType.TYPE_TEXT_FLAG_AUTO_CORRECT
        info.imeOptions = EditorInfo.IME_FLAG_NO_ENTER_ACTION or
            EditorInfo.IME_FLAG_NO_FULLSCREEN or
            EditorInfo.IME_FLAG_NO_EXTRACT_UI

        val selection = editor.selection()
        info.initialSelStart = minOf(selection.anchor, selection.head).toInt()
        info.initialSelEnd = maxOf(selection.anchor, selection.head).toInt()

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val start = maxOf(info.initialSelStart - SURROUNDING, 0)
            info.setInitialSurroundingSubText(text(start, info.initialSelEnd + SURROUNDING), start)
        }
    }

    // Reading
    private fun selectionRange(): OffsetRange {
        val selection = editor.selection()

        return OffsetRange(minOf(selection.anchor, selection.head), maxOf(selection.anchor, selection.head))
    }

    private fun text(
        start: Int,
        end: Int,
    ): String {
        val length = editor.textLength().toInt()

        return editor.textIn(OffsetRange(start.coerceIn(0, length).toUInt(), end.coerceIn(0, length).toUInt()))
    }

    override fun getTextBeforeCursor(
        n: Int,
        flags: Int,
    ): CharSequence {
        val start = selectionRange().start.toInt()

        return text(start - n, start)
    }

    override fun getTextAfterCursor(
        n: Int,
        flags: Int,
    ): CharSequence {
        val end = selectionRange().end.toInt()

        return text(end, end + n)
    }

    override fun getSelectedText(flags: Int): CharSequence? {
        val range = selectionRange()

        return if (range.start == range.end) null else editor.textIn(range)
    }

    override fun getSurroundingText(
        beforeLength: Int,
        afterLength: Int,
        flags: Int,
    ): SurroundingText? = surrounding(beforeLength, afterLength)

    private fun surrounding(
        before: Int,
        after: Int,
    ): SurroundingText? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return null

        val range = selectionRange()
        val start = maxOf(range.start.toInt() - before, 0)
        val text = text(start, range.end.toInt() + after)

        return SurroundingText(text, range.start.toInt() - start, range.end.toInt() - start, start)
    }

    override fun getCursorCapsMode(reqModes: Int): Int {
        val before = getTextBeforeCursor(CAPS_CONTEXT, 0)

        return TextUtils.getCapsMode(before, before.length, reqModes)
    }

    override fun getExtractedText(
        request: ExtractedTextRequest?,
        flags: Int,
    ): ExtractedText {
        val range = selectionRange()

        return ExtractedText().apply {
            text = editor.plainText()
            startOffset = 0
            partialStartOffset = -1
            partialEndOffset = -1
            selectionStart = range.start.toInt()
            selectionEnd = range.end.toInt()
        }
    }

    // Composing
    override fun setComposingText(
        text: CharSequence,
        newCursorPosition: Int,
    ): Boolean =
        edit("setComposingText") {
            val caret = cursorWithin(text.length, newCursorPosition).toUInt()
            editor.setMarkedText(text.toString(), OffsetRange(caret, caret))
        }

    override fun setComposingRegion(
        start: Int,
        end: Int,
    ): Boolean =
        edit("setComposingRegion") {
            editor.setComposingRegion(OffsetRange(minOf(start, end).toUInt(), maxOf(start, end).toUInt()))
        }

    override fun finishComposingText(): Boolean = edit("finishComposingText") { editor.unmarkText() }

    override fun commitText(
        text: CharSequence,
        newCursorPosition: Int,
    ): Boolean =
        edit("commitText") {
            editor.insertText(text.toString())

            if (newCursorPosition != 1) {
                val end = editor.selection().head.toInt()
                val caret = (end - text.length + cursorWithin(text.length, newCursorPosition)).coerceAtLeast(0).toUInt()
                editor.select(caret, caret)
            }
        }

    /** Where [newCursorPosition] puts the caret, relative to the start of text [length] long. */
    private fun cursorWithin(
        length: Int,
        newCursorPosition: Int,
    ) = if (newCursorPosition > 0) length + newCursorPosition - 1 else newCursorPosition

    override fun commitCompletion(text: CompletionInfo?): Boolean = commitText(text?.text ?: "", 1)

    override fun commitCorrection(correctionInfo: CorrectionInfo?): Boolean = true

    // Deleting
    override fun deleteSurroundingText(
        beforeLength: Int,
        afterLength: Int,
    ): Boolean =
        edit("deleteSurroundingText") {
            val range = selectionRange()
            val backspace = beforeLength == 1 && afterLength == 0 && range.start == range.end

            if (backspace && editor.markedRange() == null) {
                // Backspace: at the start of a list item it lifts the item.
                editor.deleteBackward()

                return@edit
            }

            val end = range.end.toInt()
            val start = range.start.toInt()
            editor.replace(OffsetRange(end.toUInt(), (end + afterLength).toUInt()), "")
            editor.replace(OffsetRange(maxOf(start - beforeLength, 0).toUInt(), start.toUInt()), "")
        }

    override fun deleteSurroundingTextInCodePoints(
        beforeLength: Int,
        afterLength: Int,
    ): Boolean {
        val before = getTextBeforeCursor(beforeLength * 2, 0).toString()
        val after = getTextAfterCursor(afterLength * 2, 0).toString()
        val beforeUnits = before.length - before.offsetByCodePoints(before.length, -codePoints(before, beforeLength))
        val afterUnits = after.offsetByCodePoints(0, codePoints(after, afterLength))

        return deleteSurroundingText(beforeUnits, afterUnits)
    }

    /** [count] code points, or as many as [text] has. */
    private fun codePoints(
        text: String,
        count: Int,
    ) = minOf(count, text.codePointCount(0, text.length))

    // Selection and actions
    override fun setSelection(
        start: Int,
        end: Int,
    ): Boolean =
        edit("setSelection") {
            editor.select(start.toUInt(), end.toUInt())
        }

    override fun sendKeyEvent(event: KeyEvent): Boolean = view.dispatchKeyEvent(event)

    override fun performEditorAction(editorAction: Int): Boolean =
        edit("performEditorAction") {
            editor.press("Enter", Modifiers(shift = false, ctrl = false, alt = false, meta = false))
        }

    override fun performContextMenuAction(id: Int): Boolean {
        when (id) {
            android.R.id.selectAll -> view.selectAll()
            android.R.id.cut -> view.cut()
            android.R.id.copy -> view.copy()
            android.R.id.paste -> view.paste()
            else -> return false
        }

        return true
    }

    override fun beginBatchEdit(): Boolean {
        view.input.batch += 1

        return true
    }

    override fun endBatchEdit(): Boolean {
        view.input.batch = maxOf(view.input.batch - 1, 0)

        if (!view.input.inBatch) view.reportToInputMethod()

        return view.input.inBatch
    }

    override fun clearMetaKeyStates(states: Int): Boolean = false

    override fun performPrivateCommand(
        action: String?,
        data: Bundle?,
    ): Boolean = false

    override fun closeConnection() {
        view.fromInput("closeConnection") { editor.unmarkText() }
        view.input.batch = 0
        view.input.connection = null
        super.closeConnection()
    }

    // The cursor's position, for floating candidate windows
    override fun requestCursorUpdates(cursorUpdateMode: Int): Boolean {
        cursorUpdates = cursorUpdateMode

        if (cursorUpdateMode and InputConnection.CURSOR_UPDATE_IMMEDIATE != 0) reportCursor()

        return true
    }

    /** Send the caret and composition bounds when the input method asked to monitor them. */
    fun reportCursor() {
        if (cursorUpdates == 0) return

        val selection = selectionRange()
        val builder =
            CursorAnchorInfo
                .Builder()
                .setSelectionRange(selection.start.toInt(), selection.end.toInt())
                .setMatrix(screenMatrix())
        editor.caretRect(editor.selection().head)?.let { caret ->
            val rect = view.contentRect(caret)
            val top = (rect.top - view.scrollY).toFloat()
            val bottom = (rect.bottom - view.scrollY).toFloat()
            builder.setInsertionMarkerLocation(rect.exactCenterX(), top, bottom, bottom, 0)
        }
        editor.markedRange()?.let { marked ->
            builder.setComposingText(marked.start.toInt(), editor.textIn(marked))
        }
        view.context.getSystemService(InputMethodManager::class.java)?.updateCursorAnchorInfo(view, builder.build())

        if (cursorUpdates and InputConnection.CURSOR_UPDATE_MONITOR == 0) cursorUpdates = 0
    }

    private fun screenMatrix(): Matrix {
        val location = IntArray(2)
        view.getLocationOnScreen(location)

        return Matrix().apply { setTranslate(location[0].toFloat(), location[1].toFloat()) }
    }

    /** Run [body] as one edit from the input method. */
    private fun edit(
        name: String,
        body: () -> Unit,
    ): Boolean {
        beginBatchEdit()
        val done = view.fromInput(name, body) != null
        endBatchEdit()

        return done
    }

    private companion object {
        /** UTF-16 units of text either side of the selection sent when connecting. */
        const val SURROUNDING = 256

        /** How much text before the cursor decides capitalization. */
        const val CAPS_CONTEXT = 64
    }
}
