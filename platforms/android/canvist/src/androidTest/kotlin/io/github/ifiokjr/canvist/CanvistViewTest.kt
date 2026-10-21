package io.github.ifiokjr.canvist

import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.KeyEvent
import android.view.View
import android.view.accessibility.AccessibilityNodeInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The view driven the way Android drives it: an input method through the
 * [InputConnection] (the calls Gboard makes while composing, correcting, and
 * deleting), hardware keys, painting, and TalkBack's accessibility actions.
 */
@RunWith(AndroidJUnit4::class)
class CanvistViewTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val kit = StarterKit()

    /** Run [body] on the main thread with a laid-out view showing [markdown]. */
    private fun <T> withView(markdown: String = "", body: (CanvistView, InputConnection) -> T): T {
        var result: Result<T>? = null

        instrumentation.runOnMainSync {
            val view = CanvistView(instrumentation.targetContext, kit.extensions)
            view.editor.load(DocFormat.MARKDOWN, markdown)
            view.measure(exactly(WIDTH), exactly(HEIGHT))
            view.layout(0, 0, WIDTH, HEIGHT)
            draw(view)
            val connection = view.onCreateInputConnection(EditorInfo())
            result = runCatching { body(view, connection) }
        }

        return result!!.getOrThrow()
    }

    private fun exactly(size: Int) = View.MeasureSpec.makeMeasureSpec(size, View.MeasureSpec.EXACTLY)

    private fun draw(view: CanvistView): Bitmap {
        val bitmap = Bitmap.createBitmap(WIDTH, HEIGHT, Bitmap.Config.ARGB_8888)
        view.draw(Canvas(bitmap))

        return bitmap
    }

    private fun selection(view: CanvistView) = view.editor.selection().let { it.anchor.toInt() to it.head.toInt() }

    @Test
    fun composesLikeGboard() = withView { view, input ->
        input.setComposingText("h", 1)
        input.setComposingText("hel", 1)
        assertEquals(OffsetRange(0u, 3u), view.editor.markedRange())

        input.commitText("hello", 1)
        input.commitText(" ", 1)

        assertEquals("hello ", view.editor.plainText())
        assertNull(view.editor.markedRange())
        assertEquals(6 to 6, selection(view))
    }

    @Test
    fun recomposesAWordForASuggestion() = withView("helo world") { view, input ->
        input.setSelection(2, 2)
        input.setComposingRegion(0, 4)
        assertEquals(OffsetRange(0u, 4u), view.editor.markedRange())

        input.commitText("hello", 1)

        assertEquals("hello world", view.editor.plainText())
        assertEquals(5 to 5, selection(view))
    }

    @Test
    fun reportsTextAroundTheCursor() = withView("First block\n\nsecond one") { view, input ->
        input.setSelection(12, 18)

        assertEquals("First block\n", input.getTextBeforeCursor(100, 0))
        assertEquals("ock\n", input.getTextBeforeCursor(4, 0))
        assertEquals("second", input.getSelectedText(0))
        assertEquals(" one", input.getTextAfterCursor(100, 0))

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val around = input.getSurroundingText(3, 3, 0)!!
            assertEquals("ck\nsecond on", around.text.toString())
            assertEquals(3, around.selectionStart)
        }

        assertTrue(view.editor.selection().anchor != view.editor.selection().head)
    }

    @Test
    fun deletesAroundTheCursorAndJoinsBlocks() = withView("ab\n\ncd😀e") { view, input ->
        input.setSelection(3, 3)
        input.deleteSurroundingText(1, 0)
        assertEquals("Backspace at a block's start joins it", "abcd😀e", view.editor.plainText())

        input.setSelection(6, 6)
        input.deleteSurroundingTextInCodePoints(1, 1)
        assertEquals("abcd", view.editor.plainText())
    }

    @Test
    fun enterSplitsBlocksAndContinuesLists() = withView("- one") { view, input ->
        input.setSelection(3, 3)
        input.commitText("\n", 1)
        input.commitText("two", 1)
        input.performEditorAction(EditorInfo.IME_ACTION_UNSPECIFIED)

        assertEquals("- one\n- two\n-", view.editor.export(DocFormat.MARKDOWN).trim())
    }

    @Test
    fun hardwareShortcutsRunKeyBindings() = withView("bold") { view, input ->
        input.setSelection(0, 4)
        val control = KeyEvent.META_CTRL_ON or KeyEvent.META_CTRL_LEFT_ON
        input.sendKeyEvent(KeyEvent(0, 0, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_B, 0, control))

        assertEquals("**bold**", view.editor.export(DocFormat.MARKDOWN))
    }

    /** LatinIME marks a hardware-typed word for suggestions; the next key adds to it. */
    @Test
    fun typedKeysEndTheComposingRegion() = withView("h") { view, input ->
        input.setSelection(1, 1)
        input.setComposingRegion(0, 1)
        view.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_E))

        assertEquals("he", view.editor.plainText())
        assertNull(view.editor.markedRange())
    }

    @Test
    fun paintsTheDocument() = withView("# Title\n\n- [x] done") { view, _ ->
        val bitmap = draw(view)
        val texts = view.items.filterIsInstance<DisplayItem.Text>().joinToString("") { it.text }

        assertEquals("Titledone", texts)
        assertTrue(view.items.any { it is DisplayItem.Checkbox })
        val pixels = IntArray(WIDTH * HEIGHT)
        bitmap.getPixels(pixels, 0, WIDTH, 0, 0, WIDTH, HEIGHT)
        val inked = pixels.count { it != pixels[0] }
        val density = view.resources.displayMetrics.density
        assertTrue("text is painted: $inked pixels of ink", inked > INKED_DP * density * density)
    }

    @Test
    fun talkBackReadsAndMovesThroughTheText() = withView("# Heading\n\nwords to read") { view, _ ->
        val provider = view.accessibilityNodeProvider
        val host = provider.createAccessibilityNodeInfo(View.NO_ID)!!
        assertEquals("Heading\nwords to read", host.text.toString())
        assertTrue(host.isEditable)
        assertTrue(host.childCount >= 2)

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            assertTrue(provider.createAccessibilityNodeInfo(1)!!.isHeading)
        }

        val select = Bundle().apply {
            putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, 8)
            putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, 8)
        }
        assertTrue(provider.performAction(View.NO_ID, AccessibilityNodeInfo.ACTION_SET_SELECTION, select))

        val word = Bundle().apply {
            val granularity = AccessibilityNodeInfo.MOVEMENT_GRANULARITY_WORD
            putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_MOVEMENT_GRANULARITY_INT, granularity)
        }
        provider.performAction(View.NO_ID, AccessibilityNodeInfo.ACTION_NEXT_AT_MOVEMENT_GRANULARITY, word)
        assertEquals(13 to 13, selection(view))
    }

    /**
     * Keystroke to paint: a character committed through the input connection,
     * the relayout, the display list, and drawing the view on a canvas.
     */
    @Test
    fun keystrokeToPaintStaysWithinBudget() {
        val paragraph = "The quick brown fox jumps over the lazy dog, again and again."
        val samples = withView(List(PARAGRAPHS) { paragraph }.joinToString("\n\n")) { view, input ->
            input.setSelection(10, 10)
            val canvas = Canvas(Bitmap.createBitmap(WIDTH, HEIGHT, Bitmap.Config.ARGB_8888))

            DoubleArray(KEYSTROKES) { index ->
                val start = System.nanoTime()

                if (index % 2 == 0) input.commitText("x", 1) else input.deleteSurroundingText(1, 0)

                view.draw(canvas)
                (System.nanoTime() - start) / NANOS_PER_MICRO
            }.sorted()
        }
        val median = samples[samples.size / 2]
        val p95 = samples[samples.size * P95 / 100]

        Log.i(TAG, "PERF kotlin(${Build.SUPPORTED_ABIS[0]}) keystroke-to-paint median=${median.toLong()}µs p95=${p95.toLong()}µs")
        assertTrue("keystroke-to-paint p95 ${p95}µs is over its budget", p95 < BUDGET_MICROS * perfSlack)
    }

    private companion object {
        const val TAG = "CanvistPerf"
        const val WIDTH = 1080
        const val HEIGHT = 1920
        const val PARAGRAPHS = 2_000
        /** Ink the title and the checkbox cover at the very least, in dp². */
        const val INKED_DP = 150
        const val KEYSTROKES = 300
        const val P95 = 95
        const val NANOS_PER_MICRO = 1_000.0

        /** One frame at 60 Hz, with room for emulators under CI load. */
        const val BUDGET_MICROS = 16_000.0
    }
}
