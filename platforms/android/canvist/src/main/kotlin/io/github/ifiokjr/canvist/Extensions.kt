package io.github.ifiokjr.canvist

import org.json.JSONObject
import org.json.JSONTokener

/** An extension to register with [ExtensionSet.add], which returns its typed handle. */
class Extension<H> internal constructor(
    /** The extension's name in the metadata. */
    val name: String,
    internal val handle: H,
)

/**
 * The extensions an editor registers, collected while handing out their typed
 * handles: a handle's commands exist whenever its extension does.
 *
 * ```kotlin
 * val extensions = ExtensionSet()
 * val bold = extensions.add(Extensions.bold)
 * val view = CanvistView(context, extensions)
 * view.editor.run(bold.toggleBold())
 * ```
 */
class ExtensionSet() {
    private val registered = mutableListOf<String>()

    /**
     * The extensions [names] names, for hosts that choose them at run time,
     * such as React Native and Lynx. There are no handles: run commands by
     * name. An editor fails to start with a name it does not know.
     */
    constructor(names: Iterable<String>) : this() {
        for (name in names) {
            if (name !in registered) registered += name
        }
    }

    /** The registered extensions' names, in registration order. */
    val names: List<String> get() = registered.toList()

    /** Register [extension] and return its handle. */
    fun <H> add(extension: Extension<H>): H {
        if (extension.name !in registered) registered += extension.name

        return extension.handle
    }
}

/** A query from a typed handle, answered by [Editor.query]. */
class Query<T> internal constructor(
    /** The query's name in the extension metadata. */
    val name: String,
    internal val decode: (Any?) -> T,
)

/**
 * The command [name] with [args]. Arguments that are `null` are left out, so
 * the command uses its default for them.
 */
internal fun command(name: String, args: Map<String, Any?> = emptyMap()): Command {
    val json = JSONObject()

    for ((key, value) in args) {
        if (value != null) json.put(key, value)
    }

    return Command(name, json.toString())
}

/** Decoders from the JSON queries answer with. */
internal object Decode {
    fun bool(json: Any?): Boolean = when (json) {
        is Boolean -> json
        else -> throw mismatch("a boolean", json)
    }

    fun int(json: Any?): Int = when (json) {
        is Number -> json.toInt()
        else -> throw mismatch("an integer", json)
    }

    fun double(json: Any?): Double = when (json) {
        is Number -> json.toDouble()
        else -> throw mismatch("a number", json)
    }

    fun string(json: Any?): String = when (json) {
        is String -> json
        else -> throw mismatch("a string", json)
    }

    fun <T> optional(decode: (Any?) -> T): (Any?) -> T? = { json ->
        when (json) {
            null, JSONObject.NULL -> null
            else -> decode(json)
        }
    }

    private fun mismatch(expected: String, json: Any?) =
        CanvistException.Failed("parse", "expected $expected, not $json")
}

/** Answer a query from a typed handle, such as `kit.heading.headingLevel()`. */
fun <T> Editor.query(query: Query<T>): T = query.decode(JSONTokener(query(query.name, "{}")).nextValue())
