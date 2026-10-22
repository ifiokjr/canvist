package io.github.ifiokjr.canvist

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.UUID
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** What React Native and Lynx call through: editors by id, JSON ranges, and failures carrying their codes. */
@RunWith(AndroidJUnit4::class)
class HostedEditorsTest {
    private val density = InstrumentationRegistry.getInstrumentation().targetContext.resources.displayMetrics.density

    private fun editor(markdown: String): Editor =
        Editor(CanvistView.editorOptions(StarterKit().extensions), PaintMeasurer(density)).apply {
            load(DocFormat.MARKDOWN, markdown)
        }

    @Test
    fun callsReachTheEditorAnIdNamesUntilItIsUnregistered() = editor("Hello").use { editor ->
        val id = "hosted-${UUID.randomUUID()}"
        HostedEditors.register(id, editor)

        assertEquals("Hello", HostedEditors.call(id) { it.export(DocFormat.TEXT) })

        HostedEditors.unregister(id)
        val failure = assertThrows(HostedEditorException::class.java) { HostedEditors.call(id) { it.canUndo() } }
        assertEquals("editor", failure.code)
    }

    @Test
    fun rangesRoundTripAsJson() = editor("# Title\n\nBody text").use { editor ->
        val range = """{"anchor":{"path":[1],"offset":0},"head":{"path":[1],"offset":4}}"""

        assertTrue(editor.selectRangeJson(range))
        assertEquals(range, editor.selectionJson())
    }

    @Test
    fun failuresCarryTheirCodes() = editor("text").use { editor ->
        val id = "hosted-${UUID.randomUUID()}"
        HostedEditors.register(id, editor)

        val position = assertThrows(HostedEditorException::class.java) {
            HostedEditors.call(id) { it.selectRangeJson("""{"anchor":{}}""") }
        }
        assertEquals("invalidPosition", position.code)
        assertEquals("invalidArgs", assertThrows(HostedEditorException::class.java) { docFormat("rtf") }.code)

        val args = assertThrows(HostedEditorException::class.java) {
            HostedEditors.call(id) { it.run(Command("toggleHeading", """{"level":9}""")) }
        }
        assertEquals("invalidArgs", args.code)
        assertTrue(args.message!!.startsWith("[canvist:invalidArgs] "))
        HostedEditors.unregister(id)
    }
}
