package io.github.ifiokjr.canvist

import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject
import java.lang.ref.WeakReference
import java.util.concurrent.ConcurrentHashMap

/**
 * The editors of views a JavaScript host shows, such as React Native or Lynx,
 * by the id the host gave each view.
 *
 * A host's JavaScript calls arrive on its own thread while the views run on
 * the main thread. That is safe: [Editor] serialises every call, and
 * [CanvistView] hears about changes on the main thread wherever they came
 * from.
 */
object HostedEditors {
    /** Weak, so an editor never outlives its view even if a host forgets to unregister it. */
    private val editors = ConcurrentHashMap<String, WeakReference<Editor>>()

    /** Find [editor] by [id] from now on. */
    fun register(
        id: String,
        editor: Editor,
    ) {
        editors[id] = WeakReference(editor)
    }

    /** Forget the editor [id] names. */
    fun unregister(id: String) {
        editors.remove(id)
    }

    /** Run [body] on the editor [id] names, with every failure as a [HostedEditorException]. */
    fun <T> call(
        id: String,
        body: (Editor) -> T,
    ): T =
        try {
            val editor =
                editors[id]?.get()
                    ?: throw HostedEditorException("editor", "no mounted editor has the id `$id`")
            body(editor)
        } catch (error: Exception) {
            throw HostedEditorException.of(error)
        }
}

/**
 * A failure for a JavaScript host to report, with the stable [code] every
 * binding shares. Its message is `[canvist:<code>] <reason>`, the form
 * JavaScript reads the code from (`@canvist/headless/native`).
 */
class HostedEditorException(
    val code: String,
    val reason: String,
    cause: Throwable? = null,
) : RuntimeException("[canvist:$code] $reason", cause) {
    companion object {
        /** [error] with its code: an engine error keeps its own, anything else is an `editor` error. */
        fun of(error: Exception): HostedEditorException =
            when (error) {
                is HostedEditorException -> error
                is CanvistException.Failed -> HostedEditorException(error.code, error.reason, error)
                else -> HostedEditorException("editor", error.message ?: error.toString(), error)
            }
    }
}

/** The format a JavaScript name stands for: `json`, `html`, `markdown`, or `text`. */
fun docFormat(name: String): DocFormat =
    when (name) {
        "json" -> DocFormat.JSON
        "html" -> DocFormat.HTML
        "markdown" -> DocFormat.MARKDOWN
        "text" -> DocFormat.TEXT
        else -> throw HostedEditorException("invalidArgs", "unknown format `$name`")
    }

/**
 * The selection as JSON, `{"anchor": position, "head": position}` with
 * positions as `{"path": [...], "offset": n}`, or `null` for a block
 * selection: the `Range` JavaScript reads.
 */
fun Editor.selectionJson(): String {
    val positions = selectedPositions()?.takeIf { it.size == 2 } ?: return "null"

    return JSONObject()
        .put("anchor", positionJson(positions[0]))
        .put("head", positionJson(positions[1]))
        .toString()
}

/** Select the JSON range [json], as [selectionJson] writes it. Returns whether the selection moved. */
fun Editor.selectRangeJson(json: String): Boolean {
    val (anchor, head) =
        try {
            val range = JSONObject(json)
            decodePosition(range.getJSONObject("anchor")) to decodePosition(range.getJSONObject("head"))
        } catch (error: JSONException) {
            throw HostedEditorException("invalidPosition", "invalid range $json: ${error.message}", error)
        }

    return selectPositions(anchor, head)
}

private fun positionJson(position: DocPosition): JSONObject =
    JSONObject()
        .put("path", JSONArray(position.path.map { it.toLong() }))
        .put("offset", position.offset.toLong())

private fun decodePosition(json: JSONObject): DocPosition {
    val path = json.getJSONArray("path")

    return DocPosition(
        path = List(path.length()) { index -> unsigned(path.getLong(index)) },
        offset = unsigned(json.getLong("offset")),
    )
}

private fun unsigned(value: Long): UInt {
    if (value !in 0..UInt.MAX_VALUE.toLong()) {
        throw HostedEditorException("invalidPosition", "$value is not a valid index")
    }

    return value.toUInt()
}
