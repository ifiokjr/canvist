package io.github.ifiokjr.canvist.sample

import android.content.Intent
import android.os.SystemClock
import android.view.WindowInsets
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.junit4.v2.createEmptyComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ActivityScenario
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.action.ViewActions.longClick
import androidx.test.espresso.action.ViewActions.click
import androidx.test.espresso.action.ViewActions.typeTextIntoFocusedView
import androidx.test.espresso.matcher.ViewMatchers.withTagValue
import androidx.test.platform.app.InstrumentationRegistry
import io.github.ifiokjr.canvist.CanvistView
import io.github.ifiokjr.canvist.DocFormat
import org.hamcrest.Matchers.`is`
import java.util.concurrent.atomic.AtomicReference
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test

/**
 * The sample as a person uses it: tapping into the editor, typing on the
 * keyboard, selecting by long press, and formatting from the toolbar.
 */
class MainActivityTest {
    @get:Rule
    val compose = createEmptyComposeRule()

    private lateinit var scenario: ActivityScenario<MainActivity>

    @Before
    fun launch() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val intent = Intent(context, MainActivity::class.java).putExtra(MainActivity.EXTRA_EMPTY_DOCUMENT, true)
        scenario = ActivityScenario.launch(intent)
    }

    @After
    fun close() = scenario.close()

    /**
     * Tap the editor tagged [tag] and type [text] on the keyboard once it
     * shows, as a person would: keys sent while the input method is still
     * starting are dropped by the input method before they reach the view.
     */
    private fun typeInto(tag: String, text: String) {
        onView(withTagValue(`is`(tag))).perform(click())
        val deadline = SystemClock.uptimeMillis() + KEYBOARD_TIMEOUT_MILLIS

        while (!read(tag) { it.rootWindowInsets?.isVisible(WindowInsets.Type.ime()) == true }) {
            check(SystemClock.uptimeMillis() < deadline) { "the keyboard did not show" }
            SystemClock.sleep(POLL_MILLIS)
        }

        onView(withTagValue(`is`(tag))).perform(typeTextIntoFocusedView(text))
    }

    /** Read the editor tagged [tag] on the main thread. */
    private fun <T> read(tag: String, body: (CanvistView) -> T): T {
        val result = AtomicReference<T>()
        scenario.onActivity { activity -> result.set(body(activity.window.decorView.findViewWithTag(tag))) }

        return result.get()
    }

    private fun markdown(tag: String = EDITOR) = read(tag) { it.editor.export(DocFormat.MARKDOWN).trim() }

    @Test
    fun typesSelectsFormatsAndUndoes() {
        typeInto(EDITOR, "hello")
        assertEquals("hello", markdown())

        onView(withTagValue(`is`(EDITOR))).perform(longClick())
        compose.waitForIdle()
        val selection = read(EDITOR) { it.editor.selection() }
        assertEquals("a long press selects the word", 0 to 5, selection.anchor.toInt() to selection.head.toInt())

        compose.onNodeWithContentDescription("Bold").performClick()
        assertEquals("**hello**", markdown())
        compose.onNodeWithContentDescription("Bold").assertIsSelected()

        compose.onNodeWithText("Undo").performClick()
        assertEquals("hello", markdown())
        compose.onNodeWithContentDescription("Bold").assertIsNotSelected()
    }

    @Test
    fun markdownShortcutsFormatAsYouType() {
        typeInto(EDITOR, "## Title")

        assertEquals("## Title", markdown())
        compose.onNodeWithContentDescription("Heading").assertIsSelected()
    }

    @Test
    fun collaboratorsSeeEachOthersTyping() {
        compose.onNodeWithText("Collaborate").performClick()
        compose.waitForIdle()

        typeInto(BOB, " Bob was here.")

        val alice = read(ALICE) { it.editor.plainText() }
        assertEquals(read(BOB) { it.editor.plainText() }, alice)
        assertTrue("Bob's typing reaches Alice", "Bob was here." in alice)
    }

    private companion object {
        const val EDITOR = "editor-editor"
        const val ALICE = "alice-editor"
        const val BOB = "bob-editor"
        const val KEYBOARD_TIMEOUT_MILLIS = 5_000L
        const val POLL_MILLIS = 50L
    }
}
