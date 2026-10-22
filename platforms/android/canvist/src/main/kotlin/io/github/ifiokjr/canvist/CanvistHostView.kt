package io.github.ifiokjr.canvist

import android.content.Context
import android.view.View
import android.view.inputmethod.InputMethodManager
import android.widget.FrameLayout

/**
 * A [CanvistView] for a JavaScript host such as React Native or Lynx: created
 * from the host element's first props, registered with [HostedEditors] under
 * the host's id, and reporting what changed through [listener], which the
 * host turns into events.
 */
class CanvistHostView(context: Context) : FrameLayout(context) {
    /** What the host reports, which the view manager turns into events. */
    interface Listener {
        fun onReady(view: CanvistHostView, editorId: String)
        fun onChange(view: CanvistHostView, change: EditorChange)
        fun onFocusChange(view: CanvistHostView, focused: Boolean)
        fun onError(view: CanvistHostView, code: String, message: String)
    }

    var listener: Listener? = null

    /** The body text size in sp before the user's font scale; 0 is the platform's default. */
    var fontSize = 0f
        set(value) {
            field = value
            applyFontSize()
        }

    private var editorView: CanvistView? = null
    private var editorId: String? = null
    private var label: CharSequence? = null

    /**
     * Create the editor, registering [extensions], and load [content] as
     * [format] (when both are given). Later calls do nothing: a view's editor
     * is fixed once created.
     */
    fun start(editorId: String, extensions: List<String>, format: String?, content: String?) {
        if (editorView != null) return

        val view = try {
            CanvistView(context, ExtensionSet(extensions))
        } catch (error: Exception) {
            report(error)
            return
        }

        view.contentDescription = label
        view.onChange = { change -> listener?.onChange(this, change) }
        view.onFocusChange = { focused -> listener?.onFocusChange(this, focused) }
        addView(view, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT))
        // React Native lays this view out itself and ignores layout requests
        // from inside it, so size the new child here.
        layOut(view)
        editorView = view
        this.editorId = editorId
        applyFontSize()

        if (format != null && content != null) {
            try {
                view.editor.load(docFormat(format), content)
            } catch (error: Exception) {
                report(error)
            }
        }

        HostedEditors.register(editorId, view.editor)
        // Announce it once this transaction has mounted and events reach JavaScript.
        post { listener?.onReady(this, editorId) }
    }

    /** Unregister the editor and remove its view. */
    fun stop() {
        editorId?.let(HostedEditors::unregister)
        editorView?.let(::removeView)
        editorView = null
        editorId = null
    }

    fun focusEditor() {
        editorView?.showKeyboard()
    }

    fun blurEditor() {
        val view = editorView ?: return
        view.clearFocus()
        context.getSystemService(InputMethodManager::class.java)?.hideSoftInputFromWindow(view.windowToken, 0)
    }

    /** The label TalkBack reads belongs to the editor, which TalkBack moves through. */
    override fun setContentDescription(contentDescription: CharSequence?) {
        label = contentDescription
        editorView?.contentDescription = contentDescription
    }

    override fun onLayout(changed: Boolean, left: Int, top: Int, right: Int, bottom: Int) {
        editorView?.let(::layOut)
    }

    private fun layOut(child: View) {
        child.measure(
            MeasureSpec.makeMeasureSpec(width, MeasureSpec.EXACTLY),
            MeasureSpec.makeMeasureSpec(height, MeasureSpec.EXACTLY),
        )
        child.layout(0, 0, width, height)
    }

    private fun applyFontSize() {
        editorView?.baseFontSize = if (fontSize > 0f) fontSize else CanvistView.DEFAULT_FONT_SIZE
    }

    private fun report(error: Exception) {
        val failure = HostedEditorException.of(error)
        listener?.onError(this, failure.code, failure.reason)
    }
}
