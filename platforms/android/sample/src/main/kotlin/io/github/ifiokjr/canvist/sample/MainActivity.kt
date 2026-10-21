package io.github.ifiokjr.canvist.sample

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView

/** The sample: a rich-text editor with a formatting toolbar, and two editors collaborating. */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val markdown = if (intent.getBooleanExtra(EXTRA_EMPTY_DOCUMENT, false)) "" else WELCOME
        setContent {
            val colors = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()
            MaterialTheme(colorScheme = colors) { SampleApp(markdown) }
        }
    }

    companion object {
        /** UI tests start from an empty document. */
        const val EXTRA_EMPTY_DOCUMENT = "emptyDocument"
    }
}

@Composable
fun SampleApp(markdown: String) {
    val context = LocalContext.current
    val editor = remember { EditorModel(context, markdown, "Editor") }
    val collaboration = remember { CollaborationModel(context) }
    var tab by remember { mutableIntStateOf(0) }

    Scaffold(
        modifier = Modifier.imePadding(),
        bottomBar = {
            NavigationBar {
                TABS.forEachIndexed { index, (title, icon) ->
                    NavigationBarItem(
                        selected = tab == index,
                        onClick = { tab = index },
                        icon = { Text(icon) },
                        label = { Text(title) },
                    )
                }
            }
        },
    ) { padding ->
        when (tab) {
            0 -> EditorScreen(editor, padding)
            else -> CollaborationScreen(collaboration, padding)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EditorScreen(model: EditorModel, padding: PaddingValues) {
    Column(Modifier.fillMaxSize().padding(padding)) {
        TopAppBar(
            title = { Text("Canvist") },
            actions = {
                TextButton(onClick = model::undo, enabled = model.canUndo) { Text("Undo") }
                TextButton(onClick = model::redo, enabled = model.canRedo) { Text("Redo") }
            },
        )
        FormatBar(model)
        HorizontalDivider()
        AndroidView(factory = { model.view }, modifier = Modifier.fillMaxSize())
    }
}

/** Toggles for the toolbar's formats, lit while the selection has them. */
@Composable
fun FormatBar(model: EditorModel) {
    Row(
        modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        for (format in Format.entries) {
            val active = format in model.active
            val highlight = if (active) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surface
            val weight = if (format == Format.BOLD) FontWeight.Bold else FontWeight.Normal
            TextButton(
                onClick = { model.toggle(format) },
                modifier = Modifier
                    .size(width = 44.dp, height = 40.dp)
                    .background(highlight, RoundedCornerShape(8.dp))
                    .semantics {
                        contentDescription = format.title
                        selected = active
                    },
                contentPadding = PaddingValues(0.dp),
            ) {
                Text(format.label, fontWeight = weight)
            }
        }
    }
}

/** Two editors joined over Yjs. */
@Composable
fun CollaborationScreen(model: CollaborationModel, padding: PaddingValues) {
    Column(Modifier.fillMaxSize().padding(padding)) {
        Peer(model.alice, Modifier.weight(1f))
        HorizontalDivider()
        Peer(model.bob, Modifier.weight(1f))
    }
}

@Composable
private fun Peer(model: EditorModel, modifier: Modifier) {
    Column(modifier) {
        Text(
            model.label,
            style = MaterialTheme.typography.labelMedium,
            modifier = Modifier.padding(start = 20.dp, top = 12.dp),
        )
        AndroidView(factory = { model.view }, modifier = Modifier.fillMaxSize())
    }
}

/** The bottom bar's tabs: a title and an icon. */
private val TABS = listOf("Editor" to "✎", "Collaborate" to "⇄")

private val WELCOME = """
    # Welcome to canvist

    This text is laid out by **Rust** and painted on an Android *Canvas*. Select it, format it, and undo it like any other text.

    - [x] Type with Gboard, voice typing, or a hardware keyboard
    - [ ] Try `##` and a space for a heading

    > Every change is a step that collaborators merge.
""".trimIndent()
