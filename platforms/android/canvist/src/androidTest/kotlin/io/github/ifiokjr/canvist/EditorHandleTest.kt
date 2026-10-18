package io.github.ifiokjr.canvist

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class EditorHandleTest {
    @Test
    fun startsEmpty() {
        EditorHandle().use { editor -> assertEquals("", editor.text()) }
    }

    @Test
    fun insertsTextThroughRust() {
        EditorHandle().use { editor ->
            editor.insertText(0u, "world")
            editor.insertText(0u, "hello ")
            assertEquals("hello world", editor.text())
        }
    }

    @Test
    fun offsetsCountUnicodeScalarValues() {
        EditorHandle().use { editor ->
            editor.insertText(0u, "héllo")
            editor.insertText(2u, "—")
            assertEquals("hé—llo", editor.text())
        }
    }

    @Test
    fun rejectsOffsetsPastTheEnd() {
        EditorHandle().use { editor ->
            val error = assertThrows(EditorException.OffsetOutOfBounds::class.java) {
                editor.insertText(1u, "x")
            }
            assertEquals(1u, error.offset)
            assertEquals(0u, error.length)
            assertEquals("", editor.text())
        }
    }

    @Test
    fun reportsTheRustCoreVersion() {
        EditorHandle().use { editor ->
            val version = editor.version()
            assertTrue("unexpected version $version", Regex("""\d+\.\d+\.\d+.*""").matches(version))
        }
    }
}
