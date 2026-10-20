package io.github.ifiokjr.canvist

import android.graphics.Rect
import android.os.Build
import android.os.Bundle
import android.view.View
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityManager
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction
import android.view.accessibility.AccessibilityNodeProvider

/**
 * TalkBack's view of a [CanvistView]: the view is an editable text field
 * holding the document's text, with its selection, so TalkBack reads, moves
 * by character, word, line, and paragraph, selects, and edits. Each text
 * block on screen is also a child node, so exploring by touch reads the
 * block under the finger and headings are navigable as headings.
 */
internal class CanvistAccessibility(private val view: CanvistView) : AccessibilityNodeProvider() {
    private val editor get() = view.editor
    private val manager = view.context.getSystemService(AccessibilityManager::class.java)

    /** The blocks on screen when TalkBack last asked, by virtual id minus one. */
    private var blocks: List<BlockInfo> = emptyList()

    /** The text TalkBack was last told about, to describe changes against. */
    private var spoken: String? = null

    override fun createAccessibilityNodeInfo(virtualViewId: Int): AccessibilityNodeInfo? =
        if (virtualViewId == View.NO_ID) host() else block(virtualViewId)

    private fun host(): AccessibilityNodeInfo {
        val info = node(View.NO_ID)
        view.onInitializeAccessibilityNodeInfo(info)
        val text = editor.plainText()
        val selection = editor.selection()
        spoken = text

        info.className = "android.widget.EditText"
        info.text = text
        info.isEditable = true
        info.isMultiLine = true
        info.isFocusable = true
        info.isFocused = view.isFocused

        val start = minOf(selection.anchor, selection.head).toInt()
        info.setTextSelection(start, maxOf(selection.anchor, selection.head).toInt())
        info.movementGranularities = GRANULARITIES
        listOf(
            AccessibilityAction.ACTION_CLICK,
            AccessibilityAction.ACTION_SET_SELECTION,
            AccessibilityAction.ACTION_NEXT_AT_MOVEMENT_GRANULARITY,
            AccessibilityAction.ACTION_PREVIOUS_AT_MOVEMENT_GRANULARITY,
            AccessibilityAction.ACTION_PASTE,
            AccessibilityAction.ACTION_SCROLL_FORWARD,
            AccessibilityAction.ACTION_SCROLL_BACKWARD,
        ).forEach(info::addAction)

        if (selection.anchor != selection.head) {
            info.addAction(AccessibilityAction.ACTION_COPY)
            info.addAction(AccessibilityAction.ACTION_CUT)
        }

        val top = view.documentY(0f)
        blocks = editor.blocks(top, top + view.height / view.density)
        blocks.indices.forEach { index -> info.addChild(view, index + 1) }

        return info
    }

    private fun block(virtualViewId: Int): AccessibilityNodeInfo? {
        val block = blocks.getOrNull(virtualViewId - 1) ?: return null
        val info = node(virtualViewId)
        info.setParent(view)
        info.packageName = view.context.packageName

        info.className = "android.widget.TextView"
        info.text = editor.textIn(block.range)
        info.isVisibleToUser = true
        info.isFocusable = true
        info.addAction(AccessibilityAction.ACTION_CLICK)
        info.addAction(AccessibilityAction.ACTION_ACCESSIBILITY_FOCUS)

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) info.isHeading = block.role is BlockRole.Heading

        val bounds = view.contentRect(block.bounds)
        bounds.offset(0, -view.scrollY)
        val location = IntArray(2)
        view.getLocationOnScreen(location)
        info.setBoundsInScreen(Rect(bounds).apply { offset(location[0], location[1]) })

        return info
    }

    override fun performAction(virtualViewId: Int, action: Int, arguments: Bundle?): Boolean {
        if (virtualViewId != View.NO_ID) return performOnBlock(virtualViewId, action)

        return when (action) {
            AccessibilityNodeInfo.ACTION_CLICK -> true.also { view.showKeyboard() }
            AccessibilityNodeInfo.ACTION_SET_SELECTION -> setSelection(arguments)
            AccessibilityNodeInfo.ACTION_NEXT_AT_MOVEMENT_GRANULARITY -> move(arguments, forward = true)
            AccessibilityNodeInfo.ACTION_PREVIOUS_AT_MOVEMENT_GRANULARITY -> move(arguments, forward = false)
            AccessibilityNodeInfo.ACTION_COPY -> true.also { view.copy() }
            AccessibilityNodeInfo.ACTION_CUT -> true.also { view.cut() }
            AccessibilityNodeInfo.ACTION_PASTE -> true.also { view.paste() }
            AccessibilityNodeInfo.ACTION_SCROLL_FORWARD -> true.also { view.scrollBy(0, view.height / 2) }
            AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD -> true.also { view.scrollBy(0, -view.height / 2) }
            else -> view.performAccessibilityAction(action, arguments)
        }
    }

    private fun performOnBlock(virtualViewId: Int, action: Int): Boolean {
        val block = blocks.getOrNull(virtualViewId - 1) ?: return false

        return when (action) {
            AccessibilityNodeInfo.ACTION_CLICK -> {
                view.fromInput("select") { editor.select(block.range.start, block.range.start) }
                view.showKeyboard()
                true
            }
            AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS -> {
                send(virtualViewId, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED)
                true
            }
            else -> false
        }
    }

    private fun setSelection(arguments: Bundle?): Boolean {
        val start = arguments?.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, -1) ?: -1
        val end = arguments?.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, -1) ?: -1

        if (start < 0 || end < 0) return false

        return view.fromInput("select") { editor.select(start.toUInt(), end.toUInt()) } != null
    }

    /** Move the caret (or extend the selection) by one unit of [arguments]' granularity. */
    private fun move(arguments: Bundle?, forward: Boolean): Boolean {
        arguments ?: return false

        val granularity = arguments.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_MOVEMENT_GRANULARITY_INT)
        val extend = arguments.getBoolean(AccessibilityNodeInfo.ACTION_ARGUMENT_EXTEND_SELECTION_BOOLEAN)
        val selection = editor.selection()
        val target = next(selection.head, granularity, forward) ?: return false

        val anchor = if (extend) selection.anchor else target
        view.fromInput("move") { editor.select(anchor, target) }

        return true
    }

    private fun next(from: UInt, granularity: Int, forward: Boolean): UInt? {
        val length = editor.textLength()

        return when (granularity) {
            AccessibilityNodeInfo.MOVEMENT_GRANULARITY_CHARACTER -> character(from, forward, length)
            AccessibilityNodeInfo.MOVEMENT_GRANULARITY_WORD -> word(from, forward, length)
            AccessibilityNodeInfo.MOVEMENT_GRANULARITY_LINE -> editor.lineEdgeOffset(from, forward)
                ?.takeIf { it != from } ?: editor.verticalOffset(from, !forward, null)
            AccessibilityNodeInfo.MOVEMENT_GRANULARITY_PARAGRAPH -> paragraph(from, forward, length)
            else -> null
        }
    }

    private fun character(from: UInt, forward: Boolean, length: UInt): UInt? = when {
        forward -> (from + 1u).coerceAtMost(length)
        from > 0u -> from - 1u
        else -> null
    }

    /** The far edge of the word after (or before) [from], or `null` at the end of the text. */
    private fun word(from: UInt, forward: Boolean, length: UInt): UInt? {
        if (forward && from >= length || !forward && from == 0u) return null

        val range = editor.wordRange(if (forward) from else from - 1u)

        return if (forward) range.end else range.start
    }

    private fun paragraph(from: UInt, forward: Boolean, length: UInt): UInt? {
        val block = editor.blockRange(from)

        return when {
            forward && from < block.end -> block.end
            forward -> (block.end + 1u).takeIf { it <= length }?.let { editor.blockRange(it).end }
            from > block.start -> block.start
            block.start > 0u -> editor.blockRange(block.start - 1u).start
            else -> null
        }
    }

    /** Tell TalkBack what changed, when it is listening. */
    fun changed(change: EditorChange) {
        if (manager?.isEnabled != true) return

        if (change.document) textChanged()

        if (change.selection || change.document) {
            val selection = editor.selection()
            val event = event(AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED)
            event.fromIndex = minOf(selection.anchor, selection.head).toInt()
            event.toIndex = maxOf(selection.anchor, selection.head).toInt()
            event.itemCount = editor.textLength().toInt()
            view.parent?.requestSendAccessibilityEvent(view, event.also(::fill))
        }
    }

    /** Describe a text change as the span that differs from what TalkBack last heard. */
    private fun textChanged() {
        val before = spoken ?: editor.plainText().also { spoken = it }
        val after = editor.plainText()
        spoken = after

        val prefix = before.commonPrefixWith(after).length
        val suffix = before.substring(prefix).commonSuffixWith(after.substring(prefix)).length
        val event = event(AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED)
        event.fromIndex = prefix

        event.removedCount = before.length - prefix - suffix
        event.addedCount = after.length - prefix - suffix
        event.beforeText = before
        event.text.add(after)
        view.parent?.requestSendAccessibilityEvent(view, event.also(::fill))
    }

    private fun send(virtualViewId: Int, type: Int) {
        val event = event(type)
        fill(event)
        event.setSource(view, virtualViewId)
        view.parent?.requestSendAccessibilityEvent(view, event)
    }

    private fun fill(event: AccessibilityEvent) {
        event.packageName = view.context.packageName
        event.className = "android.widget.EditText"
        event.setSource(view)
    }

    /** A node for the view or one of its virtual children. */
    @Suppress("DEPRECATION")
    private fun node(virtualViewId: Int): AccessibilityNodeInfo = when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && virtualViewId == View.NO_ID -> AccessibilityNodeInfo(view)
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.R -> AccessibilityNodeInfo(view, virtualViewId)
        virtualViewId == View.NO_ID -> AccessibilityNodeInfo.obtain(view)
        else -> AccessibilityNodeInfo.obtain(view, virtualViewId)
    }

    @Suppress("DEPRECATION")
    private fun event(type: Int): AccessibilityEvent =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) AccessibilityEvent(type) else AccessibilityEvent.obtain(type)

    private companion object {
        const val GRANULARITIES = AccessibilityNodeInfo.MOVEMENT_GRANULARITY_CHARACTER or
            AccessibilityNodeInfo.MOVEMENT_GRANULARITY_WORD or
            AccessibilityNodeInfo.MOVEMENT_GRANULARITY_LINE or
            AccessibilityNodeInfo.MOVEMENT_GRANULARITY_PARAGRAPH
    }
}
