package io.github.ifiokjr.canvist

import android.content.pm.ApplicationInfo
import android.os.Build
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** The Rust editor through the Kotlin binding, measuring with `Paint`. */
@RunWith(AndroidJUnit4::class)
class EditorTest {
    private val kit = StarterKit()
    private val density =
        InstrumentationRegistry
            .getInstrumentation()
            .targetContext.resources.displayMetrics.density

    private fun editor(markdown: String = ""): Editor {
        val options = defaultEditorOptions().copy(extensions = kit.extensions.names)

        return Editor(options, PaintMeasurer(density)).apply {
            load(DocFormat.MARKDOWN, markdown)
            setViewport(WIDTH, HEIGHT)
        }
    }

    @Test
    fun typingRunsInputRules() =
        editor().use { editor ->
            "## Title".forEach { character -> editor.insertText(character.toString()) }

            assertEquals("## Title", editor.export(DocFormat.MARKDOWN))
            assertEquals(2, editor.query(kit.heading.headingLevel()))
        }

    @Test
    fun typedCommandsAndQueries() =
        editor("Hello world").use { editor ->
            editor.select(6u, 11u)

            assertTrue(editor.can(kit.bold.toggleBold()))
            assertTrue(editor.run(kit.bold.toggleBold()))
            assertTrue(editor.query(kit.bold.isBold()))
            assertTrue(editor.run(kit.codeBlock.toggleCodeBlock(language = "kotlin")))
            assertEquals("kotlin", editor.query(kit.codeBlock.codeBlockLanguage()))
            assertNull(editor.query(kit.link.linkHref()))
        }

    @Test
    fun extensionSetsFromNamesKeepTheirOrderOnce() {
        val extensions = ExtensionSet(listOf("bold", "heading", "bold"))

        assertEquals(listOf("bold", "heading"), extensions.names)
        Editor(CanvistView.editorOptions(extensions), PaintMeasurer(density)).close()
        assertThrows(CanvistException::class.java) {
            Editor(CanvistView.editorOptions(ExtensionSet(listOf("nope"))), PaintMeasurer(density)).close()
        }
    }

    @Test
    fun invalidArgumentsThrowTheSharedCode() =
        editor("text").use { editor ->
            val error =
                assertThrows(CanvistException.Failed::class.java) {
                    editor.run(kit.heading.toggleHeading(level = INVALID_LEVEL))
                }

            assertEquals("invalidArgs", error.code)
        }

    @Test
    fun commandArgumentsAreJson() {
        assertEquals("""{"href":"https:\/\/example.com"}""", kit.link.setLink(href = "https://example.com").args)
        assertEquals("{}", kit.codeBlock.setCodeBlock().args)
    }

    @Test
    fun composingTextCommits() =
        editor().use { editor ->
            editor.setMarkedText("h", OffsetRange(1u, 1u))
            editor.setMarkedText("hel", OffsetRange(3u, 3u))
            assertEquals(OffsetRange(0u, 3u), editor.markedRange())
            editor.insertText("hello")

            assertNull(editor.markedRange())
            assertEquals("hello", editor.plainText())
            assertEquals(TextSelection(5u, 5u), editor.selection())
        }

    @Test
    fun framesPaintTextInTheMeasuredFont() =
        editor("# Big\n\nsmall").use { editor ->
            val frame = editor.frame()
            val runs = frame.items.orEmpty().filterIsInstance<DisplayItem.Text>()

            assertEquals(listOf("Big", "small"), runs.map { it.text })
            assertNotEquals(runs[0].font, runs[1].font)
            assertTrue(frame.height > 0f)
        }

    @Test
    fun measuredAdvancesAddUpToTheRunWidth() {
        val fonts = FontTable(density)
        fonts.register(FontFace(0u, listOf("system-ui"), BODY_SIZE, REGULAR, false))
        val paint = checkNotNull(fonts.paint(0u))
        val samples =
            listOf(
                "Typography office" to false,
                "naïve café" to false,
                "👩🏽‍💻 ok" to false,
                "مرحبا بالعالم" to true,
                "漢字かな" to false,
            )

        for ((text, rtl) in samples) {
            val advances = mutableListOf<Float>()
            PaintMeasurer.advances(text, paint, density, advances)
            // What `drawTextRun` advances the pen by for the whole run.
            val width = paint.getRunAdvance(text, 0, text.length, 0, text.length, rtl, text.length)

            assertEquals(text, text.codePointCount(0, text.length), advances.size)
            assertEquals(text, width / density, advances.sum(), TOLERANCE)
        }
    }

    @Test
    fun twoEditorsCollaborate() {
        val first = editor("Hello")
        val second = editor()

        try {
            first.share(1uL)
            second.join(first.encodeState(), 2uL)
            first.select(5u, 5u)
            first.insertText(" world")
            first.takeUpdates().forEach { update -> second.applyUpdate(update) }

            assertEquals("Hello world", second.plainText())
        } finally {
            first.close()
            second.close()
        }
    }

    /**
     * The editor half of keystroke-to-paint through the Kotlin binding, `Paint`
     * measuring included: type a character and build its frame.
     */
    @Test
    fun keystrokeToFrameStaysWithinBudget() {
        val paragraph = "The quick brown fox jumps over the lazy dog, again and again."

        editor(List(PARAGRAPHS) { paragraph }.joinToString("\n\n")).use { editor ->
            editor.setFocused(true)
            editor.frame()
            editor.select(10u, 10u)
            editor.frame()

            val samples = DoubleArray(KEYSTROKES) { index -> keystroke(editor, typing = index % 2 == 0) }.sorted()
            val median = samples[samples.size / 2]
            val p95 = samples[samples.size * P95 / 100]

            Log.i(
                TAG,
                "PERF kotlin(${build()}, ${Build.SUPPORTED_ABIS[0]}) keystroke-to-frame median=${median.toLong()}µs p95=${p95.toLong()}µs",
            )
            assertTrue("keystroke-to-frame p95 ${p95}µs is over its budget", p95 < BUDGET_MICROS * perfSlack)
        }
    }

    /** Type a character, or delete one, and build the frame; returns microseconds. */
    private fun keystroke(
        editor: Editor,
        typing: Boolean,
    ): Double {
        val start = System.nanoTime()

        if (typing) editor.insertText("x") else editor.deleteBackward()

        editor.frame()

        return (System.nanoTime() - start) / NANOS_PER_MICRO
    }

    private fun build(): String {
        val flags =
            InstrumentationRegistry
                .getInstrumentation()
                .targetContext.applicationInfo.flags

        return if (flags and ApplicationInfo.FLAG_DEBUGGABLE != 0) "debug" else "release"
    }

    private companion object {
        const val TAG = "CanvistPerf"
        const val WIDTH = 360f
        const val HEIGHT = 720f
        const val BODY_SIZE = 17f
        val REGULAR: UShort = 400u
        const val TOLERANCE = 0.01f
        const val INVALID_LEVEL = 9
        const val PARAGRAPHS = 2_000
        const val KEYSTROKES = 400
        const val P95 = 95
        const val NANOS_PER_MICRO = 1_000.0

        /** Generous for emulators under CI load; devices are far quicker. */
        const val BUDGET_MICROS = 8_000.0
    }
}
