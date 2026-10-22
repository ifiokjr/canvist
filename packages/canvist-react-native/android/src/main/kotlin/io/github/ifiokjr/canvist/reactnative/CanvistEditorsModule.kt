package io.github.ifiokjr.canvist.reactnative

import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.module.annotations.ReactModule
import io.github.ifiokjr.canvist.Command
import io.github.ifiokjr.canvist.HostedEditors
import io.github.ifiokjr.canvist.docFormat
import io.github.ifiokjr.canvist.selectRangeJson
import io.github.ifiokjr.canvist.selectionJson

/**
 * The `CanvistEditors` TurboModule: synchronous calls, on the JavaScript
 * thread, into the editors [HostedEditors] finds by `editorId`. A failure
 * throws with `[canvist:<code>] <reason>` as its message, which React Native
 * rethrows in JavaScript.
 */
@ReactModule(name = CanvistEditorsModule.NAME)
class CanvistEditorsModule(context: ReactApplicationContext) : NativeCanvistEditorsSpec(context) {
    override fun getName() = NAME

    override fun run(editorId: String, name: String, args: String): Boolean =
        HostedEditors.call(editorId) { it.run(Command(name, args)) }

    override fun can(editorId: String, name: String, args: String): Boolean =
        HostedEditors.call(editorId) { it.can(Command(name, args)) }

    override fun query(editorId: String, name: String, args: String): String =
        HostedEditors.call(editorId) { it.query(name, args) }

    override fun load(editorId: String, format: String, content: String): Boolean =
        HostedEditors.call(editorId) {
            it.load(docFormat(format), content)
            true
        }

    override fun exportDocument(editorId: String, format: String): String =
        HostedEditors.call(editorId) { it.export(docFormat(format)) }

    override fun selection(editorId: String): String = HostedEditors.call(editorId) { it.selectionJson() }

    override fun select(editorId: String, range: String): Boolean =
        HostedEditors.call(editorId) { it.selectRangeJson(range) }

    override fun selectAll(editorId: String): Boolean = HostedEditors.call(editorId) { it.selectAll() }

    override fun insertText(editorId: String, text: String): Boolean =
        HostedEditors.call(editorId) { it.insertText(text) }

    override fun undo(editorId: String): Boolean = HostedEditors.call(editorId) { it.undo() }

    override fun redo(editorId: String): Boolean = HostedEditors.call(editorId) { it.redo() }

    override fun canUndo(editorId: String): Boolean = HostedEditors.call(editorId) { it.canUndo() }

    override fun canRedo(editorId: String): Boolean = HostedEditors.call(editorId) { it.canRedo() }

    companion object {
        const val NAME = "CanvistEditors"
    }
}
