// The testkit agent: the Rust testkit daemon's instruction stream, run
// against the real canvist-android editor on this emulator.
//
// The daemon (crates/canvist_testkit/src/daemon.rs) serves one instruction
// at a time on GET /next; this test takes each, runs it against an
// `Editor`, and answers on POST /reply. The Swift agent
// (platforms/apple/Tests/CanvistTests/TestkitAgentTests.swift) and the
// loopback agent in the daemon do the same, which is what makes one test
// run on every platform.
//
// The daemon's port reaches this emulator through `adb reverse`, so
// 127.0.0.1 is this machine. Without a `testkitServer` instrumentation
// argument this test skips, so a plain `connectedDebugAndroidTest` runs as
// before.

package io.github.ifiokjr.canvist

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject
import org.junit.Assert
import org.junit.Test
import org.junit.runner.RunWith
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL

/** The Rust editor through the Kotlin binding, driven by the testkit daemon. */
@RunWith(AndroidJUnit4::class)
class TestkitAgentTest {
    private val density =
        InstrumentationRegistry
            .getInstrumentation()
            .targetContext.resources.displayMetrics.density

    /** One editor per session: parallel harnesses interleave their
     * instructions, and each owns its editor. */
    private val sessions = mutableMapOf<Int, Editor>()

    @Test
    fun runsTheTestkitInstructions() {
        val server = InstrumentationRegistry.getArguments().getString("testkitServer")

        if (server.isNullOrEmpty()) {
            return
        }

        val agent = Agent(server)

        while (true) {
            when (val flow = agent.next()) {
                Flow.Wait -> Thread.sleep(250)
                Flow.Done -> return
                is Flow.Run -> agent.reply(execute(flow.instruction))
            }
        }
    }

    /** Run one instruction against the session's editor. */
    private fun execute(instruction: Instruction): Reply =
        when (instruction) {
            is Instruction.Start -> {
                try {
                    sessions[instruction.session] = startEditor(instruction)

                    Reply(instruction.id, ok = true)
                } catch (error: Exception) {
                    Reply(instruction.id, ok = false, error = "$error")
                }
            }

            is Instruction.Op -> {
                sessions[instruction.session]?.let { run(it, instruction) }
                    ?: Reply(instruction.id, ok = false, error = "no editor for this session")
            }

            is Instruction.Read -> {
                sessions[instruction.session]?.let { answer(it, instruction) }
                    ?: Reply(instruction.id, ok = false, error = "no editor for this session")
            }

            is Instruction.Close -> {
                sessions.remove(instruction.session)?.close()

                Reply(instruction.id, ok = true)
            }
        }

    /** Build the session's editor, from a document or a shared state. */
    private fun startEditor(start: Instruction.Start): Editor {
        val options =
            defaultEditorOptions().copy(
                extensions = start.extensions,
                client = start.client.toUInt(),
                platform = if (start.apple) KeyPlatform.APPLE else KeyPlatform.OTHER,
            )
        val editor = Editor(options, PaintMeasurer(density))

        if (start.load.format == "yjs") {
            val state = android.util.Base64.decode(start.load.content, android.util.Base64.DEFAULT)

            editor.join(state, start.client.toULong())
        } else {
            editor.load(format(start.load.format), start.load.content)
        }

        return editor
    }

    private fun format(name: String): DocFormat =
        when (name) {
            "json" -> DocFormat.JSON
            "html" -> DocFormat.HTML
            "markdown" -> DocFormat.MARKDOWN
            else -> DocFormat.TEXT
        }

    /** One operation, through the same FFI calls an app makes. */
    private fun run(
        session: Editor,
        instruction: Instruction.Op,
    ): Reply =
        try {
            val value: Any? =
                when (val op = instruction.op) {
                    is TestkitOp.Load -> {
                        session.load(format(op.format), op.content)

                        null
                    }

                    is TestkitOp.TypeText -> {
                        var changed = false

                        for (character in op.text) {
                            changed = session.insertText(character.toString()) || changed
                        }

                        changed
                    }

                    is TestkitOp.InsertText -> {
                        session.insertText(op.text)
                    }

                    is TestkitOp.Press -> {
                        session.press(
                            op.key,
                            Modifiers(op.shift, op.ctrl, op.alt, op.meta),
                        )
                    }

                    is TestkitOp.Select -> {
                        val head = op.head ?: op.anchor

                        session.selectPositions(
                            DocPosition(op.anchor.path.map { it.toUInt() }, op.anchor.offset.toUInt()),
                            DocPosition(head.path.map { it.toUInt() }, head.offset.toUInt()),
                        )
                    }

                    TestkitOp.SelectAll -> {
                        session.selectAll()
                    }

                    TestkitOp.DeleteSelection -> {
                        session.press(
                            "Backspace",
                            Modifiers(shift = false, ctrl = false, alt = false, meta = false),
                        )
                    }

                    TestkitOp.Paste -> {
                        return Reply(
                            instruction.id,
                            ok = false,
                            error = "the Android binding has no paste; test pasting on another target",
                        )
                    }

                    is TestkitOp.Run -> {
                        session.run(Command(op.command, op.args ?: "null"))
                    }

                    is TestkitOp.Can -> {
                        session.can(Command(op.command, op.args ?: "null"))
                    }

                    is TestkitOp.Query -> {
                        parse(session.query(op.query, op.args ?: "null"))
                    }

                    TestkitOp.Undo -> {
                        session.undo()
                    }

                    TestkitOp.Redo -> {
                        session.redo()
                    }

                    is TestkitOp.SetMarked -> {
                        session.setMarkedText(
                            op.text,
                            OffsetRange(op.selected.first.toUInt(), op.selected.second.toUInt()),
                        )
                    }

                    TestkitOp.UnmarkText -> {
                        session.unmarkText()
                    }

                    is TestkitOp.Share -> {
                        session.share(op.client.toULong())

                        null
                    }

                    is TestkitOp.ApplyUpdate -> {
                        session.applyUpdate(
                            android.util.Base64.decode(op.update, android.util.Base64.DEFAULT),
                        )
                    }

                    TestkitOp.TakeUpdates -> {
                        JSONArray(
                            session.takeUpdates().map { update ->
                                android.util.Base64.encodeToString(update, android.util.Base64.NO_WRAP)
                            },
                        )
                    }

                    TestkitOp.EncodeState -> {
                        android.util.Base64.encodeToString(
                            session.encodeState(),
                            android.util.Base64.NO_WRAP,
                        )
                    }
                }

            Reply(instruction.id, ok = true, value = value)
        } catch (error: Exception) {
            Reply(instruction.id, ok = false, error = "$error")
        }

    /** Answer one read. */
    private fun answer(
        session: Editor,
        read: Instruction.Read,
    ): Reply {
        val value: Any? =
            when (read.what) {
                "text" -> {
                    session.export(DocFormat.TEXT)
                }

                "markdown" -> {
                    session.export(DocFormat.MARKDOWN)
                }

                "html" -> {
                    session.export(DocFormat.HTML)
                }

                "json" -> {
                    session.export(DocFormat.JSON)
                }

                "selection" -> {
                    session.selectedPositions()?.takeIf { it.size >= 2 }?.let {
                        JSONObject()
                            .put("anchor", positionJson(it[0]))
                            .put("head", positionJson(it[1]))
                    }
                }

                "canUndo" -> {
                    session.canUndo()
                }

                "canRedo" -> {
                    session.canRedo()
                }

                else -> {
                    null
                }
            }

        return Reply(read.id, ok = true, value = value)
    }

    private fun positionJson(position: DocPosition): JSONObject =
        JSONObject()
            .put("path", JSONArray(position.path.map { it.toInt() }))
            .put("offset", position.offset.toInt())

    /** A query's answer, which may be a bare `true`: JSONTokener allows
     * fragments where JSONObject does not. */
    private fun parse(json: String): Any? =
        try {
            org.json.JSONTokener(json).nextValue()
        } catch (_: JSONException) {
            json
        }
}

/** GET /next and POST /reply, over the daemon's tiny JSON HTTP. */
private class Agent(
    private val server: String,
) {
    fun next(): Flow {
        val json = JSONObject(request("GET", "/next?agent=android", null))

        return when (json.optString("kind", "wait")) {
            "done" -> Flow.Done
            "wait" -> Flow.Wait
            else -> Flow.Run(Instruction.of(json))
        }
    }

    fun reply(reply: Reply) {
        request(
            "POST",
            "/reply",
            JSONObject()
                .put("id", reply.id)
                .put("ok", reply.ok)
                .put("value", reply.value ?: JSONObject.NULL)
                .put(
                    "error",
                    reply.error ?: JSONObject.NULL,
                ),
        )
    }

    /** One synchronous request: connect, write, read. */
    private fun request(
        method: String,
        path: String,
        body: JSONObject?,
    ): String {
        val connection = URL(server.trimEnd('/') + path).openConnection() as HttpURLConnection

        connection.requestMethod = method
        connection.connectTimeout = 90_000
        connection.readTimeout = 90_000

        if (body != null) {
            connection.doOutput = true
            connection.setRequestProperty("Content-Type", "application/json")
            connection.outputStream.use { output ->
                output.write(body.toString().toByteArray())
            }
        }

        val text =
            connection.inputStream.use { input ->
                input.bufferedReader().readText()
            }
        connection.disconnect()
        Assert.assertTrue("empty reply from $path", text.isNotEmpty())

        return text
    }
}

/** What the daemon said: work, patience, or the end. */
private sealed class Flow {
    object Wait : Flow()

    object Done : Flow()

    data class Run(
        val instruction: Instruction,
    ) : Flow()
}

/** The document a session starts from. */
private data class StartDoc(
    val format: String,
    val content: String,
)

/** One instruction, from the daemon's JSON. */
private sealed class Instruction {
    data class Start(
        val id: Long,
        val session: Int,
        val extensions: List<String>,
        val client: Int,
        val apple: Boolean,
        val load: StartDoc,
    ) : Instruction()

    data class Op(
        val id: Long,
        val session: Int,
        val op: TestkitOp,
    ) : Instruction()

    data class Read(
        val id: Long,
        val session: Int,
        val what: String,
    ) : Instruction()

    data class Close(
        val id: Long,
        val session: Int,
    ) : Instruction()

    companion object {
        fun of(json: JSONObject): Instruction {
            val id = json.optLong("id")

            return when (json.optString("kind")) {
                "start" -> {
                    Start(
                        id,
                        json.optInt("session"),
                        json.optJSONArray("extensions")?.let { names ->
                            List(names.length()) { names.optString(it) }
                        } ?: emptyList(),
                        json.optInt("client", 1),
                        json.optBoolean("apple"),
                        StartDoc(
                            json.optJSONObject("load")?.optString("format") ?: "text",
                            json.optJSONObject("load")?.optString("content") ?: "",
                        ),
                    )
                }

                "op" -> {
                    Op(
                        id,
                        json.optInt("session"),
                        TestkitOp.of(json.optJSONObject("op") ?: JSONObject()),
                    )
                }

                "read" -> {
                    Read(id, json.optInt("session"), json.optString("what", "text"))
                }

                else -> {
                    Close(id, json.optInt("session"))
                }
            }
        }
    }
}

/** One operation, from the daemon's JSON. */
private sealed class TestkitOp {
    data class Load(
        val format: String,
        val content: String,
    ) : TestkitOp()

    data class TypeText(
        val text: String,
    ) : TestkitOp()

    data class InsertText(
        val text: String,
    ) : TestkitOp()

    data class Press(
        val key: String,
        val shift: Boolean,
        val ctrl: Boolean,
        val alt: Boolean,
        val meta: Boolean,
    ) : TestkitOp()

    data class Select(
        val anchor: TestPosition,
        val head: TestPosition?,
    ) : TestkitOp()

    object SelectAll : TestkitOp()

    object DeleteSelection : TestkitOp()

    object Paste : TestkitOp()

    data class Run(
        val command: String,
        val args: String?,
    ) : TestkitOp()

    data class Can(
        val command: String,
        val args: String?,
    ) : TestkitOp()

    data class Query(
        val query: String,
        val args: String?,
    ) : TestkitOp()

    object Undo : TestkitOp()

    object Redo : TestkitOp()

    data class SetMarked(
        val text: String,
        val selected: Pair<Int, Int>,
    ) : TestkitOp()

    object UnmarkText : TestkitOp()

    data class Share(
        val client: Int,
    ) : TestkitOp()

    data class ApplyUpdate(
        val update: String,
    ) : TestkitOp()

    object TakeUpdates : TestkitOp()

    object EncodeState : TestkitOp()

    data class TestPosition(
        val path: List<Int>,
        val offset: Int,
    )

    companion object {
        private fun position(json: JSONObject): TestPosition =
            TestPosition(
                json.optJSONArray("path")?.let { path ->
                    List(path.length()) { path.optInt(it) }
                } ?: emptyList(),
                json.optInt("offset"),
            )

        private fun arguments(value: Any?): String? =
            when (value) {
                null, JSONObject.NULL -> null
                else -> value.toString()
            }

        fun of(json: JSONObject): TestkitOp =
            when (json.optString("type")) {
                "load" -> {
                    Load(json.optString("format", "text"), json.optString("content"))
                }

                "typeText" -> {
                    TypeText(json.optString("text"))
                }

                "insertText" -> {
                    InsertText(json.optString("text"))
                }

                "press" -> {
                    Press(
                        json.optString("key"),
                        json.optBoolean("shift"),
                        json.optBoolean("ctrl"),
                        json.optBoolean("alt"),
                        json.optBoolean("meta"),
                    )
                }

                "select" -> {
                    Select(
                        position(json.optJSONObject("anchor") ?: JSONObject()),
                        json.optJSONObject("head")?.let(::position),
                    )
                }

                "selectAll" -> {
                    SelectAll
                }

                "deleteSelection" -> {
                    DeleteSelection
                }

                "paste" -> {
                    Paste
                }

                "run" -> {
                    Run(json.optString("command"), arguments(json.opt("args")))
                }

                "can" -> {
                    Can(json.optString("command"), arguments(json.opt("args")))
                }

                "query" -> {
                    Query(json.optString("query"), arguments(json.opt("args")))
                }

                "undo" -> {
                    Undo
                }

                "redo" -> {
                    Redo
                }

                "setMarkedText" -> {
                    val selected = json.optJSONArray("selected")

                    SetMarked(
                        json.optString("text"),
                        Pair(selected?.optInt(0) ?: 0, selected?.optInt(1) ?: 0),
                    )
                }

                "unmarkText" -> {
                    UnmarkText
                }

                "share" -> {
                    Share(json.optInt("client", 1))
                }

                "applyUpdate" -> {
                    ApplyUpdate(json.optString("update"))
                }

                "takeUpdates" -> {
                    TakeUpdates
                }

                "encodeState" -> {
                    EncodeState
                }

                else -> {
                    throw IOException("unknown op $json")
                }
            }
    }
}

/** One reply, in the shape wire.rs reads. */
private data class Reply(
    val id: Long,
    val ok: Boolean,
    val value: Any? = null,
    val error: String? = null,
)
