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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import io.github.ifiokjr.canvist.EditorException
import io.github.ifiokjr.canvist.EditorHandle

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

/** Appends text to a Rust-owned document and renders what Rust returns. */
@Composable
fun EditorScreen() {
    val editor = remember { EditorHandle() }
    DisposableEffect(editor) { onDispose { editor.close() } }

    var draft by rememberSaveable { mutableStateOf("") }
    var document by remember { mutableStateOf("") }
    var failure by remember { mutableStateOf<String?>(null) }

    fun append() {
        if (draft.isEmpty()) return
        try {
            // The placeholder core counts offsets in Unicode scalar values.
            val end = document.codePointCount(0, document.length).toUInt()
            editor.insertText(end, draft)
            document = editor.text()
            draft = ""
            failure = null
        } catch (error: EditorException) {
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
                "Rust core ${editor.version()}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
