# Quick start: iOS

In this guide you add a canvist editor to an iOS app, give it a formatting toolbar, and save what people write. It takes about five minutes.

**Status: available.** `CanvistView` is a native rich-text editor for UIKit and SwiftUI. Text goes through `UITextInput`, so the keyboard, input methods, autocorrect, dictation, the selection handles, the loupe, and the edit menu all work as they do in a text view. canvist's Rust engine lays the document out with CoreText measurements and the view paints it with Core Graphics. It follows Dark Mode and Dynamic Type, reads to VoiceOver line by line, and scrolls documents of any length.

## 1. Install

You need Xcode 16 or newer, iOS 17 or newer, and a [Rust toolchain](https://rustup.rs). The package isn't on a registry yet, so build it from the repository:

```sh
git clone https://github.com/canvist/canvist
canvist/platforms/apple/build.sh
```

`build.sh` compiles the engine into `Frameworks/CanvistFFI.xcframework` and generates its Swift bindings. In Xcode, choose **File › Add Package Dependencies › Add Local…**, pick `canvist/platforms/apple`, and add the `Canvist` library to your app target.

## 2. Show an editor

`CanvistView` is a `UIScrollView` subclass; `CanvistEditor` shows one in SwiftUI. Make the view once, in a model that outlives your view's body, and load some content:

```swift
import Canvist
import SwiftUI

@MainActor
@Observable
final class NoteModel {
    let kit = StarterKit()
    let view: CanvistView

    init() throws {
        view = try CanvistView(extensions: kit.extensions)
        try view.editor.load(format: .markdown, content: "# My note\n\nStart typing.")
    }
}

struct NoteScreen: View {
    let model: NoteModel

    var body: some View {
        CanvistEditor(model.view)
    }
}
```

Run the app and tap the text. Type `##` at the start of a line for a heading, `-` for a list, or `[ ]` for a checklist.

## 3. Add a toolbar

Every extension in `StarterKit` has a typed handle. Its commands go to `editor.run(command:)`, and its queries to `editor.query(_:)`. The view repaints itself after every command:

```swift
struct FormatBar: View {
    let model: NoteModel
    @State private var bold = false

    var body: some View {
        HStack {
            Button("Bold", systemImage: "bold") {
                _ = try? model.view.editor.run(command: model.kit.bold.toggleBold())
            }
            .symbolVariant(bold ? .fill : .none)

            Button("Heading", systemImage: "textformat.size") {
                _ = try? model.view.editor.run(command: model.kit.heading.toggleHeading(level: 2))
            }

            Button("Undo", systemImage: "arrow.uturn.backward") {
                _ = try? model.view.editor.undo()
            }
        }
        .onAppear {
            // Called after every change: keep the toolbar in step with the selection.
            model.view.onChange = { _ in
                bold = (try? model.view.editor.query(model.kit.bold.isBold())) ?? false
            }
        }
    }
}
```

Commands check their arguments against the same declarations as every other canvist binding, and throw `CanvistError.Failed(code:reason:)` with a stable code such as `invalidArgs` when they don't fit.

## 4. Save and load

Export the document in any format, and load it back later:

```swift
let markdown = model.view.editor.export(format: .markdown)
let json = model.view.editor.export(format: .json)   // ProseMirror JSON, lossless
try model.view.editor.load(format: .json, content: json)
```

## 5. Collaborate

Editors are Yjs peers. Share one document, join it from another editor, and send each editor's updates to the other over whatever transport you use:

```swift
try alice.editor.share(clientId: 1)
try bob.editor.join(state: try alice.editor.encodeState(), clientId: 2)

alice.onChange = { change in
    guard change.updates else { return }
    for update in (try? alice.editor.takeUpdates()) ?? [] {
        _ = try? bob.editor.applyUpdate(update: update)
    }
}
```

For a y-websocket server such as [canvist Sync](../cloud/sync.md), use `syncStart()`, `receiveSync(message:)`, and `flushSync()` instead. See [Collaboration](../guides/collaboration.md).

## Next steps

- The example app in `platforms/apple/Example` has a full toolbar and two editors collaborating side by side.
- [Commands and typed handles](../concepts/commands.md) lists what every extension can do.
- [Performance](../guides/performance.md) has the keystroke-to-paint budgets the view is held to.
