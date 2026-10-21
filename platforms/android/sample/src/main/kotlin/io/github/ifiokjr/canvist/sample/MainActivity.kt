package io.github.ifiokjr.canvist.sample

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import io.github.ifiokjr.canvist.CanvistException
import io.github.ifiokjr.canvist.DocFormat
import io.github.ifiokjr.canvist.Editor
import io.github.ifiokjr.canvist.PaintMeasurer
import io.github.ifiokjr.canvist.coreVersion
import io.github.ifiokjr.canvist.defaultEditorOptions

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            MaterialTheme {
                EditorScreen()
            }
        }
    }
}

/** Appends text to a Rust-owned document and renders what Rust exports. */
@Composable
fun EditorScreen() {
    val density = LocalDensity.current.density
    val editor = remember { Editor(defaultEditorOptions(), PaintMeasurer(density)) }
    DisposableEffect(editor) { onDispose { editor.close() } }

    var draft by rememberSaveable { mutableStateOf("") }
    var document by remember { mutableStateOf("") }
    var failure by remember { mutableStateOf<String?>(null) }

    fun append() {
        if (draft.isEmpty()) return
        try {
            val end = editor.textLength()
            editor.select(end, end)
            editor.insertText(draft)
            document = editor.export(DocFormat.MARKDOWN)
            draft = ""
            failure = null

        } catch (error: CanvistException) {
            failure = error.message
        }
    }

    Scaffold { padding ->
        Column(
            modifier = Modifier.fillMaxSize().padding(padding).padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text("Canvist", style = MaterialTheme.typography.headlineLarge)
            OutlinedTextField(
                value = draft,
                onValueChange = { draft = it },
                label = { Text("Text") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth().testTag("draft"),
            )
            Button(
                onClick = ::append,
                enabled = draft.isNotEmpty(),
                modifier = Modifier.testTag("append"),
            ) {
                Text("Append")
            }
            Text("Document (from Rust)", style = MaterialTheme.typography.labelLarge)
            Text(
                text = document.ifEmpty { "Empty" },
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.testTag("document"),
            )
            failure?.let { Text(it, color = MaterialTheme.colorScheme.error) }

            Text(
                "Rust core ${coreVersion()}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
