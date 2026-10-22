package io.github.ifiokjr.canvist.lynx

import android.content.Context
import com.lynx.jsbridge.LynxMethod
import com.lynx.jsbridge.LynxModule
import io.github.ifiokjr.canvist.Command
import io.github.ifiokjr.canvist.Editor
import io.github.ifiokjr.canvist.HostedEditorException
import io.github.ifiokjr.canvist.HostedEditors
import io.github.ifiokjr.canvist.docFormat
import io.github.ifiokjr.canvist.selectRangeJson
import io.github.ifiokjr.canvist.selectionJson
import org.json.JSONObject

/**
 * The `CanvistEditors` native module: synchronous calls, on the JavaScript
 * thread, into the editors of `<canvist-editor>` elements. Every method
 * answers with a JSON envelope, `{"value": ...}` or `{"error": message}`,
 * because Lynx reports native exceptions to the host rather than to
 * JavaScript.
 */
class CanvistLynxModule(context: Context) : LynxModule(context) {
    @LynxMethod
    fun run(editorId: String, name: String, args: String): String = reply(editorId) { it.run(Command(name, args)).json() }

    @LynxMethod
    fun can(editorId: String, name: String, args: String): String = reply(editorId) { it.can(Command(name, args)).json() }

    @LynxMethod
    fun query(editorId: String, name: String, args: String): String = reply(editorId) { it.query(name, args) }

    @LynxMethod
    fun load(editorId: String, format: String, content: String): String = reply(editorId) {
        it.load(docFormat(format), content)
        "true"
    }

    @LynxMethod
    fun exportDocument(editorId: String, format: String): String =
        reply(editorId) { JSONObject.quote(it.export(docFormat(format))) }

    @LynxMethod
    fun selection(editorId: String): String = reply(editorId) { it.selectionJson() }

    @LynxMethod
    fun select(editorId: String, range: String): String = reply(editorId) { it.selectRangeJson(range).json() }

    @LynxMethod
    fun selectAll(editorId: String): String = reply(editorId) { it.selectAll().json() }

    @LynxMethod
    fun insertText(editorId: String, text: String): String = reply(editorId) { it.insertText(text).json() }

    @LynxMethod
    fun undo(editorId: String): String = reply(editorId) { it.undo().json() }

    @LynxMethod
    fun redo(editorId: String): String = reply(editorId) { it.redo().json() }

    @LynxMethod
    fun canUndo(editorId: String): String = reply(editorId) { it.canUndo().json() }

    @LynxMethod
    fun canRedo(editorId: String): String = reply(editorId) { it.canRedo().json() }

    companion object {
        const val NAME = "CanvistEditors"
    }
}

/** The envelope for [body]'s JSON answer on the editor [editorId] names, or for its failure. */
private fun reply(editorId: String, body: (Editor) -> String): String = try {
    "{\"value\":${HostedEditors.call(editorId, body)}}"
} catch (error: HostedEditorException) {
    "{\"error\":${JSONObject.quote(error.message)}}"
}

private fun Boolean.json() = if (this) "true" else "false"
