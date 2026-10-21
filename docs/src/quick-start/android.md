# Quick start: Android

In this guide you add a canvist editor to an Android app, give it a formatting toolbar, and save what people write. It takes about five minutes.

**Status: available.** `CanvistView` is a native rich-text editor for Views and Jetpack Compose. Text goes through its own `InputConnection`, so Gboard and other keyboards compose, autocorrect, offer suggestions, and voice-type as they do in an `EditText`, and hardware keyboards get the editor's shortcuts. It has the platform's selection handles, magnifier, and floating text toolbar. canvist's Rust engine lays the document out with `Paint` measurements and the view paints it on a `Canvas`. It follows dark theme and font scale, reads to TalkBack, and scrolls documents of any length.

## 1. Install

You need Android Studio with an NDK, `minSdk` 24 or newer, and a [Rust toolchain](https://rustup.rs) with [cargo-ndk](https://github.com/bbqsrc/cargo-ndk). The library isn't on Maven Central yet, so build it from the repository and publish it to your local Maven repository:

```sh
git clone https://github.com/canvist/canvist
cd canvist/platforms/android
./build.sh
./gradlew :canvist:publishToMavenLocal
```

`build.sh` compiles the engine for `arm64-v8a` and `x86_64` and generates its Kotlin bindings. Then add the library to your app:

```kotlin
// settings.gradle.kts
dependencyResolutionManagement {
    repositories {
        mavenLocal()
        google()
        mavenCentral()
    }
}

// app/build.gradle.kts
dependencies {
    implementation("io.github.ifiokjr:canvist-android:0.1.0")
}
```

## 2. Show an editor

`CanvistView` is a plain `View`. Make it once, in a model that outlives your composition, and load some content:

```kotlin
import io.github.ifiokjr.canvist.CanvistView
import io.github.ifiokjr.canvist.DocFormat
import io.github.ifiokjr.canvist.StarterKit

class NoteModel(context: Context) {
    val kit = StarterKit()
    val view = CanvistView(context, kit.extensions).apply {
        editor.load(DocFormat.MARKDOWN, "# My note\n\nStart typing.")
    }
}

@Composable
fun NoteScreen(model: NoteModel) {
    AndroidView(factory = { model.view }, modifier = Modifier.fillMaxSize().imePadding())
}
```

In a View layout, add it like any other view, as `<io.github.ifiokjr.canvist.CanvistView>`. Run the app and tap the text. Type `##` at the start of a line for a heading, `-` for a list, or `[ ]` for a checklist.

## 3. Add a toolbar

Every extension in `StarterKit` has a typed handle. Its commands go to `editor.run(command)`, and its queries to `editor.query(query)`. The view repaints itself after every command:

```kotlin
@Composable
fun FormatBar(model: NoteModel) {
    val editor = model.view.editor
    var bold by remember { mutableStateOf(false) }

    DisposableEffect(model) {
        // Called after every change: keep the toolbar in step with the selection.
        model.view.onChange = { bold = editor.query(model.kit.bold.isBold()) }
        onDispose { model.view.onChange = null }
    }

    Row {
        IconToggleButton(checked = bold, onCheckedChange = { editor.run(model.kit.bold.toggleBold()) }) {
            Text("B", fontWeight = FontWeight.Bold)
        }
        TextButton(onClick = { editor.run(model.kit.heading.toggleHeading(level = 2)) }) { Text("H") }
        TextButton(onClick = { editor.undo() }) { Text("Undo") }
    }
}
```

Commands check their arguments against the same declarations as every other canvist binding, and throw `CanvistException.Failed` with a stable `code` such as `invalidArgs` when they don't fit.

## 4. Save and load

Export the document in any format, and load it back later:

```kotlin
val markdown = model.view.editor.export(DocFormat.MARKDOWN)
val json = model.view.editor.export(DocFormat.JSON) // ProseMirror JSON, lossless
model.view.editor.load(DocFormat.JSON, json)
```

## 5. Collaborate

Editors are Yjs peers. Share one document, join it from another editor, and send each editor's updates to the other over whatever transport you use:

```kotlin
alice.editor.share(1uL)
bob.editor.join(alice.editor.encodeState(), 2uL)

alice.onChange = { change ->
    if (change.updates) {
        for (update in alice.editor.takeUpdates()) bob.editor.applyUpdate(update)
    }
}
```

For a y-websocket server such as [canvist Sync](../cloud/sync.md), use `syncStart()`, `receiveSync(message)`, and `flushSync()` instead. See [Collaboration](../guides/collaboration.md).

## Next steps

- The sample app in `platforms/android/sample` has a full toolbar and two editors collaborating.
- [Commands and typed handles](../concepts/commands.md) lists what every extension can do.
- [Performance](../guides/performance.md) has the keystroke-to-paint budgets the view is held to.
