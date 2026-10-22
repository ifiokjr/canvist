package io.github.ifiokjr.canvist

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.Rect
import android.graphics.RectF
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.AttributeSet
import android.util.Log
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.accessibility.AccessibilityNodeProvider
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.widget.OverScroller
import java.lang.ref.WeakReference
import kotlin.math.max
import kotlin.math.roundToInt

/**
 * A rich-text editor view: canvist's Rust engine, painted on a [Canvas] and
 * edited through Android's input method framework.
 *
 * Input methods (Gboard and others: composition, autocorrect, suggestions,
 * voice typing) talk to the editor through its own [InputConnection]; text is
 * measured with Android's text stack ([PaintMeasurer]) and painted in the same
 * fonts. The view draws its caret, selection, and selection handles, shows the
 * floating text toolbar, scrolls documents of any length, and exposes the text
 * to TalkBack through an [AccessibilityNodeProvider].
 *
 * Run commands on [editor] with the typed handles; the view repaints itself:
 *
 * ```kotlin
 * val kit = StarterKit()
 * val view = CanvistView(context, kit.extensions)
 * view.editor.load(DocFormat.MARKDOWN, "# Hello")
 * view.editor.run(kit.bold.toggleBold())
 * ```
 *
 * It is a plain [View], so Compose (`AndroidView`), React Native, and Lynx can
 * host it.
 */
open class CanvistView(
    context: Context,
    attrs: AttributeSet?,
    /** The editor this view shows and edits. */
    val editor: Editor,
    internal val measurer: PaintMeasurer,
    /** The extensions the editor registered. */
    val extensions: ExtensionSet,
) : View(context, attrs) {
    /** A view with a new editor registering every StarterKit extension, for layouts. */
    constructor(context: Context, attrs: AttributeSet? = null) : this(context, StarterKit().extensions, attrs)

    /** A view with a new editor registering [extensions] and one empty paragraph. */
    constructor(context: Context, extensions: ExtensionSet, attrs: AttributeSet? = null) :
        this(context, attrs, PaintMeasurer(context.resources.displayMetrics.density), extensions)

    private constructor(context: Context, attrs: AttributeSet?, measurer: PaintMeasurer, extensions: ExtensionSet) :
        this(context, attrs, Editor(editorOptions(extensions), measurer), measurer, extensions)

    /** Called on the main thread after every change, for example to update a toolbar or send collaboration updates. */
    var onChange: ((EditorChange) -> Unit)? = null

    /** Called on the main thread when the view gains (`true`) or loses focus. */
    var onFocusChange: ((Boolean) -> Unit)? = null

    /** The body text size in sp, before the user's font scale. */
    var baseFontSize = DEFAULT_FONT_SIZE
        set(value) {
            field = value
            applyTheme()
        }

    internal val density get() = measurer.fonts.density

    internal val painter = DisplayPainter(measurer.fonts)
    internal val selection = SelectionController(this)
    internal val input = InputState()
    private val accessibility = CanvistAccessibility(this)
    private val scroller = OverScroller(context)
    private val inputMethods = context.getSystemService(InputMethodManager::class.java)

    private val blink = Handler(Looper.getMainLooper())
    private val blinkCaret = object : Runnable {
        override fun run() {
            caretOn = !caretOn
            editor.setCaretVisible(caretOn)
            invalidate()
            blink.postDelayed(this, BLINK_MILLIS)
        }
    }
    private var caretOn = true
    private var observer: ULong? = null
    private var theme: ThemeOptions? = null

    /** What is painted now. */
    internal var items: List<DisplayItem> = emptyList()
        private set
    private var page = Rgba(0xFFu, 0xFFu, 0xFFu, 0xFFu)
    private var contentHeight = 0
    private var revealCaret = false

    init {
        isFocusable = true
        isFocusableInTouchMode = true
        isVerticalScrollBarEnabled = true

        // The caret shows focus; the system's grey wash over the whole page
        // would appear whenever a hardware key leaves touch mode.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) defaultFocusHighlightEnabled = false

        setWillNotDraw(false)
        val horizontal = (PADDING_HORIZONTAL * density).roundToInt()
        val top = (PADDING_TOP * density).roundToInt()
        setPadding(horizontal, top, horizontal, (PADDING_BOTTOM * density).roundToInt())
        applyTheme()
    }

    // Lifecycle
    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        observer = editor.addObserver(ChangeRelay(this))
    }

    override fun onDetachedFromWindow() {
        observer?.let(editor::removeObserver)
        observer = null
        blink.removeCallbacks(blinkCaret)
        selection.dismiss()
        super.onDetachedFromWindow()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        val density = resources.displayMetrics.density

        if (density != measurer.fonts.density) {
            measurer.fonts.setDensity(density)
            editor.forgetMeasurements()
        }

        applyTheme()
    }

    /** Follow the system's dark theme and font scale. */
    private fun applyTheme() {
        val night = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK
        val theme = defaultThemeOptions().copy(
            dark = night == Configuration.UI_MODE_NIGHT_YES,
            fontSize = baseFontSize * resources.configuration.fontScale,
        )

        if (theme == this.theme) return

        this.theme = theme
        attempt("theme") { editor.setTheme(theme) }
        invalidate()
    }

    // Painting
    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val density = density
        editor.setViewport((width - paddingLeft - paddingRight) / density, height / density)
        editor.setScroll((scrollY - paddingTop) / density)

        val frame = attempt("frame") { editor.frame() } ?: return

        contentHeight = (frame.height * density).roundToInt() + paddingTop + paddingBottom
        frame.items?.let { items = it }
        page = frame.background
        val visible = RectF(0f, scrollY.toFloat(), width.toFloat(), (scrollY + height).toFloat())
        canvas.clipRect(visible)
        painter.paint(
            canvas,
            area = visible,
            items = items,
            page = page,
            originX = paddingLeft.toFloat(),
            originY = paddingTop.toFloat(),
        )
        selection.drawHandles(canvas)

        if (frame.shift != 0f) {
            // Blocks above the viewport were measured: keep the content still.
            scrollBy(0, (frame.shift * density).roundToInt())
        }

        if (revealCaret) {
            revealCaret = false
            post(::scrollCaretIntoView)
        }
    }

    // Scrolling
    private val maxScroll get() = max(contentHeight - height, 0)

    override fun scrollTo(x: Int, y: Int) {
        super.scrollTo(0, y.coerceIn(0, maxScroll))
    }

    override fun computeVerticalScrollRange() = max(contentHeight, height)

    override fun computeVerticalScrollExtent() = height

    override fun computeVerticalScrollOffset() = scrollY

    override fun computeScroll() {
        if (scroller.computeScrollOffset()) {
            scrollTo(0, scroller.currY)
            postInvalidateOnAnimation()
        }
    }

    internal fun fling(velocityY: Float) {
        scroller.fling(
            /* startX = */ 0,
            /* startY = */ scrollY,
            /* velocityX = */ 0,
            /* velocityY = */ -velocityY.roundToInt(),
            /* minX = */ 0,
            /* maxX = */ 0,
            /* minY = */ 0,
            /* maxY = */ maxScroll,
        )
        postInvalidateOnAnimation()
    }

    internal fun stopFling() {
        scroller.forceFinished(true)
    }

    /** Scroll so the caret, or the selection's moving end, is on screen. */
    fun scrollCaretIntoView() {
        val caret = editor.caretRect(editor.selection().head) ?: return
        val rect = contentRect(caret)
        val margin = (CARET_MARGIN * density).roundToInt()

        when {
            rect.top - margin < scrollY -> scrollTo(0, rect.top - margin)
            rect.bottom + margin > scrollY + height -> scrollTo(0, rect.bottom + margin - height)
        }
    }

    // Coordinates
    /** A document rectangle (dp) in content pixels, before scrolling. */
    internal fun contentRect(bounds: Bounds): Rect {
        val rect = RectF()
        rect.set(bounds, density)
        rect.offset(paddingLeft.toFloat(), paddingTop.toFloat())

        return Rect(rect.left.roundToInt(), rect.top.roundToInt(), rect.right.roundToInt(), rect.bottom.roundToInt())
    }

    /** A point in this view's coordinates, in document dp. */
    internal fun documentX(x: Float) = (x - paddingLeft) / density

    internal fun documentY(y: Float) = (y + scrollY - paddingTop) / density

    // Changes
    /** The editor changed: repaint, and tell the input method and TalkBack. */
    internal fun editorChanged(change: EditorChange) {
        if (!input.inBatch) reportToInputMethod()

        if (change.document || change.selection) {
            revealCaret = input.calls > 0 || revealCaret
            restartBlink()
        }

        accessibility.changed(change)
        selection.editorChanged(change)
        invalidate()
        onChange?.invoke(change)
    }

    /** Tell the input method where the selection and composition are. */
    internal fun reportToInputMethod() {
        val selection = editor.selection()
        val marked = editor.markedRange()
        val start = minOf(selection.anchor, selection.head).toInt()
        val end = maxOf(selection.anchor, selection.head).toInt()
        inputMethods?.updateSelection(this, start, end, marked?.start?.toInt() ?: -1, marked?.end?.toInt() ?: -1)
        input.connection?.reportCursor()
    }

    /** Run [body], counted as a call from the input method, logging why it failed. */
    internal fun <T> fromInput(name: String, body: () -> T): T? {
        input.calls += 1

        try {
            return attempt(name, body)
        } finally {
            input.calls -= 1
        }
    }

    /**
     * Run [body], logging why it failed. Input method and drawing calls cannot
     * throw, and a failed edit must not stop the next keystroke.
     */
    internal fun <T> attempt(name: String, body: () -> T): T? = try {
        body()
    } catch (error: CanvistException) {
        Log.e(TAG, "$name failed", error)
        null
    }

    // Focus and the caret
    override fun onCheckIsTextEditor() = true

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection =
        CanvistInputConnection(this).also { connection ->
            connection.configure(outAttrs)
            input.connection = connection
        }

    override fun onFocusChanged(gainFocus: Boolean, direction: Int, previouslyFocusedRect: Rect?) {
        super.onFocusChanged(gainFocus, direction, previouslyFocusedRect)
        editor.setFocused(gainFocus)

        if (gainFocus) {
            restartBlink()
        } else {
            blink.removeCallbacks(blinkCaret)
            fromInput("unmark") { editor.unmarkText() }
            selection.dismiss()
            invalidate()
        }

        onFocusChange?.invoke(gainFocus)
    }

    /** Show the caret, and blink it from now on. */
    private fun restartBlink() {
        blink.removeCallbacks(blinkCaret)
        caretOn = true
        editor.setCaretVisible(true)

        if (hasFocus()) blink.postDelayed(blinkCaret, BLINK_MILLIS)
    }

    /** Focus the editor and show the soft keyboard. */
    fun showKeyboard() {
        requestFocus()
        inputMethods?.showSoftInput(this, 0)
    }

    // Touch and keys
    override fun onTouchEvent(event: MotionEvent): Boolean {
        return selection.onTouchEvent(event) || super.onTouchEvent(event)
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean =
        onKey(event) || super.onKeyDown(keyCode, event)

    /** Run the editor's binding for [event], a clipboard shortcut, or type its character. */
    private fun onKey(event: KeyEvent): Boolean {
        val key = KeyNames.of(event) ?: return false
        val modifiers = KeyNames.modifiers(event)

        if (modifiers.ctrl && clipboardShortcut(key)) return true

        if (fromInput("press") { editor.press(key, modifiers) } == true) return true

        val typed = event.unicodeChar

        if (typed == 0 || modifiers.ctrl || modifiers.meta) return false

        // A typed key ends any composition, the keyboard's suggestion region
        // included, and goes in at the cursor rather than replacing it.
        fromInput("type") {
            editor.unmarkText()
            editor.insertText(String(Character.toChars(typed)))
        }

        return true
    }

    private fun clipboardShortcut(key: String): Boolean {
        when (key) {
            "c" -> copy()
            "x" -> cut()
            "v" -> paste()
            "a" -> selectAll()
            else -> return false
        }

        return true
    }

    // Clipboard
    private val clipboard get() = context.getSystemService(ClipboardManager::class.java)

    /** Copy the selection as HTML and plain text. */
    fun copy() {
        val content = attempt("copy") { editor.copy() } ?: return
        clipboard?.setPrimaryClip(ClipData.newHtmlText(CLIP_LABEL, content.text, content.html))
    }

    /** Copy the selection and delete it. */
    fun cut() {
        val content = fromInput("cut") { editor.cut() } ?: return
        clipboard?.setPrimaryClip(ClipData.newHtmlText(CLIP_LABEL, content.text, content.html))
    }

    /** Paste HTML when the clipboard has it, plain text otherwise. */
    fun paste() {
        val item = clipboard?.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0) ?: return
        val html = item.htmlText

        if (html != null) {
            fromInput("paste") { editor.pasteHtml(html) }
        } else {
            fromInput("paste") { editor.pasteText(item.coerceToText(context).toString()) }
        }
    }

    /** Select the whole document. */
    fun selectAll() {
        fromInput("selectAll") { editor.selectAll() }
    }

    /** Whether the clipboard holds anything to paste. */
    internal val canPaste get() = clipboard?.hasPrimaryClip() == true

    // Accessibility
    override fun getAccessibilityNodeProvider(): AccessibilityNodeProvider = accessibility

    override fun getAccessibilityClassName(): CharSequence = "android.widget.EditText"

    companion object {
        private const val TAG = "CanvistView"
        private const val CLIP_LABEL = "canvist"
        private const val BLINK_MILLIS = 500L
        private const val CARET_MARGIN = 16f
        private const val PADDING_HORIZONTAL = 20f
        private const val PADDING_TOP = 16f
        private const val PADDING_BOTTOM = 32f

        /** The default body text size in sp. */
        const val DEFAULT_FONT_SIZE = 17f

        /** How far past the viewport to lay out, in dp. */
        private const val OVERSCAN = 400f

        /**
         * Options for an editor this view drives: [extensions], Control as `Mod`
         * in key bindings, and the caret and selection in the display list.
         */
        fun editorOptions(extensions: ExtensionSet): EditorOptions = defaultEditorOptions().copy(
            extensions = extensions.names,
            platform = KeyPlatform.OTHER,
            drawsSelection = true,
            overscan = OVERSCAN,
        )
    }
}

/** What the input method framework is doing with the view. */
internal class InputState {
    /** Calls from the input method in progress. */
    var calls = 0

    /** Nested batch edits; selection reports wait for the outermost to end. */
    var batch = 0

    val inBatch get() = batch > 0

    var connection: CanvistInputConnection? = null
}

/**
 * Forwards the editor's change notifications to the view on the main thread.
 * It holds the view weakly, because the editor holds it strongly.
 */
private class ChangeRelay(view: CanvistView) : EditorObserver {
    private val view = WeakReference(view)
    private val main = Handler(Looper.getMainLooper())

    override fun changed(change: EditorChange) {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            view.get()?.editorChanged(change)
        } else {
            main.post { view.get()?.editorChanged(change) }
        }
    }
}
