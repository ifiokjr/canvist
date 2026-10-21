package io.github.ifiokjr.canvist

import android.graphics.Canvas
import android.graphics.Rect
import android.os.Build
import android.view.ActionMode
import android.view.GestureDetector
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.View
import android.widget.Magnifier

/**
 * Touch on a [CanvistView]: taps place the caret, double taps and long presses
 * select words, drags scroll and fling, and the selection handles drag the
 * selection's ends with a magnifier. While text is selected, or after a tap on
 * the caret, the floating text toolbar offers cut, copy, paste, select all,
 * and formatting.
 */
internal class SelectionController(private val view: CanvistView) {
    private val editor get() = view.editor
    private val gestures = GestureDetector(view.context, Gestures())
    private val handles = SelectionHandles.from(view)
    private val magnifier = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) Magnifier.Builder(view).build() else null

    /** The handle being dragged, and how far the finger is from the point it marks. */
    private var dragging: Handle? = null
    private var dragOffsetX = 0f
    private var dragOffsetY = 0f

    /** Whether the caret's insertion handle shows: after a tap, until typing. */
    private var showInsertionHandle = false
    private var actionMode: ActionMode? = null

    fun onTouchEvent(event: MotionEvent): Boolean {
        if (event.actionMasked == MotionEvent.ACTION_DOWN) {
            view.stopFling()
            dragging = handleAt(event.x, event.y)
        }

        if (dragging != null) {
            dragHandle(event)

            return true
        }

        return gestures.onTouchEvent(event)
    }

    /** The editor changed: typing hides the insertion handle, and the toolbar follows the selection. */
    fun editorChanged(change: EditorChange) {
        if (change.document && view.input.calls > 0 && dragging == null) showInsertionHandle = false

        val collapsed = editor.selection().let { it.anchor == it.head }

        if (collapsed && actionMode?.tag != INSERTION) {
            actionMode?.finish()
        } else {
            actionMode?.invalidateContentRect()
        }
    }

    /** Hide the handles and the toolbar. */
    fun dismiss() {
        showInsertionHandle = false
        actionMode?.finish()
    }

    // Gestures
    private inner class Gestures : GestureDetector.SimpleOnGestureListener() {
        override fun onDown(event: MotionEvent) = true

        override fun onSingleTapConfirmed(event: MotionEvent): Boolean {
            if (toggleCheckbox(event)) return true

            val offset = offsetAt(event.x, event.y) ?: return false
            val onCaret = view.hasFocus() && editor.selection().let { it.anchor == offset && it.head == offset }
            view.fromInput("tap") { editor.select(offset, offset) }
            view.showKeyboard()
            showInsertionHandle = true

            if (onCaret) showToolbar(INSERTION) else actionMode?.finish()

            view.invalidate()

            return true
        }

        override fun onDoubleTap(event: MotionEvent): Boolean {
            selectWord(event)

            return true
        }

        override fun onLongPress(event: MotionEvent) {
            view.performHapticFeedback(android.view.HapticFeedbackConstants.LONG_PRESS)
            selectWord(event)
        }

        override fun onScroll(start: MotionEvent?, event: MotionEvent, distanceX: Float, distanceY: Float): Boolean {
            view.scrollBy(0, distanceY.toInt())
            actionMode?.hide(ActionMode.DEFAULT_HIDE_DURATION.toLong())

            return true
        }

        override fun onFling(start: MotionEvent?, event: MotionEvent, velocityX: Float, velocityY: Float): Boolean {
            view.fling(velocityY)

            return true
        }
    }

    private fun offsetAt(x: Float, y: Float): UInt? = editor.offsetAt(view.documentX(x), view.documentY(y))

    private fun selectWord(event: MotionEvent) {
        val offset = offsetAt(event.x, event.y) ?: return
        val word = editor.wordRange(offset)
        view.fromInput("selectWord") { editor.select(word.start, word.end) }
        view.showKeyboard()
        showInsertionHandle = false
        showToolbar(SELECTION)
        view.invalidate()
    }

    private fun toggleCheckbox(event: MotionEvent): Boolean {
        val box = checkboxAt(view.documentX(event.x), view.documentY(event.y)) ?: return false

        view.fromInput("toggleTask") { editor.toggleTaskAt(box.bounds.x + 1, box.bounds.y + 1) }

        return true
    }

    /** The task checkbox at document point ([x], [y]), with some slop for fingers. */
    private fun checkboxAt(x: Float, y: Float): DisplayItem.Checkbox? {
        val boxes = view.items.filterIsInstance<DisplayItem.Checkbox>()

        return boxes.firstOrNull { box ->
            val bounds = box.bounds
            val insideX = x >= bounds.x - CHECKBOX_SLOP && x <= bounds.x + bounds.width + CHECKBOX_SLOP

            insideX && y >= bounds.y - CHECKBOX_SLOP && y <= bounds.y + bounds.height + CHECKBOX_SLOP
        }
    }

    // Handles
    private fun handleAt(x: Float, y: Float): Handle? {
        val contentX = x.toInt()
        val contentY = (y + view.scrollY).toInt()

        val (handle, _) = visibleHandles()
            .firstOrNull { (_, bounds) -> bounds.contains(contentX, contentY) }
            ?: return null

        dragOffsetX = x - handle.point.first
        dragOffsetY = y + view.scrollY - handle.point.second

        return handle
    }

    private fun dragHandle(event: MotionEvent) {
        val handle = dragging ?: return

        when (event.actionMasked) {
            MotionEvent.ACTION_MOVE -> dragTo(handle, event.x - dragOffsetX, event.y - dragOffsetY)
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> drop(handle)
        }
    }

    private fun dragTo(handle: Handle, x: Float, y: Float) {
        val offset = offsetAt(x, y) ?: return

        moveEnd(handle.end, offset)
        magnifier?.show(x, y)
        actionMode?.hide(ActionMode.DEFAULT_HIDE_DURATION.toLong())
    }

    private fun drop(handle: Handle) {
        dragging = null
        magnifier?.dismiss()

        if (handle.end != End.CARET) showToolbar(SELECTION)
    }

    /** Move one end of the selection to [offset], keeping the other. */
    private fun moveEnd(end: End, offset: UInt) {
        val selection = editor.selection()
        val start = minOf(selection.anchor, selection.head)
        val finish = maxOf(selection.anchor, selection.head)

        view.fromInput("drag") {
            when (end) {
                End.CARET -> editor.select(offset, offset)
                End.START -> editor.select(finish, offset.coerceAtMost(finish - 1u))
                End.END -> editor.select(start, offset.coerceAtLeast(start + 1u))
            }
        }
    }

    /** The handles on screen, in content pixels, with the bounds that catch touches. */
    private fun visibleHandles(): List<Pair<Handle, Rect>> {
        if (!view.hasFocus()) return emptyList()

        val selection = editor.selection()
        val collapsed = selection.anchor == selection.head

        if (collapsed && !showInsertionHandle) return emptyList()

        val ends = if (collapsed) {
            listOf(End.CARET to selection.head)
        } else {
            listOf(
                End.START to minOf(selection.anchor, selection.head),
                End.END to maxOf(selection.anchor, selection.head),
            )
        }

        return ends.mapNotNull { (end, offset) ->
            editor.caretRect(offset)?.let { caret -> handles.place(end, view, caret) }
        }
    }

    fun drawHandles(canvas: Canvas) {
        for ((handle, bounds) in visibleHandles()) {
            handle.drawable.bounds = bounds
            handle.drawable.draw(canvas)
        }
    }

    // The floating toolbar
    private fun showToolbar(kind: String) {
        actionMode?.finish()
        actionMode = view.startActionMode(Toolbar(), ActionMode.TYPE_FLOATING)?.apply { tag = kind }
    }

    /** Run the toolbar item [itemId]: a clipboard action or a format. */
    private fun perform(itemId: Int) {
        when (itemId) {
            android.R.id.cut -> view.cut()
            android.R.id.copy -> view.copy()
            android.R.id.paste -> view.paste()
            else -> format(itemId)
        }
    }

    private fun format(itemId: Int) {
        val format = FORMATS.getOrNull(itemId - FORMAT_IDS) ?: return

        view.fromInput("format") { editor.run(format.command()) }
    }

    private inner class Toolbar : ActionMode.Callback2() {
        override fun onCreateActionMode(mode: ActionMode, menu: Menu): Boolean {
            val selected = editor.selection().let { it.anchor != it.head }

            if (selected) {
                menu.add(Menu.NONE, android.R.id.cut, 0, android.R.string.cut)
                menu.add(Menu.NONE, android.R.id.copy, 1, android.R.string.copy)
            }

            if (view.canPaste) menu.add(Menu.NONE, android.R.id.paste, 2, android.R.string.paste)

            menu.add(Menu.NONE, android.R.id.selectAll, 3, android.R.string.selectAll)

            if (selected) addFormats(menu)

            return true
        }

        /** The formats whose extensions the editor has. */
        private fun addFormats(menu: Menu) {
            val available = FORMATS.withIndex().filter { (_, format) -> format.extension in view.extensions.names }

            for ((index, format) in available) {
                menu.add(Menu.NONE, FORMAT_IDS + index, FORMAT_ORDER + index, format.title)
            }
        }

        override fun onPrepareActionMode(mode: ActionMode, menu: Menu) = false

        override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean {
            // Select all keeps the toolbar up, now for the whole text.
            if (item.itemId == android.R.id.selectAll) return true.also { view.selectAll() }

            perform(item.itemId)
            mode.finish()

            return true
        }

        override fun onDestroyActionMode(mode: ActionMode) {
            if (actionMode === mode) actionMode = null
        }

        override fun onGetContentRect(mode: ActionMode, view: View, outRect: Rect) {
            val selection = editor.selection()

            val bounds = editor.selectionBounds() ?: editor.caretRect(selection.head) ?: return
            outRect.set(this@SelectionController.view.contentRect(bounds))
            outRect.offset(0, -view.scrollY)
        }
    }

    /** A formatting command the toolbar offers, and the extension it needs. */
    private class Format(val title: String, val extension: String, val command: () -> Command)

    private companion object {
        const val SELECTION = "selection"
        const val INSERTION = "insertion"
        const val CHECKBOX_SLOP = 8f
        const val FORMAT_IDS = 0x0CA4_0000
        const val FORMAT_ORDER = 10

        val FORMATS = listOf(
            Format("Bold", Extensions.bold.name) { Extensions.bold.handle.toggleBold() },
            Format("Italic", Extensions.italic.name) { Extensions.italic.handle.toggleItalic() },
            Format("Underline", Extensions.underline.name) { Extensions.underline.handle.toggleUnderline() },
            Format("Strikethrough", Extensions.strike.name) { Extensions.strike.handle.toggleStrike() },
            Format("Code", Extensions.code.name) { Extensions.code.handle.toggleCode() },
        )
    }
}
