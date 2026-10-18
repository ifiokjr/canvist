package io.github.ifiokjr.canvist.sample

import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import org.junit.Rule
import org.junit.Test

class MainActivityTest {
    @get:Rule
    val compose = createAndroidComposeRule<MainActivity>()

    @Test
    fun appendingTextRoundTripsThroughRust() {
        compose.onNodeWithTag("document").assertTextEquals("Empty")

        compose.onNodeWithTag("draft").performTextInput("hello")
        compose.onNodeWithTag("append").performClick()
        compose.onNodeWithTag("document").assertTextEquals("hello")

        compose.onNodeWithTag("draft").performTextInput(" world")
        compose.onNodeWithTag("append").performClick()
        compose.onNodeWithTag("document").assertTextEquals("hello world")
    }
}
