package io.github.ifiokjr.canvist.reactnative

import com.facebook.react.bridge.WritableMap
import com.facebook.react.uimanager.events.Event

/** One of the view's events, with its payload. */
internal class CanvistEditorEvent(
    surfaceId: Int,
    viewId: Int,
    private val name: String,
    private val payload: WritableMap,
) : Event<CanvistEditorEvent>(surfaceId, viewId) {
    override fun getEventName() = name

    override fun getEventData() = payload

    companion object {
        const val READY = "topEditorReady"
        const val CHANGE = "topEditorChange"
        const val FOCUS = "topEditorFocus"
        const val ERROR = "topEditorError"

        /** The JavaScript prop each event calls. */
        val registrations = mapOf(
            READY to "onEditorReady",
            CHANGE to "onEditorChange",
            FOCUS to "onEditorFocus",
            ERROR to "onEditorError",
        )
    }
}
