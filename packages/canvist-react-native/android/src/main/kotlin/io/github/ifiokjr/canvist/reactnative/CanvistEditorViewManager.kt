package io.github.ifiokjr.canvist.reactnative

import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.ReactContext
import com.facebook.react.bridge.ReadableArray
import com.facebook.react.bridge.WritableMap
import com.facebook.react.module.annotations.ReactModule
import com.facebook.react.uimanager.SimpleViewManager
import com.facebook.react.uimanager.ThemedReactContext
import com.facebook.react.uimanager.UIManagerHelper
import com.facebook.react.uimanager.ViewManagerDelegate
import com.facebook.react.viewmanagers.CanvistEditorViewManagerDelegate
import com.facebook.react.viewmanagers.CanvistEditorViewManagerInterface
import io.github.ifiokjr.canvist.CanvistHostView
import io.github.ifiokjr.canvist.EditorChange

/**
 * The Fabric view manager for `CanvistEditorView`. The editor starts once a
 * view's first props have all arrived; later changes to `extensions` and
 * `content` have no effect, as the JavaScript API documents.
 */
@ReactModule(name = CanvistEditorViewManager.NAME)
class CanvistEditorViewManager :
    SimpleViewManager<CanvistHostView>(),
    CanvistEditorViewManagerInterface<CanvistHostView> {
    private val delegate = CanvistEditorViewManagerDelegate(this)

    /** Props of views whose editors have not started yet. */
    private val pending = HashMap<CanvistHostView, StartProps>()

    override fun getDelegate(): ViewManagerDelegate<CanvistHostView> = delegate

    override fun getName() = NAME

    override fun createViewInstance(context: ThemedReactContext): CanvistHostView =
        CanvistHostView(context).apply { listener = Events }

    override fun setEditorId(view: CanvistHostView, value: String?) {
        pending.getOrPut(view, ::StartProps).editorId = value
    }

    override fun setExtensions(view: CanvistHostView, value: ReadableArray?) {
        val names = value?.toArrayList()?.filterIsInstance<String>() ?: emptyList()
        pending.getOrPut(view, ::StartProps).extensions = names
    }

    override fun setContentFormat(view: CanvistHostView, value: String?) {
        pending.getOrPut(view, ::StartProps).format = value
    }

    override fun setContent(view: CanvistHostView, value: String?) {
        pending.getOrPut(view, ::StartProps).content = value
    }

    override fun setFontSize(view: CanvistHostView, value: Float) {
        view.fontSize = value
    }

    override fun onAfterUpdateTransaction(view: CanvistHostView) {
        super.onAfterUpdateTransaction(view)
        val props = pending.remove(view) ?: return
        val editorId = props.editorId?.takeIf { it.isNotEmpty() } ?: return
        view.start(editorId, props.extensions, props.format?.ifEmpty { null }, props.content?.ifEmpty { null })
    }

    override fun onDropViewInstance(view: CanvistHostView) {
        pending.remove(view)
        view.stop()
        super.onDropViewInstance(view)
    }

    override fun focus(view: CanvistHostView) = view.focusEditor()

    override fun blur(view: CanvistHostView) = view.blurEditor()

    override fun getExportedCustomDirectEventTypeConstants(): Map<String, Any> =
        CanvistEditorEvent.registrations.mapValues { (_, prop) -> mapOf("registrationName" to prop) }

    private class StartProps {
        var editorId: String? = null
        var extensions: List<String> = emptyList()
        var format: String? = null
        var content: String? = null
    }

    /** Sends each host's reports to JavaScript as the view's events. */
    private object Events : CanvistHostView.Listener {
        override fun onReady(view: CanvistHostView, editorId: String) =
            dispatch(view, CanvistEditorEvent.READY) { putString("editorId", editorId) }

        override fun onChange(view: CanvistHostView, change: EditorChange) =
            dispatch(view, CanvistEditorEvent.CHANGE) {
                putBoolean("document", change.document)
                putBoolean("selection", change.selection)
                putBoolean("composition", change.composition)
            }

        override fun onFocusChange(view: CanvistHostView, focused: Boolean) =
            dispatch(view, CanvistEditorEvent.FOCUS) { putBoolean("focused", focused) }

        override fun onError(view: CanvistHostView, code: String, message: String) =
            dispatch(view, CanvistEditorEvent.ERROR) {
                putString("code", code)
                putString("message", message)
            }

        private fun dispatch(view: CanvistHostView, name: String, payload: WritableMap.() -> Unit) {
            val context = view.context as? ReactContext ?: return
            val dispatcher = UIManagerHelper.getEventDispatcherForReactTag(context, view.id) ?: return
            val data = Arguments.createMap().apply(payload)
            dispatcher.dispatchEvent(CanvistEditorEvent(UIManagerHelper.getSurfaceId(view), view.id, name, data))
        }
    }

    companion object {
        const val NAME = "CanvistEditorView"
    }
}
